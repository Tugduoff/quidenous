//! Languages: the account's locale, push / inbox / e-mails in each person's language, room
//! languages drawing their own deck, pack languages, share pages, English legal pages.

use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use quidenous_server::{AppState, accounts::MemoryMailer, app, config::Config, state_with_mailer};
use serde_json::{Value, json};
use tower::ServiceExt;

struct T {
    app: Router,
    state: AppState,
    mail: Arc<MemoryMailer>,
}

async fn setup() -> T {
    let dir = std::env::temp_dir().join(format!(
        "qdn-i18n-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let config = Config {
        database_url: format!("sqlite://{}?mode=rwc", dir.join("test.db").display()),
        data_dir: dir,
        group_name: "Test".into(),
        join_code: "CODE-1".into(),
        vapid_subject: "mailto:test@example.com".into(),
        auth_secret: Some("test-secret-test-secret-test-secret".into()),
        ..Config::default()
    };
    let mail = Arc::new(MemoryMailer::default());
    let state = state_with_mailer(config, mail.clone()).await.unwrap();
    T {
        app: app(state.clone()),
        state,
        mail,
    }
}

async fn request(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    room: Option<i64>,
    body: Option<Value>,
) -> (StatusCode, Vec<u8>) {
    let mut req = Request::builder().method(method).uri(uri);
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    if let Some(r) = room {
        req = req.header("x-room", r.to_string());
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
    (
        status,
        res.into_body().collect().await.unwrap().to_bytes().to_vec(),
    )
}

async fn call_in(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    room: Option<i64>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let (status, bytes) = request(app, method, &format!("/api{path}"), token, room, body).await;
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    call_in(app, method, path, token, None, body).await
}

/// A newcomer in the env room (French, like every room made before languages).
async fn join(app: &Router, name: &str) -> (String, i64) {
    let (status, body) = call(
        app,
        "POST",
        "/join",
        None,
        Some(json!({ "code": "CODE-1", "name": name })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    (
        body["token"].as_str().unwrap().to_owned(),
        body["member"]["id"].as_i64().unwrap(),
    )
}

async fn set_locale(app: &Router, token: &str, locale: &str) {
    let (s, b) = call(
        app,
        "PUT",
        "/me/locale",
        Some(token),
        Some(json!({ "locale": locale, "pref": locale })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
}

#[tokio::test]
async fn locale_round_trip_and_validation() {
    let t = setup().await;
    let (ana, _) = join(&t.app, "Ana").await;
    let (s, b) = call(&t.app, "GET", "/me/locale", Some(&ana), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(b, json!({ "locale": null, "pref": null }));
    let (s, b) = call(
        &t.app,
        "PUT",
        "/me/locale",
        Some(&ana),
        Some(json!({ "locale": "en", "pref": "auto" })),
    )
    .await;
    assert_eq!(
        (s, b.clone()),
        (StatusCode::OK, json!({ "locale": "en", "pref": "auto" }))
    );
    let (_, b) = call(&t.app, "GET", "/me/locale", Some(&ana), None).await;
    assert_eq!(b, json!({ "locale": "en", "pref": "auto" }));
    for bad in [
        json!({ "locale": "auto", "pref": "auto" }),
        json!({ "locale": "de", "pref": "fr" }),
        json!({ "locale": "fr", "pref": "de" }),
        json!({ "locale": "fr" }),
        json!({ "locale": 1, "pref": "fr" }),
        json!({}),
    ] {
        let (s, b) = call(&t.app, "PUT", "/me/locale", Some(&ana), Some(bad.clone())).await;
        assert_eq!(
            (s, b["error"].as_str()),
            (StatusCode::BAD_REQUEST, Some("bad_locale")),
            "{bad}"
        );
    }
    // Unchanged by the rejected ones.
    let (_, b) = call(&t.app, "GET", "/me/locale", Some(&ana), None).await;
    assert_eq!(b, json!({ "locale": "en", "pref": "auto" }));
    let (s, _) = call(&t.app, "GET", "/me/locale", None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (_, features) = call(&t.app, "GET", "/features", None, None).await;
    assert!(features.as_array().unwrap().contains(&json!("languages")));
}

/// Waits a little for the background senders until the recorded pushes satisfy `ok`.
async fn wait_for(state: &AppState, ok: impl Fn(&[quidenous_server::push::Sent]) -> bool) -> bool {
    for _ in 0..100 {
        if ok(&state.push.recorded()) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    false
}

async fn user_of(state: &AppState, member: i64) -> i64 {
    sqlx::query_scalar("SELECT user_id FROM members WHERE id = ?")
        .bind(member)
        .fetch_one(&state.db)
        .await
        .unwrap()
}

#[tokio::test]
async fn push_and_inbox_speak_each_recipients_language() {
    let t = setup().await;
    t.state.push.record();
    let (ana, ana_id) = join(&t.app, "Ana").await;
    let (bob, bob_id) = join(&t.app, "Bob").await;
    set_locale(&t.app, &bob, "en").await;
    for (i, token) in [&ana, &bob].into_iter().enumerate() {
        let device = json!({ "kind": "web", "endpoint": format!("https://push.example/{i}"), "p256dh": "x", "auth": "y" });
        let (s, _) = call(&t.app, "POST", "/push/subscribe", Some(token), Some(device)).await;
        assert_eq!(s, StatusCode::OK);
    }
    let (ana_user, bob_user) = (
        user_of(&t.state, ana_id).await,
        user_of(&t.state, bob_id).await,
    );

    // Ana nudges Bob (English), Bob nudges Ana (French).
    let (s, b) = call(
        &t.app,
        "POST",
        "/round/current/nudge",
        Some(&ana),
        Some(json!({ "member_id": bob_id })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (s, _) = call(
        &t.app,
        "POST",
        "/round/current/nudge",
        Some(&bob),
        Some(json!({ "member_id": ana_id })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let has = |user: i64, title: &str| {
        let title = title.to_owned();
        move |sent: &[quidenous_server::push::Sent]| {
            sent.iter()
                .any(|p| p.user_id == Some(user) && p.title == title)
        }
    };
    assert!(wait_for(&t.state, has(bob_user, "Ana is waiting for your vote")).await);
    assert!(wait_for(&t.state, has(ana_user, "Bob attend ton vote")).await);
    assert!(
        !t.state
            .push
            .recorded()
            .iter()
            .any(|p| p.title == "Ana attend ton vote")
    );

    let inbox_text = |v: &Value, kind: &str| {
        v["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["kind"] == kind)
            .map(|i| i["text"].as_str().unwrap().to_owned())
    };
    let (_, inbox) = call(&t.app, "GET", "/inbox", Some(&bob), None).await;
    assert_eq!(
        inbox_text(&inbox, "nudge").as_deref(),
        Some("Ana is waiting for your vote")
    );
    let (_, inbox) = call(&t.app, "GET", "/inbox", Some(&ana), None).await;
    assert_eq!(
        inbox_text(&inbox, "nudge").as_deref(),
        Some("Bob attend ton vote")
    );

    // A group push: the result. Ana votes last, so Bob hears about it, in English.
    let (_, round) = call(&t.app, "GET", "/round/current", Some(&ana), None).await;
    for token in [&bob, &ana] {
        let body = if round["kind"] == "rank" {
            json!({ "ranking": [ana_id, bob_id] })
        } else {
            json!({ "target_id": bob_id })
        };
        let (s, b) = call(
            &t.app,
            "POST",
            "/round/current/vote",
            Some(token),
            Some(body),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    assert!(
        wait_for(&t.state, |sent| sent.iter().any(|p| p.user_id
            == Some(bob_user)
            && p.title.starts_with("Results are in")))
        .await,
        "{:?}",
        t.state.push.recorded()
    );
    assert!(
        !t.state
            .push
            .recorded()
            .iter()
            .any(|p| p.user_id == Some(bob_user) && p.title.starts_with("Résultat"))
    );
}

#[tokio::test]
async fn account_emails_in_the_accounts_language() {
    let t = setup().await;
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/signup",
        None,
        Some(json!({ "email": "en@example.com", "password": "velo-rouge-du-dimanche", "name": "Eve", "locale": "en" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/signup",
        None,
        Some(json!({ "email": "fr@example.com", "password": "velo-rouge-du-dimanche", "name": "Léa" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let lea = b["token"].as_str().unwrap().to_owned();
    let last = |to: &str| {
        let sent = t.mail.sent.lock().unwrap();
        sent.iter().rev().find(|e| e.to == to).cloned().unwrap()
    };
    let en = last("en@example.com");
    assert!(
        en.subject.ends_with("· your Qui de nous ? code"),
        "{}",
        en.subject
    );
    assert!(en.text.starts_with("Here is your code"), "{}", en.text);
    assert!(en.html.contains("lang=\"en\""));
    assert!(t.mail.last_code("en@example.com").is_some());
    let fr = last("fr@example.com");
    assert!(
        fr.subject.ends_with("· ton code Qui de nous ?"),
        "{}",
        fr.subject
    );
    assert!(fr.html.contains("lang=\"fr\""));

    // Léa switches to English: her reset code comes in English. Sent in the background.
    set_locale(&t.app, &lea, "en").await;
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/forgot",
        None,
        Some(json!({ "email": "fr@example.com" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    for _ in 0..100 {
        if t.mail.count("fr@example.com") >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let reset = last("fr@example.com");
    assert!(
        reset.subject.contains("new Qui de nous ? password"),
        "{}",
        reset.subject
    );
    assert!(reset.text.contains("choose a new password"));
}

/// A deck question's language, and whether it is a built-in one.
async fn question_lang(state: &AppState, text: &str) -> Option<String> {
    sqlx::query_scalar("SELECT lang FROM questions WHERE group_id IS NULL AND text = ?")
        .bind(text)
        .fetch_optional(&state.db)
        .await
        .unwrap()
}

/// Built-in questions of this language a `potes` room can draw.
async fn deck_size(state: &AppState, lang: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM questions WHERE group_id IS NULL AND lang = ? \
         AND (vibes IS NULL OR vibes = '' OR (',' || vibes || ',') LIKE '%,potes,%')",
    )
    .bind(lang)
    .fetch_one(&state.db)
    .await
    .unwrap()
}

#[tokio::test]
async fn rooms_draw_the_deck_of_their_language() {
    let t = setup().await;
    let (ana, _) = join(&t.app, "Ana").await;
    let (bob, _) = join(&t.app, "Bob").await;
    let (en_size, fr_size) = (
        deck_size(&t.state, "en").await,
        deck_size(&t.state, "fr").await,
    );
    assert!(en_size > 0 && fr_size > 0);

    let (s, b) = call(
        &t.app,
        "POST",
        "/rooms",
        Some(&ana),
        Some(json!({ "name": "Bad", "language": "de" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_language"))
    );
    let (s, created) = call(
        &t.app,
        "POST",
        "/rooms",
        Some(&ana),
        Some(json!({ "name": "Friends", "language": "en" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{created}");
    assert_eq!(created["language"], "en");
    let en_room = created["id"].as_i64().unwrap();
    // No language: French, as before.
    let (_, created) = call(
        &t.app,
        "POST",
        "/rooms",
        Some(&ana),
        Some(json!({ "name": "Potes" })),
    )
    .await;
    assert_eq!(created["language"], "fr");
    let fr_room = created["id"].as_i64().unwrap();

    let (_, rooms) = call(&t.app, "GET", "/rooms", Some(&ana), None).await;
    let lang_of = |id: i64| {
        rooms
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap()["language"]
            .clone()
    };
    assert_eq!(
        (lang_of(en_room), lang_of(fr_room)),
        (json!("en"), json!("fr"))
    );

    for room in [en_room, fr_room] {
        let (_, group) = call_in(&t.app, "GET", "/group", Some(&ana), Some(room), None).await;
        let (s, b) = call(
            &t.app,
            "POST",
            "/rooms/join",
            Some(&bob),
            Some(json!({ "code": group["code"] })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    let (_, group) = call_in(&t.app, "GET", "/group", Some(&ana), Some(en_room), None).await;
    assert_eq!(
        (
            group["language"].clone(),
            group["settings"]["language"].clone()
        ),
        (json!("en"), json!("en"))
    );

    // Every question the English room gets is from the English deck, until it runs out.
    let ids = |g: &Value| -> Vec<i64> {
        g["members"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["id"].as_i64().unwrap())
            .collect()
    };
    let members = ids(&group);
    let mut drawn: i64 = 0;
    while drawn < 5 {
        let (_, round) = call_in(
            &t.app,
            "GET",
            "/round/current",
            Some(&ana),
            Some(en_room),
            None,
        )
        .await;
        if round.is_null() {
            break;
        }
        let text = round["question"].as_str().unwrap();
        assert_eq!(
            question_lang(&t.state, text).await.as_deref(),
            Some("en"),
            "{text}"
        );
        drawn += 1;
        for token in [&ana, &bob] {
            let body = if round["kind"] == "rank" {
                json!({ "ranking": members })
            } else {
                json!({ "target_id": members[0] })
            };
            let (s, b) = call_in(
                &t.app,
                "POST",
                "/round/current/vote",
                Some(token),
                Some(en_room),
                Some(body),
            )
            .await;
            assert_eq!(s, StatusCode::OK, "{b}");
        }
    }
    assert!(drawn >= 1);
    let (_, round) = call_in(
        &t.app,
        "GET",
        "/round/current",
        Some(&ana),
        Some(fr_room),
        None,
    )
    .await;
    let text = round["question"].as_str().unwrap();
    assert_eq!(
        question_lang(&t.state, text).await.as_deref(),
        Some("fr"),
        "{text}"
    );

    // deck_left counts the room's deck; switching the language switches it.
    let left = |room: i64| {
        let app = t.app.clone();
        let ana = ana.clone();
        async move {
            call_in(
                &app,
                "GET",
                "/questions/queue",
                Some(&ana),
                Some(room),
                None,
            )
            .await
            .1["deck_left"]
                .as_i64()
                .unwrap()
        }
    };
    let opened: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rounds WHERE group_id = ?")
        .bind(en_room)
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    assert!(opened >= drawn);
    assert_eq!(left(en_room).await, en_size - opened);
    assert_eq!(left(fr_room).await, fr_size - 1);
    let (s, b) = call(
        &t.app,
        "PATCH",
        &format!("/rooms/{en_room}"),
        Some(&ana),
        Some(json!({ "language": "xx" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_language"))
    );
    let (s, _) = call(
        &t.app,
        "PATCH",
        &format!("/rooms/{en_room}"),
        Some(&bob),
        Some(json!({ "language": "fr" })),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(
        &t.app,
        "PATCH",
        &format!("/rooms/{en_room}"),
        Some(&ana),
        Some(json!({ "language": "fr" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, group) = call_in(&t.app, "GET", "/group", Some(&ana), Some(en_room), None).await;
    assert_eq!(group["language"], "fr");
    assert_eq!(left(en_room).await, fr_size);
}

#[tokio::test]
async fn packs_keep_their_language() {
    let t = setup().await;
    let (ana, _) = join(&t.app, "Ana").await;
    let file = "Who has the most plants?\nWho would be the worst astronaut?\nWho cries at ads?\nWho always loses their keys?\nWho sings off key?\n";
    let (s, b) = call(
        &t.app,
        "POST",
        "/packs",
        Some(&ana),
        Some(json!({ "name": "Soft", "text": file, "language": "es" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_language"))
    );
    let (s, b) = call(
        &t.app,
        "POST",
        "/packs",
        Some(&ana),
        Some(json!({ "name": "Soft", "text": file, "language": "en" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let en = b["id"].as_i64().unwrap();
    let (_, b) = call(
        &t.app,
        "POST",
        "/packs",
        Some(&ana),
        Some(json!({ "name": "Doux", "text": file })),
    )
    .await;
    let fr = b["id"].as_i64().unwrap();
    let (_, pack) = call(&t.app, "GET", &format!("/packs/{en}"), Some(&ana), None).await;
    assert_eq!(pack["language"], "en");
    let (_, list) = call(&t.app, "GET", "/packs", Some(&ana), None).await;
    let lang_of = |id: i64| {
        list.as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == id)
            .unwrap()["language"]
            .clone()
    };
    assert_eq!((lang_of(en), lang_of(fr)), (json!("en"), json!("fr")));
}

#[tokio::test]
async fn share_pages_speak_the_sharers_language() {
    let t = setup().await;
    let (ana, _) = join(&t.app, "Ana").await;
    let (bob, _) = join(&t.app, "Bob").await;
    set_locale(&t.app, &bob, "en").await;
    let (_, round) = call(&t.app, "GET", "/round/current", Some(&ana), None).await;
    let id = round["id"].as_i64().unwrap();
    let mut tokens = Vec::new();
    for who in [&ana, &bob] {
        let (s, b) = call(
            &t.app,
            "POST",
            &format!("/rounds/{id}/share"),
            Some(who),
            Some(json!({})),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
        tokens.push(b["token"].as_str().unwrap().to_owned());
    }
    let page = |token: String| {
        let app = t.app.clone();
        async move {
            let (s, html) = request(&app, "GET", &format!("/s/{token}"), None, None, None).await;
            assert_eq!(s, StatusCode::OK);
            String::from_utf8(html).unwrap()
        }
    };
    let fr = page(tokens[0].clone()).await;
    assert!(fr.contains("<html lang=\"fr\"") && fr.contains("Ouvrir qui de nous ?"));
    assert!(fr.contains("Et toi, tu votes pour qui ?"));
    let en = page(tokens[1].clone()).await;
    assert!(
        en.contains("<html lang=\"en\"") && en.contains("Open qui de nous ?"),
        "{en}"
    );
    assert!(en.contains("Who would you vote for?"));
    // Bob switches to French: a new link in French, the old one stays English.
    set_locale(&t.app, &bob, "fr").await;
    let (_, b) = call(
        &t.app,
        "POST",
        &format!("/rounds/{id}/share"),
        Some(&bob),
        Some(json!({})),
    )
    .await;
    assert_ne!(b["token"].as_str().unwrap(), tokens[1]);
    assert!(page(tokens[1].clone()).await.contains("Open qui de nous ?"));
    let (s, png) = request(
        &t.app,
        "GET",
        &format!("/s/{}/card.png", tokens[1]),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert!(png.starts_with(b"\x89PNG"));
}

#[tokio::test]
async fn legal_pages_in_english() {
    let t = setup().await;
    for (path, other) in [
        ("/en/privacy", "/privacy"),
        ("/en/terms", "/terms"),
        ("/en/delete-account", "/delete-account"),
        ("/privacy", "/en/privacy"),
        ("/terms", "/en/terms"),
        ("/delete-account", "/en/delete-account"),
    ] {
        let (s, html) = request(&t.app, "GET", path, None, None, None).await;
        assert_eq!(s, StatusCode::OK, "{path}");
        let html = String::from_utf8(html).unwrap();
        let lang = if path.starts_with("/en/") { "en" } else { "fr" };
        assert!(html.contains(&format!("<html lang=\"{lang}\"")), "{path}");
        assert!(
            html.contains(&format!("href=\"{other}\"")),
            "{path} links to {other}"
        );
    }
}

#[tokio::test]
async fn stats_stories_in_the_readers_language() {
    let t = setup().await;
    let (ana, ana_id) = join(&t.app, "Ana").await;
    let (bob, bob_id) = join(&t.app, "Bob").await;
    set_locale(&t.app, &bob, "en").await;
    // Bob wins four in a row: a streak story.
    for _ in 0..4 {
        let (_, round) = call(&t.app, "GET", "/round/current", Some(&ana), None).await;
        for token in [&ana, &bob] {
            let body = if round["kind"] == "rank" {
                json!({ "ranking": [bob_id, ana_id] })
            } else {
                json!({ "target_id": bob_id })
            };
            let (s, b) = call(
                &t.app,
                "POST",
                "/round/current/vote",
                Some(token),
                Some(body),
            )
            .await;
            assert_eq!(s, StatusCode::OK, "{b}");
        }
    }
    let streak = |s: &Value| {
        s["stories"]
            .as_array()
            .unwrap()
            .iter()
            .find(|st| st["key"] == "streak")
            .cloned()
            .unwrap_or_else(|| panic!("no streak story: {s}"))
    };
    let (_, fr) = call(&t.app, "GET", "/stats", Some(&ana), None).await;
    let (_, en) = call(&t.app, "GET", "/stats", Some(&bob), None).await;
    let (fr, en) = (streak(&fr), streak(&en));
    assert_eq!(fr["title"], "Série en cours");
    assert_eq!(en["title"], "Current streak");
    assert_eq!(fr["big"], "4 d'affilée");
    assert!(en["big"].as_str().unwrap().contains("in a row"), "{en}");
    assert_ne!(fr["line"], en["line"]);

    // A shared story keeps the sharer's language.
    let (s, b) = call(
        &t.app,
        "POST",
        "/stats/share",
        Some(&bob),
        Some(json!({ "key": "streak" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let token = b["token"].as_str().unwrap();
    let (s, html) = request(&t.app, "GET", &format!("/s/{token}"), None, None, None).await;
    assert_eq!(s, StatusCode::OK);
    let html = String::from_utf8(html).unwrap();
    assert!(
        html.contains("<html lang=\"en\"") && html.contains("Current streak"),
        "{html}"
    );
    // The history export too.
    for (token, header) in [
        (&bob, "\u{feff}number,date,question,winner(s)"),
        (&ana, "\u{feff}numéro,date"),
    ] {
        let (s, csv) = request(
            &t.app,
            "GET",
            &format!("/api/history/export?token={token}"),
            None,
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert!(String::from_utf8(csv).unwrap().starts_with(header));
    }
}
