//! Le Tripot (docs/TRIPOT.md): bets on a question's winner with chips worth nothing.

mod common;

use axum::{Router, http::StatusCode};
use common::{call, join, room_id, setup};
use quidenous_server::{AppState, tripot};
use serde_json::{Value, json};

struct Room {
    app: Router,
    state: AppState,
    id: i64,
    /// Ana is the owner (first in), then Bob, Cyd (and Dan when asked for).
    people: Vec<(String, i64)>,
}

impl Room {
    async fn new(n: usize) -> Self {
        let (app, state) = setup().await;
        let mut people = Vec::new();
        for name in ["Ana", "Bob", "Cyd", "Dan"].into_iter().take(n) {
            people.push(join(&app, name).await);
        }
        let id = room_id(&app, &people[0].0).await;
        Self {
            app,
            state,
            id,
            people,
        }
    }

    fn tok(&self, i: usize) -> &str {
        &self.people[i].0
    }

    fn mid(&self, i: usize) -> i64 {
        self.people[i].1
    }

    async fn get(&self, i: usize, path: &str) -> Value {
        let (s, b) = call(&self.app, "GET", path, Some(self.tok(i)), None, None).await;
        assert_eq!(s, StatusCode::OK, "{path}: {b}");
        b
    }

    async fn current(&self, i: usize) -> Value {
        self.get(i, "/round/current").await
    }

    async fn round(&self, i: usize, id: i64) -> Value {
        self.get(i, &format!("/rounds/{id}")).await
    }

    async fn open_id(&self) -> i64 {
        self.current(0).await["id"].as_i64().unwrap()
    }

    async fn vote(&self, i: usize, body: Value) {
        let (s, b) = call(
            &self.app,
            "POST",
            "/round/current/vote",
            Some(self.tok(i)),
            None,
            Some(body),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }

    /// Everyone votes on the open pick question: person i for `targets[i]` (an index).
    async fn close_with(&self, targets: &[usize]) -> i64 {
        let id = self.open_id().await;
        for (i, t) in targets.iter().enumerate() {
            self.vote(i, json!({ "target_id": self.mid(*t) })).await;
        }
        assert!(self.round(0, id).await["closed_at"].is_string());
        id
    }

    async fn bet(&self, i: usize, round: i64, target: i64, amount: i64) -> (StatusCode, Value) {
        call(
            &self.app,
            "POST",
            &format!("/rounds/{round}/bet"),
            Some(self.tok(i)),
            None,
            Some(json!({ "target": target, "amount": amount })),
        )
        .await
    }

    async fn bet_ok(&self, i: usize, round: i64, target: usize, amount: i64) -> Value {
        let (s, b) = self.bet(i, round, self.mid(target), amount).await;
        assert_eq!(s, StatusCode::OK, "{b}");
        b
    }

    async fn settings(&self, body: Value) -> (StatusCode, Value) {
        call(
            &self.app,
            "PATCH",
            &format!("/rooms/{}", self.id),
            Some(self.tok(0)),
            None,
            Some(body),
        )
        .await
    }

    async fn add(&self, body: Value) {
        let (s, b) = call(
            &self.app,
            "POST",
            "/questions",
            Some(self.tok(0)),
            None,
            Some(body),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }

    async fn balance(&self, i: usize) -> i64 {
        tripot::balance(&self.state.db, self.mid(i), None)
            .await
            .unwrap()
    }

    async fn sql(&self, sql: &'static str, member: i64) {
        sqlx::query(sql)
            .bind(member)
            .execute(&self.state.db)
            .await
            .unwrap();
    }

    /// Credits someone this month (as if they had won), to set a balance up.
    async fn credit(&self, i: usize, amount: i64) {
        sqlx::query(
            "INSERT INTO tripot_ledger (group_id, member_id, month, kind, amount) \
             VALUES (?, ?, strftime('%Y-%m', 'now'), 'payout', ?)",
        )
        .bind(self.id)
        .bind(self.mid(i))
        .bind(amount)
        .execute(&self.state.db)
        .await
        .unwrap();
    }

    async fn ledger_rows(&self) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM tripot_ledger")
            .fetch_one(&self.state.db)
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn everyone_starts_the_month_with_100() {
    let room = Room::new(3).await;
    let features = room.get(0, "/features").await;
    assert!(features.as_array().unwrap().contains(&json!("tripot")));
    let group = room.get(1, "/group").await;
    assert_eq!(group["settings"]["tripot"], true);

    let open = room.current(1).await;
    let t = &open["tripot"];
    assert_eq!(t["balance"], 100, "{open}");
    assert_eq!(t["pot"], 0);
    assert_eq!(t["bettors"], 0);
    assert_eq!(t["can_bet"], true);
    assert!(t["my_bet"].is_null());

    let board = room.get(2, "/tripot").await;
    assert_eq!(board["start"], 100);
    let balances = board["balances"].as_array().unwrap();
    assert_eq!(balances.len(), 3);
    assert!(balances.iter().all(|b| b["balance"] == 100), "{board}");
    assert_eq!(board["podium"], json!([]));
}

#[tokio::test]
async fn the_daily_bonus_comes_once_a_day_up_to_the_cap() {
    let room = Room::new(3).await;
    room.close_with(&[1, 0, 0]).await;
    assert_eq!(room.balance(0).await, 120);
    assert_eq!(room.balance(1).await, 120);
    // A second vote the same day: nothing more.
    room.close_with(&[1, 0, 0]).await;
    assert_eq!(room.balance(0).await, 120);

    // The next day: another 20.
    room.sql(
        "UPDATE tripot_ledger SET day = date(day, '-1 day') WHERE member_id = ? AND kind = 'daily'",
        room.mid(0),
    )
    .await;
    room.close_with(&[1, 0, 0]).await;
    assert_eq!(room.balance(0).await, 140);

    // Near the cap: topped up to 500, never above; at the cap, the day still counts once.
    room.credit(0, 490 - 140).await;
    room.sql(
        "UPDATE tripot_ledger SET day = date(day, '-1 day') WHERE member_id = ? AND kind = 'daily'",
        room.mid(0),
    )
    .await;
    room.close_with(&[1, 0, 0]).await;
    assert_eq!(room.balance(0).await, tripot::CAP);
    room.sql(
        "UPDATE tripot_ledger SET day = date(day, '-1 day') WHERE member_id = ? AND kind = 'daily'",
        room.mid(0),
    )
    .await;
    room.close_with(&[1, 0, 0]).await;
    assert_eq!(room.balance(0).await, tripot::CAP);
}

#[tokio::test]
async fn the_month_resets_and_last_month_has_a_podium() {
    let room = Room::new(3).await;
    let r = room.open_id().await;
    room.bet_ok(0, r, 1, 50).await;
    room.close_with(&[1, 1, 1]).await;
    // Ana: 100 - 50 + 50 (alone on Bob: her own stake back as the whole pot) + 20 daily.
    assert_eq!(room.balance(0).await, 120);
    room.credit(2, 30).await;

    // A new month: everything so far was last month's.
    sqlx::query(
        "UPDATE tripot_ledger SET month = strftime('%Y-%m', 'now', 'start of month', '-1 month')",
    )
    .execute(&room.state.db)
    .await
    .unwrap();
    for i in 0..3 {
        assert_eq!(room.balance(i).await, 100);
    }
    let board = room.get(1, "/tripot").await;
    assert!(
        board["balances"]
            .as_array()
            .unwrap()
            .iter()
            .all(|b| b["balance"] == 100)
    );
    let podium = board["podium"].as_array().unwrap();
    assert_eq!(podium.len(), 3, "{board}");
    assert_eq!(podium[0]["member_id"], room.mid(2));
    assert_eq!(podium[0]["balance"], 150);
    assert_eq!(podium[1]["balance"], 120);
    assert_eq!(podium[2]["balance"], 120);
    assert_ne!(board["month"], board["last_month"]);
}

#[tokio::test]
async fn bets_are_checked() {
    let room = Room::new(3).await;
    let r = room.open_id().await;
    let bob = room.mid(1);
    for (amount, code) in [
        (0, "bad_amount"),
        (-5, "bad_amount"),
        (101, "not_enough_chips"),
    ] {
        let (s, b) = room.bet(0, r, bob, amount).await;
        assert_eq!(b["error"], code, "{amount}: {b}");
        assert!(s.is_client_error());
    }
    let (s, b) = room.bet(0, r, 999_999, 10).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("unknown_member"))
    );
    let (s, _) = room.bet(0, 999_999, bob, 10).await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // All in, on myself: fine. Once.
    let view = room.bet_ok(0, r, 0, 100).await;
    assert_eq!(view["my_bet"]["amount"], 100);
    assert_eq!(view["balance"], 0);
    assert_eq!(view["can_bet"], false);
    let (s, b) = room.bet(0, r, bob, 1).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("already_bet"))
    );

    // Closed: no more bets.
    room.close_with(&[1, 0, 0]).await;
    let (s, b) = room.bet(1, r, bob, 10).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("round_closed"))
    );

    // A choose question takes none (and shows none).
    room.add(
        json!({ "text": "Qui prendrait quoi ?", "kind": "choose", "items": ["Pizza", "Sushi"] }),
    )
    .await;
    room.close_with(&[1, 0, 0]).await;
    let open = room.current(1).await;
    assert_eq!(open["kind"], "choose");
    assert!(open.get("tripot").is_none(), "{open}");
    let (s, b) = room.bet(1, open["id"].as_i64().unwrap(), bob, 10).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("no_bets"))
    );
}

#[tokio::test]
async fn anonymous_questions_have_no_tripot() {
    let room = Room::new(3).await;
    room.add(json!({ "text": "Qui chante faux ?", "anonymous": true }))
        .await;
    room.close_with(&[1, 0, 0]).await;
    let open = room.current(1).await;
    assert_eq!(open["anonymous"], true);
    assert!(open.get("tripot").is_none(), "{open}");
    let (s, b) = room
        .bet(1, open["id"].as_i64().unwrap(), room.mid(0), 10)
        .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("no_bets"))
    );

    // A bet on an open question that the new room rule makes anonymous comes back.
    let id = room.close_with(&[1, 0, 0]).await;
    assert!(room.round(0, id).await.get("tripot").is_none());
    let r = room.open_id().await;
    room.bet_ok(1, r, 0, 40).await;
    let (s, _) = room.settings(json!({ "anonymous_votes": "always" })).await;
    assert_eq!(s, StatusCode::OK);
    assert!(room.current(1).await.get("tripot").is_none());
    assert_eq!(room.balance(1).await, 120);
}

#[tokio::test]
async fn parties_have_no_tripot() {
    let room = Room::new(3).await;
    let r = room.open_id().await;
    sqlx::query("UPDATE groups SET kind = 'party' WHERE id = ?")
        .bind(room.id)
        .execute(&room.state.db)
        .await
        .unwrap();
    let (s, b) = room.bet(0, r, room.mid(1), 10).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("tripot_off"))
    );
    assert!(room.current(0).await.get("tripot").is_none());
    let group = room.get(0, "/group").await;
    assert!(group["settings"].get("tripot").is_none(), "{group}");
    let (s, _) = room.settings(json!({ "tripot": true })).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[test]
fn the_pot_is_shared_in_proportion() {
    // Pot 65, 45 on winners: 43.3 / 14.4 / 7.2, the chip left goes to the largest stake.
    assert_eq!(
        tripot::split(&[(30, true), (20, false), (10, true), (5, true)]),
        Some(vec![44, 0, 14, 7])
    );
    // Same stakes: the earliest bet first.
    assert_eq!(
        tripot::split(&[(10, true), (10, true), (1, false)]),
        Some(vec![11, 10, 0])
    );
    // Three leftover chips over three winners.
    assert_eq!(
        tripot::split(&[(1, true), (1, true), (1, true), (1, false)]),
        Some(vec![2, 1, 1, 0])
    );
    assert_eq!(tripot::split(&[(10, false), (5, false)]), None);
    assert_eq!(tripot::split(&[(7, true)]), Some(vec![7]));
}

#[tokio::test]
async fn winners_share_the_pot() {
    let room = Room::new(3).await;
    let r = room.open_id().await;
    room.bet_ok(0, r, 1, 10).await; // Ana on Bob
    room.bet_ok(1, r, 2, 30).await; // Bob on Cyd
    room.bet_ok(2, r, 1, 15).await; // Cyd on Bob
    let open = room.current(0).await;
    assert_eq!(open["tripot"]["pot"], 55);
    assert_eq!(open["tripot"]["bettors"], 3);
    room.close_with(&[1, 1, 1]).await;
    // Pot 55, 25 on Bob: Ana 22, Cyd 33. Everyone also got the day's 20.
    assert_eq!(room.balance(0).await, 100 - 10 + 22 + 20);
    assert_eq!(room.balance(1).await, 100 - 30 + 20);
    assert_eq!(room.balance(2).await, 100 - 15 + 33 + 20);
    let closed = room.round(1, r).await;
    let t = &closed["tripot"];
    assert_eq!(t["pot"], 55);
    assert_eq!(t["refunded"], false);
    assert_eq!(t["can_bet"], false);
    assert_eq!(t["my_bet"]["payout"], 0);
    assert_eq!(
        t["winners"],
        json!([
            { "member_id": room.mid(2), "stake": 15, "won": 33 },
            { "member_id": room.mid(0), "stake": 10, "won": 22 },
        ])
    );

    // Settling again changes nothing.
    let rows = room.ledger_rows().await;
    let mut tx = room.state.db.begin().await.unwrap();
    tripot::settle(&mut tx, r).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(room.ledger_rows().await, rows);
}

#[tokio::test]
async fn a_tie_pays_bets_on_any_winner() {
    let room = Room::new(4).await;
    let r = room.open_id().await;
    room.bet_ok(0, r, 1, 10).await; // Ana on Bob
    room.bet_ok(1, r, 2, 20).await; // Bob on Cyd
    room.bet_ok(3, r, 0, 30).await; // Dan on Ana
    // Bob 2, Cyd 2.
    room.close_with(&[1, 2, 1, 2]).await;
    // Pot 60, 30 on the winners: Ana 20, Bob 40.
    assert_eq!(room.balance(0).await, 100 - 10 + 20 + 20);
    assert_eq!(room.balance(1).await, 100 - 20 + 40 + 20);
    assert_eq!(room.balance(3).await, 100 - 30 + 20);
}

#[tokio::test]
async fn a_ranking_bet_is_on_who_ends_first() {
    let room = Room::new(3).await;
    room.add(json!({ "text": "Classe-les", "kind": "rank" }))
        .await;
    room.close_with(&[1, 0, 0]).await;
    let open = room.current(0).await;
    assert_eq!(open["kind"], "rank");
    let r = open["id"].as_i64().unwrap();
    room.bet_ok(2, r, 1, 10).await; // Cyd on Bob
    room.bet_ok(0, r, 2, 10).await; // Ana on Cyd
    let (a, b, c) = (room.mid(0), room.mid(1), room.mid(2));
    room.vote(0, json!({ "ranking": [b, a, c] })).await;
    room.vote(1, json!({ "ranking": [b, c, a] })).await;
    room.vote(2, json!({ "ranking": [b, a, c] })).await;
    let closed = room.round(0, r).await;
    assert_eq!(closed["tripot"]["winners"][0]["member_id"], c, "{closed}");
    assert_eq!(closed["tripot"]["winners"][0]["won"], 20);
}

#[tokio::test]
async fn nobody_on_the_winner_means_everyone_is_refunded() {
    let room = Room::new(3).await;
    let r = room.open_id().await;
    room.bet_ok(0, r, 2, 25).await;
    room.bet_ok(1, r, 2, 40).await;
    room.close_with(&[1, 0, 0]).await; // Bob 1, Ana 2: Ana wins.
    assert_eq!(room.balance(0).await, 120);
    assert_eq!(room.balance(1).await, 120);
    let t = &room.round(0, r).await["tripot"];
    assert_eq!(t["refunded"], true);
    assert_eq!(t["winners"], json!([]));
    assert_eq!(t["my_bet"]["payout"], 25);
}

#[tokio::test]
async fn a_removed_question_refunds_its_bets() {
    let room = Room::new(3).await;
    room.add(json!({ "text": "Qui ronfle ?" })).await;
    room.close_with(&[1, 0, 0]).await;
    let open = room.current(0).await;
    let r = open["id"].as_i64().unwrap();
    room.bet_ok(1, r, 0, 60).await;
    assert_eq!(room.balance(1).await, 60);
    let qid: i64 = sqlx::query_scalar("SELECT question_id FROM rounds WHERE id = ?")
        .bind(r)
        .fetch_one(&room.state.db)
        .await
        .unwrap();
    let (s, b) = call(
        &room.app,
        "DELETE",
        &format!("/questions/{qid}"),
        Some(room.tok(0)),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(room.balance(1).await, 120);
    assert_eq!(room.round(1, r).await["tripot"]["refunded"], true);
}

#[tokio::test]
async fn turning_it_off_refunds_open_bets() {
    let room = Room::new(3).await;
    let r = room.open_id().await;
    room.bet_ok(1, r, 0, 70).await;
    let (s, _) = room.settings(json!({ "tripot": true })).await;
    assert_eq!(s, StatusCode::OK);
    // Owner only.
    let (s, _) = call(
        &room.app,
        "PATCH",
        &format!("/rooms/{}", room.id),
        Some(room.tok(1)),
        None,
        Some(json!({ "tripot": false })),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert_eq!(room.balance(1).await, 30);

    let (s, _) = room.settings(json!({ "tripot": false })).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(room.balance(1).await, 100);
    assert_eq!(room.get(1, "/group").await["settings"]["tripot"], false);
    assert!(room.current(1).await.get("tripot").is_none());
    let (s, b) = room.bet(2, r, room.mid(0), 5).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("tripot_off"))
    );
    let (s, _) = call(&room.app, "GET", "/tripot", Some(room.tok(1)), None, None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    // No daily bonus while it is off.
    room.close_with(&[1, 0, 0]).await;
    assert_eq!(room.balance(1).await, 100);

    // Back on: everyone may bet again.
    room.settings(json!({ "tripot": true })).await;
    let r = room.open_id().await;
    room.bet_ok(1, r, 0, 10).await;
}

#[tokio::test]
async fn open_bets_stay_private() {
    let room = Room::new(3).await;
    let r = room.open_id().await;
    room.bet_ok(0, r, 1, 10).await;
    room.bet_ok(1, r, 1, 20).await;
    let cyd = room.current(2).await;
    let t = &cyd["tripot"];
    assert_eq!(t["pot"], 30);
    assert_eq!(t["bettors"], 2);
    assert!(t["my_bet"].is_null());
    assert!(t["winners"].is_null());
    let mut keys: Vec<&str> = t.as_object().unwrap().keys().map(|k| k.as_str()).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "balance", "bettors", "can_bet", "my_bet", "pot", "refunded", "winners"
        ]
    );
    // Ana sees her own bet, not Bob's.
    let ana = room.current(0).await;
    assert_eq!(ana["tripot"]["my_bet"]["target_id"], room.mid(1));
    assert_eq!(ana["tripot"]["my_bet"]["amount"], 10);
}

#[tokio::test]
async fn the_board_ranks_this_month() {
    let room = Room::new(3).await;
    let r = room.open_id().await;
    room.bet_ok(2, r, 1, 50).await;
    room.bet_ok(0, r, 0, 10).await;
    room.close_with(&[1, 1, 0]).await;
    // Cyd: 100 - 50 + 60 + 20 = 130; Ana: 100 - 10 + 20 = 110; Bob: 120.
    let board = room.get(0, "/tripot").await;
    let order: Vec<(i64, i64)> = board["balances"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| {
            (
                b["member_id"].as_i64().unwrap(),
                b["balance"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        order,
        [(room.mid(2), 130), (room.mid(1), 120), (room.mid(0), 110)]
    );
}
