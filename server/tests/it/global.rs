use crate::common::{call, join, setup};
use axum::{Router, http::StatusCode};
use quidenous_server::{AppState, platform::i18n::Lang, rooms::model::MemberRow, rounds::global};
use serde_json::{Value, json};

async fn room(
    app: &Router,
    state: &AppState,
    lang: &str,
    names: &[&str],
) -> (i64, Vec<(String, i64)>) {
    let (owner, _) = join(app, names[0]).await;
    let (status, body) = call(
        app,
        "POST",
        "/rooms",
        Some(&owner),
        None,
        Some(json!({ "name": format!("Salon {}", names[0]), "language": lang })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let id = body["id"].as_i64().unwrap();
    let code: String = sqlx::query_scalar("SELECT join_code FROM groups WHERE id = ?")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .unwrap();
    let mut tokens = vec![owner];
    for name in &names[1..] {
        let (token, _) = join(app, name).await;
        let (status, body) = call(
            app,
            "POST",
            "/rooms/join",
            Some(&token),
            None,
            Some(json!({ "code": code })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        tokens.push(token);
    }
    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM members WHERE group_id = ? ORDER BY id")
        .bind(id)
        .fetch_all(&state.db)
        .await
        .unwrap();
    (id, tokens.into_iter().zip(ids).collect())
}

async fn get(app: &Router, token: &str, room: i64) -> Value {
    let (status, body) = call(app, "GET", "/global", Some(token), Some(room), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

async fn vote(app: &Router, token: &str, room: i64, target: i64) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        "/global/vote",
        Some(token),
        Some(room),
        Some(json!({ "target": target })),
    )
    .await
}

async fn member(state: &AppState, id: i64) -> MemberRow {
    sqlx::query_as("SELECT * FROM members WHERE id = ?")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .unwrap()
}

async fn shift(state: &AppState, day: &str, days: i64) -> String {
    sqlx::query_scalar("SELECT date(?, ?)")
        .bind(day)
        .bind(format!("{days:+} days"))
        .fetch_one(&state.db)
        .await
        .unwrap()
}

#[tokio::test]
async fn one_question_per_day_and_language() {
    let (app, state) = setup().await;
    let (fr1, a) = room(&app, &state, "fr", &["Ana", "Bob", "Cyd"]).await;
    let (fr2, b) = room(&app, &state, "fr", &["Dan", "Eve", "Fay"]).await;
    let (en, c) = room(&app, &state, "en", &["Gus", "Hal", "Ivy"]).await;

    let q1 = get(&app, &a[0].0, fr1).await;
    let q2 = get(&app, &b[1].0, fr2).await;
    let q3 = get(&app, &c[2].0, en).await;
    assert_eq!(q1["available"], true);
    assert_eq!(q1["question"], q2["question"]);
    assert_eq!(
        (q1["language"].as_str(), q3["language"].as_str()),
        (Some("fr"), Some("en"))
    );
    assert_ne!(q1["question"], q3["question"]);
    assert!(q1["question"].as_str().unwrap().starts_with("Qui"), "{q1}");
    assert_eq!(get(&app, &a[2].0, fr1).await["question"], q1["question"]);
    let today = q1["day"].as_str().unwrap().to_owned();
    assert_eq!(today, global::today(&state.db).await.unwrap());

    let (id, _) = global::question_for(&state.db, Lang::Fr, &today)
        .await
        .unwrap()
        .unwrap();
    let (kind, vibes): (String, String) =
        sqlx::query_as("SELECT kind, vibes FROM questions WHERE id = ? AND group_id IS NULL")
            .bind(id)
            .fetch_one(&state.db)
            .await
            .unwrap();
    assert_eq!(kind, "pick");
    assert!(
        vibes.contains("famille") || vibes.contains("boulot"),
        "{vibes}"
    );
    assert!(!vibes.contains("pimente"), "{vibes}");

    let tomorrow = shift(&state, &today, 1).await;
    let next = global::question_for(&state.db, Lang::Fr, &tomorrow)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(next.1, q1["question"].as_str().unwrap());
    let mut seen = std::collections::HashSet::from([id, next.0]);
    for n in 2..62 {
        let day = shift(&state, &today, n).await;
        let (q, _) = global::question_for(&state.db, Lang::Fr, &day)
            .await
            .unwrap()
            .unwrap();
        assert!(seen.insert(q), "repeated on day +{n}");
        assert_eq!(
            global::question_for(&state.db, Lang::Fr, &day)
                .await
                .unwrap()
                .unwrap()
                .0,
            q
        );
    }
}

#[tokio::test]
async fn one_final_vote_for_someone_of_my_room() {
    let (app, state) = setup().await;
    let (r, p) = room(&app, &state, "fr", &["Ana", "Bob", "Cyd"]).await;
    let (other, q) = room(&app, &state, "fr", &["Dan", "Eve", "Fay"]).await;
    let (ana, ana_id) = (&p[0].0, p[0].1);
    let bob_id = p[1].1;

    let before = get(&app, ana, r).await;
    assert_eq!(before["my_vote"], Value::Null);
    assert_eq!(
        before["room"]["tally"],
        Value::Null,
        "no tally before voting"
    );
    assert_eq!(before["room"]["members"], 3);

    let (s, b) = vote(&app, ana, r, q[0].1).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("unknown_member"))
    );
    let (s, _) = vote(&app, ana, r, 999_999).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    let (s, after) = vote(&app, ana, r, bob_id).await;
    assert_eq!(s, StatusCode::OK, "{after}");
    assert_eq!(after["my_vote"], bob_id);
    assert_eq!(after["room"]["voters"], 1);
    assert_eq!(
        after["room"]["tally"],
        json!([{ "target_id": bob_id, "count": 1 }])
    );
    let (s, b) = vote(&app, ana, r, ana_id).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("already_voted"))
    );
    assert_eq!(get(&app, ana, r).await["my_vote"], bob_id);

    let (s, b) = vote(&app, &p[1].0, r, bob_id).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(
        b["room"]["tally"],
        json!([{ "target_id": bob_id, "count": 2 }])
    );
    assert!(!b.to_string().contains("voter_id"), "{b}");

    let theirs = get(&app, &q[0].0, other).await;
    assert_eq!(theirs["room"]["voters"], 0);
    assert_eq!(theirs["room"]["tally"], Value::Null);

    let code: String = sqlx::query_scalar("SELECT join_code FROM groups WHERE id = ?")
        .bind(other)
        .fetch_one(&state.db)
        .await
        .unwrap();
    let (s, _) = call(
        &app,
        "POST",
        "/rooms/join",
        Some(ana),
        None,
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, b) = vote(&app, ana, other, q[1].1).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(
        b["room"]["tally"],
        json!([{ "target_id": q[1].1, "count": 1 }])
    );

    let votes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM votes")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(votes, 0);

    let (s, _) = call(&app, "GET", "/global", None, None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn parties_and_small_rooms_have_none() {
    let (app, state) = setup().await;
    let (small, p) = room(&app, &state, "fr", &["Ana", "Bob"]).await;
    let g = get(&app, &p[0].0, small).await;
    assert_eq!(
        (g["available"].as_bool(), g["reason"].as_str()),
        (Some(false), Some("small_room"))
    );
    assert_eq!(g["min_members"], 3);
    assert!(g.get("question").is_none());
    let (s, b) = vote(&app, &p[0].0, small, p[1].1).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("small_room"))
    );

    let code: String = sqlx::query_scalar("SELECT join_code FROM groups WHERE id = ?")
        .bind(small)
        .fetch_one(&state.db)
        .await
        .unwrap();
    let (cyd, _) = join(&app, "Cyd").await;
    call(
        &app,
        "POST",
        "/rooms/join",
        Some(&cyd),
        None,
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(get(&app, &p[0].0, small).await["available"], true);

    let (s, party) = call(
        &app,
        "POST",
        "/parties",
        Some(&p[0].0),
        None,
        Some(json!({})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{party}");
    let (party_id, party_code) = (
        party["id"].as_i64().unwrap(),
        party["code"].as_str().unwrap().to_owned(),
    );
    let mut guests = Vec::new();
    for name in ["Gus", "Hal", "Ivy"] {
        let (s, b) = call(
            &app,
            "POST",
            "/parties/join",
            None,
            None,
            Some(json!({ "code": party_code, "name": name, "color": "#5BC8FF" })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
        guests.push((
            b["token"].as_str().unwrap().to_owned(),
            b["member_id"].as_i64().unwrap(),
        ));
    }
    for token in [&p[0].0, &guests[0].0] {
        let g = get(&app, token, party_id).await;
        assert_eq!(
            (g["available"].as_bool(), g["reason"].as_str()),
            (Some(false), Some("party"))
        );
    }
    let (s, b) = vote(&app, &guests[0].0, party_id, guests[1].1).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("party"))
    );
}

#[tokio::test]
async fn aggregates_from_five_rooms_without_names() {
    let (app, state) = setup().await;
    let plan: [&[(usize, usize)]; 5] = [
        &[(0, 0), (1, 0), (2, 0)],
        &[(0, 2), (1, 2)],
        &[(0, 1), (1, 0), (2, 1)],
        &[(0, 2), (1, 0), (2, 2)],
        &[(0, 1), (1, 1), (2, 0)],
    ];
    let names = [
        ["Abel", "Basile", "Cosme"],
        ["Delphin", "Eloi", "Firmin"],
        ["Gaspard", "Hector", "Isidore"],
        ["Jules", "Kilian", "Lazare"],
        ["Marius", "Nestor", "Octave"],
    ];
    let mut rooms = Vec::new();
    for n in &names {
        rooms.push(room(&app, &state, "fr", n).await);
    }
    let (lone, l) = room(&app, &state, "fr", &["Prosper", "Quentin", "Romeo"]).await;
    let (s, _) = vote(&app, &l[0].0, lone, l[1].1).await;
    assert_eq!(s, StatusCode::OK);
    let (en, e) = room(&app, &state, "en", &["Sacha", "Tess", "Uma"]).await;
    vote(&app, &e[0].0, en, e[1].1).await;
    vote(&app, &e[1].0, en, e[1].1).await;

    for (i, votes) in plan.iter().enumerate() {
        let (id, people) = &rooms[i];
        for (voter, target) in *votes {
            let (s, b) = vote(&app, &people[*voter].0, *id, people[*target].1).await;
            assert_eq!(s, StatusCode::OK, "{b}");
        }
        let g = get(&app, &people[0].0, *id).await;
        assert_eq!(g["global"]["rooms"], i + 1);
        assert_eq!(g["global"]["min_rooms"], 5);
        if i < 4 {
            assert_eq!(
                g["global"]["facts"],
                Value::Null,
                "hidden below 5 rooms: {g}"
            );
        }
    }

    let (r2, p2) = &rooms[1];
    let g = get(&app, &p2[2].0, *r2).await;
    assert_eq!(g["my_vote"], Value::Null);
    assert_eq!(g["global"]["rooms"], 5);
    assert_eq!(g["global"]["voters"], 14);
    assert_eq!(g["global"]["facts"], Value::Null);

    let g = get(&app, &p2[0].0, *r2).await;
    let facts: std::collections::HashMap<String, i64> = g["global"]["facts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["id"].as_str().unwrap().to_owned(),
                f["pct"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(facts["unanimous"], 40);
    assert_eq!(facts["owner"], 20);
    assert_eq!(facts["newest"], 40);
    assert_eq!(facts["self"], 21);
    assert_eq!(facts["top_share"], 80);
    assert!(!facts.contains_key("last_winner"));
    let pcts: Vec<i64> = g["global"]["facts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["pct"].as_i64().unwrap())
        .collect();
    assert!(pcts.windows(2).all(|w| w[0] >= w[1]), "{pcts:?}");

    let text = g.to_string();
    for n in names
        .iter()
        .flatten()
        .chain(["Prosper", "Quentin", "Romeo", "Sacha", "Tess", "Uma"].iter())
    {
        assert!(!text.contains(n), "{n} in {text}");
    }
    for key in ["group_id", "room_id", "voter_id", "name"] {
        assert!(!text.contains(key), "{key} in {text}");
    }
    let mine: Vec<i64> = p2.iter().map(|(_, id)| *id).collect();
    for c in g["room"]["tally"].as_array().unwrap() {
        assert!(mine.contains(&c["target_id"].as_i64().unwrap()));
    }
    assert_eq!(
        g["room"]["tally"],
        json!([{ "target_id": p2[2].1, "count": 2 }])
    );

    let g = get(&app, &e[0].0, en).await;
    assert_eq!(g["global"]["rooms"], 1);
    assert_eq!(g["global"]["facts"], Value::Null);

    let today = global::today(&state.db).await.unwrap();
    let tomorrow = shift(&state, &today, 1).await;
    let me = member(&state, p2[0].1).await;
    let next = global::view_at(&state, &me, &tomorrow).await.unwrap();
    assert_eq!(next["my_vote"], Value::Null);
    assert_eq!(next["global"]["rooms"], 0);
    assert_ne!(next["question"], g["question"].clone());
    let y = &next["yesterday"];
    assert_eq!(y["day"], today);
    assert_eq!(y["rooms"], 5);
    assert_eq!(y["tally"], json!([{ "target_id": p2[2].1, "count": 2 }]));
    assert_eq!(y["facts"].as_array().map(Vec::len), Some(5));

    let (r1, p1) = &rooms[0];
    sqlx::query(
        "UPDATE members SET removed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(p1[0].1)
    .execute(&state.db)
    .await
    .unwrap();
    let g = get(&app, &p1[1].0, *r1).await;
    assert_eq!(
        (g["available"].as_bool(), g["reason"].as_str()),
        (Some(false), Some("small_room"))
    );
    let g = get(&app, &p2[0].0, *r2).await;
    assert_eq!(g["global"]["rooms"], 4);
    assert_eq!(g["global"]["facts"], Value::Null);
}

#[tokio::test]
async fn last_winner_fact_counts_rooms_with_a_closed_question() {
    let (app, state) = setup().await;
    let mut rooms = Vec::new();
    for i in 0..5 {
        let names = [format!("P{i}a"), format!("P{i}b"), format!("P{i}c")];
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        rooms.push(room(&app, &state, "fr", &names).await);
    }
    for (i, (id, people)) in rooms.iter().enumerate() {
        let round: i64 =
            sqlx::query_scalar("SELECT id FROM rounds WHERE group_id = ? AND closed_at IS NULL")
                .bind(id)
                .fetch_one(&state.db)
                .await
                .unwrap();
        for (_, voter) in people {
            sqlx::query("INSERT INTO votes (round_id, voter_id, target_id) VALUES (?, ?, ?)")
                .bind(round)
                .bind(voter)
                .bind(people[2].1)
                .execute(&state.db)
                .await
                .unwrap();
        }
        sqlx::query(
            "UPDATE rounds SET closed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
        )
        .bind(round)
        .execute(&state.db)
        .await
        .unwrap();
        let target = if i < 3 { people[2].1 } else { people[0].1 };
        for (token, _) in &people[..2] {
            let (s, b) = vote(&app, token, *id, target).await;
            assert_eq!(s, StatusCode::OK, "{b}");
        }
    }
    let (id, people) = &rooms[0];
    let g = get(&app, &people[0].0, *id).await;
    let last = g["global"]["facts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"] == "last_winner")
        .cloned();
    assert_eq!(last.map(|f| f["pct"].clone()), Some(json!(60)), "{g}");
}
