use super::*;

#[tokio::test]
async fn devices_can_subscribe_to_push() {
    let app = test_app().await;
    let (tug, _) = join(&app, "Tugdual").await;
    let web = json!({ "kind": "web", "endpoint": "https://fcm.googleapis.com/fcm/send/abc", "p256dh": "x", "auth": "y" });
    let (status, _) = call(
        &app,
        "POST",
        "/push/subscribe",
        Some(&tug),
        Some(web.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // Same endpoint again is an update, not a duplicate.
    let (status, _) = call(&app, "POST", "/push/subscribe", Some(&tug), Some(web)).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        &app,
        "POST",
        "/push/subscribe",
        Some(&tug),
        Some(json!({ "kind": "fcm", "token": "device-1" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let bad = json!({ "kind": "web", "endpoint": "http://plain", "p256dh": "x", "auth": "y" });
    let (status, _) = call(&app, "POST", "/push/subscribe", Some(&tug), Some(bad)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, config) = call(&app, "GET", "/push/config", None, None).await;
    assert!(config["vapid_public_key"].is_null());
}

#[tokio::test]
async fn nudges_and_unseen_results() {
    let app = test_app().await;
    let (tug, tug_id) = join(&app, "Tugdual").await;
    let (louis, louis_id) = join(&app, "Louis").await;

    // Nudge someone who has not voted: once per question, not yourself, not someone who voted.
    let nudge = |id: i64| json!({ "member_id": id });
    let (status, _) = call(
        &app,
        "POST",
        "/round/current/nudge",
        Some(&tug),
        Some(nudge(louis_id)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = call(
        &app,
        "POST",
        "/round/current/nudge",
        Some(&tug),
        Some(nudge(louis_id)),
    )
    .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::CONFLICT, Some("already_nudged"))
    );
    let (status, _) = call(
        &app,
        "POST",
        "/round/current/nudge",
        Some(&tug),
        Some(nudge(tug_id)),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Tug votes, then Louis closes the round while Tug is away.
    let (_, round) = call(&app, "GET", "/round/current", Some(&tug), None).await;
    let first = round["id"].as_i64().unwrap();
    call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&tug),
        Some(json!({ "target_id": louis_id })),
    )
    .await;
    let (_, unseen) = call(&app, "GET", "/round/unseen", Some(&tug), None).await;
    assert!(unseen.is_null(), "still open: nothing to recap");
    call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&louis),
        Some(json!({ "target_id": tug_id })),
    )
    .await;

    // Back in the app: Tug gets the recap of the finished question first.
    let (_, unseen) = call(&app, "GET", "/round/unseen", Some(&tug), None).await;
    assert_eq!(unseen["id"].as_i64(), Some(first));
    assert!(unseen["result"].is_array());
    let (status, _) = call(
        &app,
        "POST",
        &format!("/rounds/{first}/seen"),
        Some(&tug),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, unseen) = call(&app, "GET", "/round/unseen", Some(&tug), None).await;
    assert!(unseen.is_null());
}

#[tokio::test]
async fn unseen_results_can_be_skipped_at_once() {
    let app = test_app().await;
    let (tug, tug_id) = join(&app, "Tugdual").await;
    let (louis, louis_id) = join(&app, "Louis").await;

    // Three questions finish while Tug is away (he voted on each before leaving the app).
    let mut ids = vec![];
    for _ in 0..3 {
        let (_, round) = call(&app, "GET", "/round/current", Some(&tug), None).await;
        ids.push(round["id"].as_i64().unwrap());
        for (who, target) in [(&tug, louis_id), (&louis, tug_id)] {
            let (status, _) = call(
                &app,
                "POST",
                "/round/current/vote",
                Some(who),
                Some(json!({ "target_id": target })),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
        }
    }

    // The oldest comes first, with how many are waiting and the newest of them.
    let (_, unseen) = call(&app, "GET", "/round/unseen", Some(&tug), None).await;
    assert_eq!(unseen["id"].as_i64(), Some(ids[0]));
    assert_eq!(unseen["unseen_count"].as_i64(), Some(3));
    assert_eq!(unseen["unseen_last_id"].as_i64(), Some(ids[2]));

    // One seen: two left.
    call(
        &app,
        "POST",
        &format!("/rounds/{}/seen", ids[0]),
        Some(&tug),
        None,
    )
    .await;
    let (_, unseen) = call(&app, "GET", "/round/unseen", Some(&tug), None).await;
    assert_eq!(unseen["id"].as_i64(), Some(ids[1]));
    assert_eq!(unseen["unseen_count"].as_i64(), Some(2));

    // Marking the newest seen skips them all; Louis's own recaps are untouched.
    let (status, _) = call(
        &app,
        "POST",
        &format!("/rounds/{}/seen", ids[2]),
        Some(&tug),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, unseen) = call(&app, "GET", "/round/unseen", Some(&tug), None).await;
    assert!(unseen.is_null());
    let (_, theirs) = call(&app, "GET", "/round/unseen", Some(&louis), None).await;
    assert_eq!(theirs["unseen_count"].as_i64(), Some(3));
}

#[tokio::test]
async fn chat_under_results_and_inbox() {
    let app = test_app().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, b_id) = join(&app, "Bob").await;
    let (_, open) = call(&app, "GET", "/round/current", Some(&a), None).await;
    let first = open["id"].as_i64().unwrap();
    // No chat while votes are secret.
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rounds/{first}/messages"),
        Some(&a),
        Some(json!({ "text": "alors ?" })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    // Nudge lands in the inbox.
    call(
        &app,
        "POST",
        "/round/current/nudge",
        Some(&a),
        Some(json!({ "member_id": b_id })),
    )
    .await;
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&b), None).await;
    assert_eq!(inbox["items"][0]["kind"], "nudge");
    vote_as(&app, &a, b_id).await;
    vote_as(&app, &b, b_id).await;
    let (s, msg) = call(
        &app,
        "POST",
        &format!("/rounds/{first}/messages"),
        Some(&a),
        Some(json!({ "text": "  évidemment Bob  " })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{msg}");
    assert_eq!(msg["text"], "évidemment Bob");
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rounds/{first}/messages"),
        Some(&a),
        Some(json!({ "text": "   " })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (_, list) = call(
        &app,
        "GET",
        &format!("/rounds/{first}/messages"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["member_id"], a_id);
    // Bob's inbox: the result (Bob closed it, so not for him), the message, the nudge.
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&b), None).await;
    let kinds: Vec<&str> = inbox["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["message", "nudge"]);
    assert_eq!(inbox["unread"], 2);
    // Ana gets the result notice, not her own message (and the first question, opened when Bob joined).
    let (_, inbox_a) = call(&app, "GET", "/inbox", Some(&a), None).await;
    let kinds: Vec<&str> = inbox_a["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["result", "question"]);
    call(&app, "POST", "/inbox/read", Some(&b), None).await;
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&b), None).await;
    assert_eq!(inbox["unread"], 0);
    assert_eq!(inbox["items"][0]["read"], true);
}

#[tokio::test]
async fn chat_list_keeps_the_latest_messages() {
    let (app, state) = setup().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, b_id) = join(&app, "Bob").await;
    let (_, open) = call(&app, "GET", "/round/current", Some(&a), None).await;
    let first = open["id"].as_i64().unwrap();
    vote_as(&app, &a, b_id).await;
    vote_as(&app, &b, b_id).await;
    for i in 0..230 {
        sqlx::query("INSERT INTO messages (round_id, member_id, text) VALUES (?, ?, ?)")
            .bind(first)
            .bind(a_id)
            .bind(format!("m{i}"))
            .execute(&state.db)
            .await
            .unwrap();
    }
    let (s, list) = call(
        &app,
        "GET",
        &format!("/rounds/{first}/messages"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 200);
    // Oldest first, and the last ones.
    assert_eq!(list[0]["text"], "m30");
    assert_eq!(list[199]["text"], "m229");
}

#[tokio::test]
async fn notification_settings_filter_pushes() {
    use quidenous_server::realtime::push::{Kind, Target};
    // 2026-11-12 00:00 UTC.
    const MIDNIGHT: u64 = 1_794_441_600;
    let noon = MIDNIGHT + 12 * 3600;
    let (app, state) = setup().await;
    let mut people = Vec::new();
    for (i, name) in ["Tugdual", "Louis", "Ravenne"].into_iter().enumerate() {
        let (token, member) = join(&app, name).await;
        let device = json!({ "kind": "web", "endpoint": format!("https://fcm.googleapis.com/fcm/send/{i}"), "p256dh": "x", "auth": "y" });
        let (status, _) = call(&app, "POST", "/push/subscribe", Some(&token), Some(device)).await;
        assert_eq!(status, StatusCode::OK);
        let user: i64 = sqlx::query_scalar("SELECT user_id FROM members WHERE id = ?")
            .bind(member)
            .fetch_one(&state.db)
            .await
            .unwrap();
        people.push((token, member, user));
    }
    let [
        (tug, _, tug_user),
        (louis, louis_member, louis_user),
        (ravenne, _, ravenne_user),
    ] = people.try_into().unwrap();
    let (_, rooms) = call(&app, "GET", "/rooms", Some(&tug), None).await;
    let room = rooms[0]["id"].as_i64().unwrap();
    assert_eq!(rooms[0]["muted"], false);
    let group = || Target::Group {
        group_id: room,
        skip_member: None,
        from_user: None,
    };
    let everyone = vec![tug_user, louis_user, ravenne_user];
    assert_eq!(
        audience(&state, group(), &[Kind::Chat], noon).await,
        everyone
    );

    // Defaults: everything on, no quiet hours.
    let (status, prefs) = call(&app, "GET", "/notifications", Some(&louis), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        prefs,
        json!({ "enabled": true, "off": [], "quiet_start": null, "quiet_end": null, "tz_offset": 0 })
    );

    // A type turned off: no chat pushes, the rest still come; a result with the next
    // question still comes if only one of the two is off.
    let louis_prefs = json!({ "enabled": true, "off": ["chat", "result"], "quiet_start": null, "quiet_end": null, "tz_offset": 60 });
    let (status, saved) = call(
        &app,
        "PUT",
        "/notifications",
        Some(&louis),
        Some(louis_prefs),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (_, prefs) = call(&app, "GET", "/notifications", Some(&louis), None).await;
    assert_eq!(prefs["off"], json!(["chat", "result"]));
    assert_eq!(prefs["tz_offset"], 60);
    assert_eq!(
        audience(&state, group(), &[Kind::Chat], noon).await,
        vec![tug_user, ravenne_user]
    );
    assert_eq!(
        audience(&state, group(), &[Kind::Result], noon).await,
        vec![tug_user, ravenne_user]
    );
    assert_eq!(
        audience(&state, group(), &[Kind::Question], noon).await,
        everyone
    );
    assert_eq!(
        audience(&state, group(), &[Kind::Result, Kind::Question], noon).await,
        everyone
    );
    let nudge = Target::Member {
        member_id: louis_member,
        from_user: None,
    };
    assert_eq!(
        audience(&state, nudge, &[Kind::Nudge], noon).await,
        vec![louis_user]
    );

    // A muted room: nothing from it; unmuted, back.
    let (status, body) = call(
        &app,
        "PUT",
        &format!("/rooms/{room}/mute"),
        Some(&ravenne),
        None,
    )
    .await;
    assert_eq!(
        (status, body["muted"].as_bool()),
        (StatusCode::OK, Some(true))
    );
    let (_, rooms) = call(&app, "GET", "/rooms", Some(&ravenne), None).await;
    assert_eq!(rooms[0]["muted"], true);
    assert_eq!(
        audience(&state, group(), &[Kind::Question], noon).await,
        vec![tug_user, louis_user]
    );
    let (status, _) = call(
        &app,
        "DELETE",
        &format!("/rooms/{room}/mute"),
        Some(&ravenne),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        audience(&state, group(), &[Kind::Question], noon).await,
        everyone
    );
    let (status, _) = call(&app, "PUT", "/rooms/999999/mute", Some(&ravenne), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Master switch off: nothing at all.
    let off = json!({ "enabled": false, "off": [], "quiet_start": null, "quiet_end": null, "tz_offset": 0 });
    call(&app, "PUT", "/notifications", Some(&ravenne), Some(off)).await;
    assert_eq!(
        audience(&state, group(), &[Kind::Question], noon).await,
        vec![tug_user, louis_user]
    );

    // Quiet hours 22:00 → 08:00 in UTC+1: dropped from 21:00 to 07:00 UTC.
    let quiet = json!({ "enabled": true, "off": [], "quiet_start": 22 * 60, "quiet_end": 8 * 60, "tz_offset": 60 });
    let (status, _) = call(&app, "PUT", "/notifications", Some(&tug), Some(quiet)).await;
    assert_eq!(status, StatusCode::OK);
    let at = |h: u64, m: u64| MIDNIGHT + h * 3600 + m * 60;
    for (time, quiet) in [
        (at(20, 59), false),
        (at(21, 0), true),
        (at(23, 30), true),
        (at(2, 0), true),
        (at(6, 59), true),
        (at(7, 0), false),
        (noon, false),
    ] {
        let reached = audience(&state, group(), &[Kind::Question], time).await;
        assert_eq!(
            !reached.contains(&tug_user),
            quiet,
            "at {}",
            (time - MIDNIGHT) / 60
        );
    }

    // Bad settings are refused.
    for bad in [
        json!({ "enabled": true, "off": [], "quiet_start": 1440, "quiet_end": 60, "tz_offset": 0 }),
        json!({ "enabled": true, "off": [], "quiet_start": 60, "quiet_end": null, "tz_offset": 0 }),
        json!({ "enabled": true, "off": [], "quiet_start": null, "quiet_end": null, "tz_offset": 2000 }),
    ] {
        let (status, _) = call(&app, "PUT", "/notifications", Some(&tug), Some(bad)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let unknown = json!({ "enabled": true, "off": ["spam"], "quiet_start": null, "quiet_end": null, "tz_offset": 0 });
    let (status, _) = call(&app, "PUT", "/notifications", Some(&tug), Some(unknown)).await;
    assert!(status.is_client_error());
}

#[tokio::test]
async fn chat_mentions_are_validated_and_ping_through_the_chat_switch() {
    use quidenous_server::realtime::push::{Kind, Target};
    // 2026-11-12 12:00 UTC.
    const NOON: u64 = 1_794_441_600 + 12 * 3600;
    let (app, state) = setup().await;
    let mut people = Vec::new();
    for (i, name) in ["Ana", "Bob", "Cyd", "Dan"].into_iter().enumerate() {
        let (token, member) = join(&app, name).await;
        let device = json!({ "kind": "web", "endpoint": format!("https://fcm.googleapis.com/fcm/send/m{i}"), "p256dh": "x", "auth": "y" });
        call(&app, "POST", "/push/subscribe", Some(&token), Some(device)).await;
        let user: i64 = sqlx::query_scalar("SELECT user_id FROM members WHERE id = ?")
            .bind(member)
            .fetch_one(&state.db)
            .await
            .unwrap();
        people.push((token, member, user));
    }
    let [
        (ana, ana_id, ana_user),
        (bob, bob_id, bob_user),
        (cyd, cyd_id, _),
        (dan, dan_id, _),
    ] = people.try_into().unwrap();
    let (_, open) = call(&app, "GET", "/round/current", Some(&ana), None).await;
    let round = open["id"].as_i64().unwrap();
    for token in [&ana, &bob, &cyd, &dan] {
        vote_as(&app, token, bob_id).await;
    }
    // Dan leaves the room: no longer someone you can mention.
    sqlx::query("UPDATE members SET removed_at = 'x' WHERE id = ?")
        .bind(dan_id)
        .execute(&state.db)
        .await
        .unwrap();
    let path = format!("/rounds/{round}/messages");
    let latest = |inbox: &Value| {
        (
            inbox["items"][0]["kind"].clone(),
            inbox["items"][0]["actor_id"].clone(),
        )
    };

    // Only current members other than me are kept, once each.
    let (s, msg) = call(
        &app,
        "POST",
        &path,
        Some(&ana),
        Some(json!({ "text": "@Bob @Dan regarde", "mentions": [bob_id, dan_id, ana_id, 999_999, bob_id] })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{msg}");
    assert_eq!(
        (&msg["mentions"], &msg["everyone"]),
        (&json!([bob_id]), &json!(false))
    );
    let (_, list) = call(&app, "GET", &path, Some(&cyd), None).await;
    assert_eq!(list[0]["mentions"], json!([bob_id]));
    // Bob gets a mention, Cyd the plain message.
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&bob), None).await;
    assert_eq!(latest(&inbox), (json!("mention"), json!(ana_id)), "{inbox}");
    assert_eq!(inbox["items"][0]["round_id"], round);
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&cyd), None).await;
    assert_eq!(latest(&inbox), (json!("message"), json!(ana_id)));
    // An older app sends no mentions: a plain message.
    let (_, msg) = call(
        &app,
        "POST",
        &path,
        Some(&cyd),
        Some(json!({ "text": "ok" })),
    )
    .await;
    assert_eq!(
        (&msg["mentions"], &msg["everyone"]),
        (&json!([]), &json!(false))
    );

    // The push to Bob comes through with chat pushes off, not with mentions off too.
    let to_bob = || Target::Members {
        member_ids: vec![bob_id],
        from_user: Some(ana_user),
    };
    let mention = [Kind::Mention, Kind::Chat];
    assert_eq!(
        audience(&state, to_bob(), &mention, NOON).await,
        vec![bob_user]
    );
    let prefs = |off: Value| json!({ "enabled": true, "off": off, "quiet_start": null, "quiet_end": null, "tz_offset": 0 });
    call(
        &app,
        "PUT",
        "/notifications",
        Some(&bob),
        Some(prefs(json!(["chat"]))),
    )
    .await;
    assert_eq!(
        audience(&state, to_bob(), &[Kind::Chat], NOON).await,
        Vec::<i64>::new()
    );
    assert_eq!(
        audience(&state, to_bob(), &mention, NOON).await,
        vec![bob_user]
    );
    call(
        &app,
        "PUT",
        "/notifications",
        Some(&bob),
        Some(prefs(json!(["chat", "mention"]))),
    )
    .await;
    assert_eq!(
        audience(&state, to_bob(), &mention, NOON).await,
        Vec::<i64>::new()
    );
    call(
        &app,
        "PUT",
        "/notifications",
        Some(&bob),
        Some(prefs(json!(["chat"]))),
    )
    .await;
    // A muted room still mutes mentions.
    let (_, rooms) = call(&app, "GET", "/rooms", Some(&bob), None).await;
    let room = rooms[0]["id"].as_i64().unwrap();
    call(
        &app,
        "PUT",
        &format!("/rooms/{room}/mute"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(
        audience(&state, to_bob(), &mention, NOON).await,
        Vec::<i64>::new()
    );
    call(
        &app,
        "DELETE",
        &format!("/rooms/{room}/mute"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(
        audience(&state, to_bob(), &mention, NOON).await,
        vec![bob_user]
    );
    // Removed seats get nothing.
    let to_dan = Target::Members {
        member_ids: vec![dan_id],
        from_user: Some(ana_user),
    };
    assert_eq!(
        audience(&state, to_dan, &mention, NOON).await,
        Vec::<i64>::new()
    );

    // Bob blocks Ana: her mentions neither push nor show in his inbox.
    let (s, _) = call(
        &app,
        "PUT",
        &format!("/blocks/{ana_user}"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        audience(&state, to_bob(), &mention, NOON).await,
        Vec::<i64>::new()
    );
    call(
        &app,
        "POST",
        &path,
        Some(&ana),
        Some(json!({ "text": "@Bob ?", "mentions": [bob_id] })),
    )
    .await;
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&bob), None).await;
    assert!(
        inbox["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["actor_id"] != ana_id),
        "{inbox}"
    );

    // @tous: everyone still here gets a mention; again within minutes, a plain message.
    let (_, msg) = call(
        &app,
        "POST",
        &path,
        Some(&cyd),
        Some(json!({ "text": "@tous on refait ?", "everyone": true })),
    )
    .await;
    assert_eq!(msg["everyone"], true);
    for token in [&ana, &bob] {
        let (_, inbox) = call(&app, "GET", "/inbox", Some(token), None).await;
        assert_eq!(latest(&inbox), (json!("mention"), json!(cyd_id)), "{inbox}");
    }
    let (_, msg) = call(
        &app,
        "POST",
        &path,
        Some(&cyd),
        Some(json!({ "text": "@tous allô", "everyone": true })),
    )
    .await;
    assert_eq!(msg["everyone"], true);
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&ana), None).await;
    assert_eq!(latest(&inbox), (json!("message"), json!(cyd_id)));
    // ...though a name mentioned alongside still pings.
    call(
        &app,
        "POST",
        &path,
        Some(&cyd),
        Some(json!({ "text": "@tous @Ana", "everyone": true, "mentions": [ana_id] })),
    )
    .await;
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&ana), None).await;
    assert_eq!(latest(&inbox), (json!("mention"), json!(cyd_id)));
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&bob), None).await;
    assert_eq!(latest(&inbox), (json!("message"), json!(cyd_id)));
    let (_, features) = call(&app, "GET", "/features", None, None).await;
    assert!(features.as_array().unwrap().contains(&json!("mentions")));
}
