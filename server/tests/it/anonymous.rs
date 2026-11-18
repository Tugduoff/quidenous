//! Anonymous votes (docs/ANONYMOUS.md): counts only, never who voted for whom, for anyone (the
//! owner included), wherever a result goes.

use crate::common;

use crate::common::{call, join, room_id, setup};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use quidenous_server::AppState;
use serde_json::{Value, json};
use tower::ServiceExt;

struct Room {
    app: Router,
    state: AppState,
    id: i64,
    /// Ana is the owner (first in).
    ana: (String, i64),
    bob: (String, i64),
    cyd: (String, i64),
}

impl Room {
    async fn new() -> Self {
        let (app, state) = setup().await;
        let ana = join(&app, "Ana").await;
        let bob = join(&app, "Bob").await;
        let cyd = join(&app, "Cyd").await;
        let id = room_id(&app, &ana.0).await;
        Self {
            app,
            state,
            id,
            ana,
            bob,
            cyd,
        }
    }

    async fn get(&self, token: &str, path: &str) -> Value {
        let (s, b) = call(&self.app, "GET", path, Some(token), None, None).await;
        assert_eq!(s, StatusCode::OK, "{path}: {b}");
        b
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

    async fn add(&self, body: Value) -> Value {
        let (s, b) = call(
            &self.app,
            "POST",
            "/questions",
            Some(&self.ana.0),
            None,
            Some(body),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
        b
    }

    async fn current(&self, token: &str) -> Value {
        self.get(token, "/round/current").await
    }

    /// Everyone votes on the open question (a pick: `targets[i]` for the i-th of Ana, Bob, Cyd;
    /// a ranking: Ana, Bob, Cyd in that order rotated by i). Returns its id.
    async fn close(&self, targets: [i64; 3]) -> i64 {
        let round = self.current(&self.ana.0).await;
        let id = round["id"].as_i64().unwrap();
        let rank = round["kind"] == "rank";
        let ids = [self.ana.1, self.bob.1, self.cyd.1];
        for (i, (token, target)) in [&self.ana.0, &self.bob.0, &self.cyd.0]
            .into_iter()
            .zip(targets)
            .enumerate()
        {
            let body = if rank {
                let mut order = ids.to_vec();
                order.rotate_left(i);
                json!({ "ranking": order })
            } else {
                json!({ "target_id": target })
            };
            let (s, b) = call(
                &self.app,
                "POST",
                "/round/current/vote",
                Some(token),
                None,
                Some(body),
            )
            .await;
            assert_eq!(s, StatusCode::OK, "{b}");
        }
        let closed = self.get(&self.ana.0, &format!("/rounds/{id}")).await;
        assert!(closed["closed_at"].is_string(), "{closed}");
        id
    }

    async fn export(&self, token: &str) -> String {
        let req = Request::builder()
            .uri(format!("/api/history/export?token={token}"))
            .body(Body::empty())
            .unwrap();
        let res = self.app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap()
    }

    async fn stored(&self, round: i64) -> bool {
        sqlx::query_scalar("SELECT anonymous FROM rounds WHERE id = ?")
            .bind(round)
            .fetch_one(&self.state.db)
            .await
            .unwrap()
    }
}

/// Every `voter_ids` list in a result is empty, and no ranking says who ranked whom where.
fn no_voters(view: &Value) {
    assert_eq!(view["anonymous"], true, "{view}");
    for t in view["result"].as_array().unwrap() {
        assert_eq!(t["voter_ids"], json!([]), "{view}");
        assert!(t["count"].as_i64().unwrap() > 0);
    }
    if let Some(ranking) = view["ranking"].as_array() {
        for r in ranking {
            let mut keys: Vec<&str> = r.as_object().unwrap().keys().map(|k| k.as_str()).collect();
            keys.sort_unstable();
            assert_eq!(keys, ["average", "firsts", "member_id", "points"], "{view}");
        }
    }
}

#[tokio::test]
async fn the_room_rule_is_owner_only_and_checked() {
    let room = Room::new().await;
    let group = room.get(&room.ana.0, "/group").await;
    assert_eq!(group["settings"]["anonymous_votes"], "per_question");
    let features = room.get(&room.ana.0, "/features").await;
    assert!(
        features
            .as_array()
            .unwrap()
            .contains(&json!("anonymous_votes"))
    );

    let (s, b) = room
        .settings(&room.ana.0, json!({ "anonymous_votes": "sometimes" }))
        .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_setting"))
    );
    let (s, _) = room
        .settings(&room.bob.0, json!({ "anonymous_votes": "always" }))
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    for mode in ["never", "always", "per_question"] {
        let (s, b) = room
            .settings(&room.ana.0, json!({ "anonymous_votes": mode }))
            .await;
        assert_eq!(s, StatusCode::OK, "{b}");
        let group = room.get(&room.bob.0, "/group").await;
        assert_eq!(group["settings"]["anonymous_votes"], mode);
    }
}

#[tokio::test]
async fn a_round_keeps_what_was_decided_when_it_opened() {
    let room = Room::new().await;
    let (a, b, c) = (room.ana.1, room.bob.1, room.cyd.1);
    // The deck question open since Bob came in: not anonymous.
    assert_eq!(room.current(&room.ana.0).await["anonymous"], false);

    // Per question (the default): the question says.
    let q = room
        .add(json!({ "text": "Qui chante le plus faux ?", "anonymous": true }))
        .await;
    assert_eq!(q["anonymous"], true);
    // Choose questions are never anonymous: the flag is dropped.
    let q = room
        .add(json!({ "text": "Qui prendrait quoi ?", "kind": "choose", "items": ["Pizza", "Sushi"], "anonymous": true }))
        .await;
    assert_eq!(q["anonymous"], false);
    // An app that doesn't know the flag: not anonymous.
    room.add(json!({ "text": "Qui ronfle le plus fort ?" }))
        .await;
    room.add(json!({ "text": "Qui rit le plus fort ?", "anonymous": true }))
        .await;
    let queue = room.get(&room.bob.0, "/questions/queue").await;
    let flags: Vec<&Value> = queue["custom"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| &q["anonymous"])
        .collect();
    assert_eq!(
        flags,
        [&json!(true), &json!(false), &json!(false), &json!(true)]
    );

    room.close([b, c, a]).await;
    let open = room.current(&room.bob.0).await;
    assert_eq!(open["question"], "Qui chante le plus faux ?");
    assert_eq!(open["anonymous"], true);
    let first = room.close([b, b, a]).await;

    // "Never" afterwards: the past question stays anonymous.
    room.settings(&room.ana.0, json!({ "anonymous_votes": "never" }))
        .await;
    no_voters(&room.get(&room.ana.0, &format!("/rounds/{first}")).await);
    assert!(room.stored(first).await);

    // The choose question: never anonymous, whatever the rule.
    room.settings(&room.ana.0, json!({ "anonymous_votes": "always" }))
        .await;
    let choose = room.current(&room.ana.0).await;
    assert_eq!(
        (&choose["kind"], &choose["anonymous"]),
        (&json!("choose"), &json!(false))
    );
    sqlx::query("UPDATE rounds SET closed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?")
        .bind(choose["id"].as_i64().unwrap())
        .execute(&room.state.db)
        .await
        .unwrap();
    // Opened under "always": anonymous, though the question didn't ask.
    let mut tx = room.state.db.begin().await.unwrap();
    let opened = quidenous_server::rounds::game::ensure_open_round(&mut tx, room.id)
        .await
        .unwrap()
        .unwrap();
    tx.commit().await.unwrap();
    let open = room.current(&room.ana.0).await;
    assert_eq!(open["id"], opened);
    assert_eq!(open["question"], "Qui ronfle le plus fort ?");
    assert_eq!(open["anonymous"], true);
}

#[tokio::test]
async fn a_new_rule_can_only_make_the_open_question_anonymous() {
    let room = Room::new().await;
    let (a, b, c) = (room.ana.1, room.bob.1, room.cyd.1);
    room.add(json!({ "text": "Qui chante le plus faux ?" }))
        .await;
    room.add(json!({ "text": "Qui rit le plus fort ?", "anonymous": true }))
        .await;
    room.settings(&room.ana.0, json!({ "anonymous_votes": "never" }))
        .await;
    room.close([b, c, a]).await;
    // Bob voted already: he sees who voted, not for whom (nothing is shown before the close).
    let open = room.current(&room.bob.0).await;
    let open_id = open["id"].as_i64().unwrap();
    assert_eq!(open["anonymous"], false);

    // "Always": the open question turns anonymous (safe: nobody saw a result yet)...
    room.settings(&room.ana.0, json!({ "anonymous_votes": "always" }))
        .await;
    assert_eq!(room.current(&room.bob.0).await["anonymous"], true);
    // ... and never back.
    room.settings(&room.ana.0, json!({ "anonymous_votes": "never" }))
        .await;
    assert_eq!(room.current(&room.bob.0).await["anonymous"], true);
    let closed = room.close([a, a, b]).await;
    assert_eq!(closed, open_id);
    no_voters(&room.get(&room.cyd.0, &format!("/rounds/{closed}")).await);

    // Per question with an open question that asks: anonymous too. Under "never" it opened without.
    let open = room.current(&room.bob.0).await;
    assert_eq!(open["question"], "Qui rit le plus fort ?");
    assert_eq!(open["anonymous"], false);
    room.settings(&room.ana.0, json!({ "anonymous_votes": "per_question" }))
        .await;
    assert_eq!(room.current(&room.bob.0).await["anonymous"], true);
}

/// The whole tour: round views (owner and voters), history, CSV, stats, inbox, live events.
#[tokio::test]
async fn nobody_learns_who_voted_for_whom() {
    let room = Room::new().await;
    let (a, b, c) = (room.ana.1, room.bob.1, room.cyd.1);
    let mut live = room.state.live.subscribe();
    room.settings(&room.ana.0, json!({ "anonymous_votes": "always" }))
        .await;
    // The deck question, open since before: now anonymous too.
    room.add(json!({ "text": "Classe-nous du plus frileux au plus chaud", "kind": "rank" }))
        .await;
    room.add(json!({ "text": "Qui a le plus de chaussettes dépareillées ?" }))
        .await;
    // Ana votes for herself, Bob and Cyd for Ana: self votes and pairs must not show anywhere.
    let pick = room.close([a, a, a]).await;
    let rank = room.close([0, 0, 0]).await;
    let other = room.close([b, c, b]).await;

    for token in [&room.ana.0, &room.bob.0, &room.cyd.0] {
        for id in [pick, rank, other] {
            let view = room.get(token, &format!("/rounds/{id}")).await;
            no_voters(&view);
            // Who voted (the participation list) is fine.
            assert_eq!(view["voter_ids"].as_array().unwrap().len(), 3);
        }
    }
    // My own vote and ranking come back to me only.
    let view = room.get(&room.bob.0, &format!("/rounds/{pick}")).await;
    assert_eq!(view["my_vote"], a);
    assert_eq!(
        view["result"],
        json!([{ "target_id": a, "count": 3, "voter_ids": [] }])
    );
    let view = room.get(&room.bob.0, &format!("/rounds/{rank}")).await;
    assert_eq!(view["my_ranking"], json!([b, c, a]));
    let total: i64 = view["ranking"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["points"].as_i64().unwrap())
        .sum();
    assert_eq!(total, 3 * (3 + 2 + 1));

    let history = room.get(&room.cyd.0, "/history").await;
    let items = history.as_array().unwrap();
    assert!(items.len() >= 3);
    for item in items {
        assert_eq!(item["anonymous"], true, "{item}");
    }

    let csv = room.export(&room.ana.0).await;
    let line = csv.lines().find(|l| l.contains("chaussettes")).unwrap();
    assert!(
        line.ends_with(",\"Bob\",2,3,\"Bob (2) | Cyd (1)\""),
        "{csv}"
    );
    let line = csv.lines().find(|l| l.contains("frileux")).unwrap();
    assert!(line.contains("pts"), "{csv}");
    assert!(!csv.contains(" : "), "{csv}");

    let stats = room.get(&room.ana.0, "/stats?period=all").await;
    assert_eq!(stats["edges"], json!([]), "{stats}");
    assert!(stats["duo"].is_null() && stats["loyal"].is_null() && stats["twins"].is_null());
    assert!(stats["narcissist"].is_null() && stats["kingmaker"].is_null());
    assert!(stats["contrarian"].is_null() && stats["sheep"].is_null());
    for p in stats["profiles"].as_array().unwrap() {
        assert_eq!(p["top_voters"], json!([]), "{p}");
        assert_eq!(p["top_targets"], json!([]), "{p}");
        assert_eq!(p["self_votes"], 0, "{p}");
        // Votes cast and received are counts: they stay.
        assert_eq!(p["votes_cast"], 3, "{p}");
    }
    let received = |id: i64| {
        stats["profiles"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["member_id"] == id)
            .unwrap()["received"]
            .as_i64()
            .unwrap()
    };
    // 3 votes per question (a ranking counts its first places).
    assert_eq!(received(a) + received(b) + received(c), 9);
    let stories = stats["stories"].to_string();
    assert!(
        !stories.contains("\"duo\"") && !stories.contains("\"narcissist\""),
        "{stories}"
    );

    // The inbox says a result is out, never by whom.
    let inbox = room.get(&room.bob.0, "/inbox").await;
    let items = inbox["items"].as_array().unwrap();
    assert!(items.iter().any(|i| i["kind"] == "result"), "{inbox}");
    for i in items.iter().filter(|i| i["kind"] == "result") {
        assert!(i["actor_id"].is_null(), "{i}");
    }

    // Live events: who voted (to tick them off), never for whom.
    let mut seen = 0;
    while let Ok(e) = live.try_recv() {
        let event = serde_json::to_value(&e.event).unwrap();
        assert!(event.get("target_id").is_none(), "{event}");
        seen += 1;
    }
    assert!(seen > 0);
}

/// An anonymous question doesn't touch what others show: plain questions keep their voters,
/// and the stats still pair them.
#[tokio::test]
async fn plain_questions_are_unchanged() {
    let room = Room::new().await;
    let (a, b, c) = (room.ana.1, room.bob.1, room.cyd.1);
    room.add(json!({ "text": "Qui a le plus de chaussettes dépareillées ?", "anonymous": true }))
        .await;
    let plain = room.close([b, b, b]).await;
    let secret = room.close([c, c, a]).await;

    let view = room.get(&room.ana.0, &format!("/rounds/{plain}")).await;
    assert_eq!(view["anonymous"], false);
    assert_eq!(
        view["result"],
        json!([{ "target_id": b, "count": 3, "voter_ids": [a, b, c] }])
    );
    no_voters(&room.get(&room.ana.0, &format!("/rounds/{secret}")).await);

    let history = room.get(&room.ana.0, "/history").await;
    let flag = |id: i64| {
        history
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["id"] == id)
            .unwrap()["anonymous"]
            .clone()
    };
    assert_eq!((flag(plain), flag(secret)), (json!(false), json!(true)));

    let csv = room.export(&room.ana.0).await;
    assert!(csv.contains("\"Bob (3) : Ana, Bob, Cyd\""), "{csv}");
    assert!(csv.contains("\"Cyd (2) | Ana (1)\""), "{csv}");

    // Pairs come from the plain question only: Ana → Bob and Cyd → Bob, never → Cyd.
    let stats = room.get(&room.ana.0, "/stats?period=all").await;
    let edges: Vec<(i64, i64, i64)> = stats["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["voter_id"].as_i64().unwrap(),
                e["target_id"].as_i64().unwrap(),
                e["value"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(edges, [(a, b, 1), (b, b, 1), (c, b, 1)]);
    let bob = stats["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["member_id"] == b)
        .unwrap()
        .clone();
    assert_eq!(bob["self_votes"], 1);
    assert_eq!(bob["votes_cast"], 2);
    // Received: 3 (plain) + 0 (secret) for Bob; the secret one counts for Cyd and Ana.
    assert_eq!(bob["received"], 3);
}

#[tokio::test]
async fn import_and_pack_tags_carry_the_flag() {
    let room = Room::new().await;
    let file = "[anonyme] Qui a le plus de chaussettes dépareillées ?\n\
                [classement] [Anonymous] Classe-nous du plus frileux au plus chaud\n\
                [choix] [anonyme] Qui prendrait quoi au resto ? | Pizza | Sushi\n\
                Qui ronfle le plus fort ?\n\
                [PS] [anonyme] Qui garde ses tags ?\n";
    let (s, b) = call(
        &room.app,
        "POST",
        "/questions/import",
        Some(&room.ana.0),
        None,
        Some(json!({ "text": file })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let queue = room.get(&room.ana.0, "/questions/queue").await;
    let got: Vec<(String, String, bool)> = queue["custom"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| {
            (
                q["text"].as_str().unwrap().to_owned(),
                q["kind"].as_str().unwrap().to_owned(),
                q["anonymous"].as_bool().unwrap(),
            )
        })
        .collect();
    let want = |t: &str, k: &str, a: bool| (t.to_owned(), k.to_owned(), a);
    assert_eq!(
        got,
        [
            want("Qui a le plus de chaussettes dépareillées ?", "pick", true),
            want("Classe-nous du plus frileux au plus chaud", "rank", true),
            want("Qui prendrait quoi au resto ?", "choose", false),
            want("Qui ronfle le plus fort ?", "pick", false),
            // Only leading tags count: an unknown one first makes the rest part of the question.
            want("[PS] [anonyme] Qui garde ses tags ?", "pick", false),
        ]
    );

    // A pack: the tag in its file, kept when it's shown, and carried onto the room's questions.
    common::trust_all(&room.state).await;
    let pack = "# titre: Secrets\n\
                [anonyme] Qui pleure devant les pubs ?\n\
                [classement][anonyme] Classe-nous du plus au moins bavard\n\
                Qui perd toujours ses clés ?\n\
                Qui ferait le pire astronaute ?\n\
                Qui dort le plus en soirée ?\n";
    let (s, p) = call(
        &room.app,
        "POST",
        "/packs",
        Some(&room.bob.0),
        None,
        Some(json!({ "name": "Secrets", "description": "Chut", "text": pack })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{p}");
    let id = p["id"].as_i64().unwrap();
    let shown = room.get(&room.bob.0, &format!("/packs/{id}")).await;
    let flags: Vec<bool> = shown["questions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| q["anonymous"].as_bool().unwrap())
        .collect();
    assert_eq!(flags, [true, true, false, false, false]);

    // An app that doesn't know the flag edits the pack: what it had stays.
    let questions: Vec<Value> = shown["questions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| json!({ "text": q["text"], "kind": q["kind"] }))
        .collect();
    let (s, b) = call(
        &room.app,
        "PUT",
        &format!("/packs/{id}"),
        Some(&room.bob.0),
        None,
        Some(json!({ "name": "Secrets", "description": "Chut", "questions": questions })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let shown = room.get(&room.bob.0, &format!("/packs/{id}")).await;
    assert_eq!(shown["questions"][0]["anonymous"], true, "{shown}");

    let (s, b) = call(
        &room.app,
        "POST",
        &format!("/packs/{id}/install"),
        Some(&room.ana.0),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let queue = room.get(&room.ana.0, "/questions/queue").await;
    let installed = |text: &str| {
        queue["custom"]
            .as_array()
            .unwrap()
            .iter()
            .find(|q| q["text"] == text)
            .unwrap()["anonymous"]
            .clone()
    };
    assert_eq!(installed("Qui pleure devant les pubs ?"), true);
    assert_eq!(installed("Classe-nous du plus au moins bavard"), true);
    assert_eq!(installed("Qui perd toujours ses clés ?"), false);
}

#[tokio::test]
async fn a_party_is_never_anonymous() {
    let (app, state) = setup().await;
    let (host, _) = join(&app, "Hôte").await;
    let (s, party) = call(&app, "POST", "/parties", Some(&host), None, Some(json!({}))).await;
    assert_eq!(s, StatusCode::OK, "{party}");
    let id = party["id"].as_i64().unwrap();
    let code = party["code"].as_str().unwrap();
    // The rule can't be set on a party...
    let (s, b) = call(
        &app,
        "PATCH",
        &format!("/rooms/{id}"),
        Some(&host),
        None,
        Some(json!({ "anonymous_votes": "always" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_setting"))
    );
    // ... and even forced in the database, its questions open with names.
    sqlx::query("UPDATE groups SET anonymous_votes = 'always' WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await
        .unwrap();
    for name in ["Ana", "Bob"] {
        let (s, b) = call(
            &app,
            "POST",
            "/parties/join",
            None,
            None,
            Some(json!({ "code": code, "name": name, "color": "#5BC8FF" })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    let (s, b) = call(
        &app,
        "POST",
        &format!("/parties/{id}/start"),
        Some(&host),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let rounds: Vec<bool> = sqlx::query_scalar("SELECT anonymous FROM rounds WHERE group_id = ?")
        .bind(id)
        .fetch_all(&state.db)
        .await
        .unwrap();
    assert!(!rounds.is_empty());
    assert!(rounds.iter().all(|a| !a));
}
