//! Keys, password hashing, keyed code hashes, sealed TOTP secrets, TOTP itself.

use std::{
    path::Path,
    sync::{Arc, OnceLock},
};

use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use rand::{Rng, RngExt};
use sha2::Sha256;
use tokio::sync::Semaphore;

use crate::error::{AppError, AppResult};

/// Keys derived from `AUTH_SECRET` (or `DATA_DIR/auth.key`).
pub struct Keys {
    seal: [u8; 32],
    mac: [u8; 32],
}

impl Keys {
    pub fn from_secret(secret: &[u8]) -> Self {
        let hk = Hkdf::<Sha256>::new(Some(b"quidenous auth v1"), secret);
        let (mut seal, mut mac) = ([0u8; 32], [0u8; 32]);
        hk.expand(b"totp secret aes-256-gcm", &mut seal)
            .expect("32 bytes is a valid HKDF length");
        hk.expand(b"code hmac-sha256", &mut mac)
            .expect("32 bytes is a valid HKDF length");
        Self { seal, mac }
    }

    /// `AUTH_SECRET` when set, else the generated `auth.key` in the data dir (created once, 0600).
    pub fn load(secret: Option<&str>, data_dir: &Path) -> std::io::Result<Self> {
        if let Some(secret) = secret {
            if secret.len() < 32 {
                tracing::warn!("AUTH_SECRET is short: use 32+ random bytes (openssl rand -hex 32)");
            }
            return Ok(Self::from_secret(secret.as_bytes()));
        }
        let path = data_dir.join("auth.key");
        match std::fs::read_to_string(&path) {
            Ok(s) if !s.trim().is_empty() => Ok(Self::from_secret(s.trim().as_bytes())),
            _ => {
                let secret = random_hex(32);
                std::fs::create_dir_all(data_dir)?;
                write_private(&path, &secret)?;
                tracing::warn!(
                    path = %path.display(),
                    "AUTH_SECRET not set: generated a key file. Set AUTH_SECRET and keep it out of backups"
                );
                Ok(Self::from_secret(secret.as_bytes()))
            }
        }
    }

    /// Keyed hash of a short secret (codes, challenges, recovery codes), hex.
    pub fn mac(&self, parts: &[&str]) -> String {
        let mut m = <Hmac<Sha256> as KeyInit>::new_from_slice(&self.mac)
            .expect("HMAC takes any key length");
        for (i, p) in parts.iter().enumerate() {
            if i > 0 {
                m.update(b"\x1f");
            }
            m.update(p.as_bytes());
        }
        hex::encode(m.finalize().into_bytes())
    }

    /// AES-256-GCM, random nonce, `base64(nonce || ciphertext)`.
    pub fn seal(&self, plain: &[u8]) -> String {
        let cipher = Aes256Gcm::new_from_slice(&self.seal).expect("32-byte key");
        let mut nonce = [0u8; 12];
        rand::rng().fill_bytes(&mut nonce);
        let mut out = nonce.to_vec();
        out.extend(
            cipher
                .encrypt(&Nonce::from(nonce), plain)
                .expect("AES-GCM encryption does not fail on small inputs"),
        );
        STANDARD.encode(out)
    }

    pub fn open(&self, sealed: &str) -> Option<Vec<u8>> {
        let bytes = STANDARD.decode(sealed).ok()?;
        if bytes.len() < 12 {
            return None;
        }
        let (nonce, ct) = bytes.split_at(12);
        let nonce: [u8; 12] = nonce.try_into().ok()?;
        Aes256Gcm::new_from_slice(&self.seal)
            .ok()?
            .decrypt(&Nonce::from(nonce), ct)
            .ok()
    }
}

#[cfg(unix)]
fn write_private(path: &Path, content: &str) -> std::io::Result<()> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(content.as_bytes())
}

#[cfg(not(unix))]
fn write_private(path: &Path, content: &str) -> std::io::Result<()> {
    std::fs::write(path, content)
}

pub fn random_hex(bytes: usize) -> String {
    let mut b = vec![0u8; bytes];
    rand::rng().fill_bytes(&mut b);
    hex::encode(b)
}

pub fn random_b64url(bytes: usize) -> String {
    let mut b = vec![0u8; bytes];
    rand::rng().fill_bytes(&mut b);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

/// Six digits, uniform.
pub fn six_digits() -> String {
    format!("{:06}", rand::rng().random_range(0..1_000_000u32))
}

const RECOVERY_ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";

/// `xxxxx-xxxxx`, no look-alikes.
pub fn recovery_code() -> String {
    let mut rng = rand::rng();
    let mut s = String::with_capacity(11);
    for i in 0..10 {
        if i == 5 {
            s.push('-');
        }
        s.push(RECOVERY_ALPHABET[rng.random_range(0..RECOVERY_ALPHABET.len())] as char);
    }
    s
}

/// What a recovery code is hashed as: lowercase, no dash or spaces.
pub fn normalize_recovery(code: &str) -> String {
    code.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

pub fn constant_eq(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

// ---------- passwords ----------

fn argon2() -> Argon2<'static> {
    Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(19 * 1024, 2, 1, None).expect("valid argon2 params"),
    )
}

/// At most two hashes at once (each takes 19 MiB and ~50 ms of a core).
#[derive(Clone)]
pub struct Hasher(Arc<Semaphore>);

impl Default for Hasher {
    fn default() -> Self {
        Self(Arc::new(Semaphore::new(2)))
    }
}

impl Hasher {
    async fn run<T: Send + 'static>(&self, f: impl FnOnce() -> T + Send + 'static) -> AppResult<T> {
        let _permit = self
            .0
            .acquire()
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        tokio::task::spawn_blocking(f)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))
    }

    pub async fn hash(&self, password: &str) -> AppResult<String> {
        let password = password.to_owned();
        self.run(move || {
            argon2()
                .hash_password(password.as_bytes())
                .map(|h| h.to_string())
                .map_err(|e| AppError::Internal(e.to_string()))
        })
        .await?
    }

    /// `None` hash (unknown email, no password): still pays for one hash, then says no.
    pub async fn verify(&self, password: &str, hash: Option<&str>) -> AppResult<bool> {
        let password = password.to_owned();
        let hash = hash.map(str::to_owned);
        self.run(move || {
            let (phc, real) = match &hash {
                Some(h) => (h.as_str(), true),
                None => (dummy_hash(), false),
            };
            let ok = PasswordHash::new(phc)
                .map(|parsed| {
                    argon2()
                        .verify_password(password.as_bytes(), &parsed)
                        .is_ok()
                })
                .unwrap_or(false);
            ok && real
        })
        .await
    }
}

fn dummy_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| {
        argon2()
            .hash_password(random_hex(16).as_bytes())
            .map(|h| h.to_string())
            .unwrap_or_default()
    })
}

// ---------- TOTP (RFC 6238: SHA-1, 6 digits, 30 s) ----------

pub const TOTP_PERIOD: u64 = 30;

pub fn new_totp_secret() -> String {
    let mut b = [0u8; 20];
    rand::rng().fill_bytes(&mut b);
    data_encoding::BASE32_NOPAD.encode(&b)
}

fn decode_secret(secret_b32: &str) -> Option<Vec<u8>> {
    let clean: String = secret_b32
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '=')
        .map(|c| c.to_ascii_uppercase())
        .collect();
    data_encoding::BASE32_NOPAD.decode(clean.as_bytes()).ok()
}

/// The code for one 30 s step.
pub fn totp_at_step(secret_b32: &str, step: u64) -> Option<String> {
    let key = decode_secret(secret_b32)?;
    let mut m = <Hmac<sha1::Sha1> as KeyInit>::new_from_slice(&key).ok()?;
    m.update(&step.to_be_bytes());
    let h = m.finalize().into_bytes();
    let offset = (h[h.len() - 1] & 0x0f) as usize;
    let bin = (u32::from(h[offset]) & 0x7f) << 24
        | u32::from(h[offset + 1]) << 16
        | u32::from(h[offset + 2]) << 8
        | u32::from(h[offset + 3]);
    Some(format!("{:06}", bin % 1_000_000))
}

/// The code at a unix time (what an authenticator app shows).
pub fn totp_code(secret_b32: &str, unix_time: u64) -> Option<String> {
    totp_at_step(secret_b32, unix_time / TOTP_PERIOD)
}

/// The step a code matches within ±1 step of now, if any.
pub fn totp_match(secret_b32: &str, code: &str, unix_time: u64) -> Option<u64> {
    let code = code.trim();
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let now = unix_time / TOTP_PERIOD;
    [now.saturating_sub(1), now, now + 1]
        .into_iter()
        .find(|s| totp_at_step(secret_b32, *s).is_some_and(|c| constant_eq(&c, code)))
}

pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Percent-encoding for URL parts (unreserved characters pass).
pub fn pct(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub const ISSUER: &str = "Qui de nous ?";

pub fn otpauth_url(secret_b32: &str, account: &str) -> String {
    format!(
        "otpauth://totp/{}:{}?secret={secret_b32}&issuer={}&algorithm=SHA1&digits=6&period=30",
        pct(ISSUER),
        pct(account),
        pct(ISSUER)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc6238_sha1_vectors() {
        // RFC 6238 appendix B, the 20-byte ASCII secret "12345678901234567890", 8 digits truncated to 6.
        let secret = data_encoding::BASE32_NOPAD.encode(b"12345678901234567890");
        assert_eq!(totp_code(&secret, 59).unwrap(), "287082");
        assert_eq!(totp_code(&secret, 1111111109).unwrap(), "081804");
        assert_eq!(totp_code(&secret, 1234567890).unwrap(), "005924");
        assert_eq!(totp_code(&secret, 2000000000).unwrap(), "279037");
    }

    #[test]
    fn totp_window_is_one_step() {
        let secret = new_totp_secret();
        let t = 1_700_000_000;
        let prev = totp_code(&secret, t - 30).unwrap();
        let next = totp_code(&secret, t + 30).unwrap();
        let far = totp_code(&secret, t + 90).unwrap();
        assert_eq!(totp_match(&secret, &prev, t), Some(t / 30 - 1));
        assert_eq!(totp_match(&secret, &next, t), Some(t / 30 + 1));
        if far != prev && far != next && far != totp_code(&secret, t).unwrap() {
            assert_eq!(totp_match(&secret, &far, t), None);
        }
    }

    #[test]
    fn sealing_round_trips_and_detects_tampering() {
        let keys = Keys::from_secret(b"test secret");
        let sealed = keys.seal(b"JBSWY3DPEHPK3PXP");
        assert_eq!(keys.open(&sealed).unwrap(), b"JBSWY3DPEHPK3PXP");
        assert!(Keys::from_secret(b"other").open(&sealed).is_none());
        assert_ne!(keys.mac(&["a", "b"]), keys.mac(&["ab"]));
    }

    #[test]
    fn recovery_codes_look_right() {
        let c = recovery_code();
        assert_eq!(c.len(), 11);
        assert_eq!(&c[5..6], "-");
        assert_eq!(normalize_recovery(&c.to_uppercase()), c.replace('-', ""));
    }
}
