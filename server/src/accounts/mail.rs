use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use futures::future::BoxFuture;

use crate::{
    platform::config::Mailgun,
    platform::i18n::{self, Lang},
};

#[derive(Clone, Debug)]
pub struct Email {
    pub to: String,
    pub subject: String,
    pub text: String,
    pub html: String,
    pub notice: bool,
}

pub trait Mailer: Send + Sync {
    fn configured(&self) -> bool;
    fn send(&self, email: Email) -> BoxFuture<'_, Result<(), String>>;
}

pub struct LogMailer;

impl Mailer for LogMailer {
    fn configured(&self) -> bool {
        false
    }

    fn send(&self, email: Email) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(async move {
            tracing::info!(to = %mask_email(&email.to), subject = %email.subject, "email (not sent: Mailgun not configured)");
            tracing::debug!(to = %email.to, body = %email.text, "email body (not sent)");
            Ok(())
        })
    }
}

pub(crate) fn mask_email(email: &str) -> String {
    match email.split_once('@') {
        Some((user, domain)) => {
            let first: String = user.chars().take(1).collect();
            format!("{first}***@{domain}")
        }
        None => "***".to_owned(),
    }
}

pub struct MailgunMailer {
    config: Mailgun,
    http: reqwest::Client,
}

impl MailgunMailer {
    pub(crate) fn new(config: Mailgun) -> Self {
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

pub(crate) const FAILURE_WINDOW: Duration = Duration::from_mins(15);

pub struct Watched {
    inner: Arc<dyn Mailer>,
    failed_at: Mutex<Option<Instant>>,
}

impl Watched {
    pub(crate) fn new(inner: Arc<dyn Mailer>) -> Self {
        Self {
            inner,
            failed_at: Mutex::new(None),
        }
    }

    pub(crate) fn configured(&self) -> bool {
        self.inner.configured()
    }

    pub(crate) fn healthy(&self) -> bool {
        let failed = self
            .failed_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        failed.is_none_or(|t| t.elapsed() >= FAILURE_WINDOW)
    }

    pub async fn send(&self, email: Email) -> Result<(), String> {
        let result = self.inner.send(email).await;
        *self
            .failed_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            result.is_err().then(Instant::now);
        result
    }
}

#[derive(Default)]
pub struct MemoryMailer {
    pub sent: Mutex<Vec<Email>>,
    pub notices: Mutex<Vec<Email>>,
    pub fail: AtomicBool,
}

impl MemoryMailer {
    pub fn last_code(&self, to: &str) -> Option<String> {
        let sent = self
            .sent
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let email = sent.iter().rev().find(|e| e.to == to)?;
        email
            .text
            .split(|c: char| !c.is_ascii_digit())
            .find(|w| w.len() == 6)
            .map(str::to_owned)
    }

    pub fn count(&self, to: &str) -> usize {
        let sent = self
            .sent
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        sent.iter().filter(|e| e.to == to).count()
    }

    pub fn notices(&self, to: &str) -> Vec<String> {
        let sent = self
            .notices
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        sent.iter()
            .filter(|e| e.to == to)
            .map(|e| e.subject.clone())
            .collect()
    }
}

impl Mailer for MemoryMailer {
    fn configured(&self) -> bool {
        true
    }

    fn send(&self, email: Email) -> BoxFuture<'_, Result<(), String>> {
        if self.fail.load(Ordering::Relaxed) {
            return Box::pin(async { Err("refused (test)".to_owned()) });
        }
        let list = if email.notice {
            &self.notices
        } else {
            &self.sent
        };
        list.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(email);
        Box::pin(async { Ok(()) })
    }
}

fn layout(lang: Lang, title: &str, intro: &str, code: &str, outro: &str) -> String {
    let lang = lang.as_str();
    format!(
        "<!doctype html><html lang=\"{lang}\"><body style=\"margin:0;padding:24px;background:#f4f1ec;font-family:-apple-system,Segoe UI,Roboto,sans-serif;color:#1d1a16\">\
<div style=\"max-width:420px;margin:0 auto;background:#fff;border-radius:16px;padding:28px\">\
<h1 style=\"font-size:20px;margin:0 0 12px\">{title}</h1>\
<p style=\"font-size:15px;line-height:1.5;margin:0 0 20px\">{intro}</p>\
<p style=\"font-size:36px;font-weight:700;letter-spacing:8px;margin:0 0 20px;text-align:center\">{code}</p>\
<p style=\"font-size:13px;line-height:1.5;color:#6b645b;margin:0\">{outro}</p>\
</div></body></html>"
    )
}

fn code_email(to: &str, code: &str, lang: Lang, m: i18n::CodeMail) -> Email {
    Email {
        to: to.to_owned(),
        subject: m.subject,
        text: format!("{}\n\n{code}\n\n{}\n", m.intro, m.outro),
        html: layout(lang, m.heading, m.intro, code, m.outro),
        notice: false,
    }
}

pub(crate) fn verify_email(to: &str, code: &str, lang: Lang) -> Email {
    code_email(to, code, lang, i18n::verify_email(lang, code))
}

pub(crate) fn reset_password(to: &str, code: &str, lang: Lang) -> Email {
    code_email(to, code, lang, i18n::reset_password(lang, code))
}

pub(crate) fn delete_account(to: &str, code: &str, lang: Lang) -> Email {
    code_email(to, code, lang, i18n::delete_account(lang, code))
}

pub(crate) fn change_email(to: &str, code: &str, lang: Lang) -> Email {
    code_email(to, code, lang, i18n::change_email(lang, code))
}

pub(crate) fn mfa_reset(to: &str, code: &str, lang: Lang) -> Email {
    code_email(to, code, lang, i18n::mfa_reset(lang, code))
}

pub(crate) fn notice(to: &str, lang: Lang, n: &i18n::Notice) -> Email {
    let m = i18n::notice(lang, n);
    let html = format!(
        "<!doctype html><html lang=\"{}\"><body style=\"margin:0;padding:24px;background:#f4f1ec;font-family:-apple-system,Segoe UI,Roboto,sans-serif;color:#1d1a16\">\
<div style=\"max-width:420px;margin:0 auto;background:#fff;border-radius:16px;padding:28px\">\
<h1 style=\"font-size:20px;margin:0 0 12px\">{}</h1>\
<p style=\"font-size:15px;line-height:1.5;margin:0 0 20px\">{}</p>\
<p style=\"font-size:13px;line-height:1.5;color:#6b645b;margin:0\">{}</p>\
</div></body></html>",
        lang.as_str(),
        escape(&m.subject),
        escape(&m.body),
        escape(&m.outro)
    );
    Email {
        to: to.to_owned(),
        text: format!("{}\n\n{}\n", m.body, m.outro),
        subject: m.subject,
        html,
        notice: true,
    }
}

pub(crate) fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub(crate) fn report(to: &str, subject: &str, lines: &[(&str, String)]) -> Email {
    let text: String = lines.iter().map(|(k, v)| format!("{k} : {v}\n")).collect();
    let rows: String = lines
        .iter()
        .map(|(k, v)| {
            format!(
                "<tr><td style=\"padding:4px 12px 4px 0;color:#6b645b;vertical-align:top;white-space:nowrap\">{}</td>\
<td style=\"padding:4px 0;white-space:pre-wrap\">{}</td></tr>",
                escape(k),
                escape(v)
            )
        })
        .collect();
    let html = format!(
        "<!doctype html><html lang=\"fr\"><body style=\"margin:0;padding:24px;background:#f4f1ec;font-family:-apple-system,Segoe UI,Roboto,sans-serif;color:#1d1a16\">\
<div style=\"max-width:560px;margin:0 auto;background:#fff;border-radius:16px;padding:28px\">\
<h1 style=\"font-size:20px;margin:0 0 12px\">{}</h1>\
<table style=\"font-size:14px;line-height:1.5;border-collapse:collapse\">{rows}</table>\
</div></body></html>",
        escape(subject)
    );
    Email {
        to: to.to_owned(),
        subject: subject.to_owned(),
        text,
        html,
        notice: false,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn logged_addresses_are_masked() {
        assert_eq!(super::mask_email("ana@example.com"), "a***@example.com");
        assert_eq!(super::mask_email("nope"), "***");
    }
}
