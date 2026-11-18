//! Accounts: email + password, codes, 2FA, sessions, rate limits, legacy accounts, OAuth (mock providers).

use crate::common;
use crate::common::{T, with_mailer};

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
    accounts::{MemoryMailer, totp_code},
    app,
    platform::config::{Config, OAuthClient, OAuthEndpoints},
    state, state_with_mailer,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tower::ServiceExt;

mod deletion;
mod oauth;
mod recovery;
mod security;
mod sign_in;

const PUBLIC_URL: &str = "https://qdn.test";

const PASSWORD: &str = "velo-rouge-du-dimanche";

fn base_config() -> Config {
    let dir = std::env::temp_dir().join(format!(
        "qdn-auth-test-{}-{}",
        std::process::id(),
        common::unique()
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

async fn setup() -> T {
    with_mailer(base_config()).await
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

/// Proves this session's owner with the password: sensitive changes need it (10 min).
async fn reauth(t: &T, token: &str) {
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/reauth",
        Some(token),
        Some(json!({ "password": PASSWORD })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
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

const TEST_KEY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/test_rsa_key.pem"
));

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
    let t = with_mailer(Config {
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
    oauth_round_with(t, grants, provider, token, json!({ "link": link }), profile).await
}

/// `oauth_round` with more fields in the start body (`link`, `reauth`).
async fn oauth_round_with(
    t: &T,
    grants: &Grants,
    provider: &str,
    token: Option<&str>,
    mut start: Value,
    profile: impl FnOnce(&str) -> Value,
) -> (String, String) {
    start["app_redirect"] = json!("quidenous://oauth");
    let (s, b) = call(
        &t.app,
        "POST",
        &format!("/auth/oauth/{provider}/start"),
        token,
        Some(start),
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

fn rand_code() -> String {
    common::unique()
}

fn discord(id: &str, email: &str, verified: bool) -> impl FnOnce(&str) -> Value {
    let profile = json!({ "id": id, "username": "gamer", "global_name": "Gégé", "email": email, "verified": verified });
    move |_| profile
}

/// A fresh provider round that only proves the session's owner (`reauth: true`).
async fn oauth_reauth(
    t: &T,
    grants: &Grants,
    provider: &str,
    token: &str,
    profile: impl FnOnce(&str) -> Value,
) -> (StatusCode, Value) {
    let (location, key) = oauth_round_with(
        t,
        grants,
        provider,
        Some(token),
        json!({ "reauth": true }),
        profile,
    )
    .await;
    redeem(t, &location, &key).await
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
    sqlx::query("DELETE FROM questions WHERE group_id IS NULL AND kind <> 'pick'")
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

async fn status_error(
    t: &T,
    method: &str,
    path: &str,
    token: &str,
    body: Option<Value>,
) -> (StatusCode, Option<String>) {
    let (s, b) = call(&t.app, method, path, Some(token), body).await;
    (s, b["error"].as_str().map(str::to_owned))
}

fn reauth_required() -> (StatusCode, Option<String>) {
    (StatusCode::FORBIDDEN, Some("reauth_required".into()))
}

/// The reset code e-mailed after `forgot` (sent in the background).
async fn forgot_code(t: &T, email: &str) -> String {
    let before = t.mail.count(email);
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/forgot",
        None,
        Some(json!({ "email": email })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    wait_for_code(&t.mail, email, before + 1).await
}

fn wrong_code(code: &str) -> &'static str {
    if code == "000000" { "111111" } else { "000000" }
}

/// Security notices are sent in the background.
async fn wait_notices(mail: &MemoryMailer, to: &str, count: usize) -> Vec<String> {
    for _ in 0..100 {
        let n = mail.notices(to);
        if n.len() >= count {
            return n;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("no notice {count} to {to}: {:?}", mail.notices(to));
}

async fn enable_2fa(t: &T, token: &str) -> String {
    reauth(t, token).await;
    let (s, setup) = call(&t.app, "POST", "/auth/2fa/setup", Some(token), None).await;
    assert_eq!(s, StatusCode::OK, "{setup}");
    let secret = setup["secret"].as_str().unwrap().to_owned();
    let code = totp_code(&secret, now()).unwrap();
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/2fa/enable",
        Some(token),
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    secret
}

async fn ticket(t: &T, token: &str, ip: &str) -> (StatusCode, Value) {
    let r = send(
        &t.app,
        "POST",
        "/auth/ticket",
        Some(token),
        Some(json!({ "purpose": "export" })),
        &[("x-forwarded-for", ip)],
    )
    .await;
    (r.status, r.body)
}

async fn suspend_in_db(t: &T, user: i64) {
    sqlx::query(
        "UPDATE users SET suspended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(user)
    .execute(&t.state.db)
    .await
    .unwrap();
}
