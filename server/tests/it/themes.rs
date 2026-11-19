use crate::common;

use crate::common::{call, join, room_id, setup};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use quidenous_server::{
    AppState, app,
    platform::{config::Config, seasons::Seasons},
    rooms::themes::THEMES,
    state,
};
use serde_json::{Value, json};
use tower::ServiceExt;

struct Room {
    app: Router,
    state: AppState,
    id: i64,
    owner: String,
    member: String,
}

impl Room {
    async fn new() -> Self {
        let (app, state) = setup().await;
        let (owner, _) = join(&app, "Ana").await;
        let (member, _) = join(&app, "Bob").await;
        let id = room_id(&app, &owner).await;
        Self {
            app,
            state,
            id,
            owner,
            member,
        }
    }

    async fn settings(&self, token: &str, body: Value) -> (StatusCode, Value) {
        call(
            &self.app,
            "PATCH",
            &format!("/rooms/{}", self.id),
            Some(token),
            None,
            Some(body),
        )
        .await
    }

    async fn get(&self, token: &str, path: &str) -> Value {
        let (s, b) = call(&self.app, "GET", path, Some(token), None, None).await;
        assert_eq!(s, StatusCode::OK, "{path}: {b}");
        b
    }

    async fn theme_everyone_sees(&self) -> Value {
        let mut seen = Vec::new();
        for token in [&self.owner, &self.member] {
            let group = self.get(token, "/group").await;
            seen.push(group["settings"]["theme"].clone());
            let rooms = self.get(token, "/rooms").await;
            let room = rooms
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == self.id)
                .unwrap();
            assert!(room.get("theme").is_some(), "{room}");
            seen.push(room["theme"].clone());
        }
        assert!(seen.iter().all(|t| *t == seen[0]), "{seen:?}");
        seen.swap_remove(0)
    }

    async fn stored(&self) -> (Option<String>, String) {
        sqlx::query_as("SELECT theme, name FROM groups WHERE id = ?")
            .bind(self.id)
            .fetch_one(&self.state.db)
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn the_owner_picks_a_theme_for_everyone() {
    let room = Room::new().await;
    assert_eq!(room.theme_everyone_sees().await, Value::Null);

    for id in ["lime", "violet", "piscine", "karaoke", "arcade"] {
        let (s, b) = room.settings(&room.owner, json!({ "theme": id })).await;
        assert_eq!(s, StatusCode::OK, "{id}: {b}");
        assert_eq!(room.theme_everyone_sees().await, json!(id));
    }
    for id in THEMES {
        let (s, _) = room.settings(&room.owner, json!({ "theme": id })).await;
        assert_eq!(s, StatusCode::OK, "{id}");
    }

    let (s, _) = room
        .settings(&room.owner, json!({ "skip_vote": false }))
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(room.theme_everyone_sees().await, json!("piscine"));

    let (s, _) = room.settings(&room.owner, json!({ "theme": "" })).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(room.theme_everyone_sees().await, Value::Null);
    assert_eq!(room.stored().await.0, None);
}

#[tokio::test]
async fn unknown_themes_change_nothing() {
    let room = Room::new().await;
    room.settings(&room.owner, json!({ "theme": "pyjama" }))
        .await;
    let before = room.stored().await;
    for bad in [json!("noir"), json!("Lime"), json!("winter"), json!(3)] {
        let (s, b) = room.settings(&room.owner, json!({ "theme": bad })).await;
        assert_eq!(s.as_u16() / 100, 4, "{bad}: {b}");
        if bad.is_string() {
            assert_eq!(
                (s, b["error"].as_str()),
                (StatusCode::BAD_REQUEST, Some("bad_setting"))
            );
        }
    }
    let (s, b) = room
        .settings(&room.owner, json!({ "name": "Renamed", "theme": "noir" }))
        .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_setting"))
    );
    assert_eq!(room.stored().await, before);
    assert_eq!(room.theme_everyone_sees().await, json!("pyjama"));
}

#[tokio::test]
async fn only_the_owner_sets_it() {
    let room = Room::new().await;
    let (s, b) = room
        .settings(&room.member, json!({ "theme": "rose" }))
        .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("owner_only"))
    );
    assert_eq!(room.stored().await.0, None);
}

#[tokio::test]
async fn parties_have_no_theme() {
    let room = Room::new().await;
    sqlx::query("UPDATE groups SET kind = 'party' WHERE id = ?")
        .bind(room.id)
        .execute(&room.state.db)
        .await
        .unwrap();
    for theme in ["feu", ""] {
        let (s, b) = room.settings(&room.owner, json!({ "theme": theme })).await;
        assert_eq!(
            (s, b["error"].as_str()),
            (StatusCode::BAD_REQUEST, Some("bad_setting"))
        );
    }
    assert_eq!(room.stored().await.0, None);
    let group = room.get(&room.owner, "/group").await;
    assert!(group["settings"].get("theme").is_none(), "{group}");
}

#[tokio::test]
async fn a_change_reaches_the_room_live() {
    let room = Room::new().await;
    let mut rx = room.state.live.subscribe();
    let (s, _) = room
        .settings(&room.owner, json!({ "theme": "apero" }))
        .await;
    assert_eq!(s, StatusCode::OK);
    let mut told = Vec::new();
    while let Ok(e) = rx.try_recv() {
        told.push((e.group_id.raw(), serde_json::to_value(&e.event).unwrap()));
    }
    assert!(
        told.contains(&(room.id, json!({ "type": "room_updated" }))),
        "{told:?}"
    );
}

#[tokio::test]
async fn features_list_both_flags() {
    let (app, _) = setup().await;
    let (s, f) = call(&app, "GET", "/features", None, None, None).await;
    assert_eq!(s, StatusCode::OK);
    let f: Vec<&str> = f
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(f.contains(&"room_themes"), "{f:?}");
    assert!(f.contains(&"seasons"), "{f:?}");
}

#[tokio::test]
async fn season_is_public_and_cached() {
    let (app, _) = setup().await;
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/season")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::CACHE_CONTROL], "public, max-age=3600");
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        body["skins"],
        json!([{ "id": "winter", "from": "12-01", "until": "01-06" }])
    );
    let active = &body["active"];
    assert!(active.is_null() || *active == json!("winter"), "{body}");
    assert_eq!(body.as_object().unwrap().len(), 2, "{body}");
}

#[tokio::test]
async fn seasons_can_be_switched_off() {
    let config = Config {
        seasons: Seasons::parse("off"),
        ..common::config(&common::temp_dir("qdn-themes"))
    };
    let app = app(state(config).await.unwrap());
    let (s, body) = call(&app, "GET", "/season", None, None, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body, json!({ "active": null, "skins": [] }));
}

#[test]
fn bad_season_entries_are_warned_about() {
    let config = Config {
        seasons: Seasons::parse("winter:12-01..01-06,summer:07-01"),
        ..Config::default()
    };
    assert!(
        config
            .warnings()
            .iter()
            .any(|w| w.contains("SEASONS") && w.contains("summer:07-01")),
        "{:?}",
        config.warnings()
    );
    assert!(
        !Config::default()
            .warnings()
            .iter()
            .any(|w| w.contains("SEASONS"))
    );
}
