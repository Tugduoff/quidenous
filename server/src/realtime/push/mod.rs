use crate::platform::ids::{MemberId, RoomId, UserId};
use std::{sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::platform::i18n::{Lang, Text};

mod endpoints;
mod fcm;
mod presence;
mod webpush;
pub use endpoints::*;
pub(crate) use fcm::*;
pub(crate) use presence::*;
pub(crate) use webpush::*;

const RECORD_SIZE: u32 = 4096;
const TTL_SECONDS: &str = "86400";
const JWT_LIFETIME_SECONDS: u64 = 12 * 3600;
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const MAX_DEVICES: i64 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Question,
    Result,
    Chat,
    Nudge,
    Added,
    Ask,
    Mention,
    Streak,
}

impl Kind {
    pub(crate) const ALL: [Kind; 8] = [
        Kind::Question,
        Kind::Result,
        Kind::Chat,
        Kind::Nudge,
        Kind::Added,
        Kind::Ask,
        Kind::Mention,
        Kind::Streak,
    ];

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Kind::Question => "question",
            Kind::Result => "result",
            Kind::Chat => "chat",
            Kind::Nudge => "nudge",
            Kind::Added => "added",
            Kind::Ask => "ask",
            Kind::Mention => "mention",
            Kind::Streak => "streak",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Notification {
    pub kinds: Vec<Kind>,
    pub title: Text,
    pub body: Text,
    pub tag: String,
    pub url: String,
    pub room: RoomId,
}

impl Notification {
    fn with_room_link(mut self) -> Self {
        let sep = if self.url.contains('?') { '&' } else { '?' };
        self.url = format!("{}{sep}room={}", self.url, self.room);
        self
    }

    fn payload(&self, lang: Lang) -> serde_json::Value {
        serde_json::json!({
            "title": self.title.get(lang),
            "body": self.body.get(lang),
            "tag": self.tag,
            "url": self.url,
            "room": self.room,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sent {
    pub user_id: Option<UserId>,
    pub title: String,
    pub body: String,
}

#[derive(sqlx::FromRow)]
pub struct Recipient {
    id: i64,
    kind: String,
    endpoint: String,
    p256dh: Option<String>,
    auth: Option<String>,
    pub user_id: Option<UserId>,
    pub group_id: RoomId,
    muted: bool,
    enabled: Option<bool>,
    off: Option<String>,
    quiet_start: Option<i64>,
    quiet_end: Option<i64>,
    tz_offset: Option<i64>,
    locale: Option<String>,
}

impl Recipient {
    pub(crate) fn lang(&self) -> Lang {
        Lang::from_db(self.locale.as_deref())
    }

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

pub(crate) fn in_quiet_hours(start: i64, end: i64, tz_offset: i64, now: u64) -> bool {
    let local = (now as i64 / 60 + tz_offset).rem_euclid(24 * 60);
    match start.cmp(&end) {
        std::cmp::Ordering::Equal => false,
        std::cmp::Ordering::Less => (start..end).contains(&local),
        std::cmp::Ordering::Greater => local >= start || local < end,
    }
}

#[derive(Clone)]
pub struct Push {
    http: reqwest::Client,
    vapid: Option<Arc<Vapid>>,
    fcm: Option<Arc<Fcm>>,
    presence: Presence,
    recorded: Arc<std::sync::Mutex<Option<Vec<Sent>>>>,
}

pub enum Target {
    Group {
        group_id: RoomId,
        skip_member: Option<MemberId>,
        from_user: Option<UserId>,
    },
    Member {
        member_id: MemberId,
        from_user: Option<UserId>,
    },
    Members {
        member_ids: Vec<MemberId>,
        from_user: Option<UserId>,
    },
}

enum Delivery {
    Sent,
    Gone,
    Failed(String),
}

impl Push {
    pub(crate) fn new(vapid: Option<Vapid>, fcm: Option<Fcm>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(HTTP_TIMEOUT)
                .connect_timeout(CONNECT_TIMEOUT)
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap_or_default(),
            vapid: vapid.map(Arc::new),
            fcm: fcm.map(Arc::new),
            presence: Presence::default(),
            recorded: Arc::default(),
        }
    }

    pub fn record(&self) {
        let mut rec = self
            .recorded
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        rec.get_or_insert_with(Vec::new);
    }

    pub fn recorded(&self) -> Vec<Sent> {
        let rec = self
            .recorded
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        rec.clone().unwrap_or_default()
    }

    pub(crate) fn presence(&self) -> &Presence {
        &self.presence
    }

    pub(crate) fn vapid_public_key(&self) -> Option<&str> {
        self.vapid.as_ref().map(|v| v.public_b64.as_str())
    }

    pub(crate) fn notify_group(
        &self,
        db: SqlitePool,
        group_id: RoomId,
        skip_member: Option<MemberId>,
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

    pub(crate) fn notify_member(
        &self,
        db: SqlitePool,
        member_id: MemberId,
        from_user: Option<UserId>,
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

    pub(crate) fn notify_members(
        &self,
        db: SqlitePool,
        member_ids: Vec<MemberId>,
        from: &crate::rooms::model::MemberRow,
        n: Notification,
    ) {
        if member_ids.is_empty() {
            return;
        }
        self.notify(
            db,
            Target::Members {
                member_ids,
                from_user: from.user_id,
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
            {
                let mut rec = push
                    .recorded
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if let Some(rec) = rec.as_mut() {
                    rec.extend(subs.iter().map(|sub| Sent {
                        user_id: sub.user_id,
                        title: n.title.get(sub.lang()).to_owned(),
                        body: n.body.get(sub.lang()).to_owned(),
                    }));
                    return;
                }
            }
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
                        tracing::warn!(kind = %sub.kind, error = %e, "push failed");
                    }
                }
            }
        });
    }

    pub async fn recipients(
        &self,
        db: &SqlitePool,
        target: &Target,
        kinds: &[Kind],
        now: u64,
    ) -> sqlx::Result<Vec<Recipient>> {
        macro_rules! select {
            ($where:literal) => {
                concat!(
                    "SELECT s.id, s.kind, s.endpoint, s.p256dh, s.auth, s.user_id, m.group_id, \
                     m.muted, p.enabled, p.off, p.quiet_start, p.quiet_end, p.tz_offset, u.locale \
                     FROM push_subscriptions s JOIN members m ON m.user_id = s.user_id \
                     LEFT JOIN users u ON u.id = s.user_id \
                     LEFT JOIN notification_prefs p ON p.user_id = s.user_id WHERE ",
                    $where,
                    " AND NOT EXISTS (SELECT 1 FROM blocks b WHERE b.user_id = s.user_id AND b.blocked_id = ?)"
                )
            };
        }
        let subs: Vec<Recipient> = match target {
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
            Target::Members {
                member_ids,
                from_user,
            } => {
                if member_ids.is_empty() {
                    return Ok(Vec::new());
                }
                let ids = serde_json::to_string(member_ids).unwrap_or_else(|_| "[]".into());
                sqlx::query_as(select!(
                    "m.id IN (SELECT value FROM json_each(?)) AND m.removed_at IS NULL"
                ))
                .bind(ids)
                .bind(from_user)
                .fetch_all(db)
                .await?
            }
        };
        Ok(subs
            .into_iter()
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
        if !web_endpoint_allowed(&sub.endpoint) {
            return Delivery::Gone;
        }
        let payload = serde_json::to_vec(&n.payload(sub.lang())).unwrap_or_default();
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

    pub(crate) fn widget_ping(
        &self,
        db: SqlitePool,
        group_id: RoomId,
        tokens: Vec<(UserId, String)>,
    ) {
        {
            let mut rec = self
                .recorded
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(rec) = rec.as_mut() {
                rec.extend(tokens.iter().map(|(user_id, _)| Sent {
                    user_id: Some(*user_id),
                    title: WIDGET_PING.into(),
                    body: group_id.to_string(),
                }));
                return;
            }
        }
        let push = self.clone();
        tokio::spawn(async move {
            let sends = tokens
                .iter()
                .map(|(_, t)| push.deliver_widget_ping(t, group_id));
            let results = futures::future::join_all(sends).await;
            for ((_, token), result) in tokens.iter().zip(results) {
                match result {
                    Delivery::Sent => {}
                    Delivery::Gone => {
                        let _ = sqlx::query(
                            "UPDATE widget_tokens SET fcm_token = NULL WHERE fcm_token = ?",
                        )
                        .bind(token)
                        .execute(&db)
                        .await;
                    }
                    Delivery::Failed(e) => tracing::warn!(error = %e, "widget ping failed"),
                }
            }
        });
    }

    async fn deliver_widget_ping(&self, fcm_token: &str, group_id: RoomId) -> Delivery {
        let Some(fcm) = &self.fcm else {
            return Delivery::Failed("FCM not configured".into());
        };
        let token = match fcm.access_token(&self.http).await {
            Ok(t) => t,
            Err(e) => return Delivery::Failed(e),
        };
        let message = widget_ping_message(fcm_token, group_id);
        let url = format!(
            "https://fcm.googleapis.com/v1/projects/{}/messages:send",
            fcm.account.project_id
        );
        match self
            .http
            .post(url)
            .bearer_auth(token)
            .json(&message)
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => Delivery::Sent,
            Ok(r) => {
                let status = r.status().as_u16();
                let text = r.text().await.unwrap_or_default();
                if fcm_token_gone(status, &text) {
                    Delivery::Gone
                } else {
                    Delivery::Failed(format!("{status} {text}"))
                }
            }
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
                "notification": { "title": n.title.get(sub.lang()), "body": n.body.get(sub.lang()) },
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
            Ok(r) => {
                let status = r.status().as_u16();
                let text = r.text().await.unwrap_or_default();
                if fcm_token_gone(status, &text) {
                    Delivery::Gone
                } else {
                    Delivery::Failed(format!("{status} {text}"))
                }
            }
            Err(e) => Delivery::Failed(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use aes_gcm::{Aes128Gcm, KeyInit, Nonce, aead::Aead};
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
    use hkdf::Hkdf;
    use p256::elliptic_curve::{Generate, sec1::ToSec1Point};
    use p256::{PublicKey, SecretKey};
    use sha2::Sha256;

    use super::*;

    #[test]
    fn only_known_push_services_are_accepted() {
        for ok in [
            "https://fcm.googleapis.com/fcm/send/abc:def",
            "https://updates.push.services.mozilla.com/wpush/v2/gAAA",
            "https://web.push.apple.com/QGx",
            "https://wns2-par02p.notify.windows.com/w/?token=x",
        ] {
            assert!(web_endpoint_allowed(ok), "{ok}");
        }
        for bad in [
            "http://fcm.googleapis.com/fcm/send/abc",
            "https://192.168.1.1/push",
            "https://[::1]/push",
            "https://evilfcm.googleapis.com.example.com/x",
            "https://fcm.googleapis.com.evil.com/x",
            "https://notfcm.googleapis.co/x",
            "https://fcm.googleapis.com:8443/x",
            "https://user@fcm.googleapis.com/x",
            "https://localhost/x",
            "not a url",
        ] {
            assert!(!web_endpoint_allowed(bad), "{bad}");
        }
        let long = format!("https://fcm.googleapis.com/{}", "a".repeat(1024));
        assert!(!web_endpoint_allowed(&long));
    }

    #[test]
    fn fcm_drops_a_token_only_when_the_token_is_the_problem() {
        let unregistered = r#"{"error":{"code":404,"status":"NOT_FOUND","details":[{"errorCode":"UNREGISTERED"}]}}"#;
        assert!(fcm_token_gone(404, unregistered));
        assert!(fcm_token_gone(
            400,
            r#"{"error":{"status":"INVALID_ARGUMENT","details":[{"errorCode":"UNREGISTERED"}]}}"#
        ));
        let bad_token = r#"{"error":{"code":400,"message":"The registration token is not a valid FCM registration token","status":"INVALID_ARGUMENT","details":[{"@type":"type.googleapis.com/google.firebase.fcm.v1.FcmError","errorCode":"INVALID_ARGUMENT"}]}}"#;
        assert!(fcm_token_gone(400, bad_token));
        let field = r#"{"error":{"status":"INVALID_ARGUMENT","details":[{"@type":"type.googleapis.com/google.rpc.BadRequest","fieldViolations":[{"field":"message.token","description":"Invalid registration token"}]}]}}"#;
        assert!(fcm_token_gone(400, field));
        let payload = r#"{"error":{"code":400,"message":"Invalid value at 'message.android.notification.tag'","status":"INVALID_ARGUMENT","details":[{"@type":"type.googleapis.com/google.rpc.BadRequest","fieldViolations":[{"field":"message.android.notification.tag"}]}]}}"#;
        assert!(!fcm_token_gone(400, payload));
        assert!(!fcm_token_gone(500, "internal"));
        assert!(!fcm_token_gone(
            429,
            r#"{"error":{"status":"RESOURCE_EXHAUSTED"}}"#
        ));
    }

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
        assert!(in_quiet_hours(1320, 480, 0, at(23, 0)));
        assert!(in_quiet_hours(1320, 480, 0, at(0, 0)));
        assert!(!in_quiet_hours(1320, 480, 0, at(8, 0)));
        assert!(!in_quiet_hours(1320, 480, 0, at(21, 59)));
        assert!(!in_quiet_hours(1320, 480, -300, at(2, 0)));
        assert!(in_quiet_hours(1320, 480, -300, at(4, 0)));
        assert!(in_quiet_hours(780, 840, 0, at(13, 30)));
        assert!(!in_quiet_hours(780, 840, 0, at(14, 0)));
        assert!(!in_quiet_hours(600, 600, 0, at(10, 0)));
    }
}

#[cfg(test)]
mod presence_tests {
    use super::Presence;
    use crate::platform::ids::{RoomId, UserId};

    fn seat(user: i64, room: i64) -> (UserId, RoomId) {
        (UserId::from_raw(user), RoomId::from_raw(room))
    }

    #[test]
    fn presence_per_room_and_connection() {
        let presence = Presence::default();
        let (phone, laptop) = (presence.connection(), presence.connection());
        assert!(!presence.is_here(seat(1, 10).0, seat(1, 10).1));
        presence.here(seat(1, 10).0, seat(1, 10).1, phone);
        presence.here(seat(1, 10).0, seat(1, 10).1, laptop);
        assert!(presence.is_here(seat(1, 10).0, seat(1, 10).1));
        assert!(
            !presence.is_here(seat(1, 11).0, seat(1, 11).1),
            "another room of the same account"
        );
        assert!(
            !presence.is_here(seat(2, 10).0, seat(2, 10).1),
            "another account in the same room"
        );
        presence.away(seat(1, 10).0, seat(1, 10).1, phone);
        assert!(
            presence.is_here(seat(1, 10).0, seat(1, 10).1),
            "still open on the other device"
        );
        presence.away(seat(1, 10).0, seat(1, 10).1, laptop);
        assert!(!presence.is_here(seat(1, 10).0, seat(1, 10).1));
    }
}
