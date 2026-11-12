//! Push notifications: Web Push (RFC 8030 + 8291 aes128gcm + 8292 VAPID) for the web app,
//! FCM HTTP v1 for the Android build. Pure Rust, no OpenSSL.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use aes_gcm::{Aes128Gcm, KeyInit, Nonce, aead::Aead};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use hkdf::Hkdf;
use p256::{
    PublicKey, SecretKey,
    ecdsa::{Signature, SigningKey, signature::Signer},
    elliptic_curve::{Generate, sec1::ToSec1Point},
};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sqlx::SqlitePool;
use tokio::sync::Mutex;

const RECORD_SIZE: u32 = 4096;
const TTL_SECONDS: &str = "86400";
const JWT_LIFETIME_SECONDS: u64 = 12 * 3600;

/// What a push is about: each kind can be turned off in the notification settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Question,
    Result,
    Chat,
    Nudge,
    Added,
    Ask,
}

impl Kind {
    pub const ALL: [Kind; 6] = [
        Kind::Question,
        Kind::Result,
        Kind::Chat,
        Kind::Nudge,
        Kind::Added,
        Kind::Ask,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Question => "question",
            Kind::Result => "result",
            Kind::Chat => "chat",
            Kind::Nudge => "nudge",
            Kind::Added => "added",
            Kind::Ask => "ask",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Notification {
    /// Sent if the person wants any of these (a result with the next question is both).
    #[serde(skip)]
    pub kinds: Vec<Kind>,
    pub title: String,
    pub body: String,
    /// Groups notifications on the phone: a newer one replaces the older one.
    pub tag: String,
    /// Path in the app; the room is added as `?room=` on the way out, so a tap opens the right room.
    pub url: String,
    pub room: i64,
}

impl Notification {
    /// `url` with the room in it (`/round/3?room=7`).
    fn with_room_link(mut self) -> Self {
        let sep = if self.url.contains('?') { '&' } else { '?' };
        self.url = format!("{}{sep}room={}", self.url, self.room);
        self
    }
}

/// One device, the room it would hear about, and what its owner wants pushed.
#[derive(sqlx::FromRow)]
pub struct Recipient {
    id: i64,
    kind: String,
    endpoint: String,
    p256dh: Option<String>,
    auth: Option<String>,
    pub user_id: Option<i64>,
    pub group_id: i64,
    muted: bool,
    enabled: Option<bool>,
    off: Option<String>,
    quiet_start: Option<i64>,
    quiet_end: Option<i64>,
    tz_offset: Option<i64>,
}

impl Recipient {
    /// Their settings let this through: room not muted, master switch on, one of the kinds on,
    /// and not their quiet hours (dropped then: the inbox has it).
    fn wants(&self, kinds: &[Kind], now: u64) -> bool {
        let off = self.off.as_deref().unwrap_or("");
        let wanted = kinds
            .iter()
            .any(|k| !off.split(',').any(|o| o == k.as_str()));
        let quiet = match (self.quiet_start, self.quiet_end) {
            (Some(start), Some(end)) => {
                in_quiet_hours(start, end, self.tz_offset.unwrap_or(0), now)
            }
            _ => false,
        };
        !self.muted && self.enabled.unwrap_or(true) && wanted && !quiet
    }
}

/// `now` (unix seconds) falls in [start, end) local time; minutes after midnight, may wrap.
pub fn in_quiet_hours(start: i64, end: i64, tz_offset: i64, now: u64) -> bool {
    let local = (now as i64 / 60 + tz_offset).rem_euclid(24 * 60);
    if start == end {
        false
    } else if start < end {
        (start..end).contains(&local)
    } else {
        local >= start || local < end
    }
}

/// An app that has said nothing for this long is gone (iOS can freeze it without closing the socket).
const PRESENCE_TTL: Duration = Duration::from_secs(45);

static NEXT_CONNECTION: AtomicU64 = AtomicU64::new(1);

/// Who has the app open on the screen, per account and room: their devices get no push for that room.
#[derive(Clone, Default)]
pub struct Presence(Arc<std::sync::Mutex<HashMap<Seat, Connections>>>);

/// (account, room)
type Seat = (i64, i64);
/// Each live connection and when it last said it was on the screen.
type Connections = HashMap<u64, Instant>;

impl Presence {
    /// A new id for one live connection.
    pub fn connection() -> u64 {
        NEXT_CONNECTION.fetch_add(1, Ordering::Relaxed)
    }

    /// The app is in front on this room (sent again every few seconds while it stays there).
    pub fn here(&self, user_id: i64, group_id: i64, connection: u64) {
        let mut map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        map.entry((user_id, group_id))
            .or_default()
            .insert(connection, Instant::now());
    }

    /// The app went to the background, or the connection closed.
    pub fn away(&self, user_id: i64, group_id: i64, connection: u64) {
        let mut map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(connections) = map.get_mut(&(user_id, group_id)) {
            connections.remove(&connection);
            if connections.is_empty() {
                map.remove(&(user_id, group_id));
            }
        }
    }

    pub fn is_here(&self, user_id: i64, group_id: i64) -> bool {
        let map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        map.get(&(user_id, group_id))
            .is_some_and(|c| c.values().any(|seen| seen.elapsed() < PRESENCE_TTL))
    }
}

pub struct Vapid {
    key: SigningKey,
    pub public_b64: String,
    subject: String,
}

impl Vapid {
    /// `private_b64`: base64url raw 32-byte P-256 private key.
    pub fn new(private_b64: &str, subject: String) -> Result<Self, String> {
        let bytes = B64
            .decode(private_b64.trim())
            .map_err(|e| format!("VAPID key: {e}"))?;
        let secret = SecretKey::from_slice(&bytes).map_err(|e| format!("VAPID key: {e}"))?;
        let public = secret.public_key().to_sec1_point(false);
        Ok(Self {
            key: SigningKey::from(&secret),
            public_b64: B64.encode(public.as_bytes()),
            subject,
        })
    }

    fn authorization(&self, endpoint: &str) -> Option<String> {
        let url = reqwest::Url::parse(endpoint).ok()?;
        let aud = format!("{}://{}", url.scheme(), url.host_str()?);
        let exp = now_secs() + JWT_LIFETIME_SECONDS;
        let header = B64.encode(br#"{"typ":"JWT","alg":"ES256"}"#);
        let claims = B64.encode(
            serde_json::json!({ "aud": aud, "exp": exp, "sub": self.subject })
                .to_string()
                .as_bytes(),
        );
        let signing_input = format!("{header}.{claims}");
        let signature: Signature = self.key.sign(signing_input.as_bytes());
        let jwt = format!("{signing_input}.{}", B64.encode(signature.to_bytes()));
        Some(format!("vapid t={jwt}, k={}", self.public_b64))
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// RFC 8291 message encryption with the aes128gcm content coding (single record).
pub fn encrypt(payload: &[u8], p256dh_b64: &str, auth_b64: &str) -> Result<Vec<u8>, String> {
    let ua_public_bytes = B64.decode(p256dh_b64).map_err(|e| e.to_string())?;
    let auth_secret = B64.decode(auth_b64).map_err(|e| e.to_string())?;
    let ua_public = PublicKey::from_sec1_bytes(&ua_public_bytes).map_err(|e| e.to_string())?;

    let mut rng = rand::rng();
    let as_secret = p256::ecdh::EphemeralSecret::generate_from_rng(&mut rng);
    let as_public = as_secret.public_key().to_sec1_point(false);
    let shared = as_secret.diffie_hellman(&ua_public);

    let mut key_info = b"WebPush: info\0".to_vec();
    key_info.extend_from_slice(&ua_public_bytes);
    key_info.extend_from_slice(as_public.as_bytes());
    let mut ikm = [0u8; 32];
    Hkdf::<Sha256>::new(Some(&auth_secret), shared.raw_secret_bytes())
        .expand(&key_info, &mut ikm)
        .map_err(|e| e.to_string())?;

    let mut salt = [0u8; 16];
    rand::Rng::fill_bytes(&mut rng, &mut salt);
    let prk = Hkdf::<Sha256>::new(Some(&salt), &ikm);
    let mut cek = [0u8; 16];
    let mut nonce = [0u8; 12];
    prk.expand(b"Content-Encoding: aes128gcm\0", &mut cek)
        .map_err(|e| e.to_string())?;
    prk.expand(b"Content-Encoding: nonce\0", &mut nonce)
        .map_err(|e| e.to_string())?;

    let mut plaintext = payload.to_vec();
    plaintext.push(2); // last-record delimiter
    let cipher = Aes128Gcm::new_from_slice(&cek).map_err(|e| e.to_string())?;
    let ciphertext = cipher
        .encrypt(&Nonce::from(nonce), plaintext.as_slice())
        .map_err(|e| e.to_string())?;

    let mut body = Vec::with_capacity(16 + 4 + 1 + 65 + ciphertext.len());
    body.extend_from_slice(&salt);
    body.extend_from_slice(&RECORD_SIZE.to_be_bytes());
    body.push(as_public.as_bytes().len() as u8);
    body.extend_from_slice(as_public.as_bytes());
    body.extend_from_slice(&ciphertext);
    Ok(body)
}

#[derive(Deserialize)]
struct ServiceAccount {
    project_id: String,
    client_email: String,
    private_key: String,
    token_uri: String,
}

pub struct Fcm {
    account: ServiceAccount,
    token: Mutex<Option<(String, u64)>>,
}

impl Fcm {
    pub fn from_json(json: &str) -> Result<Self, String> {
        let account: ServiceAccount =
            serde_json::from_str(json).map_err(|e| format!("FCM service account: {e}"))?;
        Ok(Self {
            account,
            token: Mutex::new(None),
        })
    }

    async fn access_token(&self, http: &reqwest::Client) -> Result<String, String> {
        let mut cached = self.token.lock().await;
        if let Some((token, exp)) = cached.as_ref()
            && *exp > now_secs() + 60
        {
            return Ok(token.clone());
        }
        let now = now_secs();
        let claims = serde_json::json!({
            "iss": self.account.client_email,
            "scope": "https://www.googleapis.com/auth/firebase.messaging",
            "aud": self.account.token_uri,
            "iat": now,
            "exp": now + 3600,
        });
        let key = jsonwebtoken::EncodingKey::from_rsa_pem(self.account.private_key.as_bytes())
            .map_err(|e| e.to_string())?;
        let assertion = jsonwebtoken::encode(
            &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256),
            &claims,
            &key,
        )
        .map_err(|e| e.to_string())?;
        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            expires_in: u64,
        }
        let res: TokenResponse = http
            .post(&self.account.token_uri)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", assertion.as_str()),
            ])
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        *cached = Some((res.access_token.clone(), now + res.expires_in));
        Ok(res.access_token)
    }
}

#[derive(Clone)]
pub struct Push {
    http: reqwest::Client,
    vapid: Option<Arc<Vapid>>,
    fcm: Option<Arc<Fcm>>,
    presence: Presence,
}

pub enum Target {
    Group {
        group_id: i64,
        skip_member: Option<i64>,
        /// Whoever caused it (an account): people who blocked them get nothing.
        from_user: Option<i64>,
    },
    Member {
        member_id: i64,
        from_user: Option<i64>,
    },
}

/// What happened to one delivery: keep the subscription, or drop it because the device is gone.
enum Delivery {
    Sent,
    Gone,
    Failed(String),
}

impl Push {
    pub fn new(vapid: Option<Vapid>, fcm: Option<Fcm>) -> Self {
        Self {
            http: reqwest::Client::new(),
            vapid: vapid.map(Arc::new),
            fcm: fcm.map(Arc::new),
            presence: Presence::default(),
        }
    }

    pub fn presence(&self) -> &Presence {
        &self.presence
    }

    pub fn vapid_public_key(&self) -> Option<&str> {
        self.vapid.as_ref().map(|v| v.public_b64.as_str())
    }

    /// Sends to every device of the group's members, except `skip_member`. Runs in the background.
    pub fn notify_group(
        &self,
        db: SqlitePool,
        group_id: i64,
        skip_member: Option<i64>,
        n: Notification,
    ) {
        self.notify(
            db,
            Target::Group {
                group_id,
                skip_member,
                from_user: None,
            },
            n,
        );
    }

    /// Same, for something a person did (a message): skips them and whoever blocked them.
    pub fn notify_group_from(
        &self,
        db: SqlitePool,
        group_id: i64,
        from: &crate::models::MemberRow,
        n: Notification,
    ) {
        self.notify(
            db,
            Target::Group {
                group_id,
                skip_member: Some(from.id),
                from_user: from.user_id,
            },
            n,
        );
    }

    /// Sends to every device of one member, unless they blocked `from_user`. Runs in the background.
    pub fn notify_member(
        &self,
        db: SqlitePool,
        member_id: i64,
        from_user: Option<i64>,
        n: Notification,
    ) {
        self.notify(
            db,
            Target::Member {
                member_id,
                from_user,
            },
            n,
        );
    }

    fn notify(&self, db: SqlitePool, target: Target, n: Notification) {
        let push = self.clone();
        let n = n.with_room_link();
        tokio::spawn(async move {
            let subs = match push.recipients(&db, &target, &n.kinds, now_secs()).await {
                Ok(subs) => subs,
                Err(e) => return tracing::error!(error = %e, "push: load subscriptions"),
            };
            // Everyone at once: a slow push service must not delay the others.
            let results =
                futures::future::join_all(subs.iter().map(|sub| push.deliver(sub, &n))).await;
            for (sub, result) in subs.iter().zip(results) {
                match result {
                    Delivery::Sent => {}
                    Delivery::Gone => {
                        let _ = sqlx::query("DELETE FROM push_subscriptions WHERE id = ?")
                            .bind(sub.id)
                            .execute(&db)
                            .await;
                    }
                    Delivery::Failed(e) => {
                        tracing::warn!(kind = %sub.kind, error = %e, "push failed")
                    }
                }
            }
        });
    }

    /// The devices a push for `target` goes to at `now` (unix seconds), after blocks, the app
    /// being open on the room, room mutes and notification settings.
    pub async fn recipients(
        &self,
        db: &SqlitePool,
        target: &Target,
        kinds: &[Kind],
        now: u64,
    ) -> sqlx::Result<Vec<Recipient>> {
        // Same columns for both, written out: sqlx wants static SQL.
        macro_rules! select {
            ($where:literal) => {
                concat!(
                    "SELECT s.id, s.kind, s.endpoint, s.p256dh, s.auth, s.user_id, m.group_id, \
                     m.muted, p.enabled, p.off, p.quiet_start, p.quiet_end, p.tz_offset \
                     FROM push_subscriptions s JOIN members m ON m.user_id = s.user_id \
                     LEFT JOIN notification_prefs p ON p.user_id = s.user_id WHERE ",
                    $where,
                    " AND NOT EXISTS (SELECT 1 FROM blocks b WHERE b.user_id = s.user_id AND b.blocked_id = ?)"
                )
            };
        }
        let subs: Vec<Recipient> = match *target {
            // Every device of every active member's account (one account can sit in several rooms).
            Target::Group {
                group_id,
                skip_member,
                from_user,
            } => {
                sqlx::query_as(select!(
                    "m.group_id = ? AND m.removed_at IS NULL AND m.id IS NOT ?"
                ))
                .bind(group_id)
                .bind(skip_member)
                .bind(from_user)
                .fetch_all(db)
                .await?
            }
            Target::Member {
                member_id,
                from_user,
            } => {
                sqlx::query_as(select!("m.id = ?"))
                    .bind(member_id)
                    .bind(from_user)
                    .fetch_all(db)
                    .await?
            }
        };
        Ok(subs
            .into_iter()
            // The app is open on this room: the screen already shows it, no notification.
            .filter(|s| {
                !s.user_id
                    .is_some_and(|u| self.presence.is_here(u, s.group_id))
            })
            .filter(|s| s.wants(kinds, now))
            .collect())
    }

    async fn deliver(&self, sub: &Recipient, n: &Notification) -> Delivery {
        match sub.kind.as_str() {
            "web" => self.deliver_web(sub, n).await,
            "fcm" => self.deliver_fcm(sub, n).await,
            _ => Delivery::Gone,
        }
    }

    async fn deliver_web(&self, sub: &Recipient, n: &Notification) -> Delivery {
        let Some(vapid) = &self.vapid else {
            return Delivery::Failed("VAPID not configured".into());
        };
        let (Some(p256dh), Some(auth)) = (&sub.p256dh, &sub.auth) else {
            return Delivery::Gone;
        };
        let payload = serde_json::to_vec(n).unwrap_or_default();
        let body = match encrypt(&payload, p256dh, auth) {
            Ok(b) => b,
            Err(e) => return Delivery::Failed(e),
        };
        let Some(authorization) = vapid.authorization(&sub.endpoint) else {
            return Delivery::Gone;
        };
        let res = self
            .http
            .post(&sub.endpoint)
            .header("Authorization", authorization)
            .header("Content-Encoding", "aes128gcm")
            .header("Content-Type", "application/octet-stream")
            .header("TTL", TTL_SECONDS)
            .header("Urgency", "high")
            .body(body)
            .send()
            .await;
        match res {
            Ok(r) if r.status().is_success() => Delivery::Sent,
            Ok(r) if matches!(r.status().as_u16(), 404 | 410) => Delivery::Gone,
            Ok(r) => Delivery::Failed(format!(
                "{} {}",
                r.status(),
                r.text().await.unwrap_or_default()
            )),
            Err(e) => Delivery::Failed(e.to_string()),
        }
    }

    async fn deliver_fcm(&self, sub: &Recipient, n: &Notification) -> Delivery {
        let Some(fcm) = &self.fcm else {
            return Delivery::Failed("FCM not configured".into());
        };
        let token = match fcm.access_token(&self.http).await {
            Ok(t) => t,
            Err(e) => return Delivery::Failed(e),
        };
        let message = serde_json::json!({
            "message": {
                "token": sub.endpoint,
                "notification": { "title": n.title, "body": n.body },
                "data": { "url": n.url, "room": n.room.to_string() },
                "android": {
                    "priority": "high",
                    "notification": { "tag": n.tag, "channel_id": "default" }
                }
            }
        });
        let url = format!(
            "https://fcm.googleapis.com/v1/projects/{}/messages:send",
            fcm.account.project_id
        );
        let res = self
            .http
            .post(url)
            .bearer_auth(token)
            .json(&message)
            .send()
            .await;
        match res {
            Ok(r) if r.status().is_success() => Delivery::Sent,
            Ok(r) if matches!(r.status().as_u16(), 404) => Delivery::Gone,
            Ok(r) => {
                let text = r.text().await.unwrap_or_default();
                if text.contains("UNREGISTERED") || text.contains("INVALID_ARGUMENT") {
                    Delivery::Gone
                } else {
                    Delivery::Failed(text)
                }
            }
            Err(e) => Delivery::Failed(e.to_string()),
        }
    }
}

/// Prints a new VAPID key pair: `server --gen-vapid`.
pub fn generate_vapid() -> (String, String) {
    let secret = SecretKey::generate_from_rng(&mut rand::rng());
    let public = secret.public_key().to_sec1_point(false);
    (B64.encode(secret.to_bytes()), B64.encode(public.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decrypts like a browser would (RFC 8291 from the user agent side) to check `encrypt`.
    fn decrypt(body: &[u8], ua_secret: &SecretKey, auth: &[u8]) -> Vec<u8> {
        let salt = &body[..16];
        let id_len = body[20] as usize;
        let as_public_bytes = &body[21..21 + id_len];
        let ciphertext = &body[21 + id_len..];
        let as_public = PublicKey::from_sec1_bytes(as_public_bytes).unwrap();
        let shared =
            p256::ecdh::diffie_hellman(ua_secret.to_nonzero_scalar(), as_public.as_affine());
        let ua_public = ua_secret.public_key().to_sec1_point(false);
        let mut key_info = b"WebPush: info\0".to_vec();
        key_info.extend_from_slice(ua_public.as_bytes());
        key_info.extend_from_slice(as_public_bytes);
        let mut ikm = [0u8; 32];
        Hkdf::<Sha256>::new(Some(auth), shared.raw_secret_bytes())
            .expand(&key_info, &mut ikm)
            .unwrap();
        let prk = Hkdf::<Sha256>::new(Some(salt), &ikm);
        let (mut cek, mut nonce) = ([0u8; 16], [0u8; 12]);
        prk.expand(b"Content-Encoding: aes128gcm\0", &mut cek)
            .unwrap();
        prk.expand(b"Content-Encoding: nonce\0", &mut nonce)
            .unwrap();
        let mut plain = Aes128Gcm::new_from_slice(&cek)
            .unwrap()
            .decrypt(&Nonce::from(nonce), ciphertext)
            .unwrap();
        assert_eq!(plain.pop(), Some(2));
        plain
    }

    #[test]
    fn web_push_payload_round_trips() {
        let ua_secret = SecretKey::generate_from_rng(&mut rand::rng());
        let p256dh = B64.encode(ua_secret.public_key().to_sec1_point(false).as_bytes());
        let auth = [7u8; 16];
        let body = encrypt(b"{\"title\":\"hey\"}", &p256dh, &B64.encode(auth)).unwrap();
        assert_eq!(&body[16..20], &RECORD_SIZE.to_be_bytes());
        assert_eq!(decrypt(&body, &ua_secret, &auth), b"{\"title\":\"hey\"}");
    }

    #[test]
    fn vapid_header_has_a_signed_jwt_and_the_public_key() {
        let (private, public) = generate_vapid();
        let vapid = Vapid::new(&private, "mailto:test@example.com".into()).unwrap();
        assert_eq!(vapid.public_b64, public);
        let header = vapid
            .authorization("https://fcm.googleapis.com/fcm/send/abc")
            .unwrap();
        let jwt = header
            .strip_prefix("vapid t=")
            .unwrap()
            .split(',')
            .next()
            .unwrap();
        let claims: serde_json::Value =
            serde_json::from_slice(&B64.decode(jwt.split('.').nth(1).unwrap()).unwrap()).unwrap();
        assert_eq!(claims["aud"], "https://fcm.googleapis.com");
        assert!(header.ends_with(&format!("k={public}")));
    }
}

#[cfg(test)]
mod quiet_hours_tests {
    use super::in_quiet_hours;

    #[test]
    fn quiet_hours_wrap_past_midnight_and_follow_the_offset() {
        let at = |h: u64, m: u64| h * 3600 + m * 60;
        // 22:00 → 08:00 local.
        assert!(in_quiet_hours(1320, 480, 0, at(23, 0)));
        assert!(in_quiet_hours(1320, 480, 0, at(0, 0)));
        assert!(!in_quiet_hours(1320, 480, 0, at(8, 0)));
        assert!(!in_quiet_hours(1320, 480, 0, at(21, 59)));
        // UTC-5: 02:00 UTC is 21:00 local, 04:00 UTC is 23:00.
        assert!(!in_quiet_hours(1320, 480, -300, at(2, 0)));
        assert!(in_quiet_hours(1320, 480, -300, at(4, 0)));
        // 13:00 → 14:00, and an empty window.
        assert!(in_quiet_hours(780, 840, 0, at(13, 30)));
        assert!(!in_quiet_hours(780, 840, 0, at(14, 0)));
        assert!(!in_quiet_hours(600, 600, 0, at(10, 0)));
    }
}

#[cfg(test)]
mod presence_tests {
    use super::Presence;

    #[test]
    fn presence_per_room_and_connection() {
        let presence = Presence::default();
        let (phone, laptop) = (Presence::connection(), Presence::connection());
        assert!(!presence.is_here(1, 10));
        presence.here(1, 10, phone);
        presence.here(1, 10, laptop);
        assert!(presence.is_here(1, 10));
        assert!(!presence.is_here(1, 11), "another room of the same account");
        assert!(!presence.is_here(2, 10), "another account in the same room");
        presence.away(1, 10, phone);
        assert!(presence.is_here(1, 10), "still open on the other device");
        presence.away(1, 10, laptop);
        assert!(!presence.is_here(1, 10));
    }
}
