//! Email: Mailgun's HTTP API in production, a log line when not configured, a recorder in tests.

use std::{sync::Mutex, time::Duration};

use futures::future::BoxFuture;

use crate::config::Mailgun;

#[derive(Clone, Debug)]
pub struct Email {
    pub to: String,
    pub subject: String,
    pub text: String,
    pub html: String,
}

pub trait Mailer: Send + Sync {
    /// Does this mailer really deliver? (`false`: the API says `email_sent: false`.)
    fn configured(&self) -> bool;
    fn send(&self, email: Email) -> BoxFuture<'_, Result<(), String>>;
}

/// Not configured: the email body goes to the log (so a dev or the admin can still read the code).
pub struct LogMailer;

impl Mailer for LogMailer {
    fn configured(&self) -> bool {
        false
    }

    fn send(&self, email: Email) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(async move {
            tracing::info!(to = %email.to, subject = %email.subject, body = %email.text, "email (not sent: Mailgun not configured)");
            Ok(())
        })
    }
}

pub struct MailgunMailer {
    config: Mailgun,
    http: reqwest::Client,
}

impl MailgunMailer {
    pub fn new(config: Mailgun) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self { config, http }
    }
}

impl Mailer for MailgunMailer {
    fn configured(&self) -> bool {
        true
    }

    fn send(&self, email: Email) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(async move {
            let url = format!(
                "{}/v3/{}/messages",
                self.config.base_url, self.config.domain
            );
            let res = self
                .http
                .post(url)
                .basic_auth("api", Some(&self.config.api_key))
                .form(
                    &[
                        ("from", self.config.from.as_str()),
                        ("to", email.to.as_str()),
                        ("subject", email.subject.as_str()),
                        ("text", email.text.as_str()),
                        ("html", email.html.as_str()),
                    ]
                    .into_iter()
                    .chain(self.config.reply_to.as_deref().map(|r| ("h:Reply-To", r)))
                    .collect::<Vec<_>>(),
                )
                .send()
                .await
                .map_err(|e| e.to_string())?;
            if res.status().is_success() {
                Ok(())
            } else {
                let status = res.status();
                let body = res.text().await.unwrap_or_default();
                Err(format!(
                    "mailgun {status}: {}",
                    body.chars().take(200).collect::<String>()
                ))
            }
        })
    }
}

/// Keeps every email in memory (tests read the codes from here).
#[derive(Default)]
pub struct MemoryMailer {
    pub sent: Mutex<Vec<Email>>,
}

impl MemoryMailer {
    /// The six-digit code in the last email to this address.
    pub fn last_code(&self, to: &str) -> Option<String> {
        let sent = self.sent.lock().unwrap_or_else(|e| e.into_inner());
        let email = sent.iter().rev().find(|e| e.to == to)?;
        email
            .text
            .split(|c: char| !c.is_ascii_digit())
            .find(|w| w.len() == 6)
            .map(str::to_owned)
    }

    pub fn count(&self, to: &str) -> usize {
        let sent = self.sent.lock().unwrap_or_else(|e| e.into_inner());
        sent.iter().filter(|e| e.to == to).count()
    }
}

impl Mailer for MemoryMailer {
    fn configured(&self) -> bool {
        true
    }

    fn send(&self, email: Email) -> BoxFuture<'_, Result<(), String>> {
        self.sent
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(email);
        Box::pin(async { Ok(()) })
    }
}

fn layout(title: &str, intro: &str, code: &str, outro: &str) -> String {
    format!(
        "<!doctype html><html lang=\"fr\"><body style=\"margin:0;padding:24px;background:#f4f1ec;font-family:-apple-system,Segoe UI,Roboto,sans-serif;color:#1d1a16\">\
<div style=\"max-width:420px;margin:0 auto;background:#fff;border-radius:16px;padding:28px\">\
<h1 style=\"font-size:20px;margin:0 0 12px\">{title}</h1>\
<p style=\"font-size:15px;line-height:1.5;margin:0 0 20px\">{intro}</p>\
<p style=\"font-size:36px;font-weight:700;letter-spacing:8px;margin:0 0 20px;text-align:center\">{code}</p>\
<p style=\"font-size:13px;line-height:1.5;color:#6b645b;margin:0\">{outro}</p>\
</div></body></html>"
    )
}

pub fn verify_email(to: &str, code: &str) -> Email {
    let intro = "Voici ton code pour vérifier ton adresse e-mail sur Qui de nous ?";
    let outro = "Il expire dans 15 minutes. Si tu n'as rien demandé, ignore cet e-mail.";
    Email {
        to: to.to_owned(),
        subject: format!("{code} · ton code Qui de nous ?"),
        text: format!("{intro}\n\n{code}\n\n{outro}\n"),
        html: layout("Vérifie ton adresse", intro, code, outro),
    }
}

pub fn reset_password(to: &str, code: &str) -> Email {
    let intro = "Voici ton code pour choisir un nouveau mot de passe sur Qui de nous ?";
    let outro = "Il expire dans 30 minutes. Si tu n'as rien demandé, ignore cet e-mail : ton mot de passe ne change pas.";
    Email {
        to: to.to_owned(),
        subject: format!("{code} · nouveau mot de passe Qui de nous ?"),
        text: format!("{intro}\n\n{code}\n\n{outro}\n"),
        html: layout("Mot de passe oublié", intro, code, outro),
    }
}
