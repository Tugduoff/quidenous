//! Live reactions on results (docs/REACTIONS.md): the palette, the cap, who may react to what, blocks,
//! parties (guests, the old toggle, the state), account deletion and the live event's opt-in.

use crate::common::{Socket, connect, next_event, serve};

use crate::common::{call, join, room_id, setup};
use axum::{Router, http::StatusCode};
use quidenous_server::{
    AppState,
    live::{Event, GroupEvent},
    party,
};
use serde_json::{Value, json};

/// Ana, Bob and Carl in the env room, the first question closed. (tokens, member ids, round, room)
async fn closed() -> (Router, AppState, [String; 3], [i64; 3], i64, i64) {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let (carl, carl_id) = join(&app, "Carl").await;
    let room = room_id(&app, &ana).await;
    let (_, open) = call(&app, "GET", "/round/current", Some(&ana), Some(room), None).await;
    let round = open["id"].as_i64().unwrap();
    for t in [&ana, &bob, &carl] {
        let (s, b) = call(
            &app,
            "POST",
            "/round/current/vote",
            Some(t),
            Some(room),
            Some(json!({ "target_id": bob_id })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    (
        app,
        state,
        [ana, bob, carl],
        [ana_id, bob_id, carl_id],
        round,
        room,
    )
}

/// A party hosted by `host` with one guest, its first question revealed: (id, code, round, guest token, guest member).
async fn revealed_party(
    app: &Router,
    state: &AppState,
    host: &str,
) -> (i64, String, i64, String, i64) {
    let (s, p) = call(app, "POST", "/parties", Some(host), None, Some(json!({}))).await;
    assert_eq!(s, StatusCode::OK, "{p}");
    let id = p["id"].as_i64().unwrap();
    let code = p["code"].as_str().unwrap().to_owned();
    let (s, g) = call(
        app,
        "POST",
        "/parties/join",
        None,
        None,
        Some(json!({ "code": code, "name": "Léa", "color": "#5BC8FF" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{g}");
    let guest = g["token"].as_str().unwrap().to_owned();
    let guest_id = g["member_id"].as_i64().unwrap();
    let (s, b) = call(
        app,
        "POST",
        &format!("/parties/{id}/start"),
        Some(host),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let round = b["round"]["id"].as_i64().unwrap();
    let later: String =
        sqlx::query_scalar("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+31 seconds')")
            .fetch_one(&state.db)
            .await
            .unwrap();
    party::sweep_at(state, &later).await.unwrap();
    (id, code, round, guest, guest_id)
}

async fn get(app: &Router, token: &str, round: i64) -> (StatusCode, Value) {
    call(
        app,
        "GET",
        &format!("/rounds/{round}/reactions"),
        Some(token),
        None,
        None,
    )
    .await
}

async fn tap(app: &Router, token: &str, round: i64, taps: Value) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        &format!("/rounds/{round}/reactions"),
        Some(token),
        None,
        Some(json!({ "taps": taps })),
    )
    .await
}

async fn user_of(app: &Router, token: &str, room: i64, member: i64) -> i64 {
    let (_, g) = call(app, "GET", "/group", Some(token), Some(room), None).await;
    g["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == member)
        .unwrap()["user_id"]
        .as_i64()
        .unwrap()
}

/// The taps of the `result_reacted` events heard so far, summed per (member, emoji), and the
/// number of `party_updated` events.
fn heard(
    rx: &mut tokio::sync::broadcast::Receiver<GroupEvent>,
) -> (Vec<(i64, String, i64)>, usize) {
    let mut taps: Vec<(i64, String, i64)> = Vec::new();
    let mut party = 0;
    while let Ok(e) = rx.try_recv() {
        match e.event {
            Event::ResultReacted { taps: list, .. } => {
                for t in list {
                    match taps
                        .iter_mut()
                        .find(|x| x.0 == t.member_id && x.1 == t.emoji)
                    {
                        Some(x) => x.2 += t.count,
                        None => taps.push((t.member_id, t.emoji, t.count)),
                    }
                }
            }
            Event::PartyUpdated { .. } => party += 1,
            _ => {}
        }
    }
    (taps, party)
}

#[tokio::test]
async fn taps_are_counted_in_palette_order() {
    let (app, _, [ana, bob, _], [ana_id, bob_id, _], round, room) = closed().await;
    let (_, features) = call(&app, "GET", "/features", None, None, None).await;
    assert!(
        features
            .as_array()
            .unwrap()
            .contains(&json!("result_reactions"))
    );

    let (s, b) = get(&app, &ana, round).await;
    assert_eq!(
        (s, b),
        (
            StatusCode::OK,
            json!({ "round_id": round, "cap": 30, "mine": 0, "reactions": [] })
        )
    );
    // Sent 🙈 then 😂: shown in palette order; a bare heart is the heart.
    let (s, b) = tap(
        &app,
        &ana,
        round,
        json!([{ "emoji": "🙈", "count": 2 }, { "emoji": "😂", "count": 3 }]),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(
        b,
        json!({
            "round_id": round, "cap": 30, "mine": 5, "accepted": 5,
            "reactions": [
                { "emoji": "😂", "count": 3, "mine": 3, "members": [{ "member_id": ana_id, "count": 3 }] },
                { "emoji": "🙈", "count": 2, "mine": 2, "members": [{ "member_id": ana_id, "count": 2 }] },
            ],
        })
    );
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let (s, b) = tap(
        &app,
        &bob,
        round,
        json!([{ "emoji": "😂", "count": 1 }, { "emoji": "❤", "count": 1 }]),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(
        (b["mine"].as_i64(), b["accepted"].as_i64()),
        (Some(2), Some(2))
    );
    let (s, b) = get(&app, &ana, round).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        b,
        json!({
            "round_id": round, "cap": 30, "mine": 5,
            "reactions": [
                { "emoji": "😂", "count": 4, "mine": 3, "members": [
                    { "member_id": ana_id, "count": 3 }, { "member_id": bob_id, "count": 1 },
                ] },
                { "emoji": "❤️", "count": 1, "mine": 0, "members": [{ "member_id": bob_id, "count": 1 }] },
                { "emoji": "🙈", "count": 2, "mine": 2, "members": [{ "member_id": ana_id, "count": 2 }] },
            ],
        })
    );

    // Bad requests.
    for (taps, error) in [
        (json!([]), "bad_taps"),
        (json!([{ "emoji": "😂", "count": 0 }]), "bad_taps"),
        (json!([{ "emoji": "😂", "count": -1 }]), "bad_taps"),
        (json!([{ "emoji": "😂", "count": 31 }]), "bad_taps"),
        (
            json!(vec![json!({ "emoji": "😂", "count": 1 }); 9]),
            "bad_taps",
        ),
        (json!([{ "emoji": "👍", "count": 1 }]), "bad_emoji"),
        (json!([{ "emoji": "", "count": 1 }]), "bad_emoji"),
        (json!([{ "emoji": "😂😂", "count": 1 }]), "bad_emoji"),
    ] {
        let (s, b) = tap(&app, &ana, round, taps.clone()).await;
        assert_eq!(
            (s, b["error"].as_str()),
            (StatusCode::BAD_REQUEST, Some(error)),
            "{taps}"
        );
    }
    let (s, b) = call(
        &app,
        "POST",
        &format!("/rounds/{round}/reactions"),
        Some(&ana),
        None,
        Some(json!({})),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_taps"))
    );

    // The open question: not yet. No such round: 404. No account: 401.
    let (_, open) = call(&app, "GET", "/round/current", Some(&ana), Some(room), None).await;
    let open = open["id"].as_i64().unwrap();
    assert_ne!(open, round);
    let (s, b) = get(&app, &ana, open).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("not_reveal"))
    );
    let (s, b) = tap(&app, &ana, open, json!([{ "emoji": "😂", "count": 1 }])).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("not_reveal"))
    );
    for r in [999_999, 0] {
        let (s, b) = get(&app, &ana, r).await;
        assert_eq!(
            (s, b["error"].as_str()),
            (StatusCode::NOT_FOUND, Some("not_found"))
        );
    }
    let (s, _) = call(
        &app,
        "GET",
        &format!("/rounds/{round}/reactions"),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn thirty_taps_per_person_and_result() {
    let (app, state, [ana, bob, _], [ana_id, bob_id, _], round, room) = closed().await;
    let mut rx = state.live.subscribe();
    let (s, b) = tap(&app, &ana, round, json!([{ "emoji": "😂", "count": 20 }])).await;
    assert_eq!((s, b["accepted"].as_i64()), (StatusCode::OK, Some(20)));
    // In request order until the cap, all emojis together.
    let (s, b) = tap(
        &app,
        &ana,
        round,
        json!([{ "emoji": "🔥", "count": 8 }, { "emoji": "😱", "count": 5 }, { "emoji": "👀", "count": 1 }]),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(
        (b["accepted"].as_i64(), b["mine"].as_i64()),
        (Some(10), Some(30))
    );
    let emojis: Vec<(&str, i64)> = b["reactions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["emoji"].as_str().unwrap(), r["count"].as_i64().unwrap()))
        .collect();
    assert_eq!(emojis, vec![("😂", 20), ("🔥", 8), ("😱", 2)]);
    // Full: fine, nothing counted. Others still can.
    let (s, b) = tap(&app, &ana, round, json!([{ "emoji": "😂", "count": 1 }])).await;
    assert_eq!(
        (s, b["accepted"].as_i64(), b["mine"].as_i64()),
        (StatusCode::OK, Some(0), Some(30))
    );
    let (s, b) = tap(&app, &bob, round, json!([{ "emoji": "😂", "count": 30 }])).await;
    assert_eq!((s, b["accepted"].as_i64()), (StatusCode::OK, Some(30)));
    assert_eq!(b["reactions"][0]["count"], 50);

    // The room heard exactly the counted taps (coalesced: wait for the trailing event); a daily room
    // gets no `party_updated`.
    tokio::time::sleep(quidenous_server::live::RESULT_REACT_GAP * 3).await;
    let (taps, party) = heard(&mut rx);
    assert_eq!(
        taps,
        vec![
            (ana_id, "😂".into(), 20),
            (ana_id, "🔥".into(), 8),
            (ana_id, "😱".into(), 2),
            (bob_id, "😂".into(), 30),
        ]
    );
    assert_eq!(party, 0);
    let _ = room;
}

#[tokio::test]
async fn blocked_people_are_left_out() {
    let (app, _, [ana, bob, carl], [ana_id, bob_id, carl_id], round, room) = closed().await;
    for (t, n) in [(&bob, 4), (&carl, 2), (&ana, 1)] {
        let (s, _) = tap(&app, t, round, json!([{ "emoji": "💀", "count": n }])).await;
        assert_eq!(s, StatusCode::OK);
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    let bob_user = user_of(&app, &ana, room, bob_id).await;
    let (s, _) = call(
        &app,
        "PUT",
        &format!("/blocks/{bob_user}"),
        Some(&ana),
        Some(room),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, b) = get(&app, &ana, round).await;
    assert_eq!(
        b["reactions"],
        json!([{ "emoji": "💀", "count": 3, "mine": 1, "members": [
            { "member_id": carl_id, "count": 2 }, { "member_id": ana_id, "count": 1 },
        ] }])
    );
    // Bob is still counted for Carl.
    let (_, b) = get(&app, &carl, round).await;
    assert_eq!(b["reactions"][0]["count"], 7);
    assert_eq!(b["reactions"][0]["members"][0]["member_id"], bob_id);
}

#[tokio::test]
async fn other_rooms_results_are_out_of_reach_and_deleted_accounts_take_their_taps() {
    let (app, state, [ana, _, carl], [_, _, carl_id], round, _) = closed().await;
    // A party Ana isn't in.
    let (host, _) = join(&app, "Hôte").await;
    let (_, _, party_round, _, _) = revealed_party(&app, &state, &host).await;
    let (s, b) = get(&app, &ana, party_round).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::NOT_FOUND, Some("not_found"))
    );
    let (s, b) = tap(
        &app,
        &ana,
        party_round,
        json!([{ "emoji": "😂", "count": 1 }]),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::NOT_FOUND, Some("not_found"))
    );

    let (s, _) = tap(&app, &carl, round, json!([{ "emoji": "👏", "count": 3 }])).await;
    assert_eq!(s, StatusCode::OK);
    let (s, b) = call(&app, "DELETE", "/auth/account", Some(&carl), None, None).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM round_reactions WHERE member_id = ?")
        .bind(carl_id)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(left, 0);
    let (_, b) = get(&app, &ana, round).await;
    assert_eq!(b["reactions"], json!([]));
}

#[tokio::test]
async fn party_guests_react_and_older_apps_still_toggle() {
    let (app, state) = setup().await;
    let (host, _) = join(&app, "Hôte").await;
    let (id, _, round, guest, guest_id) = revealed_party(&app, &state, &host).await;
    let mut rx = state.live.subscribe();

    // A guest (no e-mail) taps; the party state counts the taps.
    let (s, b) = tap(&app, &guest, round, json!([{ "emoji": "😂", "count": 3 }])).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["accepted"], 3);
    let (_, p) = call(
        &app,
        "GET",
        &format!("/parties/{id}"),
        Some(&guest),
        None,
        None,
    )
    .await;
    assert_eq!(
        p["round"]["reactions"],
        json!([{ "emoji": "😂", "count": 3, "mine": true }])
    );

    // The old toggle: none of mine there, so one tap; again, mine go.
    let react = |emoji: &'static str| {
        let app = app.clone();
        let host = host.clone();
        async move {
            call(
                &app,
                "POST",
                &format!("/parties/{id}/react"),
                Some(&host),
                None,
                Some(json!({ "round_id": round, "emoji": emoji })),
            )
            .await
        }
    };
    let (s, p) = react("😂").await;
    assert_eq!(s, StatusCode::OK, "{p}");
    assert_eq!(
        p["round"]["reactions"],
        json!([{ "emoji": "😂", "count": 4, "mine": true }])
    );
    let (_, p) = react("🙈").await;
    assert_eq!(
        p["round"]["reactions"],
        json!([{ "emoji": "😂", "count": 4, "mine": true }, { "emoji": "🙈", "count": 1, "mine": true }])
    );
    let (_, p) = react("😂").await;
    assert_eq!(
        p["round"]["reactions"],
        json!([{ "emoji": "😂", "count": 3, "mine": false }, { "emoji": "🙈", "count": 1, "mine": true }])
    );
    let (s, b) = react("👍").await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_emoji"))
    );
    // The guest taps the rest of the cap; the toggle adds nothing over it.
    let (_, b) = tap(&app, &guest, round, json!([{ "emoji": "🔥", "count": 30 }])).await;
    assert_eq!(b["accepted"], 27);
    let (_, b) = call(
        &app,
        "POST",
        &format!("/parties/{id}/react"),
        Some(&guest),
        None,
        Some(json!({ "round_id": round, "emoji": "💀" })),
    )
    .await;
    assert!(
        b["round"]["reactions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["emoji"] != "💀")
    );

    // Live: the counted taps, and `party_updated` for older apps.
    tokio::time::sleep(quidenous_server::live::RESULT_REACT_GAP * 3).await;
    let (taps, party) = heard(&mut rx);
    let host_id = p["me"]["member_id"].as_i64().unwrap();
    assert_eq!(
        taps,
        vec![
            (guest_id, "😂".into(), 3),
            (host_id, "😂".into(), 1),
            (host_id, "🙈".into(), 1),
            (guest_id, "🔥".into(), 27),
        ]
    );
    assert!(party > 0);
}

// ---------- the socket ----------

/// The types of the events heard until `ms` of silence.
async fn types(socket: &mut Socket, ms: u64) -> Vec<String> {
    let mut seen = Vec::new();
    while let Some(e) = next_event(socket, ms).await {
        seen.push(e["type"].as_str().unwrap().to_owned());
    }
    seen
}

/// `result_reacted` reaches only the sockets opened with `events=reactions`, members and the big screen.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn only_sockets_that_ask_hear_the_taps() {
    let (app, state) = setup().await;
    let (host, _) = join(&app, "Hôte").await;
    let (id, code, round, guest, guest_id) = revealed_party(&app, &state, &host).await;
    let addr = serve(app.clone()).await;
    let mut sockets = [
        connect(addr, &format!("token={host}&room={id}&events=reactions")).await,
        connect(addr, &format!("screen={code}&events=chat,reactions")).await,
        connect(addr, &format!("token={host}&room={id}")).await,
        connect(addr, &format!("screen={code}")).await,
    ];
    // Each socket subscribes once its upgrade is done: ping the room until all hear it.
    for socket in &mut sockets {
        let mut ok = false;
        for _ in 0..50 {
            state.live.send(id, Event::RoomUpdated);
            if next_event(socket, 100).await.is_some() {
                ok = true;
                break;
            }
        }
        assert!(ok);
    }
    for socket in &mut sockets {
        types(socket, 200).await;
    }

    let (s, _) = tap(&app, &guest, round, json!([{ "emoji": "😱", "count": 2 }])).await;
    assert_eq!(s, StatusCode::OK);
    let user: Option<i64> = sqlx::query_scalar("SELECT user_id FROM members WHERE id = ?")
        .bind(guest_id)
        .fetch_one(&state.db)
        .await
        .unwrap();
    for (i, socket) in sockets.iter_mut().enumerate() {
        let mut seen = Vec::new();
        while let Some(e) = next_event(socket, 400).await {
            seen.push(e);
        }
        let reacted: Vec<&Value> = seen
            .iter()
            .filter(|e| e["type"] == "result_reacted")
            .collect();
        assert!(
            seen.iter().any(|e| e["type"] == "party_updated"),
            "{i}: {seen:?}"
        );
        if i < 2 {
            assert_eq!(
                reacted,
                vec![&json!({
                    "type": "result_reacted",
                    "round_id": round,
                    "taps": [{ "member_id": guest_id, "user_id": user, "emoji": "😱", "count": 2 }],
                })],
                "{i}"
            );
        } else {
            assert!(reacted.is_empty(), "{i}: {seen:?}");
        }
    }
}
