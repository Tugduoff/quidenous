use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::common::{call, join, setup};

async fn status_and_error(
    app: &axum::Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Option<String>) {
    let (s, b) = call(app, method, path, token, None, body).await;
    (s, b["error"].as_str().map(str::to_owned))
}

fn room_routes(
    room: i64,
    member: i64,
    user: i64,
) -> Vec<(&'static str, String, Option<Value>, bool)> {
    vec![
        ("POST", format!("/rooms/{room}/leave"), None, false),
        ("GET", format!("/rooms/{room}/invites"), None, false),
        (
            "DELETE",
            format!("/rooms/{room}/invites/{user}"),
            None,
            false,
        ),
        ("PUT", format!("/rooms/{room}/mute"), None, false),
        ("DELETE", format!("/rooms/{room}/mute"), None, false),
        (
            "POST",
            format!("/rooms/{room}/owner"),
            Some(json!({ "member_id": member })),
            false,
        ),
        (
            "POST",
            format!("/rooms/{room}/add"),
            Some(json!({ "user_id": user })),
            false,
        ),
        ("PATCH", format!("/rooms/{room}"), Some(json!({})), true),
        ("DELETE", format!("/rooms/{room}"), None, true),
        ("POST", format!("/rooms/{room}/code"), None, true),
        (
            "DELETE",
            format!("/rooms/{room}/members/{member}"),
            None,
            true,
        ),
        ("POST", format!("/rooms/{room}/next"), None, true),
    ]
}

#[tokio::test]
async fn routes_with_the_room_in_the_path_check_the_seat_then_the_role() {
    let (app, _) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    let (cyd, _) = join(&app, "Cyd").await;
    let (s, created) = call(
        &app,
        "POST",
        "/rooms",
        Some(&ana),
        None,
        Some(json!({ "name": "Les cousins" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{created}");
    let room = created["id"].as_i64().unwrap();
    let (_, group) = call(&app, "GET", "/group", Some(&ana), Some(room), None).await;
    let ana_member = group["members"][0]["id"].as_i64().unwrap();
    let ana_user = group["members"][0]["user_id"].as_i64().unwrap();
    let code = group["code"].as_str().unwrap().to_owned();

    for (method, path, body, _) in room_routes(room, ana_member, ana_user) {
        assert_eq!(
            status_and_error(&app, method, &path, None, body.clone()).await,
            (StatusCode::UNAUTHORIZED, Some("unauthorized".into())),
            "{method} {path}"
        );
        assert_eq!(
            status_and_error(&app, method, &path, Some(&cyd), body).await,
            (StatusCode::FORBIDDEN, Some("not_in_room".into())),
            "{method} {path}"
        );
    }

    let (s, _) = call(
        &app,
        "POST",
        "/rooms/join",
        Some(&bob),
        None,
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    for (method, path, body, owner_only) in room_routes(room, ana_member, ana_user) {
        if owner_only {
            assert_eq!(
                status_and_error(&app, method, &path, Some(&bob), body).await,
                (StatusCode::FORBIDDEN, Some("owner_only".into())),
                "{method} {path}"
            );
        }
    }

    let (s, _) = call(&app, "GET", "/rooms/abc/invites", Some(&cyd), None, None).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/rooms/{room}/members/abc"),
        Some(&cyd),
        None,
        None,
    )
    .await;
    assert_eq!(
        s,
        StatusCode::BAD_REQUEST,
        "the whole path is read before the seat"
    );
    assert_eq!(
        status_and_error(&app, "GET", &format!("/parties/{room}"), Some(&cyd), None).await,
        (StatusCode::FORBIDDEN, Some("not_in_room".into()))
    );
}

#[tokio::test]
async fn admin_routes_need_an_admin_account() {
    let (app, _) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    for (method, path, body) in [
        ("GET", "/admin/review", None),
        ("POST", "/admin/packs/1/approve", None),
        ("POST", "/admin/packs/1/remove", None),
        (
            "POST",
            "/admin/packs/1/feature",
            Some(json!({ "featured": true })),
        ),
        ("POST", "/admin/suggestions/1/approve", None),
        ("POST", "/admin/suggestions/1/reject", None),
        (
            "POST",
            "/admin/reports",
            Some(json!({ "kind": "message", "id": 1, "action": "dismiss" })),
        ),
    ] {
        assert_eq!(
            status_and_error(&app, method, path, None, body.clone()).await,
            (StatusCode::UNAUTHORIZED, Some("unauthorized".into())),
            "{method} {path}"
        );
        assert_eq!(
            status_and_error(&app, method, path, Some(&ana), body).await,
            (StatusCode::FORBIDDEN, Some("not_admin".into())),
            "{method} {path}"
        );
    }
}
