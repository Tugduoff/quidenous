//! Community v1 (docs/COMMUNITY.md): packs 2.0, admin review, suggestions, share → play, challenges.

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

/// A fresh server; the first account to join (user 1) is the admin.
async fn setup() -> T {
    let dir = std::env::temp_dir().join(format!(
        "qdn-community-{}",
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
        admin_user_ids: vec![1],
        ..Config::default()
    };
    let mail = Arc::new(MemoryMailer::default());
    let state = state_with_mailer(config, mail.clone()).await.unwrap();
    // Votes here pick one person: keep the deck to pick questions.
    sqlx::query("DELETE FROM questions WHERE group_id IS NULL AND kind <> 'pick'")
        .execute(&state.db)
        .await
        .unwrap();
    T {
        app: app(state.clone()),
        state,
        mail,
    }
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn request(
    method: &str,
    path: &str,
    token: Option<&str>,
    room: Option<i64>,
    body: Option<Value>,
) -> Request<Body> {
    let mut req = Request::builder().method(method).uri(format!("/api{path}"));
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    if let Some(r) = room {
        req = req.header("x-room", r.to_string());
    }
    match body {
        Some(b) => req
            .header("content-type", "application/json")
            .body(Body::from(b.to_string())),
        None => req.body(Body::empty()),
    }
    .unwrap()
}

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    send(app, request(method, path, token, None, body)).await
}

async fn call_room(
    app: &Router,
    method: &str,
    path: &str,
    token: &str,
    room: i64,
    body: Option<Value>,
) -> (StatusCode, Value) {
    send(app, request(method, path, Some(token), Some(room), body)).await
}

/// Joins the env room "Test": (token, member id).
async fn join(app: &Router, name: &str) -> (String, i64) {
    let (status, body) = call(
        app,
        "POST",
        "/join",
        None,
        Some(json!({ "code": "code-1", "name": name })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    (
        body["token"].as_str().unwrap().to_owned(),
        body["member"]["id"].as_i64().unwrap(),
    )
}

async fn user_of(t: &T, member: i64) -> i64 {
    sqlx::query_scalar("SELECT user_id FROM members WHERE id = ?")
        .bind(member)
        .fetch_one(&t.state.db)
        .await
        .unwrap()
}

async fn page(app: &Router, path: &str) -> (StatusCode, String) {
    let res = app
        .clone()
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

/// A new room owned by `owner`, with `others` in it: its id.
async fn room_with(
    app: &Router,
    owner: &str,
    name: &str,
    others: &[&str],
    vibe: Option<&str>,
) -> i64 {
    let (s, room) = call(
        app,
        "POST",
        "/rooms",
        Some(owner),
        Some(json!({ "name": name, "vibe": vibe })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{room}");
    let id = room["id"].as_i64().unwrap();
    let (_, g) = call_room(app, "GET", "/group", owner, id, None).await;
    let code = g["code"].as_str().unwrap().to_owned();
    for t in others {
        let (s, b) = call(
            app,
            "POST",
            "/rooms/join",
            Some(t),
            Some(json!({ "code": code })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    id
}

/// Five (or more) distinct questions for a pack file.
fn questions(prefix: &str, n: usize) -> Vec<Value> {
    (0..n)
        .map(|i| json!({ "text": format!("{prefix} question numéro {i} ?"), "kind": "pick" }))
        .collect()
}

async fn publish(app: &Router, token: &str, body: Value) -> i64 {
    let (s, b) = call(app, "POST", "/packs", Some(token), Some(body)).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    b["id"].as_i64().unwrap()
}

fn ids(list: &Value) -> Vec<i64> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_i64().unwrap())
        .collect()
}

#[tokio::test]
async fn packs_have_categories_spicy_opt_in_likes_and_sorts() {
    let t = setup().await;
    let app = &t.app;
    let (admin, _) = join(app, "Admin").await;
    let (ana, _) = join(app, "Ana").await;
    let (bob, _) = join(app, "Bob").await;
    let (cyd, _) = join(app, "Cyd").await;
    let (_, features) = call(app, "GET", "/features", None, None).await;
    assert!(features.as_array().unwrap().contains(&json!("community")));

    // Categories: known ids, 3 at most.
    for tags in [
        json!(["nope"]),
        json!(["potes", "boulot", "famille", "culture"]),
    ] {
        let (s, b) = call(
            app,
            "POST",
            "/packs",
            Some(&ana),
            Some(json!({ "name": "X", "questions": questions("x", 5), "tags": tags })),
        )
        .await;
        assert_eq!(
            (s, b["error"].as_str()),
            (StatusCode::BAD_REQUEST, Some("bad_tags"))
        );
    }
    let p1 = publish(
        app,
        &ana,
        json!({ "name": "Classiques", "questions": questions("a", 5), "tags": ["potes", "potes"] }),
    )
    .await;
    // Tagged pimente: always 16+.
    let p2 = publish(
        app,
        &ana,
        json!({ "name": "Hot", "questions": questions("b", 5), "tags": ["pimente"] }),
    )
    .await;
    let p3 = publish(
        app,
        &bob,
        json!({ "name": "Soirée", "questions": questions("c", 5), "tags": ["soiree"], "spicy": true }),
    )
    .await;
    let (_, d) = call(app, "GET", &format!("/packs/{p2}"), Some(&bob), None).await;
    assert_eq!(
        (d["spicy"].clone(), d["tags"].clone()),
        (json!(true), json!(["pimente"]))
    );
    assert_eq!(d["questions"].as_array().unwrap().len(), 5);
    let (_, d) = call(app, "GET", &format!("/packs/{p1}"), Some(&bob), None).await;
    assert_eq!(d["tags"], json!(["potes"]));
    for key in [
        "likes",
        "liked",
        "installed",
        "featured",
        "status",
        "version",
        "author_id",
        "sample",
    ] {
        assert!(d.get(key).is_some(), "{key} in {d}");
    }

    // 16+ packs: hidden until opted in.
    let (_, list) = call(app, "GET", "/packs", Some(&bob), None).await;
    assert_eq!(ids(&list), [p1]);
    let (_, list) = call(app, "GET", "/packs?tag=pimente", Some(&bob), None).await;
    assert_eq!(ids(&list), Vec::<i64>::new());
    let (s, me) = call(
        app,
        "PUT",
        "/community/me",
        Some(&bob),
        Some(json!({ "show_spicy": true })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        (me["show_spicy"].clone(), me["admin"].clone()),
        (json!(true), json!(false))
    );
    assert_eq!(me["suggestions_left"], 5);
    let (_, me) = call(app, "GET", "/community/me", Some(&admin), None).await;
    assert_eq!(me["admin"], true);
    let (_, list) = call(app, "GET", "/packs?sort=new", Some(&bob), None).await;
    assert_eq!(ids(&list), [p3, p2, p1]);
    let (_, list) = call(app, "GET", "/packs?tag=pimente", Some(&bob), None).await;
    assert_eq!(ids(&list), [p2]);
    // ... or in a room whose vibe is pimente.
    let hot_room = room_with(app, &cyd, "Hot", &[], Some("pimente")).await;
    let (_, test_room) = call(app, "GET", "/group", Some(&admin), None).await;
    let test_room = test_room["id"].as_i64().unwrap();
    let (_, list) = call_room(app, "GET", "/packs?sort=new", &cyd, hot_room, None).await;
    assert_eq!(ids(&list), [p3, p2, p1]);
    let (_, list) = call_room(app, "GET", "/packs?sort=new", &cyd, test_room, None).await;
    assert_eq!(ids(&list), [p1]);

    // One like per person, never on my own pack.
    for _ in 0..2 {
        let (s, b) = call(app, "POST", &format!("/packs/{p1}/like"), Some(&bob), None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(b, json!({ "likes": 1, "liked": true }));
    }
    let (_, b) = call(
        app,
        "DELETE",
        &format!("/packs/{p1}/like"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(b, json!({ "likes": 0, "liked": false }));
    let (s, b) = call(app, "POST", &format!("/packs/{p1}/like"), Some(&ana), None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("own_pack"))
    );
    // Likes: p2 ×2, p1 ×1.
    call(app, "POST", &format!("/packs/{p2}/like"), Some(&bob), None).await;
    call(app, "POST", &format!("/packs/{p2}/like"), Some(&cyd), None).await;
    call(app, "POST", &format!("/packs/{p1}/like"), Some(&cyd), None).await;
    let (_, list) = call(app, "GET", "/packs?sort=liked", Some(&bob), None).await;
    assert_eq!(ids(&list), [p2, p1, p3]);
    assert_eq!(list[0]["liked"], true);
    assert_eq!(list[0]["likes"], 2);
    // Trending: p3 installed in two rooms this week (2), p2 liked twice (2, fewer installs), p1 (1).
    let bob_room = room_with(app, &bob, "Chez Bob", &[], None).await;
    for room in [bob_room, test_room] {
        let (s, b) = call_room(
            app,
            "POST",
            &format!("/packs/{p3}/install"),
            &bob,
            room,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    let (_, list) = call(app, "GET", "/packs?sort=trending", Some(&bob), None).await;
    assert_eq!(ids(&list), [p3, p2, p1]);
    let (_, list) = call(app, "GET", "/packs", Some(&bob), None).await;
    assert_eq!(ids(&list), [p3, p2, p1], "trending is the default");
    // Featured: admins only.
    let (s, b) = call(
        app,
        "POST",
        &format!("/admin/packs/{p1}/feature"),
        Some(&bob),
        Some(json!({ "featured": true })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("not_admin"))
    );
    let (s, b) = call(
        app,
        "POST",
        &format!("/admin/packs/{p1}/feature"),
        Some(&admin),
        Some(json!({ "featured": true })),
    )
    .await;
    assert_eq!((s, b), (StatusCode::OK, json!({ "featured": true })));
    let (_, list) = call(app, "GET", "/packs?sort=featured", Some(&bob), None).await;
    assert_eq!(ids(&list), [p1]);
    assert_eq!(list[0]["featured"], true);
    // Languages: all by default, filtered when asked.
    let p4 = publish(
        app,
        &cyd,
        json!({ "name": "English", "language": "en", "questions": questions("Who", 5) }),
    )
    .await;
    let (_, list) = call(app, "GET", "/packs?lang=en", Some(&bob), None).await;
    assert_eq!(ids(&list), [p4]);
    let (_, list) = call(app, "GET", "/packs?sort=new&q=engl", Some(&bob), None).await;
    assert_eq!(ids(&list), [p4]);
    let (_, list) = call(app, "GET", "/packs?sort=new", Some(&bob), None).await;
    assert_eq!(list.as_array().unwrap().len(), 4);
}

#[tokio::test]
async fn installs_count_per_room_and_authors_edit_and_delete() {
    let t = setup().await;
    let app = &t.app;
    let (admin, _) = join(app, "Admin").await;
    let (ana, _) = join(app, "Ana").await;
    let (bob, _) = join(app, "Bob").await;
    let pack = publish(
        app,
        &ana,
        json!({ "name": "Pack", "description": "Doux", "questions": questions("p", 6) }),
    )
    .await;
    let r1 = room_with(app, &bob, "Un", &[], None).await;
    let r2 = room_with(app, &bob, "Deux", &[], None).await;
    let (s, b) = call_room(
        app,
        "POST",
        &format!("/packs/{pack}/install"),
        &bob,
        r1,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b, json!({ "added": 6, "skipped": 0, "installed": true }));
    let (_, b) = call_room(
        app,
        "POST",
        &format!("/packs/{pack}/install"),
        &bob,
        r1,
        None,
    )
    .await;
    assert_eq!(
        (b["added"].clone(), b["skipped"].clone()),
        (json!(0), json!(6))
    );
    let (_, d) = call_room(app, "GET", &format!("/packs/{pack}"), &bob, r1, None).await;
    assert_eq!(
        (d["installs"].clone(), d["installed"].clone()),
        (json!(1), json!(true))
    );
    let (_, list) = call_room(app, "GET", "/packs", &bob, r2, None).await;
    assert_eq!(list[0]["installed"], false);
    call_room(
        app,
        "POST",
        &format!("/packs/{pack}/install"),
        &bob,
        r2,
        None,
    )
    .await;
    let (_, list) = call_room(app, "GET", "/packs", &bob, r2, None).await;
    assert_eq!(
        (list[0]["installs"].clone(), list[0]["installed"].clone()),
        (json!(2), json!(true))
    );

    // Editing: only the author; new questions bump the version, reinstalling adds only those.
    let mut qs = questions("p", 6);
    qs.push(json!({ "text": "Qui est arrivé avec la version deux ?", "kind": "rank" }));
    let (s, b) = call(
        app,
        "PUT",
        &format!("/packs/{pack}"),
        Some(&bob),
        Some(json!({ "name": "Volé", "questions": qs })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("not_author"))
    );
    let (s, b) = call(
        app,
        "PUT",
        &format!("/packs/{pack}"),
        Some(&ana),
        Some(json!({ "name": "Pack v2", "description": "Doux", "language": "fr", "tags": ["famille"], "spicy": false, "questions": qs })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(
        (b["version"].clone(), b["count"].clone(), b["name"].clone()),
        (json!(2), json!(7), json!("Pack v2"))
    );
    assert_eq!(b["tags"], json!(["famille"]));
    let (s, b) = call(
        app,
        "PUT",
        &format!("/packs/{pack}"),
        Some(&ana),
        Some(json!({ "name": "Pack", "questions": qs })),
    )
    .await;
    assert_eq!(
        (s, b["version"].clone()),
        (StatusCode::OK, json!(2)),
        "same questions: same version"
    );
    let (s, b) = call(
        app,
        "PUT",
        &format!("/packs/{pack}"),
        Some(&ana),
        Some(json!({ "questions": questions("z", 2) })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("pack_size"))
    );
    let (_, b) = call_room(
        app,
        "POST",
        &format!("/packs/{pack}/install"),
        &bob,
        r1,
        None,
    )
    .await;
    assert_eq!(b["added"], 1);
    let (_, d) = call(app, "GET", &format!("/packs/{pack}"), Some(&bob), None).await;
    assert_eq!(d["installs"], 2);

    // Mine, then delete.
    let (_, mine) = call(app, "GET", "/packs/mine", Some(&ana), None).await;
    assert_eq!(ids(&mine), [pack]);
    assert_eq!(mine[0]["status"], "public");
    assert_eq!(mine[0]["mine"], true);
    let (s, _) = call(app, "DELETE", &format!("/packs/{pack}"), Some(&bob), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(app, "DELETE", &format!("/packs/{pack}"), Some(&ana), None).await;
    assert_eq!(s, StatusCode::OK);
    let (_, mine) = call(app, "GET", "/packs/mine", Some(&ana), None).await;
    assert_eq!(mine, json!([]));
    let (s, _) = call(app, "GET", &format!("/packs/{pack}"), Some(&bob), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, b) = call(
        app,
        "PUT",
        &format!("/packs/{pack}"),
        Some(&ana),
        Some(json!({ "name": "Back" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("pack_removed"))
    );

    // Ten packs in public/review; deleted and removed ones don't count.
    let mut made = Vec::new();
    for i in 0..10 {
        made.push(
            publish(
                app,
                &ana,
                json!({ "name": format!("P{i}"), "questions": questions(&format!("n{i}"), 5) }),
            )
            .await,
        );
    }
    let (s, b) = call(
        app,
        "POST",
        "/packs",
        Some(&ana),
        Some(json!({ "name": "Onze", "questions": questions("o", 5) })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("too_many_packs"))
    );
    let (s, _) = call(
        app,
        "POST",
        &format!("/admin/packs/{}/remove", made[0]),
        Some(&admin),
        Some(json!({ "reason": "Hors charte" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    publish(
        app,
        &ana,
        json!({ "name": "Onze", "questions": questions("o", 5) }),
    )
    .await;
    let (s, b) = call(
        app,
        "PUT",
        &format!("/packs/{}", made[0]),
        Some(&ana),
        Some(json!({ "name": "Back" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("pack_removed"))
    );
}

#[tokio::test]
async fn reports_send_a_pack_to_review_and_admins_decide() {
    let t = setup().await;
    let app = &t.app;
    let (admin, _) = join(app, "Admin").await;
    let (ana, ana_member) = join(app, "Ana Martin").await;
    let (bob, bob_member) = join(app, "Bob").await;
    let ana_user = user_of(&t, ana_member).await;
    let bob_user = user_of(&t, bob_member).await;
    let pack = publish(
        app,
        &ana,
        json!({ "name": "Douteux", "questions": questions("d", 5) }),
    )
    .await;
    let mut reporters = Vec::new();
    for name in ["Cyd", "Dan", "Eve"] {
        reporters.push(join(app, name).await.0);
    }
    let (s, b) = call(
        app,
        "POST",
        &format!("/packs/{pack}/report"),
        Some(&reporters[0]),
        Some(json!({ "reason": "nope" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_report"))
    );
    // Old apps send no body at all.
    let (s, _) = call(
        app,
        "POST",
        &format!("/packs/{pack}/report"),
        Some(&reporters[0]),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(
        app,
        "POST",
        &format!("/packs/{pack}/report"),
        Some(&reporters[1]),
        Some(json!({ "reason": "spam", "details": "de la pub" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, list) = call(app, "GET", "/packs", Some(&bob), None).await;
    assert_eq!(ids(&list), [pack], "two reports: still listed");
    call(
        app,
        "POST",
        &format!("/packs/{pack}/report"),
        Some(&reporters[2]),
        Some(json!({ "reason": "offensive" })),
    )
    .await;
    // Once per person: no new report, no new e-mail.
    call(
        app,
        "POST",
        &format!("/packs/{pack}/report"),
        Some(&reporters[2]),
        None,
    )
    .await;
    let (_, list) = call(app, "GET", "/packs", Some(&bob), None).await;
    assert_eq!(list, json!([]));
    let (_, mine) = call(app, "GET", "/packs/mine", Some(&ana), None).await;
    assert_eq!(mine[0]["status"], "review");
    let (s, d) = call(app, "GET", &format!("/packs/{pack}"), Some(&ana), None).await;
    assert_eq!((s, d["status"].clone()), (StatusCode::OK, json!("review")));
    let (s, _) = call(app, "GET", &format!("/packs/{pack}"), Some(&bob), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = call(app, "GET", &format!("/packs/{pack}"), Some(&admin), None).await;
    assert_eq!(s, StatusCode::OK);

    // Each report was e-mailed; the third says it went to review.
    for _ in 0..100 {
        if t.mail.count("contact@coflop.fr") >= 3 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    {
        let sent = t.mail.sent.lock().unwrap();
        let reports: Vec<_> = sent
            .iter()
            .filter(|e| e.to == "contact@coflop.fr")
            .collect();
        assert_eq!(reports.len(), 3);
        assert!(reports[1].text.contains("spam") && reports[1].text.contains("de la pub"));
        assert!(reports[2].text.contains("Douteux") && reports[2].text.contains("Ana Martin"));
        assert!(reports[2].text.contains("Signalements : 3") && reports[2].text.contains("oui"));
        assert!(reports[0].text.contains("En revue : non"));
    }

    // Admin review.
    let (s, b) = call(app, "GET", "/admin/review", Some(&bob), None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("not_admin"))
    );
    let (s, review) = call(app, "GET", "/admin/review", Some(&admin), None).await;
    assert_eq!(s, StatusCode::OK, "{review}");
    assert_eq!(ids(&review["packs"]), [pack]);
    let reports = review["packs"][0]["reports"].as_array().unwrap();
    assert_eq!(reports.len(), 3);
    assert_eq!(reports[1]["reason"], "spam");
    assert!(reports[1]["created_at"].is_string());
    let (s, _) = call(
        app,
        "POST",
        &format!("/admin/packs/{pack}/approve"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, b) = call(
        app,
        "POST",
        &format!("/admin/packs/{pack}/approve"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!((s, b["status"].clone()), (StatusCode::OK, json!("public")));
    let (_, list) = call(app, "GET", "/packs", Some(&bob), None).await;
    assert_eq!(ids(&list), [pack]);
    // Reports were cleared: one new report doesn't send it back.
    call(
        app,
        "POST",
        &format!("/packs/{pack}/report"),
        Some(&reporters[0]),
        None,
    )
    .await;
    let (_, list) = call(app, "GET", "/packs", Some(&bob), None).await;
    assert_eq!(ids(&list), [pack]);

    // Creator page: public packs only; mine even when empty.
    let (s, c) = call(
        app,
        "GET",
        &format!("/creators/{ana_user}"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{c}");
    assert_eq!(
        (c["name"].clone(), c["id"].clone()),
        (json!("Ana Martin"), json!(ana_user))
    );
    assert_eq!(ids(&c["packs"]), [pack]);
    assert_eq!(
        (c["installs"].clone(), c["likes"].clone()),
        (json!(0), json!(0))
    );
    assert!(c.get("color").is_some() && c.get("photo_url").is_some() && c.get("avatar").is_some());
    let (s, c) = call(
        app,
        "GET",
        &format!("/creators/{bob_user}"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!((s, c["packs"].clone()), (StatusCode::OK, json!([])));
    let (s, _) = call(
        app,
        "GET",
        &format!("/creators/{bob_user}"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // Removed: its author sees why; nobody else sees it.
    let (s, b) = call(
        app,
        "POST",
        &format!("/admin/packs/{pack}/remove"),
        Some(&admin),
        Some(json!({ "reason": "Hors charte" })),
    )
    .await;
    assert_eq!((s, b["status"].clone()), (StatusCode::OK, json!("removed")));
    let (_, mine) = call(app, "GET", "/packs/mine", Some(&ana), None).await;
    assert_eq!(
        (mine[0]["status"].clone(), mine[0]["removed_reason"].clone()),
        (json!("removed"), json!("Hors charte"))
    );
    let (_, list) = call(app, "GET", "/packs", Some(&bob), None).await;
    assert_eq!(list, json!([]));
    let (s, _) = call(app, "GET", &format!("/packs/{pack}"), Some(&bob), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = call(
        app,
        "GET",
        &format!("/creators/{ana_user}"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn suggestions_are_voted_then_approved_into_the_deck() {
    let t = setup().await;
    let app = &t.app;
    let (admin, _) = join(app, "Admin").await;
    let (ana, ana_member) = join(app, "Ana").await;
    let (bob, _) = join(app, "Bob").await;
    let ana_user = user_of(&t, ana_member).await;

    let (s, b) = call(
        app,
        "POST",
        "/suggestions",
        Some(&ana),
        Some(json!({ "text": "Qui ?" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_question"))
    );
    let (s, b) = call(
        app,
        "POST",
        "/suggestions",
        Some(&ana),
        Some(json!({ "text": "Qui parle allemand ici ?", "lang": "de" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_language"))
    );
    let mut made = Vec::new();
    for i in 0..5 {
        let (s, b) = call(
            app,
            "POST",
            "/suggestions",
            Some(&ana),
            Some(json!({ "text": format!("Qui a proposé la question {i} ?"), "kind": if i == 4 { "rank" } else { "pick" } })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
        assert_eq!(
            (b["status"].clone(), b["lang"].clone(), b["mine"].clone()),
            (json!("voting"), json!("fr"), json!(true))
        );
        made.push(b["id"].as_i64().unwrap());
    }
    let (s, b) = call(
        app,
        "POST",
        "/suggestions",
        Some(&ana),
        Some(json!({ "text": "Une sixième question de trop ?" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::TOO_MANY_REQUESTS, Some("too_many_suggestions"))
    );
    let (_, mine) = call(app, "GET", "/suggestions/mine", Some(&ana), None).await;
    assert_eq!(mine["left"], 0);
    let mine_ids: Vec<i64> = mine["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_i64().unwrap())
        .collect();
    assert_eq!(mine_ids, made.iter().rev().copied().collect::<Vec<_>>());
    let (_, me) = call(app, "GET", "/community/me", Some(&ana), None).await;
    assert_eq!(me["suggestions_left"], 0);
    // Duplicates: of a suggestion (any case) or of a deck question.
    let (s, b) = call(
        app,
        "POST",
        "/suggestions",
        Some(&bob),
        Some(json!({ "text": "QUI A PROPOSÉ LA QUESTION 0 ?" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("duplicate"))
    );
    let deck: String = sqlx::query_scalar(
        "SELECT text FROM questions WHERE group_id IS NULL AND lang = 'fr' LIMIT 1",
    )
    .fetch_one(&t.state.db)
    .await
    .unwrap();
    let (s, b) = call(
        app,
        "POST",
        "/suggestions",
        Some(&bob),
        Some(json!({ "text": deck })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("duplicate"))
    );

    // Voting: not mine, skip hides it, votes can change (up/down stay in sync).
    let target = made[0];
    let (_, next) = call(app, "GET", "/suggestions/next", Some(&ana), None).await;
    assert!(!ids(&next).contains(&target));
    let (_, next) = call(app, "GET", "/suggestions/next?lang=fr", Some(&bob), None).await;
    assert_eq!(ids(&next), made, "oldest first");
    assert_eq!(next[0]["author"], "Ana");
    let (_, next) = call(app, "GET", "/suggestions/next?lang=en", Some(&bob), None).await;
    assert_eq!(next, json!([]));
    let (s, b) = call(
        app,
        "POST",
        &format!("/suggestions/{target}/vote"),
        Some(&ana),
        Some(json!({ "value": 1 })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("own_suggestion"))
    );
    let (_, b) = call(
        app,
        "POST",
        &format!("/suggestions/{target}/vote"),
        Some(&bob),
        Some(json!({ "value": 0 })),
    )
    .await;
    assert_eq!(b, json!({ "up": 0, "down": 0, "status": "voting" }));
    let (_, next) = call(app, "GET", "/suggestions/next", Some(&bob), None).await;
    assert!(!ids(&next).contains(&target));
    let (_, b) = call(
        app,
        "POST",
        &format!("/suggestions/{target}/vote"),
        Some(&bob),
        Some(json!({ "value": 1 })),
    )
    .await;
    assert_eq!(b, json!({ "up": 1, "down": 0, "status": "voting" }));
    let (_, b) = call(
        app,
        "POST",
        &format!("/suggestions/{target}/vote"),
        Some(&bob),
        Some(json!({ "value": -1 })),
    )
    .await;
    assert_eq!(b, json!({ "up": 0, "down": 1, "status": "voting" }));
    // 10 up, 1 down: score 9, still voting; the 11th up makes it 10 at 92 %: queued.
    for i in 0..11 {
        let (voter, _) = join(app, &format!("Voter{i}")).await;
        let (s, b) = call(
            app,
            "POST",
            &format!("/suggestions/{target}/vote"),
            Some(&voter),
            Some(json!({ "value": 1 })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
        let expected = if i < 10 { "voting" } else { "queued" };
        assert_eq!(b["status"], expected, "after {} up", i + 1);
    }
    let (s, b) = call(
        app,
        "POST",
        &format!("/suggestions/{target}/vote"),
        Some(&bob),
        Some(json!({ "value": 1 })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("closed"))
    );

    // Admins approve into the deck, or reject.
    let (_, review) = call(app, "GET", "/admin/review", Some(&admin), None).await;
    assert_eq!(ids(&review["suggestions"]), [target]);
    assert_eq!(review["suggestions"][0]["author"], "Ana");
    let (s, _) = call(
        app,
        "POST",
        &format!("/admin/suggestions/{target}/approve"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, b) = call(
        app,
        "POST",
        &format!("/admin/suggestions/{target}/approve"),
        Some(&admin),
        Some(json!({ "tags": ["nope"] })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_tags"))
    );
    let (s, b) = call(
        app,
        "POST",
        &format!("/admin/suggestions/{target}/approve"),
        Some(&admin),
        Some(json!({ "tags": ["potes"] })),
    )
    .await;
    assert_eq!(
        (s, b["status"].clone()),
        (StatusCode::OK, json!("approved"))
    );
    let (question, by, group, vibes): (i64, Option<i64>, Option<i64>, Option<String>) = sqlx::query_as(
        "SELECT q.id, q.suggested_by, q.group_id, q.vibes FROM suggestions s JOIN questions q ON q.id = s.question_id WHERE s.id = ?",
    )
    .bind(target)
    .fetch_one(&t.state.db)
    .await
    .unwrap();
    assert_eq!(
        (by, group, vibes.as_deref()),
        (Some(ana_user), None, Some("potes"))
    );
    let (s, b) = call(
        app,
        "POST",
        &format!("/admin/suggestions/{}/reject", made[1]),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(
        (s, b["status"].clone()),
        (StatusCode::OK, json!("rejected"))
    );
    let (s, _) = call(
        app,
        "POST",
        &format!("/admin/suggestions/{}/reject", made[1]),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);

    // The startup clean-up never moves it out of the shared deck.
    let first_group: i64 = sqlx::query_scalar("SELECT MIN(id) FROM groups")
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    quidenous_server::deck::adopt_imports(&t.state.db, first_group)
        .await
        .unwrap();
    let group: Option<i64> = sqlx::query_scalar("SELECT group_id FROM questions WHERE id = ?")
        .bind(question)
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    assert_eq!(group, None);

    // Drawn in a room: the round says who proposed it. Leave it the only fresh deck question.
    sqlx::query("UPDATE questions SET lang = 'xx' WHERE group_id IS NULL AND id != ?")
        .bind(question)
        .execute(&t.state.db)
        .await
        .unwrap();
    let room = room_with(app, &bob, "Tirage", &[&ana], None).await;
    let (_, round) = call_room(app, "GET", "/round/current", &bob, room, None).await;
    assert_eq!(round["question"], "Qui a proposé la question 0 ?");
    assert_eq!(round["suggested_by"], "Ana");
    let (_, round) = call(app, "GET", "/round/current", Some(&admin), None).await;
    assert!(round["suggested_by"].is_null(), "{round}");
}

async fn vote_in(app: &Router, token: &str, room: i64, target: i64) {
    let (s, b) = call_room(
        app,
        "POST",
        "/round/current/vote",
        token,
        room,
        Some(json!({ "target_id": target })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
}

/// My member id in a room.
async fn seat(app: &Router, token: &str, room: i64) -> i64 {
    let (_, me) = call_room(app, "GET", "/me", token, room, None).await;
    me["id"].as_i64().unwrap()
}

#[tokio::test]
async fn shared_questions_play_in_other_rooms_and_challenge_them() {
    let t = setup().await;
    let app = &t.app;
    let (ana, ana_id) = join(app, "Ana Martin").await;
    let (bob, bob_id) = join(app, "Bob").await;
    let (_, g) = call(app, "GET", "/group", Some(&ana), None).await;
    let test_room = g["id"].as_i64().unwrap();
    let (cyd, _) = join(app, "Cyd").await;
    let cousins = room_with(app, &ana, "Cousins", &[&cyd], None).await;
    let ana_c = seat(app, &ana, cousins).await;
    let cyd_c = seat(app, &cyd, cousins).await;
    // Cyd is also in Test: three there. Close Test's question with everyone.
    let (_, r0) = call_room(app, "GET", "/round/current", &ana, test_room, None).await;
    let r0_id = r0["id"].as_i64().unwrap();
    let r0_text = r0["question"].as_str().unwrap().to_owned();
    let (_, share) = call_room(
        app,
        "POST",
        &format!("/rounds/{r0_id}/share"),
        &ana,
        test_room,
        Some(json!({})),
    )
    .await;
    let token = share["token"].as_str().unwrap().to_owned();

    // The share page leads to playing it.
    let (s, html) = page(app, &format!("/s/{token}")).await;
    assert_eq!(s, StatusCode::OK);
    assert!(html.contains(&format!("/play?share={token}")), "{html}");
    assert!(
        html.contains("Jouer cette question avec tes potes")
            && html.contains("Ouvrir qui de nous ?")
    );

    // Public view of the shared question.
    let (s, p) = call(app, "GET", &format!("/play/{token}"), None, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        p,
        json!({ "question": r0_text, "kind": "pick", "lang": "fr", "room": "Test", "closed": false })
    );
    let (s, _) = call(app, "GET", "/play/nope", None, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // Played while open: front of Cousins' queue, no challenge.
    let (_, written) = call_room(
        app,
        "POST",
        "/questions",
        &ana,
        cousins,
        Some(json!({ "text": "Qui a écrit la première question ?" })),
    )
    .await;
    let (s, played) = call_room(app, "POST", &format!("/play/{token}"), &ana, cousins, None).await;
    assert_eq!(s, StatusCode::OK, "{played}");
    assert!(played["challenge_id"].is_null());
    let qid = played["question_id"].as_i64().unwrap();
    let (_, queue) = call_room(app, "GET", "/questions/queue", &ana, cousins, None).await;
    let queued: Vec<i64> = queue["custom"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| q["id"].as_i64().unwrap())
        .collect();
    assert_eq!(queued, [qid, written["id"].as_i64().unwrap()]);

    // Test closes it: playing it again makes a challenge (same queued copy).
    vote_in(app, &ana, test_room, bob_id).await;
    vote_in(app, &bob, test_room, bob_id).await;
    vote_in(app, &cyd, test_room, ana_id).await;
    let (_, p) = call(app, "GET", &format!("/play/{token}"), None, None).await;
    assert_eq!(p["closed"], true);
    let (s, again) = call_room(app, "POST", &format!("/play/{token}"), &cyd, cousins, None).await;
    assert_eq!(s, StatusCode::OK, "{again}");
    assert_eq!(again["question_id"], qid);
    let challenge = again["challenge_id"].as_i64().unwrap();
    // Not in its own room: it was asked there.
    let (s, b) = call_room(
        app,
        "POST",
        &format!("/play/{token}"),
        &ana,
        test_room,
        None,
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("already_asked"))
    );

    // Cousins closes its current question; the challenge opens next.
    vote_in(app, &ana, cousins, cyd_c).await;
    vote_in(app, &cyd, cousins, cyd_c).await;
    let (_, cr) = call_room(app, "GET", "/round/current", &ana, cousins, None).await;
    assert_eq!(cr["question"], r0_text);
    let cr_id = cr["id"].as_i64().unwrap();
    let (s, list) = call_room(
        app,
        "GET",
        &format!("/rounds/{r0_id}/challenges"),
        &ana,
        test_room,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{list}");
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(
        (
            list[0]["room"].clone(),
            list[0]["round_id"].clone(),
            list[0]["closed"].clone()
        ),
        (json!("Cousins"), json!(cr_id), json!(false))
    );
    assert!(list[0]["podium"].is_null());
    // Both closed: both podiums.
    vote_in(app, &ana, cousins, ana_c).await;
    vote_in(app, &cyd, cousins, ana_c).await;
    let (_, list) = call_room(
        app,
        "GET",
        &format!("/rounds/{r0_id}/challenges"),
        &ana,
        test_room,
        None,
    )
    .await;
    assert_eq!(list[0]["closed"], true);
    assert_eq!(list[0]["podium"][0]["name"], "Ana", "first names only");
    assert_eq!(list[0]["podium"][0]["score"], 2);
    assert_eq!(list[0]["mine_podium"][0]["name"], "Bob");
    assert_eq!(list[0]["mine_podium"][0]["score"], 2);
    assert!(list[0]["mine_podium"][0].get("photo_url").is_some());
    let (_, back) = call_room(
        app,
        "GET",
        &format!("/rounds/{cr_id}/challenges"),
        &cyd,
        cousins,
        None,
    )
    .await;
    assert_eq!(
        (back[0]["room"].clone(), back[0]["round_id"].clone()),
        (json!("Test"), json!(r0_id))
    );
    assert_eq!(back[0]["podium"][0]["name"], "Bob");
    // Played now: already asked, the challenge still there.
    let (s, b) = call_room(app, "POST", &format!("/play/{token}"), &ana, cousins, None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("already_asked"))
    );
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM challenges WHERE id = ?")
        .bind(challenge)
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    assert_eq!(n, 1);

    // Challenge a closed round straight into another of my rooms.
    let (s, b) = call_room(
        app,
        "POST",
        &format!("/rounds/{r0_id}/challenge"),
        &ana,
        test_room,
        Some(json!({ "room_id": test_room })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("same_room"))
    );
    let (s, b) = call_room(
        app,
        "POST",
        &format!("/rounds/{r0_id}/challenge"),
        &bob,
        test_room,
        Some(json!({ "room_id": cousins })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("not_in_room"))
    );
    let (_, r1) = call_room(app, "GET", "/round/current", &ana, test_room, None).await;
    let r1_id = r1["id"].as_i64().unwrap();
    let (s, b) = call_room(
        app,
        "POST",
        &format!("/rounds/{r1_id}/challenge"),
        &ana,
        test_room,
        Some(json!({ "room_id": cousins })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("round_open"))
    );
    vote_in(app, &ana, test_room, bob_id).await;
    vote_in(app, &bob, test_room, bob_id).await;
    vote_in(app, &cyd, test_room, bob_id).await;
    let (s, b) = call_room(
        app,
        "POST",
        &format!("/rounds/{r1_id}/challenge"),
        &ana,
        test_room,
        Some(json!({ "room_id": cousins })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert!(b["challenge_id"].is_i64());
    let (_, queue) = call_room(app, "GET", "/questions/queue", &cyd, cousins, None).await;
    assert_eq!(queue["custom"][0]["text"], r1["question"]);
    let (_, list) = call_room(
        app,
        "GET",
        &format!("/rounds/{r1_id}/challenges"),
        &ana,
        test_room,
        None,
    )
    .await;
    assert_eq!(
        (list[0]["room"].clone(), list[0]["round_id"].clone()),
        (json!("Cousins"), Value::Null)
    );
}

#[tokio::test]
async fn public_pack_pages() {
    let t = setup().await;
    let app = &t.app;
    let (ana, _) = join(app, "Ana").await;
    let fr = publish(
        app,
        &ana,
        json!({ "name": "Soirée <chill>", "description": "Tout doux", "questions": questions("s", 6) }),
    )
    .await;
    let en = publish(
        app,
        &ana,
        json!({ "name": "Chill", "language": "en", "questions": questions("Who", 5) }),
    )
    .await;
    let (s, html) = page(app, &format!("/p/{fr}")).await;
    assert_eq!(s, StatusCode::OK);
    assert!(html.contains("<html lang=\"fr\""));
    assert!(html.contains("og:image") && html.contains(&format!("/p/{fr}/card.png")));
    assert!(
        html.contains(&format!("href=\"/pack/{fr}\"")) && html.contains("Installer dans un salon")
    );
    assert!(html.contains("Soirée &lt;chill&gt;") && html.contains("6 questions par Ana"));
    let (_, html) = page(app, &format!("/p/{en}")).await;
    assert!(html.contains("<html lang=\"en\"") && html.contains("Install in a room"));
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/p/{fr}/card.png"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["content-type"], "image/png");
    let png = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&png[1..4], b"PNG");
    // Not public: 404.
    call(app, "DELETE", &format!("/packs/{fr}"), Some(&ana), None).await;
    let (s, _) = page(app, &format!("/p/{fr}")).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = page(app, &format!("/p/{fr}/card.png")).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = page(app, "/p/999").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}
