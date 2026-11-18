use aes_gcm::{Aes128Gcm, KeyInit, Nonce, aead::Aead};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use hkdf::Hkdf;
use p256::{
    PublicKey, SecretKey,
    ecdsa::{Signature, SigningKey, signature::Signer},
    elliptic_curve::{Generate, sec1::ToSec1Point},
};
use sha2::Sha256;

use super::*;

pub struct Vapid {
    key: SigningKey,
    pub public_b64: String,
    subject: String,
}

impl Vapid {
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

    pub(crate) fn authorization(&self, endpoint: &str) -> Option<String> {
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

pub(super) fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

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
    plaintext.push(2);
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

pub fn generate_vapid() -> (String, String) {
    let secret = SecretKey::generate_from_rng(&mut rand::rng());
    let public = secret.public_key().to_sec1_point(false);
    (B64.encode(secret.to_bytes()), B64.encode(public.as_bytes()))
}
