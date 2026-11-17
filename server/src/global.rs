//! The global question of the day (contract: `docs/GLOBAL_QUESTION.md`): one built-in deck question per
//! UTC day and language, the same in every room of that language, apart from the room's own
//! questions. Each seat answers it once, about someone of its room; after voting it sees its
//! room's tally and anonymous aggregates across rooms (only once enough rooms answered).

use std::collections::HashMap;

use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use crate::{
    AppState,
    auth::Me,
    error::{AppError, AppResult},
    i18n::Lang,
    models::MemberRow,
};

/// A room needs this many people for the global question (fewer: the card is hidden).
pub const MIN_MEMBERS: i64 = 3;
/// Aggregates across rooms are only shown once this many rooms answered...
pub const MIN_ROOMS: usize = 5;
/// ...each with at least this many votes.
pub const MIN_ROOM_VOTES: usize = 2;
/// A question is not asked again for this many days (unless the deck runs out).
pub const NO_REPEAT_DAYS: i64 = 90;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/global", get(get_today))
        .route("/global/vote", post(vote))
}

/// Today, the UTC date (`2026-11-17`), from the database clock.
pub async fn today(db: &SqlitePool) -> sqlx::Result<String> {
    sqlx::query_scalar("SELECT date('now')").fetch_one(db).await
}

async fn day_before(db: &SqlitePool, day: &str) -> sqlx::Result<String> {
    sqlx::query_scalar("SELECT date(?, '-1 day')")
        .bind(day)
        .fetch_one(db)
        .await
}

/// Deck questions fit for every room (`?1` = language): pick questions tagged for families or
/// colleagues (the vibes any room can take), never spicy (16+). With the last day each was the
/// global question before `?2` (`NULL`: never), by id.
const ELIGIBLE: &str = "SELECT q.id, q.text, (SELECT MAX(g.day) FROM global_questions g \
     WHERE g.question_id = q.id AND g.language = ?1 AND g.day < ?2) FROM questions q \
     WHERE q.group_id IS NULL AND q.kind = 'pick' AND q.lang = ?1 \
     AND (',' || q.vibes || ',' LIKE '%,famille,%' OR ',' || q.vibes || ',' LIKE '%,boulot,%') \
     AND ',' || q.vibes || ',' NOT LIKE '%,pimente,%' ORDER BY q.id";

/// The question of `day` in `lang`: the one stored, else one picked now and stored (the first
/// request of the day picks it; two at once pick the same one and the second insert is a no-op).
/// The pick is deterministic: a hash of the day and language over the eligible deck questions
/// not asked in the last [`NO_REPEAT_DAYS`] days (all asked lately: the ones asked longest ago),
/// by id. `None` when the deck has none.
pub async fn question_for(
    db: &SqlitePool,
    lang: Lang,
    day: &str,
) -> sqlx::Result<Option<(i64, String)>> {
    let stored = stored_question(db, lang, day).await?;
    if stored.is_some() {
        return Ok(stored);
    }
    let deck: Vec<(i64, String, Option<String>)> = sqlx::query_as(ELIGIBLE)
        .bind(lang.as_str())
        .bind(day)
        .fetch_all(db)
        .await?;
    let cutoff: String = sqlx::query_scalar("SELECT date(?, ?)")
        .bind(day)
        .bind(format!("-{NO_REPEAT_DAYS} days"))
        .fetch_one(db)
        .await?;
    let fresh: Vec<&(i64, String, Option<String>)> = deck
        .iter()
        .filter(|(_, _, last)| last.as_ref().is_none_or(|l| *l <= cutoff))
        .collect();
    let candidates = if fresh.is_empty() {
        let oldest = deck.iter().filter_map(|(_, _, last)| last.as_ref()).min();
        deck.iter()
            .filter(|(_, _, last)| last.as_ref() == oldest)
            .collect()
    } else {
        fresh
    };
    if candidates.is_empty() {
        return Ok(None);
    }
    let (id, text, _) = candidates[pick_index(day, lang, candidates.len())];
    sqlx::query(
        "INSERT INTO global_questions (day, language, question_id, text) VALUES (?, ?, ?, ?) \
         ON CONFLICT DO NOTHING",
    )
    .bind(day)
    .bind(lang.as_str())
    .bind(id)
    .bind(text)
    .execute(db)
    .await?;
    stored_question(db, lang, day).await
}

async fn stored_question(
    db: &SqlitePool,
    lang: Lang,
    day: &str,
) -> sqlx::Result<Option<(i64, String)>> {
    sqlx::query_as("SELECT question_id, text FROM global_questions WHERE day = ? AND language = ?")
        .bind(day)
        .bind(lang.as_str())
        .fetch_optional(db)
        .await
}

fn pick_index(day: &str, lang: Lang, len: usize) -> usize {
    let hash = Sha256::digest(format!("global:{day}:{}", lang.as_str()).as_bytes());
    let mut first = [0u8; 8];
    first.copy_from_slice(&hash[..8]);
    (u64::from_be_bytes(first) % len as u64) as usize
}

/// Can this seat answer the global question? Its room's language, or why not:
/// `party` (a party), `guest` (a party guest account), `small_room` (fewer than
/// [`MIN_MEMBERS`] people).
async fn gate(db: &SqlitePool, me: &MemberRow) -> AppResult<Result<Lang, &'static str>> {
    let (kind, language): (String, String) =
        sqlx::query_as("SELECT kind, language FROM groups WHERE id = ?")
            .bind(me.group_id)
            .fetch_one(db)
            .await?;
    if kind != "room" {
        return Ok(Err("party"));
    }
    match me.user_id {
        Some(user) if !crate::auth::is_guest(db, user).await? => {}
        _ => return Ok(Err("guest")),
    }
    if members(db, me.group_id).await? < MIN_MEMBERS {
        return Ok(Err("small_room"));
    }
    Ok(Ok(Lang::from_db(Some(&language))))
}

async fn members(db: &SqlitePool, room: i64) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT COUNT(*) FROM members WHERE group_id = ? AND removed_at IS NULL")
        .bind(room)
        .fetch_one(db)
        .await
}

/// Votes that count: in a live room (not a party), voter and target still in it.
/// `(room, voter, target)`, for one question (day + language).
async fn counted_votes(
    db: &SqlitePool,
    lang: Lang,
    day: &str,
    room: Option<i64>,
) -> sqlx::Result<Vec<(i64, i64, i64)>> {
    sqlx::query_as(
        "SELECT v.group_id, v.voter_id, v.target_id FROM global_votes v \
         JOIN groups g ON g.id = v.group_id AND g.deleted_at IS NULL AND g.kind = 'room' \
         JOIN members a ON a.id = v.voter_id AND a.removed_at IS NULL \
         JOIN members b ON b.id = v.target_id AND b.removed_at IS NULL \
         WHERE v.day = ?1 AND v.language = ?2 AND (?3 IS NULL OR v.group_id = ?3)",
    )
    .bind(day)
    .bind(lang.as_str())
    .bind(room)
    .fetch_all(db)
    .await
}

/// One person's count in my room's tally (no voter ids: the global question is anonymous).
#[derive(Debug, Serialize, PartialEq)]
pub struct Count {
    pub target_id: i64,
    pub count: i64,
}

/// Most votes first, then by id.
fn tally(targets: impl Iterator<Item = i64>) -> Vec<Count> {
    let mut counts: HashMap<i64, i64> = HashMap::new();
    for t in targets {
        *counts.entry(t).or_default() += 1;
    }
    let mut out: Vec<Count> = counts
        .into_iter()
        .map(|(target_id, count)| Count { target_id, count })
        .collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then(a.target_id.cmp(&b.target_id)));
    out
}

/// Whoever has the most votes (all of them on a tie).
fn winners(tally: &[Count]) -> Vec<i64> {
    let top = tally.first().map_or(0, |c| c.count);
    tally
        .iter()
        .filter(|c| c.count == top && top > 0)
        .map(|c| c.target_id)
        .collect()
}

/// One aggregate across rooms: `pct` of `of` (rooms or voters, see `docs/GLOBAL_QUESTION.md`).
#[derive(Debug, Serialize)]
pub struct Fact {
    pub id: &'static str,
    pub pct: i64,
}

/// Rooms (or voters) a fact is about, and how many match.
#[derive(Default)]
struct Ratio {
    hits: usize,
    of: usize,
}

impl Ratio {
    fn add(&mut self, hit: bool) {
        self.of += 1;
        self.hits += usize::from(hit);
    }

    fn pct(&self) -> i64 {
        ((self.hits * 100 + self.of / 2) / self.of.max(1)) as i64
    }
}

/// How many rooms count for the aggregates (at least [`MIN_ROOM_VOTES`] votes), how many people
/// voted in them, and the aggregates (most striking first) when there are at least [`MIN_ROOMS`].
async fn aggregates(
    db: &SqlitePool,
    lang: Lang,
    day: &str,
) -> AppResult<(usize, usize, Option<Vec<Fact>>)> {
    let mut by_room: HashMap<i64, Vec<(i64, i64)>> = HashMap::new();
    for (room, voter, target) in counted_votes(db, lang, day, None).await? {
        by_room.entry(room).or_default().push((voter, target));
    }
    by_room.retain(|_, votes| votes.len() >= MIN_ROOM_VOTES);
    let rooms = by_room.len();
    let voters = by_room.values().map(Vec::len).sum();
    if rooms < MIN_ROOMS {
        return Ok((rooms, voters, None));
    }
    let mut unanimous = Ratio::default();
    let mut owner = Ratio::default();
    let mut newest = Ratio::default();
    let mut again = Ratio::default();
    let mut themselves = Ratio::default();
    let mut top_share = 0.0;
    for (room, votes) in &by_room {
        let counts = tally(votes.iter().map(|(_, t)| *t));
        let won = winners(&counts);
        unanimous.add(counts.len() == 1);
        top_share += counts[0].count as f64 / votes.len() as f64;
        for (voter, target) in votes {
            themselves.add(voter == target);
        }
        // The owner, and whoever joined last (the owner never counts as the newcomer).
        let people: Vec<(i64, String)> = sqlx::query_as(
            "SELECT id, role FROM members WHERE group_id = ? AND removed_at IS NULL \
             ORDER BY created_at DESC, id DESC",
        )
        .bind(room)
        .fetch_all(db)
        .await?;
        if let Some((id, _)) = people.iter().find(|(_, role)| role == "owner") {
            owner.add(won.contains(id));
        }
        if let Some((id, _)) = people.iter().find(|(_, role)| role != "owner") {
            newest.add(won.contains(id));
        }
        // The room's latest closed pick question that got votes (a skipped one keeps none).
        let last: Vec<i64> = sqlx::query_scalar(
            "SELECT target_id FROM votes WHERE round_id = (SELECT r.id FROM rounds r \
             JOIN questions q ON q.id = r.question_id AND q.kind = 'pick' \
             WHERE r.group_id = ? AND r.closed_at IS NOT NULL \
             AND EXISTS (SELECT 1 FROM votes v WHERE v.round_id = r.id) \
             ORDER BY r.number DESC LIMIT 1)",
        )
        .bind(room)
        .fetch_all(db)
        .await?;
        let last = winners(&tally(last.into_iter()));
        if !last.is_empty() {
            again.add(won.iter().any(|w| last.contains(w)));
        }
    }
    let mut facts: Vec<Fact> = [
        ("unanimous", &unanimous),
        ("owner", &owner),
        ("newest", &newest),
        ("last_winner", &again),
        ("self", &themselves),
    ]
    .into_iter()
    // Each fact over enough rooms too (the latest winner is only known in some).
    .filter(|(id, r)| *id == "self" || r.of >= MIN_ROOMS)
    .map(|(id, r)| Fact { id, pct: r.pct() })
    .collect();
    facts.push(Fact {
        id: "top_share",
        pct: (top_share * 100.0 / rooms as f64).round() as i64,
    });
    // Most striking first (stable: ties keep the order above).
    facts.sort_by_key(|f| std::cmp::Reverse(f.pct));
    Ok((rooms, voters, Some(facts)))
}

#[derive(Serialize)]
struct RoomPart {
    members: i64,
    voters: usize,
    /// Only once I voted.
    tally: Option<Vec<Count>>,
}

#[derive(Serialize)]
struct GlobalPart {
    rooms: usize,
    voters: usize,
    min_rooms: usize,
    /// Only once I voted and at least `min_rooms` rooms answered.
    facts: Option<Vec<Fact>>,
}

#[derive(Serialize)]
struct Yesterday {
    day: String,
    question: String,
    tally: Vec<Count>,
    rooms: usize,
    facts: Option<Vec<Fact>>,
}

/// `GET /api/global` for `day` (tests pass another day): `available: false` with a `reason`
/// (the app hides the card), or the question, my vote, my room's tally once I voted, the number
/// of rooms that answered and the aggregates, and yesterday's final numbers.
pub async fn view_at(state: &AppState, me: &MemberRow, day: &str) -> AppResult<Value> {
    let db = &state.db;
    let lang = match gate(db, me).await? {
        Ok(lang) => lang,
        Err(reason) => return Ok(unavailable(reason)),
    };
    let Some((_, question)) = question_for(db, lang, day).await? else {
        return Ok(unavailable("no_question"));
    };
    let my_vote: Option<i64> = sqlx::query_scalar(
        "SELECT target_id FROM global_votes WHERE day = ? AND language = ? AND voter_id = ?",
    )
    .bind(day)
    .bind(lang.as_str())
    .bind(me.id)
    .fetch_optional(db)
    .await?;
    let mine = counted_votes(db, lang, day, Some(me.group_id)).await?;
    let room = RoomPart {
        members: members(db, me.group_id).await?,
        voters: mine.len(),
        tally: my_vote.map(|_| tally(mine.iter().map(|(_, _, t)| *t))),
    };
    let (rooms, voters, facts) = aggregates(db, lang, day).await?;
    let global = GlobalPart {
        rooms,
        voters,
        min_rooms: MIN_ROOMS,
        facts: facts.filter(|_| my_vote.is_some()),
    };
    let before = day_before(db, day).await?;
    let yesterday = match stored_question(db, lang, &before).await? {
        Some((_, text)) => {
            let votes = counted_votes(db, lang, &before, Some(me.group_id)).await?;
            let (rooms, _, facts) = aggregates(db, lang, &before).await?;
            Some(Yesterday {
                day: before,
                question: text,
                tally: tally(votes.iter().map(|(_, _, t)| *t)),
                rooms,
                facts,
            })
        }
        None => None,
    };
    Ok(json!({
        "available": true,
        "day": day,
        "language": lang.as_str(),
        "question": question,
        "my_vote": my_vote,
        "room": room,
        "global": global,
        "yesterday": yesterday,
    }))
}

fn unavailable(reason: &str) -> Value {
    json!({ "available": false, "reason": reason, "min_members": MIN_MEMBERS })
}

async fn get_today(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    let day = today(&state.db).await?;
    Ok(Json(view_at(&state, &me, &day).await?))
}

#[derive(Deserialize)]
struct VoteBody {
    #[serde(alias = "target_id")]
    target: i64,
}

/// Answers the question of `day` for someone of my room. Final: `409 already_voted` after.
/// `403 party | guest | small_room` when the room can't answer it, `400 unknown_member` for
/// someone not in my room.
pub async fn vote_at(state: &AppState, me: &MemberRow, target: i64, day: &str) -> AppResult<()> {
    let lang = gate(&state.db, me).await?.map_err(AppError::Forbidden)?;
    if question_for(&state.db, lang, day).await?.is_none() {
        return Err(AppError::Conflict("no_question"));
    }
    let mut tx = crate::db::write_tx(&state.db).await?;
    let in_room: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM members WHERE id = ? AND group_id = ? AND removed_at IS NULL)",
    )
    .bind(target)
    .bind(me.group_id)
    .fetch_one(&mut *tx)
    .await?;
    if !in_room {
        return Err(AppError::BadRequest("unknown_member"));
    }
    let inserted = sqlx::query(
        "INSERT INTO global_votes (day, language, group_id, voter_id, target_id) VALUES (?, ?, ?, ?, ?) \
         ON CONFLICT DO NOTHING",
    )
    .bind(day)
    .bind(lang.as_str())
    .bind(me.group_id)
    .bind(me.id)
    .bind(target)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(AppError::Conflict("already_voted"));
    }
    tx.commit().await?;
    Ok(())
}

async fn vote(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<VoteBody>,
) -> AppResult<Json<Value>> {
    let day = today(&state.db).await?;
    vote_at(&state, &me, body.target, &day).await?;
    Ok(Json(view_at(&state, &me, &day).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tallies_and_winners() {
        let t = tally([3, 1, 3, 2, 1].into_iter());
        assert_eq!(
            t,
            vec![
                Count {
                    target_id: 1,
                    count: 2
                },
                Count {
                    target_id: 3,
                    count: 2
                },
                Count {
                    target_id: 2,
                    count: 1
                },
            ]
        );
        assert_eq!(winners(&t), vec![1, 3]);
        assert!(winners(&[]).is_empty());
        let r = Ratio { hits: 2, of: 3 };
        assert_eq!(r.pct(), 67);
        assert_eq!(Ratio::default().pct(), 0);
    }

    #[test]
    fn the_pick_depends_on_day_and_language() {
        let a = pick_index("2026-11-17", Lang::Fr, 1000);
        assert_eq!(a, pick_index("2026-11-17", Lang::Fr, 1000));
        assert_ne!(a, pick_index("2026-11-18", Lang::Fr, 1000));
        assert!(pick_index("2026-11-17", Lang::En, 7) < 7);
    }
}
