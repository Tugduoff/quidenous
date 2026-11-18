//! « Qui a dit ça ? » questions (docs/SAID.md): writing, guessing, the reveal, and everything
//! around them (older apps, small rooms, skips, leaves, reports, history, parties).

mod common;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use quidenous_server::{AppState, app, config::Config, flow, party, state};
use serde_json::{Value, json};
use tower::ServiceExt;

/// A fresh server, its deck reduced to pick questions; account 1 (the first to join) is an admin.
async fn setup() -> (Router, AppState) {
    let config = Config {
        admin_user_ids: vec![1],
        ..common::config(&common::temp_dir("qdn-said"))
    };
    let state = state(config).await.unwrap();
    sqlx::query("DELETE FROM questions WHERE group_id IS NULL AND kind <> 'pick'")
        .execute(&state.db)
        .await
        .unwrap();
    (app(state.clone()), state)
}

/// One call; `kinds` = the `x-kinds` header (`None`: an app from before choose questions).
async fn call_with(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    kinds: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(format!("/api{path}"));
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    if let Some(k) = kinds {
        req = req.header("x-kinds", k);
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
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// A call from a new app (`x-kinds: choose,said`).
async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    call_with(app, method, path, token, Some("choose,said"), body).await
}

async fn current(app: &Router, token: &str) -> Value {
    let (s, round) = call(app, "GET", "/round/current", Some(token), None).await;
    assert_eq!(s, StatusCode::OK, "{round}");
    round
}

async fn write(app: &Router, token: &str, text: &str) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        "/round/current/answer",
        Some(token),
        Some(json!({ "text": text })),
    )
    .await
}

async fn vote(app: &Router, token: &str, body: Value) -> (StatusCode, Value) {
    call(app, "POST", "/round/current/vote", Some(token), Some(body)).await
}

/// `{guesses}` from (answer, member) pairs.
fn guesses(pairs: &[(i64, i64)]) -> Value {
    json!({ "guesses": pairs.iter().map(|(a, m)| json!({ "answer_id": a, "member_id": m })).collect::<Vec<_>>() })
}

fn err(r: &(StatusCode, Value)) -> (StatusCode, Option<&str>) {
    (r.0, r.1["error"].as_str())
}

async fn add_said(app: &Router, token: &str, text: &str) {
    let (s, q) = call(
        app,
        "POST",
        "/questions",
        Some(token),
        Some(json!({ "text": text, "kind": "said", "anonymous": true })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{q}");
    assert_eq!(q["kind"], "said");
    assert_eq!(q["anonymous"], false, "never anonymous");
}

/// Everyone picks `target` on the open (pick) question, which closes it.
async fn close_pick(app: &Router, tokens: &[&str], target: i64) {
    for t in tokens {
        let (s, b) = vote(app, t, json!({ "target_id": target })).await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
}

/// `n` people in the env room, the first (automatic, pick) question closed and a said question open.
async fn room_with_said(app: &Router, names: &[&str]) -> Vec<(String, i64)> {
    let mut people = Vec::new();
    for name in names {
        people.push(common::join(app, name).await);
    }
    add_said(app, &people[0].0, "Le pire cadeau qu'on t'ait fait ?").await;
    let tokens: Vec<&str> = people.iter().map(|p| p.0.as_str()).collect();
    close_pick(app, &tokens, people[0].1).await;
    let open = current(app, &people[0].0).await;
    assert_eq!(open["kind"], "said", "{open}");
    people
}

/// The answer ids of the open (or given) round, by author: read from the database (the API never says).
async fn answer_of(state: &AppState, member: i64) -> i64 {
    sqlx::query_scalar(
        "SELECT id FROM said_answers WHERE member_id = ? ORDER BY round_id DESC LIMIT 1",
    )
    .bind(member)
    .fetch_one(&state.db)
    .await
    .unwrap()
}

#[tokio::test]
async fn features_list_said() {
    let (app, _) = setup().await;
    let (_, f) = call(&app, "GET", "/features", None, None).await;
    assert!(f.as_array().unwrap().contains(&json!("said")), "{f}");
}

#[tokio::test]
async fn write_guess_and_reveal() {
    let (app, state) = setup().await;
    let people = room_with_said(&app, &["Ana", "Bob", "Cyd"]).await;
    let [(a, a_id), (b, b_id), (c, c_id)] =
        [people[0].clone(), people[1].clone(), people[2].clone()];

    let open = current(&app, &a).await;
    let round = open["id"].as_i64().unwrap();
    assert_eq!(open["question"], "Le pire cadeau qu'on t'ait fait ?");
    assert_eq!(open["anonymous"], false);
    assert!(open.get("tripot").is_none(), "no bets on a said question");
    let said = &open["said"];
    assert_eq!(said["phase"], "writing");
    assert!(said["guess_at"].is_null());
    assert_eq!(said["writer_ids"], json!([]));
    assert!(said["my_answer"].is_null() && said["answers"].is_null() && said["result"].is_null());

    // 1 to 140 characters once cleaned.
    for bad in ["", "  \u{200B} ", &"x".repeat(141)] {
        assert_eq!(
            err(&write(&app, &a, bad).await),
            (StatusCode::BAD_REQUEST, Some("bad_answer")),
            "{bad:?}"
        );
    }
    let (s, view) = write(&app, &a, "  Un pull\nqui gratte ").await;
    assert_eq!(s, StatusCode::OK, "{view}");
    assert_eq!(view["said"]["my_answer"]["text"], "Un pull qui gratte");
    let first_id = view["said"]["my_answer"]["id"].as_i64().unwrap();
    // Rewriting replaces it (same answer).
    let (_, view) = write(&app, &a, "Un pull qui gratte, tricoté main").await;
    assert_eq!(view["said"]["my_answer"]["id"], first_id);
    assert_eq!(
        view["said"]["my_answer"]["text"],
        "Un pull qui gratte, tricoté main"
    );
    assert_eq!(view["said"]["writer_ids"], json!([a_id]));

    // Bob sees who wrote, never what.
    let bob_view = current(&app, &b).await;
    assert_eq!(bob_view["said"]["writer_ids"], json!([a_id]));
    assert!(bob_view["said"]["my_answer"].is_null());
    assert!(!bob_view.to_string().contains("pull"), "{bob_view}");
    // Guesses wait for the guessing; a vote needs them.
    assert_eq!(
        err(&vote(&app, &b, guesses(&[(first_id, a_id)])).await),
        (StatusCode::CONFLICT, Some("not_guessing"))
    );
    assert_eq!(
        err(&vote(&app, &b, json!({})).await),
        (StatusCode::BAD_REQUEST, Some("guesses_needed"))
    );
    // Writing counts as done: no skip or nudge on Ana.
    let (s, _) = call(
        &app,
        "PUT",
        &format!("/round/current/skip/{a_id}"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (s, _) = call(
        &app,
        "POST",
        "/round/current/nudge",
        Some(&b),
        Some(json!({ "member_id": a_id })),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (s, body) = call(
        &app,
        "POST",
        "/round/current/nudge",
        Some(&a),
        Some(json!({ "member_id": b_id })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");

    write(&app, &b, "Une boule à neige de Dunkerque").await;
    let (_, view) = write(&app, &c, "Un abonnement à la salle").await;
    // Everyone wrote: the guessing starts.
    let said = &view["said"];
    assert_eq!(said["phase"], "guessing", "{view}");
    assert!(said["guess_at"].is_string());
    assert_eq!(said["writer_ids"], json!([a_id, b_id, c_id]));
    let answers = said["answers"].as_array().unwrap();
    assert_eq!(answers.len(), 3);
    let ids: Vec<i64> = answers.iter().map(|x| x["id"].as_i64().unwrap()).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "listed by id");
    assert!(
        ids.windows(2).any(|w| w[1] - w[0] > 1),
        "ids are not sequential: {ids:?}"
    );
    for x in answers {
        let mut keys: Vec<&String> = x.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(keys, ["hidden", "id", "text"], "no author before the end");
        assert_eq!(x["hidden"], false);
    }
    assert!(said["result"].is_null());
    // Frozen.
    assert_eq!(
        err(&write(&app, &a, "Autre chose").await),
        (StatusCode::CONFLICT, Some("not_writing"))
    );

    let (aa, ba, ca) = (
        answer_of(&state, a_id).await,
        answer_of(&state, b_id).await,
        answer_of(&state, c_id).await,
    );
    assert_eq!(aa, first_id);
    // One guess per answer but mine, writers other than me, nobody twice.
    for bad in [
        guesses(&[(ba, b_id)]),
        guesses(&[(ba, b_id), (ca, c_id), (aa, a_id)]),
        guesses(&[(ba, a_id), (ca, c_id)]),
        guesses(&[(ba, c_id), (ca, c_id)]),
        guesses(&[(ba, b_id), (ca, 9999)]),
        guesses(&[(ba, b_id), (ba, c_id)]),
        guesses(&[(ba, b_id), (12345, c_id)]),
    ] {
        assert_eq!(
            err(&vote(&app, &a, bad.clone()).await),
            (StatusCode::BAD_REQUEST, Some("bad_guesses")),
            "{bad}"
        );
    }
    assert_eq!(
        err(&vote(&app, &a, json!({})).await),
        (StatusCode::BAD_REQUEST, Some("guesses_needed"))
    );
    // Ana and Cyd: right on both. Bob: wrong on both.
    let (s, body) = vote(&app, &a, guesses(&[(ca, c_id), (ba, b_id)])).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let mine = current(&app, &a).await;
    assert_eq!(mine["my_vote"], a_id);
    assert_eq!(
        mine["said"]["my_guesses"].as_array().unwrap().len(),
        2,
        "{mine}"
    );
    assert!(mine["said"]["result"].is_null());
    assert_eq!(
        err(&vote(&app, &a, guesses(&[(ca, c_id), (ba, b_id)])).await),
        (StatusCode::CONFLICT, Some("already_voted"))
    );
    vote(&app, &b, guesses(&[(aa, c_id), (ca, a_id)])).await;
    vote(&app, &c, guesses(&[(aa, a_id), (ba, b_id)])).await;

    let (_, done) = call(&app, "GET", &format!("/rounds/{round}"), Some(&b), None).await;
    assert!(done["closed_at"].is_string(), "{done}");
    let said = &done["said"];
    assert_eq!(said["phase"], "done");
    let result = &said["result"];
    // Bob's answer: found by Ana and Cyd. Cyd's: found by Ana, missed by Bob. Ana's: found by
    // Cyd, missed by Bob.
    let reveal: Vec<(i64, i64, i64, i64)> = result["answers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| {
            (
                x["id"].as_i64().unwrap(),
                x["author_id"].as_i64().unwrap(),
                x["found"].as_i64().unwrap(),
                x["fooled"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(reveal[0], (ba, b_id, 2, 0));
    let mut rest = reveal[1..].to_vec();
    rest.sort_unstable();
    let mut expected = vec![(aa, a_id, 1, 1), (ca, c_id, 1, 1)];
    expected.sort_unstable();
    assert_eq!(rest, expected);
    // Ana 2 found + 1 fooled, Cyd 2 + 1, Bob 0 + 0.
    assert_eq!(
        result["scores"],
        json!([
            { "member_id": a_id, "points": 3, "found": 2, "fooled": 1 },
            { "member_id": c_id, "points": 3, "found": 2, "fooled": 1 },
            { "member_id": b_id, "points": 0, "found": 0, "fooled": 0 },
        ])
    );
    // Bob still gets his own answer and guesses.
    assert_eq!(said["my_answer"]["text"], "Une boule à neige de Dunkerque");
    assert_eq!(said["my_guesses"].as_array().unwrap().len(), 2);

    // History: the top scorers share the win.
    let (_, history) = call(&app, "GET", "/history", Some(&a), None).await;
    assert_eq!(history[0]["kind"], "said");
    assert_eq!(history[0]["winner_ids"], json!([a_id, c_id]));
    assert_eq!(history[0]["top_count"], 3);
    // Stats leave it out; the share card and the CSV export still work.
    let (s, _) = call(&app, "GET", "/stats", Some(&a), None).await;
    assert_eq!(s, StatusCode::OK);
    let (s, share) = call(
        &app,
        "POST",
        &format!("/rounds/{round}/share"),
        Some(&a),
        Some(json!({ "result": true })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{share}");
    let token = share["token"].as_str().unwrap();
    let req = Request::builder()
        .uri(format!("/s/{token}/card.png"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn answering_needs_an_open_said_question() {
    let (app, _) = setup().await;
    let (a, _) = common::join(&app, "Ana").await;
    assert_eq!(
        err(&write(&app, &a, "Bonjour").await),
        (StatusCode::CONFLICT, Some("no_open_round"))
    );
    common::join(&app, "Bob").await;
    assert_eq!(
        err(&write(&app, &a, "Bonjour").await),
        (StatusCode::CONFLICT, Some("not_said"))
    );
}

#[tokio::test]
async fn older_apps_are_asked_to_update_and_pass() {
    let (app, state) = setup().await;
    let people = room_with_said(&app, &["Ana", "Bob", "Cyd", "Dan"]).await;
    let [(a, a_id), (b, b_id), (c, c_id), (d, d_id)] = [
        people[0].clone(),
        people[1].clone(),
        people[2].clone(),
        people[3].clone(),
    ];
    for kinds in [None, Some("choose"), Some("rank, choose")] {
        let (_, old) = call_with(&app, "GET", "/round/current", Some(&d), kinds, None).await;
        assert_eq!(
            old["question"], "Mets l'app à jour pour répondre : Le pire cadeau qu'on t'ait fait ?",
            "{kinds:?}"
        );
    }
    let (_, new) = call_with(
        &app,
        "GET",
        "/round/current",
        Some(&d),
        Some("said , choose"),
        None,
    )
    .await;
    assert_eq!(new["question"], "Le pire cadeau qu'on t'ait fait ?");

    // Dan's old app picks someone while people write: a pass (written, nothing to guess).
    let (s, body) = call_with(
        &app,
        "POST",
        "/round/current/vote",
        Some(&d),
        None,
        Some(json!({ "target_id": a_id })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let view = current(&app, &a).await;
    assert_eq!(view["said"]["writer_ids"], json!([d_id]));
    write(&app, &a, "Des chaussettes").await;
    write(&app, &b, "Un livre déjà lu").await;
    let (_, view) = write(&app, &c, "Rien du tout").await;
    assert_eq!(view["said"]["phase"], "guessing", "{view}");
    // The pass is not an answer.
    assert_eq!(view["said"]["answers"].as_array().unwrap().len(), 3);
    // Dan already counts as voted: the others close it.
    let (aa, ba, ca) = (
        answer_of(&state, a_id).await,
        answer_of(&state, b_id).await,
        answer_of(&state, c_id).await,
    );
    vote(&app, &a, guesses(&[(ba, b_id), (ca, c_id)])).await;
    vote(&app, &b, guesses(&[(aa, a_id), (ca, d_id)])).await;
    let (s, body) = vote(&app, &c, guesses(&[(aa, b_id), (ba, a_id)])).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let (_, history) = call(&app, "GET", "/history", Some(&a), None).await;
    assert_eq!(history[0]["kind"], "said", "nobody waited for Dan");
    // Ana found both and fooled Cyd: 3. Bob found Ana and fooled Cyd: 2. Cyd fooled Bob: 1.
    assert_eq!(history[0]["winner_ids"], json!([a_id]));
    assert_eq!(history[0]["top_count"], 3);
}

#[tokio::test]
async fn an_old_app_votes_without_guesses_while_guessing() {
    let (app, state) = setup().await;
    let people = room_with_said(&app, &["Ana", "Bob", "Cyd"]).await;
    let [(a, a_id), (b, b_id), (c, _)] = [people[0].clone(), people[1].clone(), people[2].clone()];
    write(&app, &a, "Un mug").await;
    write(&app, &b, "Une plante").await;
    // Cyd's old app passes: everyone wrote, two answers: guessing.
    call_with(
        &app,
        "POST",
        "/round/current/vote",
        Some(&c),
        None,
        Some(json!({ "target_id": a_id })),
    )
    .await;
    let view = current(&app, &a).await;
    assert_eq!(view["said"]["phase"], "guessing", "{view}");
    let ba = answer_of(&state, b_id).await;
    vote(&app, &a, guesses(&[(ba, b_id)])).await;
    // Bob's other phone runs an old app: a vote without guesses.
    let (s, body) = call_with(
        &app,
        "POST",
        "/round/current/vote",
        Some(&b),
        None,
        Some(json!({ "target_id": a_id })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let (_, history) = call(&app, "GET", "/history", Some(&a), None).await;
    assert_eq!(history[0]["kind"], "said");
    // Ana found Bob; nobody guessed Ana's answer.
    assert_eq!(history[0]["winner_ids"], json!([a_id]));
}

#[tokio::test]
async fn too_few_answers_close_with_no_result() {
    let (app, _) = setup().await;
    let people = room_with_said(&app, &["Ana", "Bob", "Cyd"]).await;
    let [(a, a_id), (b, _), (c, _)] = [people[0].clone(), people[1].clone(), people[2].clone()];
    let round = current(&app, &a).await["id"].as_i64().unwrap();
    write(&app, &a, "Un mug").await;
    for t in [&b, &c] {
        call_with(
            &app,
            "POST",
            "/round/current/vote",
            Some(t),
            None,
            Some(json!({ "target_id": a_id })),
        )
        .await;
    }
    let (_, done) = call(&app, "GET", &format!("/rounds/{round}"), Some(&a), None).await;
    assert!(done["closed_at"].is_string(), "{done}");
    let said = &done["said"];
    assert_eq!(said["phase"], "done");
    assert!(said["guess_at"].is_null() && said["answers"].is_null() && said["result"].is_null());
    let (_, history) = call(&app, "GET", "/history", Some(&a), None).await;
    assert_eq!(history[0]["winner_ids"], json!([]));
}

#[tokio::test]
async fn a_small_room_waits_with_its_said_question() {
    let (app, _) = setup().await;
    let (a, a_id) = common::join(&app, "Ana").await;
    let (b, _) = common::join(&app, "Bob").await;
    add_said(&app, &a, "Ta chanson honteuse préférée ?").await;
    call(
        &app,
        "POST",
        "/questions",
        Some(&a),
        Some(json!({ "text": "Qui chante le plus faux ?" })),
    )
    .await;
    close_pick(&app, &[&a, &b], a_id).await;
    // Two people: the pick question goes first.
    let open = current(&app, &a).await;
    assert_eq!(open["kind"], "pick");
    assert_eq!(open["question"], "Qui chante le plus faux ?");
    assert!(open["said"].is_null());
    let (c, _) = common::join(&app, "Cyd").await;
    close_pick(&app, &[&a, &b, &c], a_id).await;
    let open = current(&app, &a).await;
    assert_eq!(open["kind"], "said", "three now: {open}");
}

#[tokio::test]
async fn skips_and_the_late_quorum_end_the_writing() {
    let (app, state) = setup().await;
    let people = room_with_said(&app, &["Ana", "Bob", "Cyd", "Dan"]).await;
    let [(a, a_id), (b, b_id), (c, c_id), (d, _)] = [
        people[0].clone(),
        people[1].clone(),
        people[2].clone(),
        people[3].clone(),
    ];
    let round = current(&app, &a).await["id"].as_i64().unwrap();
    write(&app, &a, "Un mug").await;
    write(&app, &b, "Une plante").await;
    // Two of four, a day later: half is enough.
    sqlx::query("UPDATE rounds SET opened_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-25 hours') WHERE id = ?")
        .bind(round)
        .execute(&state.db)
        .await
        .unwrap();
    assert_eq!(
        flow::close_late(&state).await.unwrap(),
        0,
        "moved on, not closed"
    );
    let view = current(&app, &a).await;
    assert_eq!(view["said"]["phase"], "guessing", "{view}");
    // The guessing's own clock: Ana and Bob guessed, not late yet.
    let (aa, ba) = (answer_of(&state, a_id).await, answer_of(&state, b_id).await);
    vote(&app, &a, guesses(&[(ba, b_id)])).await;
    vote(&app, &b, guesses(&[(aa, a_id)])).await;
    flow::close_late(&state).await.unwrap();
    assert!(current(&app, &a).await["closed_at"].is_null());
    // Cyd and Dan never wrote: they can still guess (any writer).
    let view = current(&app, &c).await;
    assert!(view["said"]["my_answer"].is_null());
    sqlx::query("UPDATE rounds SET said_guess_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-25 hours') WHERE id = ?")
        .bind(round)
        .execute(&state.db)
        .await
        .unwrap();
    assert_eq!(flow::close_late(&state).await.unwrap(), 1);
    let (_, done) = call(&app, "GET", &format!("/rounds/{round}"), Some(&a), None).await;
    assert!(done["closed_at"].is_string());

    // Next said question: Cyd and Dan wrote, the room skips Ana and Bob.
    add_said(&app, &a, "Ton plat de flemme ?").await;
    close_pick(&app, &[&a, &b, &c, &d], a_id).await;
    let round = current(&app, &a).await;
    assert_eq!(round["kind"], "said", "{round}");
    write(&app, &c, "Des pâtes").await;
    write(&app, &d, "Un bol de céréales").await;
    // Skipping someone who wrote: no.
    let (s, _) = call(
        &app,
        "PUT",
        &format!("/round/current/skip/{c_id}"),
        Some(&a),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    for (from, target) in [(&c, a_id), (&d, a_id), (&c, b_id)] {
        let (s, body) = call(
            &app,
            "PUT",
            &format!("/round/current/skip/{target}"),
            Some(from),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{body}");
    }
    assert_eq!(
        current(&app, &a).await["said"]["phase"],
        "writing",
        "Bob still waited for"
    );
    let (s, body) = call(
        &app,
        "PUT",
        &format!("/round/current/skip/{b_id}"),
        Some(&d),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert_eq!(current(&app, &a).await["said"]["phase"], "guessing");
}

#[tokio::test]
async fn leaving_takes_the_answer_and_its_guesses() {
    let (app, state) = setup().await;
    let people = room_with_said(&app, &["Ana", "Bob", "Cyd", "Dan"]).await;
    let ids: Vec<i64> = people.iter().map(|p| p.1).collect();
    for (p, text) in people
        .iter()
        .zip(["Un mug", "Une plante", "Un pull", "Rien"])
    {
        write(&app, &p.0, text).await;
    }
    let mut answers = Vec::new();
    for id in &ids {
        answers.push(answer_of(&state, *id).await);
    }
    // Dan guesses (Ana's answer among them), Ana guesses too, then Ana leaves.
    vote(
        &app,
        &people[3].0,
        guesses(&[
            (answers[0], ids[0]),
            (answers[1], ids[1]),
            (answers[2], ids[2]),
        ]),
    )
    .await;
    vote(
        &app,
        &people[0].0,
        guesses(&[
            (answers[1], ids[1]),
            (answers[2], ids[2]),
            (answers[3], ids[3]),
        ]),
    )
    .await;
    let room = common::room_id(&app, &people[0].0).await;
    let (s, body) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/leave"),
        Some(&people[0].0),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let view = current(&app, &people[1].0).await;
    assert_eq!(view["said"]["writer_ids"], json!([ids[1], ids[2], ids[3]]));
    assert_eq!(view["said"]["answers"].as_array().unwrap().len(), 3);
    let left: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT voter_id, answer_id FROM said_guesses WHERE answer_id = ? OR voter_id = ?",
    )
    .bind(answers[0])
    .bind(ids[0])
    .fetch_all(&state.db)
    .await
    .unwrap();
    assert!(left.is_empty(), "{left:?}");
    // Dan's other guesses stay; his vote still counts.
    let dan = current(&app, &people[3].0).await;
    assert_eq!(dan["said"]["my_guesses"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn reported_answers_hide_after_three_until_an_admin_decides() {
    let (app, state) = setup().await;
    let people = room_with_said(&app, &["Ana", "Bob", "Cyd", "Dan"]).await;
    common::trust_all(&state).await;
    let report = |token: String, id: i64| {
        let app = app.clone();
        async move {
            call(
                &app,
                "POST",
                "/reports",
                Some(&token),
                Some(json!({ "kind": "answer", "id": id, "reason": "offensive" })),
            )
            .await
        }
    };
    write(&app, &people[1].0, "Un truc vraiment méchant").await;
    let rude = answer_of(&state, people[1].1).await;
    // Nothing shown yet: nothing to report.
    assert_eq!(
        report(people[0].0.clone(), rude).await.0,
        StatusCode::NOT_FOUND
    );
    for p in [&people[0], &people[2], &people[3]] {
        write(&app, &p.0, "Une réponse").await;
    }
    assert_eq!(
        current(&app, &people[0].0).await["said"]["phase"],
        "guessing"
    );
    // Not my own.
    assert_eq!(
        err(&report(people[1].0.clone(), rude).await),
        (StatusCode::BAD_REQUEST, Some("bad_target"))
    );
    for p in [&people[0], &people[2], &people[3]] {
        let (s, body) = report(p.0.clone(), rude).await;
        assert_eq!(s, StatusCode::OK, "{body}");
    }
    let shown = |v: &Value, id: i64| -> Value {
        v["said"]["answers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["id"] == id)
            .cloned()
            .unwrap()
    };
    let view = current(&app, &people[2].0).await;
    assert_eq!(
        shown(&view, rude),
        json!({ "id": rude, "text": null, "hidden": true })
    );
    // Still in the game: it is guessed like the others.
    let (_, list) = call(&app, "GET", "/admin/review", Some(&people[0].0), None).await;
    let r = list["reports"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["kind"] == "answer")
        .unwrap()
        .clone();
    assert_eq!(r["count"], 3);
    assert_eq!(r["content"], "Un truc vraiment méchant");
    assert_eq!(r["hidden"], true);
    assert_eq!(r["person"]["name"], "Bob");
    let act = |action: &'static str, id: i64| {
        let app = app.clone();
        let admin = people[0].0.clone();
        async move {
            call(
                &app,
                "POST",
                "/admin/reports",
                Some(&admin),
                Some(json!({ "kind": "answer", "id": id, "action": action })),
            )
            .await
        }
    };
    let (s, body) = act("dismiss", rude).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let view = current(&app, &people[2].0).await;
    assert_eq!(shown(&view, rude)["text"], "Un truc vraiment méchant");
    // Another answer, reported once and deleted by the admin: blanked for good.
    let other = answer_of(&state, people[2].1).await;
    report(people[3].0.clone(), other).await;
    let (s, body) = act("delete", other).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let view = current(&app, &people[0].0).await;
    assert_eq!(
        shown(&view, other),
        json!({ "id": other, "text": null, "hidden": true })
    );
    let text: Option<String> = sqlx::query_scalar("SELECT text FROM said_answers WHERE id = ?")
        .bind(other)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(text.as_deref(), Some(""));
    // Nothing left to report on it.
    assert_eq!(
        report(people[0].0.clone(), other).await.0,
        StatusCode::NOT_FOUND
    );
}

/// The party API, with its own helpers.
mod parties {
    use super::*;

    async fn post(
        app: &Router,
        id: i64,
        what: &str,
        token: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        call(
            app,
            "POST",
            &format!("/parties/{id}/{what}"),
            Some(token),
            body,
        )
        .await
    }

    async fn get(app: &Router, id: i64, token: &str) -> Value {
        let (s, body) = call(app, "GET", &format!("/parties/{id}"), Some(token), None).await;
        assert_eq!(s, StatusCode::OK, "{body}");
        body
    }

    async fn at(state: &AppState, offset: &str) -> String {
        sqlx::query_scalar("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?)")
            .bind(offset)
            .fetch_one(&state.db)
            .await
            .unwrap()
    }

    /// A started party whose deck holds only said questions: (id, code, [(token, member)] host first).
    async fn started(
        app: &Router,
        state: &AppState,
        guests: &[&str],
    ) -> (i64, String, Vec<(String, i64)>) {
        // `setup` left only pick questions: the said ones come back, the rest goes.
        quidenous_server::deck::seed(&state.db).await.unwrap();
        sqlx::query("DELETE FROM questions WHERE group_id IS NULL AND kind <> 'said'")
            .execute(&state.db)
            .await
            .unwrap();
        let (host, _) = common::join(app, "Hôte").await;
        let (s, p) = call(app, "POST", "/parties", Some(&host), Some(json!({}))).await;
        assert_eq!(s, StatusCode::OK, "{p}");
        let (id, code) = (
            p["id"].as_i64().unwrap(),
            p["code"].as_str().unwrap().to_owned(),
        );
        let mut players = Vec::new();
        for name in guests {
            let (s, g) = call(
                app,
                "POST",
                "/parties/join",
                None,
                Some(json!({ "code": code, "name": name, "color": "#5BC8FF" })),
            )
            .await;
            assert_eq!(s, StatusCode::OK, "{g}");
            players.push((
                g["token"].as_str().unwrap().to_owned(),
                g["member_id"].as_i64().unwrap(),
            ));
        }
        let (s, body) = post(app, id, "start", &host, None).await;
        assert_eq!(s, StatusCode::OK, "{body}");
        let host_id = body["me"]["member_id"].as_i64().unwrap();
        players.insert(0, (host, host_id));
        (id, code, players)
    }

    #[tokio::test]
    async fn a_party_writes_guesses_and_reveals_on_its_deadlines() {
        let (app, state) = setup().await;
        let (id, code, p) = started(&app, &state, &["Léa", "Max"]).await;
        let s = get(&app, id, &p[0].0).await;
        let round = &s["round"];
        assert_eq!(round["kind"], "said", "{s}");
        assert_eq!(round["said"]["phase"], "writing");
        let round_id = round["id"].as_i64().unwrap();
        // Writing: twice the 30 s timer.
        let secs: f64 = sqlx::query_scalar("SELECT (julianday(deadline_at) - julianday(opened_at)) * 86400 FROM rounds WHERE id = ?")
            .bind(round_id)
            .fetch_one(&state.db)
            .await
            .unwrap();
        assert!((secs - 60.0).abs() < 0.5, "{secs}");

        let (s, body) = post(
            &app,
            id,
            "answer",
            &p[0].0,
            Some(json!({ "text": "Un mug" })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{body}");
        assert_eq!(body["round"]["said"]["my_answer"]["text"], "Un mug");
        post(
            &app,
            id,
            "answer",
            &p[1].0,
            Some(json!({ "text": "Une plante" })),
        )
        .await;
        assert_eq!(
            get(&app, id, &p[2].0).await["round"]["said"]["phase"],
            "writing"
        );
        // Not yet.
        assert_eq!(
            party::sweep_at(&state, &at(&state, "+30 seconds").await)
                .await
                .unwrap()
                .closed,
            0
        );
        assert_eq!(
            get(&app, id, &p[2].0).await["round"]["said"]["phase"],
            "writing"
        );
        // The writing deadline with two answers: the guessing starts, with its own deadline.
        let later = at(&state, "+61 seconds").await;
        assert_eq!(party::sweep_at(&state, &later).await.unwrap().closed, 0);
        let s = get(&app, id, &p[2].0).await;
        let said = &s["round"]["said"];
        assert_eq!(said["phase"], "guessing", "{s}");
        assert_eq!(said["answers"].as_array().unwrap().len(), 2);
        assert!(s["round"]["deadline_at"].as_str().unwrap() > later.as_str());
        assert_eq!(
            err(&post(
                &app,
                id,
                "answer",
                &p[2].0,
                Some(json!({ "text": "Trop tard" }))
            )
            .await),
            (StatusCode::CONFLICT, Some("not_writing"))
        );

        let (a0, a1) = (
            answer_of(&state, p[0].1).await,
            answer_of(&state, p[1].1).await,
        );
        let (s, body) = post(&app, id, "vote", &p[0].0, Some(guesses(&[(a1, p[1].1)]))).await;
        assert_eq!(s, StatusCode::OK, "{body}");
        post(&app, id, "vote", &p[1].0, Some(guesses(&[(a0, p[0].1)]))).await;
        // Max wrote nothing: he guesses both.
        let (s, body) = post(
            &app,
            id,
            "vote",
            &p[2].0,
            Some(guesses(&[(a0, p[1].1), (a1, p[0].1)])),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{body}");
        assert_eq!(body["phase"], "reveal", "{body}");
        let result = &body["round"]["said"]["result"];
        // Hôte and Léa found each other and fooled Max: 2 each.
        assert_eq!(result["scores"][0]["points"], 2, "{result}");
        assert_eq!(result["scores"][1]["points"], 2, "{result}");

        // The big screen: the reveal, no `my_*`.
        let (s, screen) = call(&app, "GET", &format!("/parties/screen/{code}"), None, None).await;
        assert_eq!(s, StatusCode::OK, "{screen}");
        let said = &screen["round"]["said"];
        assert_eq!(said["phase"], "done");
        assert!(said["my_answer"].is_null() && said["my_guesses"].is_null());
        assert_eq!(said["result"]["answers"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn too_few_answers_at_the_writing_deadline_close_it() {
        let (app, state) = setup().await;
        let (id, _, p) = started(&app, &state, &["Léa", "Max"]).await;
        post(
            &app,
            id,
            "answer",
            &p[0].0,
            Some(json!({ "text": "Un mug" })),
        )
        .await;
        let swept = party::sweep_at(&state, &at(&state, "+61 seconds").await)
            .await
            .unwrap();
        assert_eq!(swept.closed, 1);
        let s = get(&app, id, &p[1].0).await;
        assert_eq!(s["phase"], "reveal");
        assert!(s["round"]["said"]["result"].is_null());
    }

    #[tokio::test]
    async fn two_players_never_get_a_said_question() {
        let (app, state) = setup().await;
        let (id, _, p) = started(&app, &state, &["Léa"]).await;
        let s = get(&app, id, &p[0].0).await;
        assert!(s["round"].is_null(), "{s}");
    }
}

/// Today (docs/TODAY.md): writing counts as voted while the question is in its writing phase.
#[tokio::test]
async fn today_counts_a_written_answer() {
    let (app, _state) = setup().await;
    let people = room_with_said(&app, &["Ana", "Bob", "Cyd"]).await;
    let room = |b: &Value| b["rooms"][0].clone();
    let (s, before) = call(&app, "GET", "/today", Some(&people[0].0), None).await;
    assert_eq!(s, StatusCode::OK, "{before}");
    assert_eq!(room(&before)["voted"], false, "{before}");
    assert_eq!(room(&before)["votes"], 0, "{before}");
    let w = write(&app, &people[0].0, "Une plante en plastique").await;
    assert_eq!(w.0, StatusCode::OK, "{:?}", w.1);
    let (_, after) = call(&app, "GET", "/today", Some(&people[0].0), None).await;
    assert_eq!(room(&after)["voted"], true, "{after}");
    assert_eq!(room(&after)["votes"], 1, "{after}");
    let (_, other) = call(&app, "GET", "/today", Some(&people[1].0), None).await;
    assert_eq!(room(&other)["voted"], false, "{other}");
}
