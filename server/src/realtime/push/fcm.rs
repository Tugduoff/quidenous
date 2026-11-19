use serde::Deserialize;
use tokio::sync::Mutex;

use super::now_secs;

#[derive(Deserialize)]
pub(crate) struct ServiceAccount {
    pub(crate) project_id: String,
    client_email: String,
    private_key: String,
    token_uri: String,
}

pub(crate) struct Fcm {
    pub(crate) account: ServiceAccount,
    token: Mutex<Option<(String, u64)>>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
}

impl Fcm {
    pub(crate) fn from_json(json: &str) -> Result<Self, String> {
        let account: ServiceAccount =
            serde_json::from_str(json).map_err(|e| format!("FCM service account: {e}"))?;
        Ok(Self {
            account,
            token: Mutex::new(None),
        })
    }

    pub(crate) async fn access_token(&self, http: &reqwest::Client) -> Result<String, String> {
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
