//! Accounts: email + password, codes, 2FA, sessions, rate limits, legacy accounts, OAuth (mock providers).

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    Form, Router,
    body::Body,
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    routing::{get, post},
};
use base64::Engine;
use http_body_util::BodyExt;
use quidenous_server::{
    AppState,
    accounts::{MemoryMailer, totp_code},
    app,
    config::{Config, OAuthClient, OAuthEndpoints},
    state, state_with_mailer,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tower::ServiceExt;

const PUBLIC_URL: &str = "https://qdn.test";
const PASSWORD: &str = "velo-rouge-du-dimanche";

fn base_config() -> Config {
    let dir = std::env::temp_dir().join(format!(
        "qdn-auth-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    Config {
        database_url: format!("sqlite://{}?mode=rwc", dir.join("test.db").display()),
        data_dir: dir,
        group_name: "Test".into(),
        join_code: "CODE-1".into(),
        vapid_subject: "mailto:test@example.com".into(),
        auth_secret: Some("test-secret-test-secret-test-secret".into()),
        // Tests pick their client IP with X-Forwarded-For.
        trust_proxy: true,
        ..Config::default()
    }
}

struct T {
    app: Router,
    state: AppState,
    mail: Arc<MemoryMailer>,
}

async fn setup_with(config: Config) -> T {
    let mail = Arc::new(MemoryMailer::default());
    let state = state_with_mailer(config, mail.clone()).await.unwrap();
    T {
        app: app(state.clone()),
        state,
        mail,
    }
}

async fn setup() -> T {
    setup_with(base_config()).await
}

struct Res {
    status: StatusCode,
    body: Value,
    headers: HeaderMap,
}

async fn send(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
    headers: &[(&str, &str)],
) -> Res {
    let mut req = Request::builder().method(method).uri(format!("/api{path}"));
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let req = match body {
        Some(b) => req
            .header("content-type", "application/json")
            .body(Body::from(b.to_string())),
        None => req.body(Body::empty()),
    }
    .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    Res {
        status,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        headers,
    }
}

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let r = send(app, method, path, token, body, &[]).await;
    (r.status, r.body)
}

async fn signup(t: &T, email: &str, name: &str) -> (String, i64) {
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/signup",
        None,
        Some(json!({ "email": email, "password": PASSWORD, "name": name })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["new"], true);
    (
        b["token"].as_str().unwrap().to_owned(),
        b["user_id"].as_i64().unwrap(),
    )
}

async fn verify_email(t: &T, token: &str, email: &str) {
    let code = t.mail.last_code(email).unwrap();
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/verify",
        Some(token),
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!((s, &b["email_verified"]), (StatusCode::OK, &json!(true)));
}

async fn login(t: &T, email: &str, password: &str) -> (StatusCode, Value) {
    call(
        &t.app,
        "POST",
        "/auth/login",
        None,
        Some(json!({ "email": email, "password": password })),
    )
    .await
}

async fn legacy_join(app: &Router, name: &str) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        "/join",
        None,
        Some(json!({ "code": "code-1", "name": name })),
    )
    .await
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// The reset email is sent in the background.
async fn wait_for_code(mail: &MemoryMailer, to: &str, count: usize) -> String {
    for _ in 0..100 {
        if mail.count(to) >= count {
            return mail.last_code(to).unwrap();
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("no email to {to}");
}

#[tokio::test]
async fn sign_up_verify_sign_in_and_join_a_room() {
    let t = setup().await;
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/email",
        None,
        Some(json!({ "email": "ana@example.com" })),
    )
    .await;
    assert_eq!((s, &b["account"]), (StatusCode::OK, &json!(false)));
    for (email, password, name, error) in [
        ("nope", PASSWORD, "Ana", "bad_email"),
        ("ana@example.com", PASSWORD, " ", "bad_name"),
        ("ana@example.com", "short", "Ana", "weak_password"),
        ("ana@example.com", "password123", "Ana", "common_password"),
        ("ana@example.com", "ana@example.com", "Ana", "weak_password"),
    ] {
        let (s, b) = call(
            &t.app,
            "POST",
            "/auth/signup",
            None,
            Some(json!({ "email": email, "password": password, "name": name })),
        )
        .await;
        assert_eq!(
            (s, b["error"].as_str()),
            (StatusCode::BAD_REQUEST, Some(error))
        );
    }
    let r = send(
        &t.app,
        "POST",
        "/auth/signup",
        None,
        Some(json!({ "email": " Ana@Example.com", "password": PASSWORD, "name": "Ana" })),
        &[("x-device", "iPhone%20%C2%B7%20iOS%2018")],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["email_sent"], true);
    let token = r.body["token"].as_str().unwrap().to_owned();
    assert_eq!(t.mail.count("ana@example.com"), 1);
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/signup",
        None,
        Some(json!({ "email": "ana@example.com", "password": PASSWORD, "name": "Ana" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("email_taken"))
    );
    let (_, b) = call(
        &t.app,
        "POST",
        "/auth/email",
        None,
        Some(json!({ "email": "ANA@example.com" })),
    )
    .await;
    assert_eq!(b["account"], true);

    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&token), None).await;
    assert_eq!(acct["email"], "ana@example.com");
    assert_eq!(acct["email_verified"], false);
    assert_eq!(acct["has_password"], true);
    assert_eq!(acct["secured"], true);
    assert_eq!(acct["totp"], false);

    // Verify: a wrong code costs a try, the right one works; resend replaces the code.
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/verify",
        Some(&token),
        Some(json!({ "code": "000000" })),
    )
    .await;
    if t.mail.last_code("ana@example.com").as_deref() != Some("000000") {
        assert_eq!(
            (s, b["error"].as_str()),
            (StatusCode::BAD_REQUEST, Some("bad_code"))
        );
    }
    let old = t.mail.last_code("ana@example.com").unwrap();
    let (s, b) = call(&t.app, "POST", "/auth/verify/send", Some(&token), None).await;
    assert_eq!((s, &b["email_sent"]), (StatusCode::OK, &json!(true)));
    let new = t.mail.last_code("ana@example.com").unwrap();
    if old != new {
        let (s, _) = call(
            &t.app,
            "POST",
            "/auth/verify",
            Some(&token),
            Some(json!({ "code": old })),
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
    }
    verify_email(&t, &token, "ana@example.com").await;
    let (s, b) = call(&t.app, "POST", "/auth/verify/send", Some(&token), None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("already_verified"))
    );

    // No room yet: 403 no_room (not 401, which would sign the app out).
    let (s, b) = call(&t.app, "GET", "/me", Some(&token), None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("no_room"))
    );
    let (s, room) = call(
        &t.app,
        "POST",
        "/rooms",
        Some(&token),
        Some(json!({ "name": "Les potes" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert!(room["id"].as_i64().is_some());
    let (s, me) = call(&t.app, "GET", "/me", Some(&token), None).await;
    assert_eq!((s, me["name"].as_str()), (StatusCode::OK, Some("Ana")));

    // Sign in: wrong password and unknown email look the same.
    let (s1, b1) = login(&t, "ana@example.com", "wrong-password-1").await;
    let (s2, b2) = login(&t, "nobody@example.com", PASSWORD).await;
    assert_eq!(
        (s1, &b1),
        (
            StatusCode::BAD_REQUEST,
            &json!({ "error": "bad_credentials" })
        )
    );
    assert_eq!((s2, &b2), (s1, &b1));
    let (s, b) = login(&t, "ANA@example.com", PASSWORD).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["new"], false);
    let second = b["token"].as_str().unwrap();
    let (s, _) = call(&t.app, "GET", "/me", Some(second), None).await;
    assert_eq!(s, StatusCode::OK);

    // The session remembers the decoded device label.
    let (_, sessions) = call(&t.app, "GET", "/auth/sessions", Some(&token), None).await;
    let mine = sessions
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["current"] == true)
        .unwrap();
    assert_eq!(mine["device"], "iPhone · iOS 18");
    assert!(mine["created_at"].as_str().unwrap().contains('T'));
}

#[tokio::test]
async fn failed_sign_ins_lock_the_account_and_ips_are_limited() {
    let t = setup().await;
    signup(&t, "bob@example.com", "Bob").await;
    for _ in 0..5 {
        let (s, _) = login(&t, "bob@example.com", "not-the-password").await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
    }
    // Locked, even with the right password.
    let r = send(
        &t.app,
        "POST",
        "/auth/login",
        None,
        Some(json!({ "email": "bob@example.com", "password": PASSWORD })),
        &[],
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(r.body["error"], "rate_limited");
    let wait = r.body["retry_after"].as_u64().unwrap();
    assert!((1..=180).contains(&wait), "{wait}");
    assert!(r.headers.get("retry-after").is_some());

    // Per IP: 10 sign-in attempts per 10 minutes, whatever the email. The IP is the last
    // X-Forwarded-For entry (the proxy's): changing the ones before it changes nothing.
    for i in 0..10 {
        let forwarded = format!("198.18.0.{i}, 203.0.113.7");
        let r = send(
            &t.app,
            "POST",
            "/auth/login",
            None,
            Some(json!({ "email": format!("x{i}@example.com"), "password": "whatever-123" })),
            &[("x-forwarded-for", &forwarded)],
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST);
    }
    let r = send(
        &t.app,
        "POST",
        "/auth/login",
        None,
        Some(json!({ "email": "x@example.com", "password": "whatever-123" })),
        &[("x-forwarded-for", "203.0.113.7")],
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
    let r = send(
        &t.app,
        "POST",
        "/auth/login",
        None,
        Some(json!({ "email": "x@example.com", "password": "whatever-123" })),
        &[("x-forwarded-for", "203.0.113.8")],
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);

    // Legacy join/start: 10 per hour per IP.
    for i in 0..10 {
        let r = send(
            &t.app,
            "POST",
            "/join",
            None,
            Some(json!({ "code": "code-1", "name": format!("P{i}") })),
            &[("x-forwarded-for", "198.51.100.1")],
        )
        .await;
        assert_eq!(r.status, StatusCode::OK);
    }
    let r = send(
        &t.app,
        "POST",
        "/start",
        None,
        Some(json!({ "name": "Zoe", "room": "Chez Zoe" })),
        &[("x-forwarded-for", "198.51.100.1")],
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn two_factor_setup_sign_in_recovery_and_disable() {
    let t = setup().await;
    let (token, user) = signup(&t, "cyd@example.com", "Cyd").await;
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/2fa/enable",
        Some(&token),
        Some(json!({ "code": "123456" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("no_setup"))
    );
    let (s, setup) = call(&t.app, "POST", "/auth/2fa/setup", Some(&token), None).await;
    assert_eq!(s, StatusCode::OK);
    let secret = setup["secret"].as_str().unwrap().to_owned();
    let url = setup["otpauth_url"].as_str().unwrap();
    assert!(url.starts_with("otpauth://totp/Qui%20de%20nous%20%3F:cyd%40example.com?secret="));
    assert!(url.contains("issuer=Qui%20de%20nous%20%3F"));

    // Another device, to be signed out when 2FA goes on.
    let (_, other) = login(&t, "cyd@example.com", PASSWORD).await;
    let other = other["token"].as_str().unwrap().to_owned();

    let wrong = if totp_code(&secret, now()).unwrap() == "000000" {
        "111111"
    } else {
        "000000"
    };
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/2fa/enable",
        Some(&token),
        Some(json!({ "code": wrong })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let used = totp_code(&secret, now()).unwrap();
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/2fa/enable",
        Some(&token),
        Some(json!({ "code": used, "revoke_others": true })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["revoked"], 1);
    let recovery: Vec<String> = b["recovery_codes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(recovery.len(), 10);
    let (s, _) = call(&t.app, "GET", "/auth/account", Some(&other), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&token), None).await;
    assert_eq!(
        (&acct["totp"], &acct["recovery_codes_left"]),
        (&json!(true), &json!(10))
    );

    // Sign in: the password alone only gets a challenge.
    let (s, b) = login(&t, "cyd@example.com", PASSWORD).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(b["mfa"], "totp");
    assert!(b.get("token").is_none());
    let challenge = b["challenge"].as_str().unwrap().to_owned();
    // The code used to turn 2FA on can't be replayed.
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/login/2fa",
        None,
        Some(json!({ "challenge": challenge, "code": used })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_code"))
    );
    let next = totp_code(&secret, now() + 30).unwrap();
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/login/2fa",
        None,
        Some(json!({ "challenge": challenge, "code": next })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["user_id"], user);
    // The challenge is single use, and so is that code.
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/login/2fa",
        None,
        Some(json!({ "challenge": challenge, "code": next })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    // A recovery code works once.
    for expected in [StatusCode::OK, StatusCode::BAD_REQUEST] {
        let (_, b) = login(&t, "cyd@example.com", PASSWORD).await;
        let (s, _) = call(
            &t.app,
            "POST",
            "/auth/login/2fa",
            None,
            Some(json!({ "challenge": b["challenge"], "code": recovery[0].to_uppercase() })),
        )
        .await;
        assert_eq!(s, expected);
    }
    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&token), None).await;
    assert_eq!(acct["recovery_codes_left"], 9);

    // A challenge dies after 5 wrong codes.
    let (_, b) = login(&t, "cyd@example.com", PASSWORD).await;
    let challenge = b["challenge"].as_str().unwrap().to_owned();
    for _ in 0..5 {
        call(
            &t.app,
            "POST",
            "/auth/login/2fa",
            None,
            Some(json!({ "challenge": challenge, "code": "aaaaa-bbbbb" })),
        )
        .await;
    }
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/login/2fa",
        None,
        Some(json!({ "challenge": challenge, "code": recovery[1] })),
    )
    .await;
    assert!(
        s == StatusCode::TOO_MANY_REQUESTS || b["error"] == "expired",
        "{s} {b}"
    );

    // Disable with a recovery code (same field as TOTP codes).
    sqlx::query("UPDATE users SET totp_last_step = NULL")
        .execute(&t.state.db)
        .await
        .unwrap();
    // The dead challenge above also used up the per-account failure budget (in memory):
    // a fresh server on the same database starts with empty buckets.
    let config = &t.state.config;
    let fresh = app(state_with_mailer(
        Config {
            database_url: config.database_url.clone(),
            data_dir: config.data_dir.clone(),
            auth_secret: config.auth_secret.clone(),
            group_name: "Test".into(),
            join_code: "CODE-1".into(),
            ..Config::default()
        },
        t.mail.clone(),
    )
    .await
    .unwrap());
    let (s, b) = call(
        &fresh,
        "POST",
        "/auth/2fa/recovery",
        Some(&token),
        Some(json!({ "code": recovery[2] })),
    )
    .await;
    assert_eq!(
        s,
        StatusCode::BAD_REQUEST,
        "recovery codes can't make new ones: {b}"
    );
    let (s, b) = call(
        &fresh,
        "POST",
        "/auth/2fa/recovery",
        Some(&token),
        Some(json!({ "code": totp_code(&secret, now()).unwrap() })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let renewed: Vec<String> = serde_json::from_value(b["recovery_codes"].clone()).unwrap();
    let (s, _) = call(
        &fresh,
        "POST",
        "/auth/2fa/disable",
        Some(&token),
        Some(json!({ "code": recovery[3] })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "old recovery codes are gone");
    let (s, b) = call(
        &fresh,
        "POST",
        "/auth/2fa/disable",
        Some(&token),
        Some(json!({ "code": renewed[0] })),
    )
    .await;
    assert_eq!((s, &b["ok"]), (StatusCode::OK, &json!(true)));
    let (s, b) = login(&t, "cyd@example.com", PASSWORD).await;
    assert_eq!(s, StatusCode::OK);
    assert!(b["token"].is_string());
}

#[tokio::test]
async fn forgot_password_resets_and_signs_out_everywhere() {
    let t = setup().await;
    let (first, _) = signup(&t, "dan@example.com", "Dan").await;
    let (_, b) = login(&t, "dan@example.com", PASSWORD).await;
    let second = b["token"].as_str().unwrap().to_owned();
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/forgot",
        None,
        Some(json!({ "email": "ghost@example.com" })),
    )
    .await;
    assert_eq!((s, &b["sent"]), (StatusCode::OK, &json!(true)));
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/forgot",
        None,
        Some(json!({ "email": "dan@example.com" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    // One verification email from sign-up, then the reset one.
    let code = wait_for_code(&t.mail, "dan@example.com", 2).await;
    let new_password = "un-nouveau-mot-de-passe";
    let wrong = if code == "000000" { "111111" } else { "000000" };
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/reset",
        None,
        Some(json!({ "email": "dan@example.com", "code": wrong, "password": new_password })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_code"))
    );
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/reset",
        None,
        Some(json!({ "email": "dan@example.com", "code": code, "password": "password" })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/reset",
        None,
        Some(json!({ "email": "dan@example.com", "code": code, "password": new_password })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let fresh = b["token"].as_str().unwrap().to_owned();
    for old in [&first, &second] {
        let (s, _) = call(&t.app, "GET", "/auth/account", Some(old), None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    }
    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&fresh), None).await;
    assert_eq!(acct["email_verified"], true, "a reset proves the email");
    // Single use.
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/reset",
        None,
        Some(json!({ "email": "dan@example.com", "code": code, "password": new_password })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(
        login(&t, "dan@example.com", PASSWORD).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        login(&t, "dan@example.com", new_password).await.0,
        StatusCode::OK
    );

    // Without Mailgun the server says so instead of pretending.
    let no_mail = app(state(base_config()).await.unwrap());
    let (s, b) = call(
        &no_mail,
        "POST",
        "/auth/forgot",
        None,
        Some(json!({ "email": "dan@example.com" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Some("email_not_configured")
        )
    );
    let (_, b) = call(
        &no_mail,
        "POST",
        "/auth/signup",
        None,
        Some(json!({ "email": "eve@example.com", "password": PASSWORD, "name": "Eve" })),
    )
    .await;
    assert_eq!(b["email_sent"], false);
}

#[tokio::test]
async fn sessions_list_revoke_and_password_change() {
    let t = setup().await;
    let (a, _) = signup(&t, "fay@example.com", "Fay").await;
    let mut tokens = vec![a];
    for device in ["Web%20%C2%B7%20Safari", "Pixel"] {
        let r = send(
            &t.app,
            "POST",
            "/auth/login",
            None,
            Some(json!({ "email": "fay@example.com", "password": PASSWORD })),
            &[("x-device", device)],
        )
        .await;
        tokens.push(r.body["token"].as_str().unwrap().to_owned());
    }
    let (_, list) = call(&t.app, "GET", "/auth/sessions", Some(&tokens[0]), None).await;
    let list = list.as_array().unwrap().clone();
    assert_eq!(list.len(), 3);
    assert_eq!(list.iter().filter(|s| s["current"] == true).count(), 1);
    assert!(list.iter().any(|s| s["device"] == "Web · Safari"));
    let pixel = list.iter().find(|s| s["device"] == "Pixel").unwrap();
    assert_eq!(pixel["id"].as_str().unwrap().len(), 16);
    let (s, _) = call(
        &t.app,
        "DELETE",
        &format!("/auth/sessions/{}", pixel["id"].as_str().unwrap()),
        Some(&tokens[0]),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(&t.app, "GET", "/auth/account", Some(&tokens[2]), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _) = call(
        &t.app,
        "DELETE",
        "/auth/sessions/0123456789abcdef",
        Some(&tokens[0]),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // Password change: the current one is needed, the others can be signed out.
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/password",
        Some(&tokens[0]),
        Some(json!({ "current": "nope-nope-nope", "password": "encore-un-autre-mdp" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_credentials"))
    );
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/password",
        Some(&tokens[0]),
        Some(json!({ "current": PASSWORD, "password": "encore-un-autre-mdp", "revoke_others": true })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["revoked"], 1);
    let (s, _) = call(&t.app, "GET", "/auth/account", Some(&tokens[1]), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    assert_eq!(
        login(&t, "fay@example.com", "encore-un-autre-mdp").await.0,
        StatusCode::OK
    );

    let (_, b) = call(
        &t.app,
        "POST",
        "/auth/sessions/revoke-others",
        Some(&tokens[0]),
        None,
    )
    .await;
    assert_eq!(b["revoked"], 1);
    let (s, _) = call(&t.app, "POST", "/auth/logout", Some(&tokens[0]), None).await;
    assert_eq!(s, StatusCode::OK);
    let (s, b) = call(&t.app, "GET", "/auth/account", Some(&tokens[0]), None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("unauthorized"))
    );
}

#[tokio::test]
async fn legacy_accounts_keep_working_and_can_be_secured() {
    let t = setup().await;
    let (s, b) = legacy_join(&t.app, "Louis").await;
    assert_eq!(s, StatusCode::OK);
    let louis = b["token"].as_str().unwrap().to_owned();
    let louis_id = b["member"]["id"].as_i64().unwrap();
    let (s, b) = legacy_join(&t.app, "Marc").await;
    assert_eq!(s, StatusCode::OK);
    let marc = b["token"].as_str().unwrap().to_owned();
    // Legacy accounts are still claimable by first name (old behaviour).
    let (s, b) = legacy_join(&t.app, "marc").await;
    assert_eq!(s, StatusCode::OK);
    assert_ne!(b["token"].as_str().unwrap(), marc);

    // A session from before accounts (no device label) gets one from its next request.
    let (_, list) = call(&t.app, "GET", "/auth/sessions", Some(&louis), None).await;
    assert_eq!(list[0]["device"], Value::Null);
    let r = send(
        &t.app,
        "GET",
        "/me",
        Some(&louis),
        None,
        &[("x-device", "iPhone%20%C2%B7%20iOS%2018")],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    let (_, list) = call(&t.app, "GET", "/auth/sessions", Some(&louis), None).await;
    assert_eq!(list[0]["device"], "iPhone · iOS 18");

    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&louis), None).await;
    assert_eq!(acct["secured"], false);
    assert_eq!(acct["email"], Value::Null);
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/credentials",
        Some(&louis),
        Some(json!({ "email": "louis@example.com", "password": PASSWORD })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["email_sent"], true);
    assert!(t.mail.last_code("louis@example.com").is_some());
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/credentials",
        Some(&louis),
        Some(json!({ "email": "other@example.com", "password": PASSWORD })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("already_secured"))
    );
    // Same seat, same rooms, the old token still works.
    let (s, me) = call(&t.app, "GET", "/me", Some(&louis), None).await;
    assert_eq!((s, me["id"].as_i64()), (StatusCode::OK, Some(louis_id)));
    // Now a first name is not enough.
    let (s, b) = legacy_join(&t.app, "louis").await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("account_protected"))
    );
    let (s, b) = login(&t, "louis@example.com", PASSWORD).await;
    assert_eq!(s, StatusCode::OK);
    let (s, me) = call(&t.app, "GET", "/me", b["token"].as_str(), None).await;
    assert_eq!((s, me["id"].as_i64()), (StatusCode::OK, Some(louis_id)));

    // AUTH_REQUIRED: no anonymous way in any more, tokens still fine.
    let strict = setup_with(Config {
        auth_required: true,
        ..base_config()
    })
    .await;
    let (s, b) = legacy_join(&strict.app, "Nina").await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::GONE, Some("sign_in_required"))
    );
    let (s, _) = call(
        &strict.app,
        "POST",
        "/start",
        None,
        Some(json!({ "name": "Nina", "room": "Chez Nina" })),
    )
    .await;
    assert_eq!(s, StatusCode::GONE);
}

#[tokio::test]
async fn features_advertise_only_what_is_configured() {
    let plain = app(state(base_config()).await.unwrap());
    let (_, f) = call(&plain, "GET", "/features", None, None).await;
    let f: Vec<String> = serde_json::from_value(f).unwrap();
    assert!(f.contains(&"auth".into()) && f.contains(&"rooms".into()));
    assert!(!f.iter().any(|x| x == "email" || x.starts_with("oauth_")));
    let (_, p) = call(&plain, "GET", "/auth/providers", None, None).await;
    assert_eq!(p, json!([]));

    // Client ids without PUBLIC_URL: still off.
    let client = OAuthClient {
        id: "id".into(),
        secret: "secret".into(),
        endpoints: None,
    };
    let no_url = setup_with(Config {
        discord: Some(client.clone()),
        ..base_config()
    })
    .await;
    let (_, f) = call(&no_url.app, "GET", "/features", None, None).await;
    assert!(f.as_array().unwrap().contains(&json!("email")));
    assert!(!f.as_array().unwrap().contains(&json!("oauth_discord")));

    let full = setup_with(Config {
        public_url: Some(PUBLIC_URL.into()),
        google: Some(client.clone()),
        discord: Some(client),
        ..base_config()
    })
    .await;
    let (_, f) = call(&full.app, "GET", "/features", None, None).await;
    for name in ["auth", "email", "oauth_google", "oauth_discord"] {
        assert!(f.as_array().unwrap().contains(&json!(name)), "{name}");
    }
    let (_, p) = call(&full.app, "GET", "/auth/providers", None, None).await;
    assert_eq!(p, json!(["google", "discord"]));
}

// ---------- OAuth with a mock provider ----------

const TEST_KEY: &str = include_str!("fixtures/test_rsa_key.pem");
const TEST_N: &str = "ocbStqmOau3_j-9Kdp3YTm5jI1-N7P-qYkU1lZtKhQrmqWuNIl273xt7VkMhMguMtnEafUejbKawcHhsrJVvVjPSaDiXUPA26FVspQ1DxOKpFxtN5RgAmYI0BNOX6rwji0_CycIQ3eH_Uf9JxjHBhf69iRfKUsssWmnckZAW8pbT_ZEApgIftKFgtburWbwa-xg1cPWT7kcbd0iw7QtHxS2qUWJY3x0bfipZ-BRxxJDLX-seOZS2fMIpBdlGaBK9cjusBkQX3NUDXTsnyhlk9DQbO-giiMhgvipSp6FpkOWT6j9C2GE06PQW5QJIFjF8DAHdLf-O-2mnEZHFmgahwQ";

/// What the mock provider will say for an authorization code.
#[derive(Clone)]
struct Grant {
    /// PKCE challenge from the authorize URL: the token request must bring the matching verifier.
    challenge: String,
    /// Discord: the `/users/@me` profile. Google: the ID token claims.
    profile: Value,
}

type Grants = Arc<Mutex<HashMap<String, Grant>>>;

async fn token_endpoint(
    State(grants): State<Grants>,
    Form(form): Form<HashMap<String, String>>,
) -> Result<axum::Json<Value>, StatusCode> {
    let code = form.get("code").ok_or(StatusCode::BAD_REQUEST)?;
    let grant = grants
        .lock()
        .unwrap()
        .get(code)
        .cloned()
        .ok_or(StatusCode::BAD_REQUEST)?;
    let verifier = form.get("code_verifier").ok_or(StatusCode::BAD_REQUEST)?;
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(Sha256::digest(verifier.as_bytes()));
    if challenge != grant.challenge
        || form.get("client_secret").map(String::as_str) != Some("shh")
        || form.get("grant_type").map(String::as_str) != Some("authorization_code")
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some("test-key".into());
    let id_token = jsonwebtoken::encode(
        &header,
        &grant.profile,
        &jsonwebtoken::EncodingKey::from_rsa_pem(TEST_KEY.as_bytes()).unwrap(),
    )
    .unwrap();
    Ok(axum::Json(
        json!({ "access_token": code, "id_token": id_token, "token_type": "Bearer" }),
    ))
}

async fn me_endpoint(
    State(grants): State<Grants>,
    headers: HeaderMap,
) -> Result<axum::Json<Value>, StatusCode> {
    let code = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let grants = grants.lock().unwrap();
    Ok(axum::Json(
        grants
            .get(code)
            .ok_or(StatusCode::UNAUTHORIZED)?
            .profile
            .clone(),
    ))
}

async fn mock_provider() -> (String, Grants) {
    let grants: Grants = Arc::default();
    let router = Router::new()
        .route("/token", post(token_endpoint))
        .route("/me", get(me_endpoint))
        .route(
            "/jwks",
            get(|| async {
                axum::Json(json!({ "keys": [{
                    "kty": "RSA", "kid": "test-key", "alg": "RS256", "use": "sig",
                    "n": TEST_N, "e": "AQAB"
                }]}))
            }),
        )
        .with_state(grants.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (format!("http://{addr}"), grants)
}

async fn oauth_setup() -> (T, Grants) {
    let (base, grants) = mock_provider().await;
    let endpoints = OAuthEndpoints {
        authorize: format!("{base}/authorize"),
        token: format!("{base}/token"),
        userinfo: format!("{base}/me"),
        jwks: format!("{base}/jwks"),
    };
    let client = OAuthClient {
        id: "client-id".into(),
        secret: "shh".into(),
        endpoints: Some(endpoints),
    };
    let t = setup_with(Config {
        public_url: Some(PUBLIC_URL.into()),
        google: Some(client.clone()),
        discord: Some(client),
        ..base_config()
    })
    .await;
    (t, grants)
}

fn query_param(url: &str, name: &str) -> Option<String> {
    reqwest::Url::parse(url)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.into_owned())
}

/// Runs start → (provider) → callback. Returns the redirect back to the app and the key.
async fn oauth_round(
    t: &T,
    grants: &Grants,
    provider: &str,
    token: Option<&str>,
    link: bool,
    profile: impl FnOnce(&str) -> Value,
) -> (String, String) {
    let (s, b) = call(
        &t.app,
        "POST",
        &format!("/auth/oauth/{provider}/start"),
        token,
        Some(json!({ "app_redirect": "quidenous://oauth", "link": link })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let url = b["url"].as_str().unwrap();
    let key = b["key"].as_str().unwrap().to_owned();
    assert_eq!(query_param(url, "client_id").as_deref(), Some("client-id"));
    assert_eq!(
        query_param(url, "redirect_uri").unwrap(),
        format!("{PUBLIC_URL}/api/auth/oauth/{provider}/callback")
    );
    assert_eq!(
        query_param(url, "code_challenge_method").as_deref(),
        Some("S256")
    );
    let csrf = query_param(url, "state").unwrap();
    let nonce = query_param(url, "nonce").unwrap_or_default();
    let code = format!("code-{}", rand_code());
    grants.lock().unwrap().insert(
        code.clone(),
        Grant {
            challenge: query_param(url, "code_challenge").unwrap(),
            profile: profile(&nonce),
        },
    );
    let r = send(
        &t.app,
        "GET",
        &format!("/auth/oauth/{provider}/callback?code={code}&state={csrf}"),
        None,
        None,
        &[],
    )
    .await;
    assert!(r.status.is_redirection(), "{} {}", r.status, r.body);
    let location = r.headers["location"].to_str().unwrap().to_owned();
    assert!(location.starts_with("quidenous://oauth?"), "{location}");
    // The state is single use.
    let again = send(
        &t.app,
        "GET",
        &format!("/auth/oauth/{provider}/callback?code={code}&state={csrf}"),
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(again.status, StatusCode::BAD_REQUEST);
    (location, key)
}

fn rand_code() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

fn discord(id: &str, email: &str, verified: bool) -> impl FnOnce(&str) -> Value {
    let profile = json!({ "id": id, "username": "gamer", "global_name": "Gégé", "email": email, "verified": verified });
    move |_| profile
}

async fn redeem(t: &T, location: &str, key: &str) -> (StatusCode, Value) {
    let code = query_param(location, "code").expect("a code, not an error");
    call(
        &t.app,
        "POST",
        "/auth/oauth/redeem",
        None,
        Some(json!({ "code": code, "key": key })),
    )
    .await
}

#[tokio::test]
async fn oauth_discord_new_account_auto_link_and_link_mode() {
    let (t, grants) = oauth_setup().await;
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/oauth/discord/start",
        None,
        Some(json!({ "app_redirect": "https://evil.example/oauth" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_redirect"))
    );
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/oauth/apple/start",
        None,
        Some(json!({ "app_redirect": "quidenous://oauth" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::NOT_FOUND, Some("unknown_provider"))
    );
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/oauth/discord/start",
        None,
        Some(json!({ "app_redirect": format!("{PUBLIC_URL}/oauth") })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    // New account from a verified Discord email.
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        None,
        false,
        discord("111", "gege@example.com", true),
    )
    .await;
    // The code alone (hijacked redirect) gets nothing.
    let (s, b) = redeem(&t, &location, "not-the-key").await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_code"))
    );
    let (s, b) = redeem(&t, &location, &key).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["new"], true);
    let gege = b["token"].as_str().unwrap().to_owned();
    let gege_id = b["user_id"].as_i64().unwrap();
    let (s, _) = redeem(&t, &location, &key).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "one time only");
    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&gege), None).await;
    assert_eq!(acct["name"], "Gégé");
    assert_eq!(acct["email"], "gege@example.com");
    assert_eq!(acct["email_verified"], true);
    assert_eq!(acct["providers"], json!(["discord"]));
    assert_eq!(acct["secured"], true);
    let (s, b) = call(&t.app, "GET", "/me", Some(&gege), None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("no_room"))
    );
    // Signing in again finds the same account.
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        None,
        false,
        discord("111", "gege@example.com", true),
    )
    .await;
    let (_, b) = redeem(&t, &location, &key).await;
    assert_eq!(
        (b["user_id"].as_i64(), &b["new"]),
        (Some(gege_id), &json!(false))
    );
    // Last way in: can't unlink.
    let (s, b) = call(&t.app, "DELETE", "/auth/oauth/discord", Some(&gege), None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("last_method"))
    );

    // Auto-link: same verified email on both sides.
    let (hal, hal_id) = signup(&t, "hal@example.com", "Hal").await;
    verify_email(&t, &hal, "hal@example.com").await;
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        None,
        false,
        discord("222", "hal@example.com", true),
    )
    .await;
    let (_, b) = redeem(&t, &location, &key).await;
    assert_eq!(
        (b["user_id"].as_i64(), &b["new"]),
        (Some(hal_id), &json!(false))
    );
    // Hal has a password too: unlinking is fine.
    let (s, _) = call(&t.app, "DELETE", "/auth/oauth/discord", Some(&hal), None).await;
    assert_eq!(s, StatusCode::OK);

    // Never on an unverified email: neither the account's nor the provider's.
    signup(&t, "ivy@example.com", "Ivy").await;
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        None,
        false,
        discord("333", "ivy@example.com", true),
    )
    .await;
    let (s, b) = redeem(&t, &location, &key).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("email_taken"))
    );
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        None,
        false,
        discord("444", "hal@example.com", false),
    )
    .await;
    let (s, b) = redeem(&t, &location, &key).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(b["new"], true);
    assert_ne!(b["user_id"].as_i64(), Some(hal_id));

    // Link mode: a legacy account adds Discord.
    let (_, b) = legacy_join(&t.app, "Jo").await;
    let jo = b["token"].as_str().unwrap().to_owned();
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/oauth/discord/start",
        None,
        Some(json!({ "app_redirect": "quidenous://oauth", "link": true })),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "link mode needs a session");
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        Some(&jo),
        true,
        discord("555", "jo@example.com", true),
    )
    .await;
    let (s, b) = redeem(&t, &location, &key).await;
    assert_eq!((s, &b), (StatusCode::OK, &json!({ "linked": "discord" })));
    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&jo), None).await;
    assert_eq!(acct["providers"], json!(["discord"]));
    assert_eq!(acct["secured"], true);
    assert_eq!(
        acct["email"], "jo@example.com",
        "the provider vouches for it"
    );
    let (s, b) = legacy_join(&t.app, "jo").await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("account_protected"))
    );
    // Someone else's identity can't be linked.
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        Some(&hal),
        true,
        discord("111", "gege@example.com", true),
    )
    .await;
    let (s, b) = redeem(&t, &location, &key).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("identity_taken"))
    );

    // The provider says no: back to the app with an error.
    let (_, b) = call(
        &t.app,
        "POST",
        "/auth/oauth/discord/start",
        None,
        Some(json!({ "app_redirect": "quidenous://oauth" })),
    )
    .await;
    let csrf = query_param(b["url"].as_str().unwrap(), "state").unwrap();
    let r = send(
        &t.app,
        "GET",
        &format!("/auth/oauth/discord/callback?error=access_denied&state={csrf}"),
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(r.headers["location"], "quidenous://oauth?error=cancelled");
}

#[tokio::test]
async fn oauth_google_id_token_is_verified() {
    let (t, grants) = oauth_setup().await;
    let claims = |sub: &'static str, aud: &'static str, good_nonce: bool| {
        move |nonce: &str| {
            json!({
                "iss": "https://accounts.google.com",
                "aud": aud,
                "sub": sub,
                "exp": now() + 600,
                "iat": now(),
                "nonce": if good_nonce { nonce.to_owned() } else { "forged".to_owned() },
                "email": "kim@example.com",
                "email_verified": true,
                "given_name": "Kim",
            })
        }
    };
    let (location, key) = oauth_round(
        &t,
        &grants,
        "google",
        None,
        false,
        claims("g-1", "client-id", true),
    )
    .await;
    let (s, b) = redeem(&t, &location, &key).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["new"], true);
    let (_, acct) = call(&t.app, "GET", "/auth/account", b["token"].as_str(), None).await;
    assert_eq!(
        (&acct["name"], &acct["providers"]),
        (&json!("Kim"), &json!(["google"]))
    );

    // Wrong nonce or audience: refused.
    for bad in [
        claims("g-2", "client-id", false),
        claims("g-3", "someone-else", true),
    ] {
        let (location, _) = oauth_round(&t, &grants, "google", None, false, bad).await;
        assert_eq!(location, "quidenous://oauth?error=provider_error");
    }

    // 2FA still applies to OAuth sign-ins.
    let token = b["token"].as_str().unwrap().to_owned();
    let (_, setup) = call(&t.app, "POST", "/auth/2fa/setup", Some(&token), None).await;
    let secret = setup["secret"].as_str().unwrap().to_owned();
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/2fa/enable",
        Some(&token),
        Some(json!({ "code": totp_code(&secret, now()).unwrap() })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (location, key) = oauth_round(
        &t,
        &grants,
        "google",
        None,
        false,
        claims("g-1", "client-id", true),
    )
    .await;
    let (_, b) = redeem(&t, &location, &key).await;
    assert_eq!(b["mfa"], "totp");
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/login/2fa",
        None,
        Some(
            json!({ "challenge": b["challenge"], "code": totp_code(&secret, now() + 30).unwrap() }),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
}

async fn session_id(t: &T, token: &str) -> String {
    let (_, list) = call(&t.app, "GET", "/auth/sessions", Some(token), None).await;
    list.as_array()
        .unwrap()
        .iter()
        .find(|s| s["current"] == true)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn count(t: &T, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql)
        .fetch_one(&t.state.db)
        .await
        .unwrap()
}

#[tokio::test]
async fn parallel_guesses_share_the_same_budgets() {
    let t = setup().await;
    let (token, _) = signup(&t, "gus@example.com", "Gus").await;
    let code = t.mail.last_code("gus@example.com").unwrap();
    let wrong = if code == "000000" { "111111" } else { "000000" };
    // 12 guesses at once: a code still only gets 5.
    let guesses = (0..12).map(|_| {
        call(
            &t.app,
            "POST",
            "/auth/verify",
            Some(&token),
            Some(json!({ "code": wrong })),
        )
    });
    let results = futures::future::join_all(guesses).await;
    let errors = |code: &str| results.iter().filter(|(_, b)| b["error"] == code).count();
    assert_eq!((errors("bad_code"), errors("expired")), (5, 7));
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/verify",
        Some(&token),
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("expired"))
    );

    // Sign-in: successes give their token back, parallel failures each take one.
    for _ in 0..6 {
        assert_eq!(
            login(&t, "gus@example.com", PASSWORD).await.0,
            StatusCode::OK
        );
    }
    let tries = (0..8).map(|_| login(&t, "gus@example.com", "not-the-password"));
    let results = futures::future::join_all(tries).await;
    let status = |s: StatusCode| results.iter().filter(|(r, _)| *r == s).count();
    assert_eq!(
        (
            status(StatusCode::BAD_REQUEST),
            status(StatusCode::TOO_MANY_REQUESTS)
        ),
        (5, 3)
    );
    assert_eq!(
        login(&t, "gus@example.com", PASSWORD).await.0,
        StatusCode::TOO_MANY_REQUESTS
    );

    // Reset: 10 tries per hour per email, whatever the IP.
    for i in 0..10 {
        let ip = format!("192.0.2.{i}");
        let r = send(
            &t.app,
            "POST",
            "/auth/reset",
            None,
            Some(json!({ "email": "gus@example.com", "code": "123456", "password": "un-nouveau-mot-de-passe" })),
            &[("x-forwarded-for", &ip)],
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST);
    }
    let r = send(
        &t.app,
        "POST",
        "/auth/reset",
        None,
        Some(json!({ "email": "gus@example.com", "code": "123456", "password": "un-nouveau-mot-de-passe" })),
        &[("x-forwarded-for", "192.0.2.99")],
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn a_name_grab_cannot_lock_the_first_device_out() {
    let t = setup().await;
    let (_, b) = legacy_join(&t.app, "Rita").await;
    let rita = b["token"].as_str().unwrap().to_owned();
    let (s, b) = legacy_join(&t.app, "rita").await;
    assert_eq!(s, StatusCode::OK);
    let grab = b["token"].as_str().unwrap().to_owned();
    let rita_session = session_id(&t, &rita).await;
    for (method, path, body) in [
        (
            "POST",
            "/auth/credentials".to_owned(),
            Some(json!({ "email": "grab@example.com", "password": PASSWORD })),
        ),
        (
            "POST",
            "/auth/password".into(),
            Some(json!({ "password": PASSWORD })),
        ),
        ("POST", "/auth/2fa/setup".into(), None),
        (
            "POST",
            "/auth/2fa/enable".into(),
            Some(json!({ "code": "123456" })),
        ),
        ("POST", "/auth/sessions/revoke-others".into(), None),
        ("DELETE", format!("/auth/sessions/{rita_session}"), None),
    ] {
        let (s, b) = call(&t.app, method, &path, Some(&grab), body).await;
        assert_eq!(
            (s, b["error"].as_str()),
            (StatusCode::FORBIDDEN, Some("claimed_session")),
            "{path}"
        );
    }
    // It still plays in the room.
    assert_eq!(
        call(&t.app, "GET", "/me", Some(&grab), None).await.0,
        StatusCode::OK
    );
    // The first device (it created the account) can sign the claimer out.
    let (_, b) = call(
        &t.app,
        "POST",
        "/auth/sessions/revoke-others",
        Some(&rita),
        None,
    )
    .await;
    assert_eq!(b["revoked"], 1);
    assert_eq!(
        call(&t.app, "GET", "/auth/account", Some(&grab), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );

    // Alone on the account, a claimed session may secure it: nothing is taken from anyone.
    let (_, b) = legacy_join(&t.app, "RITA").await;
    let grab = b["token"].as_str().unwrap().to_owned();
    call(&t.app, "POST", "/auth/logout", Some(&rita), None).await;
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/credentials",
        Some(&grab),
        Some(json!({ "email": "rita@example.com", "password": PASSWORD })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    // A password sign-in is never claimed; the claimed one is limited again, but can sign itself out.
    let (_, b) = login(&t, "rita@example.com", PASSWORD).await;
    let fresh = b["token"].as_str().unwrap().to_owned();
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/sessions/revoke-others",
        Some(&grab),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let own = session_id(&t, &grab).await;
    let (s, _) = call(
        &t.app,
        "DELETE",
        &format!("/auth/sessions/{own}"),
        Some(&grab),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, b) = call(
        &t.app,
        "POST",
        "/auth/sessions/revoke-others",
        Some(&fresh),
        None,
    )
    .await;
    assert_eq!(b["revoked"], 0);
}

#[tokio::test]
async fn a_reset_takes_back_an_email_someone_else_typed() {
    let (t, grants) = oauth_setup().await;
    // Someone signs up with Vic's address (never verified), links their own Discord, turns 2FA on.
    let (squatter, squatter_id) = signup(&t, "vic@example.com", "Vic").await;
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        Some(&squatter),
        true,
        discord("666", "squatter@example.com", true),
    )
    .await;
    assert_eq!(
        redeem(&t, &location, &key).await.1,
        json!({ "linked": "discord" })
    );
    let (_, setup) = call(&t.app, "POST", "/auth/2fa/setup", Some(&squatter), None).await;
    let secret = setup["secret"].as_str().unwrap().to_owned();
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/2fa/enable",
        Some(&squatter),
        Some(json!({ "code": totp_code(&secret, now()).unwrap() })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    // Vic resets the password: the code proves the address, the account is Vic's alone.
    let forgot = || {
        call(
            &t.app,
            "POST",
            "/auth/forgot",
            None,
            Some(json!({ "email": "vic@example.com" })),
        )
    };
    forgot().await;
    let code = wait_for_code(&t.mail, "vic@example.com", 2).await;
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/reset",
        None,
        Some(
            json!({ "email": "vic@example.com", "code": code, "password": "le-vrai-mot-de-passe" }),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["user_id"].as_i64(), Some(squatter_id));
    let vic = b["token"]
        .as_str()
        .expect("no 2FA step: someone else set it up")
        .to_owned();
    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&vic), None).await;
    assert_eq!(
        (
            &acct["providers"],
            &acct["totp"],
            &acct["recovery_codes_left"],
            &acct["email_verified"]
        ),
        (&json!([]), &json!(false), &json!(0), &json!(true))
    );
    assert_eq!(
        call(&t.app, "GET", "/auth/account", Some(&squatter), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    // The squatter's Discord doesn't lead there any more.
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        None,
        false,
        discord("666", "squatter@example.com", true),
    )
    .await;
    let (_, b) = redeem(&t, &location, &key).await;
    assert_eq!(b["new"], true);
    assert_ne!(b["user_id"].as_i64(), Some(squatter_id));

    // Once the address is proven, 2FA set up by its owner still applies to a reset.
    let (_, setup) = call(&t.app, "POST", "/auth/2fa/setup", Some(&vic), None).await;
    let secret = setup["secret"].as_str().unwrap().to_owned();
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/2fa/enable",
        Some(&vic),
        Some(json!({ "code": totp_code(&secret, now()).unwrap() })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    forgot().await;
    let code = wait_for_code(&t.mail, "vic@example.com", 3).await;
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/reset",
        None,
        Some(
            json!({ "email": "vic@example.com", "code": code, "password": "encore-un-autre-mdp" }),
        ),
    )
    .await;
    assert_eq!((s, &b["mfa"]), (StatusCode::OK, &json!("totp")), "{b}");
}

#[tokio::test]
async fn oauth_redirect_code_alone_links_and_creates_nothing() {
    let (t, grants) = oauth_setup().await;
    let (_, b) = legacy_join(&t.app, "Ola").await;
    let ola = b["token"].as_str().unwrap().to_owned();
    // Ola starts a link, then gets someone else's browser through the provider: the redirect code
    // without the key from `start` must not attach that person's identity to Ola's account.
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        Some(&ola),
        true,
        discord("777", "victim@example.com", true),
    )
    .await;
    let users = count(&t, "SELECT COUNT(*) FROM users").await;
    let (s, b) = redeem(&t, &location, "not-the-key").await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_code"))
    );
    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&ola), None).await;
    assert_eq!(
        (&acct["providers"], &acct["email"]),
        (&json!([]), &Value::Null)
    );
    // Sign-in mode: no account is created either.
    let (other, _) = oauth_round(
        &t,
        &grants,
        "discord",
        None,
        false,
        discord("888", "new@example.com", true),
    )
    .await;
    let (s, _) = redeem(&t, &other, "not-the-key").await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(count(&t, "SELECT COUNT(*) FROM users").await, users);
    assert_eq!(count(&t, "SELECT COUNT(*) FROM oauth_identities").await, 0);
    // With the key, the link goes through.
    let (s, b) = redeem(&t, &location, &key).await;
    assert_eq!((s, b), (StatusCode::OK, json!({ "linked": "discord" })));

    // Link mode from a claimed session (first-name grab): refused while another device exists.
    let (_, b) = legacy_join(&t.app, "Kat").await;
    let kat = b["token"].as_str().unwrap().to_owned();
    let (_, b) = legacy_join(&t.app, "kat").await;
    let grab = b["token"].as_str().unwrap().to_owned();
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/oauth/discord/start",
        Some(&grab),
        Some(json!({ "app_redirect": "quidenous://oauth", "link": true })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("claimed_session"))
    );
    // The first device signs out: the claimer is alone and may start a link...
    call(&t.app, "POST", "/auth/logout", Some(&kat), None).await;
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        Some(&grab),
        true,
        discord("999", "kat@example.com", true),
    )
    .await;
    // ...but redeem checks again: someone else claimed the name meanwhile.
    legacy_join(&t.app, "KAT").await;
    let (s, b) = redeem(&t, &location, &key).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("claimed_session"))
    );
    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&grab), None).await;
    assert_eq!(acct["providers"], json!([]));
}

// ---------- account deletion, reports ----------

/// Same as `call`, in a given room (`X-Room`).
async fn in_room(
    t: &T,
    method: &str,
    path: &str,
    token: &str,
    room: i64,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let room = room.to_string();
    let r = send(
        &t.app,
        method,
        path,
        Some(token),
        body,
        &[("x-room", &room)],
    )
    .await;
    (r.status, r.body)
}

/// A room created by `owner`, joined by `others` in that order. Deck ranking questions are
/// dropped so votes pick one person.
async fn room_with(t: &T, owner: &str, others: &[&str]) -> i64 {
    sqlx::query("DELETE FROM questions WHERE group_id IS NULL AND kind = 'rank'")
        .execute(&t.state.db)
        .await
        .unwrap();
    let (s, b) = call(
        &t.app,
        "POST",
        "/rooms",
        Some(owner),
        Some(json!({ "name": "Le salon" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let room = b["id"].as_i64().unwrap();
    let (_, g) = in_room(t, "GET", "/group", owner, room, None).await;
    for other in others {
        let (s, b) = call(
            &t.app,
            "POST",
            "/rooms/join",
            Some(other),
            Some(json!({ "code": g["code"] })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    room
}

/// Everyone votes for `target`: the open question closes. Returns its id.
async fn close_round(t: &T, room: i64, voters: &[&str], target: i64) -> i64 {
    let (_, round) = in_room(t, "GET", "/round/current", voters[0], room, None).await;
    for v in voters {
        let (s, b) = in_room(
            t,
            "POST",
            "/round/current/vote",
            v,
            room,
            Some(json!({ "target_id": target })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    round["id"].as_i64().unwrap()
}

async fn member_id(t: &T, token: &str, room: i64) -> i64 {
    let (_, me) = in_room(t, "GET", "/me", token, room, None).await;
    me["id"].as_i64().unwrap()
}

#[tokio::test]
async fn deleting_an_account_from_the_app() {
    let t = setup().await;
    let (ana, ana_user) = signup(&t, "ana@example.com", "Ana").await;
    let (bob, _) = signup(&t, "bob@example.com", "Bob").await;
    let room = room_with(&t, &ana, &[&bob]).await;
    let ana_seat = member_id(&t, &ana, room).await;
    let bob_seat = member_id(&t, &bob, room).await;
    let round = close_round(&t, room, &[&ana, &bob], bob_seat).await;
    let (s, _) = in_room(
        &t,
        "POST",
        &format!("/rounds/{round}/messages"),
        &ana,
        room,
        Some(json!({ "text": "bravo Bob" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, inbox) = in_room(&t, "GET", "/inbox", &bob, room, None).await;
    assert_eq!(inbox["items"][0]["kind"], "message");

    // A password account proves it is them.
    let (s, b) = call(&t.app, "DELETE", "/auth/account", Some(&ana), None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("reauth_required"))
    );
    let (s, b) = call(
        &t.app,
        "DELETE",
        "/auth/account",
        Some(&ana),
        Some(json!({ "password": "pas-le-bon-mot-de-passe" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_credentials"))
    );
    let (_, other_device) = login(&t, "ana@example.com", PASSWORD).await;
    let other_device = other_device["token"].as_str().unwrap().to_owned();
    let (s, b) = call(
        &t.app,
        "DELETE",
        "/auth/account",
        Some(&ana),
        Some(json!({ "password": PASSWORD })),
    )
    .await;
    assert_eq!((s, &b), (StatusCode::OK, &json!({ "ok": true })));
    // Every session is gone; the email no longer signs in, and is free again.
    for token in [&ana, &other_device] {
        let (s, _) = call(&t.app, "GET", "/auth/account", Some(token), None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    }
    let (s, _) = login(&t, "ana@example.com", PASSWORD).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    // Bob owns the room now, alone among the active members.
    let (_, g) = in_room(&t, "GET", "/group", &bob, room, None).await;
    let members = g["members"].as_array().unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(
        (&members[0]["id"], &g["my_role"]),
        (&json!(bob_seat), &json!("owner"))
    );
    // The history still counts Ana's vote, under an anonymous name; her message is gone.
    let (s, r) = in_room(&t, "GET", &format!("/rounds/{round}"), &bob, room, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(r["result"][0]["count"], 2);
    assert!(
        r["result"][0]["voter_ids"]
            .as_array()
            .unwrap()
            .contains(&json!(ana_seat))
    );
    let (s, h) = in_room(&t, "GET", "/history", &bob, room, None).await;
    assert_eq!((s, &h[0]["vote_count"]), (StatusCode::OK, &json!(2)));
    let (s, _) = in_room(&t, "GET", "/stats", &bob, room, None).await;
    assert_eq!(s, StatusCode::OK);
    let (_, msgs) = in_room(
        &t,
        "GET",
        &format!("/rounds/{round}/messages"),
        &bob,
        room,
        None,
    )
    .await;
    assert_eq!(msgs, json!([]));
    let (_, inbox) = in_room(&t, "GET", "/inbox", &bob, room, None).await;
    assert!(
        inbox["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["kind"] != "message")
    );
    let (_, friends) = call(&t.app, "GET", "/friends", Some(&bob), None).await;
    assert_eq!(friends["friends"], json!([]));
    let r = send(
        &t.app,
        "GET",
        &format!("/history/export?token={bob}&room={room}"),
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);

    let (name, email, deleted): (String, Option<String>, Option<String>) =
        sqlx::query_as("SELECT name, email, deleted_at FROM users WHERE id = ?")
            .bind(ana_user)
            .fetch_one(&t.state.db)
            .await
            .unwrap();
    assert_eq!((name.as_str(), email), ("Compte supprimé", None));
    assert!(deleted.is_some());
    let (name, user, removed): (String, Option<i64>, Option<String>) =
        sqlx::query_as("SELECT name, user_id, removed_at FROM members WHERE id = ?")
            .bind(ana_seat)
            .fetch_one(&t.state.db)
            .await
            .unwrap();
    assert_eq!((name.as_str(), user), ("Compte supprimé", None));
    assert!(removed.is_some());
    for sql in [
        "SELECT COUNT(*) FROM user_tokens WHERE user_id = (SELECT id FROM users WHERE deleted_at IS NOT NULL)",
        "SELECT COUNT(*) FROM auth_codes WHERE email = 'ana@example.com'",
        "SELECT COUNT(*) FROM messages",
    ] {
        assert_eq!(count(&t, sql).await, 0, "{sql}");
    }
    let (_, again) = signup(&t, "ana@example.com", "Ana").await;
    assert_ne!(again, ana_user);
}

#[tokio::test]
async fn deleting_with_2fa_legacy_and_alone_in_a_room() {
    let t = setup().await;
    let (cyd, _) = signup(&t, "cyd@example.com", "Cyd").await;
    let (_, setup) = call(&t.app, "POST", "/auth/2fa/setup", Some(&cyd), None).await;
    let secret = setup["secret"].as_str().unwrap().to_owned();
    let (_, b) = call(
        &t.app,
        "POST",
        "/auth/2fa/enable",
        Some(&cyd),
        Some(json!({ "code": totp_code(&secret, now()).unwrap() })),
    )
    .await;
    let recovery = b["recovery_codes"][0].as_str().unwrap().to_owned();
    let room = room_with(&t, &cyd, &[]).await;
    // A 2FA code instead of the password.
    let (s, b) = call(
        &t.app,
        "DELETE",
        "/auth/account",
        Some(&cyd),
        Some(json!({ "code": "00000-00000" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_code"))
    );
    let (s, b) = call(
        &t.app,
        "DELETE",
        "/auth/account",
        Some(&cyd),
        Some(json!({ "code": recovery })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    // Alone in the room: it is deleted.
    let deleted: Option<String> = sqlx::query_scalar("SELECT deleted_at FROM groups WHERE id = ?")
        .bind(room)
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    assert!(deleted.is_some());
    assert_eq!(count(&t, "SELECT COUNT(*) FROM recovery_codes").await, 0);

    // A legacy account has nothing to prove, but a name grab can't delete it while the owner
    // has another session.
    let (s, _) = legacy_join(&t.app, "Yann").await;
    assert_eq!(s, StatusCode::OK);
    let (_, b) = legacy_join(&t.app, "Zoé").await;
    let zoe = b["token"].as_str().unwrap().to_owned();
    let (_, b) = legacy_join(&t.app, "zoé").await;
    let grab = b["token"].as_str().unwrap().to_owned();
    let (s, b) = call(&t.app, "DELETE", "/auth/account", Some(&grab), None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("claimed_session"))
    );
    let (s, _) = call(&t.app, "DELETE", "/auth/account", Some(&zoe), None).await;
    assert_eq!(s, StatusCode::OK);
    for token in [&zoe, &grab] {
        let (s, _) = call(&t.app, "GET", "/me", Some(token), None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    }
    // Her seat is not up for grabs by first name.
    let (s, _) = legacy_join(&t.app, "Zoé").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        count(
            &t,
            "SELECT COUNT(*) FROM members WHERE name = 'Zoé' AND removed_at IS NULL"
        )
        .await,
        1
    );
}

#[tokio::test]
async fn deleting_from_the_public_page() {
    let t = setup().await;
    let (dan, _) = signup(&t, "dan@example.com", "Dan").await;
    let page = t
        .app
        .clone()
        .oneshot(Request::get("/delete-account").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(page.status(), StatusCode::OK);
    let html = page.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8_lossy(&html);
    assert!(html.contains("Supprimer mon compte") && html.contains("/api/auth/delete/"));

    // Unknown address: same answer, nothing sent.
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/delete/send",
        None,
        Some(json!({ "email": "nobody@example.com" })),
    )
    .await;
    assert_eq!((s, &b), (StatusCode::OK, &json!({ "sent": true })));
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/delete/send",
        None,
        Some(json!({ "email": " Dan@Example.com " })),
    )
    .await;
    assert_eq!((s, &b), (StatusCode::OK, &json!({ "sent": true })));
    // The signup sent the verification code first.
    let code = wait_for_code(&t.mail, "dan@example.com", 2).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(t.mail.count("nobody@example.com"), 0);
    let sent = t.mail.sent.lock().unwrap().last().unwrap().clone();
    assert!(sent.subject.contains("supprimer ton compte"));
    let wrong = if code == "000000" { "111111" } else { "000000" };
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/delete/confirm",
        None,
        Some(json!({ "email": "dan@example.com", "code": wrong })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_code"))
    );
    // A password reset code is not a deletion code.
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/reset",
        None,
        Some(json!({ "email": "dan@example.com", "code": code, "password": "un-autre-mot-de-passe" })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/delete/confirm",
        None,
        Some(json!({ "email": "dan@example.com", "code": code })),
    )
    .await;
    assert_eq!((s, &b), (StatusCode::OK, &json!({ "deleted": true })));
    let (s, _) = call(&t.app, "GET", "/auth/account", Some(&dan), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/delete/confirm",
        None,
        Some(json!({ "email": "dan@example.com", "code": code })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    // No email configured: the page says so.
    let plain = app(state(base_config()).await.unwrap());
    let (s, b) = call(
        &plain,
        "POST",
        "/auth/delete/send",
        None,
        Some(json!({ "email": "dan@example.com" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Some("email_not_configured")
        )
    );
}

async fn wait_for_mail(mail: &MemoryMailer, to: &str, count: usize) {
    for _ in 0..100 {
        if mail.count(to) >= count {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("no email to {to}");
}

async fn report(t: &T, token: &str, body: Value) -> (StatusCode, Value) {
    call(&t.app, "POST", "/reports", Some(token), Some(body)).await
}

#[tokio::test]
async fn reports_are_stored_emailed_and_limited() {
    let t = setup_with(Config {
        report_email: Some("moderation@example.com".into()),
        ..base_config()
    })
    .await;
    let (ana, _) = signup(&t, "ana@example.com", "Ana").await;
    let (bob, bob_user) = signup(&t, "bob@example.com", "Bob").await;
    let (eve, _) = signup(&t, "eve@example.com", "Eve").await;
    let room = room_with(&t, &ana, &[&bob]).await;
    room_with(&t, &eve, &[]).await;
    let ana_seat = member_id(&t, &ana, room).await;
    let bob_seat = member_id(&t, &bob, room).await;
    let round = close_round(&t, room, &[&ana, &bob], bob_seat).await;
    let (_, msg) = in_room(
        &t,
        "POST",
        &format!("/rounds/{round}/messages"),
        &bob,
        room,
        Some(json!({ "text": "un message <b>limite</b>" })),
    )
    .await;
    assert_eq!(msg["user_id"], bob_user);
    let msg = msg["id"].as_i64().unwrap();

    let (s, b) = report(
        &t,
        &ana,
        json!({ "kind": "message", "id": msg, "reason": "harassment", "details": "pas sympa" }),
    )
    .await;
    assert_eq!((s, &b), (StatusCode::OK, &json!({ "ok": true })));
    wait_for_mail(&t.mail, "moderation@example.com", 1).await;
    let email = t.mail.sent.lock().unwrap().last().unwrap().clone();
    assert!(email.text.contains("un message <b>limite</b>") && email.text.contains("pas sympa"));
    assert!(email.html.contains("&lt;b&gt;limite") && !email.html.contains("<b>limite"));
    // Twice: stored once, emailed once, still ok.
    let (s, _) = report(
        &t,
        &ana,
        json!({ "kind": "message", "id": msg, "reason": "spam" }),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = report(
        &t,
        &ana,
        json!({ "kind": "member", "id": bob_seat, "reason": "other" }),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    wait_for_mail(&t.mail, "moderation@example.com", 2).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(t.mail.count("moderation@example.com"), 2);
    let (content, reported): (String, Option<i64>) = sqlx::query_as(
        "SELECT content, reported_user_id FROM reports WHERE kind = 'message' AND target_id = ?",
    )
    .bind(msg)
    .fetch_one(&t.state.db)
    .await
    .unwrap();
    assert_eq!(
        (content.as_str(), reported),
        ("un message <b>limite</b>", Some(bob_user))
    );
    assert_eq!(count(&t, "SELECT COUNT(*) FROM reports").await, 2);

    // A question someone added, from the room.
    let (_, q) = in_room(
        &t,
        "POST",
        "/questions",
        &ana,
        room,
        Some(json!({ "text": "Qui ferait le pire colocataire ?" })),
    )
    .await;
    let (s, _) = report(
        &t,
        &bob,
        json!({ "kind": "question", "id": q["id"], "reason": "offensive" }),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    // Not myself, not from outside the room, not nonsense.
    let (s, b) = report(
        &t,
        &ana,
        json!({ "kind": "member", "id": ana_seat, "reason": "spam" }),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_target"))
    );
    let (s, b) = report(
        &t,
        &eve,
        json!({ "kind": "message", "id": msg, "reason": "spam" }),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::NOT_FOUND, Some("not_found"))
    );
    let (s, _) = report(
        &t,
        &eve,
        json!({ "kind": "member", "id": 999_999, "reason": "spam" }),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = report(
        &t,
        &ana,
        json!({ "kind": "message", "id": msg, "reason": "boring" }),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let long = "a".repeat(501);
    let (s, _) = report(
        &t,
        &ana,
        json!({ "kind": "message", "id": msg, "reason": "other", "details": long }),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    // 20 an hour per person (Ana has used 4).
    for _ in 0..16 {
        let (s, _) = report(
            &t,
            &ana,
            json!({ "kind": "member", "id": bob_seat, "reason": "spam" }),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
    }
    let (s, b) = report(
        &t,
        &ana,
        json!({ "kind": "member", "id": bob_seat, "reason": "spam" }),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::TOO_MANY_REQUESTS, Some("rate_limited"))
    );
}
