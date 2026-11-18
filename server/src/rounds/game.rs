//! The round loop: one open question per group, closed when every member has voted.

use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::{
    platform::error::{AppError, AppResult},
    realtime::live::Event,
    rounds::model::{
        Choice, ChooseGuess, ChoosePerson, ChooseResult, ChooseScore, RankEntry, RoundRow,
        RoundView, SkipState, Tally,
    },
};

macro_rules! round_select {
    ($where:literal) => {
        concat!(
            "SELECT r.id, r.group_id, r.question_id, r.number, r.opened_at, r.closed_at, q.text, ",
            "(q.group_id IS NOT NULL) AS custom, q.kind, q.items, r.anonymous FROM rounds r JOIN questions q ON q.id = r.question_id ",
            $where
        )
    };
}

/// A choose question nobody can vote on (no items): never opened (older servers made such copies).
macro_rules! votable {
    () => {
        "NOT (q.kind = 'choose' AND q.items IS NULL)"
    };
}

/// What a room can draw: a party only pick and said questions; a said question needs
/// [`crate::rounds::said::MIN_PEOPLE`] people (`?1` = the room).
macro_rules! drawable {
    () => {
        "(q.kind IN ('pick', 'said') OR (SELECT kind FROM groups WHERE id = ?1) = 'room') \
         AND (q.kind <> 'said' OR (SELECT COUNT(*) FROM members WHERE group_id = ?1 AND removed_at IS NULL) >= 3)"
    };
}

macro_rules! unused {
    () => {
        "NOT EXISTS (SELECT 1 FROM rounds r WHERE r.group_id = ?1 AND r.question_id = q.id)"
    };
}
pub(crate) use unused;

/// The room's own question with this text, else a built-in one the room was already asked, if
/// any: the one rule for "already in this room" when adding a question, importing a file,
/// installing a pack or playing a shared question. Case-insensitive beyond ASCII ("ÉTÉ" = "été",
/// see [`crate::platform::text::key`]): same length in SQL, then compared here.
pub(crate) async fn same_question(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: i64,
    text: &str,
) -> AppResult<Option<i64>> {
    let rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, text FROM (\
           SELECT 0 AS o, id, text FROM questions WHERE group_id = ?1 AND length(text) = length(?2) \
           UNION ALL SELECT 1, q.id, q.text FROM rounds r JOIN questions q ON q.id = r.question_id \
           WHERE r.group_id = ?1 AND q.group_id IS NULL AND length(q.text) = length(?2)) ORDER BY o, id",
    )
    .bind(group_id)
    .bind(text)
    .fetch_all(&mut **tx)
    .await?;
    let key = crate::platform::text::key(text);
    Ok(rows
        .into_iter()
        .find(|(_, t)| crate::platform::text::key(t) == key)
        .map(|(id, _)| id))
}

pub async fn open_round(db: &SqlitePool, group_id: i64) -> AppResult<Option<RoundRow>> {
    Ok(sqlx::query_as::<_, RoundRow>(round_select!(
        "WHERE r.group_id = ? AND r.closed_at IS NULL"
    ))
    .bind(group_id)
    .fetch_optional(db)
    .await?)
}

/// Oldest finished question I voted on whose result I have not seen yet.
pub async fn unseen_round(
    db: &SqlitePool,
    me: &crate::rooms::model::MemberRow,
) -> AppResult<Option<RoundRow>> {
    Ok(sqlx::query_as::<_, RoundRow>(round_select!(
        "WHERE r.group_id = ? AND r.closed_at IS NOT NULL AND r.number > ? \
         AND EXISTS (SELECT 1 FROM votes v WHERE v.round_id = r.id AND v.voter_id = ?) \
         ORDER BY r.number LIMIT 1"
    ))
    .bind(me.group_id)
    .bind(me.seen_round)
    .bind(me.id)
    .fetch_optional(db)
    .await?)
}

/// Every finished question I voted on whose result I have not seen, newest first (ids): how many, and
/// the one to mark seen to catch up at once (`seen_round` only moves forward).
pub async fn unseen_ids(
    db: &SqlitePool,
    me: &crate::rooms::model::MemberRow,
) -> AppResult<Vec<i64>> {
    Ok(sqlx::query_scalar(
        "SELECT r.id FROM rounds r WHERE r.group_id = ? AND r.closed_at IS NOT NULL AND r.number > ? \
         AND EXISTS (SELECT 1 FROM votes v WHERE v.round_id = r.id AND v.voter_id = ?) \
         ORDER BY r.number DESC",
    )
    .bind(me.group_id)
    .bind(me.seen_round)
    .bind(me.id)
    .fetch_all(db)
    .await?)
}

pub async fn round(db: &SqlitePool, group_id: i64, round_id: i64) -> AppResult<RoundRow> {
    sqlx::query_as::<_, RoundRow>(round_select!("WHERE r.group_id = ? AND r.id = ?"))
        .bind(group_id)
        .bind(round_id)
        .fetch_optional(db)
        .await?
        .ok_or(AppError::NotFound)
}

/// Built-in deck questions this room can still get (the deck of its language), with their vibe tags.
/// Parties draw pick and said questions only, said ones with 3 people or more (see [`drawable`]).
/// One the room has its own copy of (added by hand) is left to that copy.
async fn deck_candidates<'e, E: sqlx::SqliteExecutor<'e>>(
    db: E,
    group_id: i64,
) -> AppResult<Vec<(i64, Option<String>)>> {
    Ok(sqlx::query_as(concat!(
        "SELECT q.id, q.vibes FROM questions q WHERE q.group_id IS NULL \
         AND q.lang = (SELECT language FROM groups WHERE id = ?1) \
         AND NOT EXISTS (SELECT 1 FROM questions c WHERE c.group_id = ?1 AND lower(c.text) = lower(q.text)) AND ",
        drawable!(),
        " AND ",
        votable!(),
        " AND ",
        unused!()
    ))
    .bind(group_id)
    .fetch_all(db)
    .await?)
}

/// A party's settings: seconds per question, and whether it is over (`party_ends_at` passed).
#[derive(Debug, Clone, Copy)]
pub struct Party {
    pub timer: i64,
    pub over: bool,
}

/// `Some` when the room is a party (see `party`).
pub async fn party<'e, E: sqlx::SqliteExecutor<'e>>(
    db: E,
    group_id: i64,
) -> AppResult<Option<Party>> {
    let row: Option<(bool, i64, bool)> = sqlx::query_as(
        "SELECT kind = 'party', party_timer, \
         party_ends_at IS NOT NULL AND party_ends_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         FROM groups WHERE id = ?",
    )
    .bind(group_id)
    .fetch_optional(db)
    .await?;
    Ok(row
        .filter(|r| r.0)
        .map(|(_, timer, over)| Party { timer, over }))
}

async fn room_vibe<'e, E: sqlx::SqliteExecutor<'e>>(
    db: E,
    group_id: i64,
) -> AppResult<Option<String>> {
    Ok(sqlx::query_scalar("SELECT vibe FROM groups WHERE id = ?")
        .bind(group_id)
        .fetch_optional(db)
        .await?
        .flatten())
}

/// Deck questions left for this room, given its vibe and language.
pub async fn deck_left(db: &SqlitePool, group_id: i64) -> AppResult<i64> {
    let draws = crate::rooms::vibes::draws(room_vibe(db, group_id).await?.as_deref());
    Ok(deck_candidates(db, group_id)
        .await?
        .iter()
        .filter(|(_, tags)| {
            draws
                .iter()
                .any(|d| crate::rooms::vibes::has_tag(tags.as_deref(), d))
        })
        .count() as i64)
}

/// Questions a room can still open: its own unasked ones plus what's left in its deck.
pub async fn questions_left(db: &SqlitePool, group_id: i64) -> AppResult<i64> {
    let custom: i64 = sqlx::query_scalar(concat!(
        "SELECT COUNT(*) FROM questions q WHERE q.group_id = ?1 AND ",
        votable!(),
        " AND ",
        unused!()
    ))
    .bind(group_id)
    .fetch_one(db)
    .await?;
    Ok(custom + deck_left(db, group_id).await?)
}

/// Group questions first (queue order: oldest first unless shuffled; a said question waits while the
/// room is too small), then a random deck question
/// the group has not seen, in the room's language, from its vibe (its favourite tag first).
async fn next_question(tx: &mut Transaction<'_, Sqlite>, group_id: i64) -> AppResult<Option<i64>> {
    use rand::seq::IndexedRandom;
    let custom: Option<i64> = sqlx::query_scalar(concat!(
        "SELECT q.id FROM questions q WHERE q.group_id = ?1 AND ",
        drawable!(),
        " AND ",
        votable!(),
        " AND ",
        unused!(),
        " ORDER BY COALESCE(q.sort_key, q.id), q.id LIMIT 1"
    ))
    .bind(group_id)
    .fetch_optional(&mut **tx)
    .await?;
    if custom.is_some() {
        return Ok(custom);
    }
    let draws = crate::rooms::vibes::draws(room_vibe(&mut **tx, group_id).await?.as_deref());
    let candidates = deck_candidates(&mut **tx, group_id).await?;
    for tag in &draws {
        let pool: Vec<i64> = candidates
            .iter()
            .filter(|(_, tags)| crate::rooms::vibes::has_tag(tags.as_deref(), tag))
            .map(|(id, _)| *id)
            .collect();
        if let Some(id) = pool.choose(&mut rand::rng()) {
            return Ok(Some(*id));
        }
    }
    Ok(None)
}

/// Opens the next round when none is open and at least two people are in the room (alone,
/// there is nobody to vote for: the room waits for its second member). Returns the new round id, if any.
/// A party question gets a deadline (now + the party's timer; a said question's writing phase, see
/// [`crate::rounds::said::writing_secs`]) and the list of who may vote on it.
pub async fn ensure_open_round(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: i64,
) -> AppResult<Option<i64>> {
    if active_count(tx, group_id).await? < 2 {
        return Ok(None);
    }
    let party = party(&mut **tx, group_id).await?;
    if party.is_some_and(|p| p.over) {
        return Ok(None);
    }
    // Paced room: the closed question waits for the owner or the delay (see `pacing`).
    let paused: bool = sqlx::query_scalar("SELECT paused_at IS NOT NULL FROM groups WHERE id = ?")
        .bind(group_id)
        .fetch_optional(&mut **tx)
        .await?
        .unwrap_or(false);
    if paused {
        return Ok(None);
    }
    let open: Option<i64> =
        sqlx::query_scalar("SELECT id FROM rounds WHERE group_id = ? AND closed_at IS NULL")
            .bind(group_id)
            .fetch_optional(&mut **tx)
            .await?;
    if open.is_some() {
        return Ok(None);
    }
    let Some(question_id) = next_question(tx, group_id).await? else {
        return Ok(None);
    };
    let seconds = match party {
        Some(p) => {
            let kind: String = sqlx::query_scalar("SELECT kind FROM questions WHERE id = ?")
                .bind(question_id)
                .fetch_one(&mut **tx)
                .await?;
            Some(if kind == "said" {
                crate::rounds::said::writing_secs(p.timer)
            } else {
                p.timer
            })
        }
        None => None,
    };
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (group_id, question_id, number, deadline_at) \
         VALUES (?1, ?2, (SELECT COALESCE(MAX(number), 0) + 1 FROM rounds WHERE group_id = ?1), \
                 CASE WHEN ?3 IS NOT NULL THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+' || ?3 || ' seconds') END) \
         RETURNING id",
    )
    .bind(group_id)
    .bind(question_id)
    .bind(seconds)
    .fetch_one(&mut **tx)
    .await?;
    crate::rooms::anonymous::decide(tx, id).await?;
    if party.is_some() {
        sqlx::query(
            "INSERT INTO party_players (round_id, member_id) \
             SELECT ?, id FROM members WHERE group_id = ? AND removed_at IS NULL",
        )
        .bind(id)
        .bind(group_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(Some(id))
}

/// Records a vote; closes the round and opens the next one when everyone has voted.
/// `guesses`: a said question's matching (see [`crate::rounds::said::ballot`]).
pub async fn vote(
    db: &SqlitePool,
    group_id: i64,
    voter_id: i64,
    target_id: Option<i64>,
    ranking: Option<Vec<i64>>,
    choices: Option<Vec<Choice>>,
    guesses: Option<Vec<crate::rounds::said::Guess>>,
) -> AppResult<Vec<Event>> {
    let mut tx = crate::platform::db::write_tx(db).await?;
    // A party that is over is a read-only memory.
    let party = party(&mut *tx, group_id).await?;
    if party.is_some_and(|p| p.over) {
        return Err(AppError::Gone("party_over"));
    }
    let (round_id, kind, items, time_up): (i64, String, Option<String>, bool) = sqlx::query_as(
        "SELECT r.id, q.kind, q.items, \
         COALESCE(r.deadline_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 0) \
         FROM rounds r JOIN questions q ON q.id = r.question_id \
         WHERE r.group_id = ? AND r.closed_at IS NULL",
    )
    .bind(group_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::Conflict("no_open_round"))?;
    // A party question past its countdown takes no more votes, even before the sweep closes it.
    if time_up {
        return Err(AppError::Conflict("time_up"));
    }
    // Party: only who was there when the question opened votes on it (and can be picked).
    if party.is_some() && !party_player(&mut tx, round_id, voter_id).await? {
        return Err(AppError::Conflict("joined_late"));
    }
    // A question left open in a room that has since dropped to one person waits for company.
    if active_count(&mut tx, group_id).await? < 2 {
        return Err(AppError::Conflict("alone"));
    }
    // A ranking must place every active member exactly once; its #1 is also stored as the vote.
    let target_id = if kind == "said" {
        // Guesses (or an older app's pass); the votes row points at the voter.
        crate::rounds::said::ballot(&mut tx, round_id, voter_id, target_id, guesses).await?;
        voter_id
    } else if (kind == "rank" && ranking.is_none() || kind == "choose" && choices.is_none())
        && target_id.is_some()
    {
        // An app too old to know ranking (or choose) questions sends a single pick: it counts as
        // voted (nobody waits for them) but adds nothing to the result.
        target_id.unwrap_or_default()
    } else if kind == "choose" {
        // Every active member wired exactly once, to an existing item; the votes row points at the voter.
        let choices = choices.ok_or(AppError::BadRequest("choices_needed"))?;
        let count =
            crate::rounds::model::parse_items(items.as_deref()).map_or(0, |i| i.len()) as i64;
        let active: Vec<i64> = sqlx::query_scalar(
            "SELECT id FROM members WHERE group_id = ? AND removed_at IS NULL ORDER BY id",
        )
        .bind(group_id)
        .fetch_all(&mut *tx)
        .await?;
        let mut given: Vec<i64> = choices.iter().map(|c| c.member_id).collect();
        given.sort_unstable();
        if given != active || choices.iter().any(|c| !(0..count).contains(&c.item)) {
            return Err(AppError::BadRequest("bad_choices"));
        }
        // A ballot left from an earlier seat (left and came back): this one replaces it (a second
        // vote on the same seat still fails below, rolling this back).
        clear_ballot(&mut tx, round_id, voter_id).await?;
        for c in &choices {
            sqlx::query(
                "INSERT INTO choices (round_id, voter_id, member_id, item) VALUES (?, ?, ?, ?)",
            )
            .bind(round_id)
            .bind(voter_id)
            .bind(c.member_id)
            .bind(c.item)
            .execute(&mut *tx)
            .await?;
        }
        voter_id
    } else if kind == "rank" {
        let ranking = ranking.ok_or(AppError::BadRequest("ranking_needed"))?;
        let mut active: Vec<i64> = sqlx::query_scalar(
            "SELECT id FROM members WHERE group_id = ? AND removed_at IS NULL ORDER BY id",
        )
        .bind(group_id)
        .fetch_all(&mut *tx)
        .await?;
        let mut given = ranking.clone();
        given.sort_unstable();
        active.sort_unstable();
        if given != active {
            return Err(AppError::BadRequest("bad_ranking"));
        }
        clear_ballot(&mut tx, round_id, voter_id).await?;
        for (i, member_id) in ranking.iter().enumerate() {
            sqlx::query(
                "INSERT INTO rankings (round_id, voter_id, member_id, position) VALUES (?, ?, ?, ?)",
            )
            .bind(round_id)
            .bind(voter_id)
            .bind(member_id)
            .bind(i as i64 + 1)
            .execute(&mut *tx)
            .await?;
        }
        ranking[0]
    } else {
        target_id.ok_or(AppError::BadRequest("target_needed"))?
    };
    let target_in_group: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM members WHERE id = ? AND group_id = ? AND removed_at IS NULL",
    )
    .bind(target_id)
    .bind(group_id)
    .fetch_optional(&mut *tx)
    .await?;
    if target_in_group.is_none()
        || (party.is_some() && !party_player(&mut tx, round_id, target_id).await?)
    {
        return Err(AppError::BadRequest("unknown_member"));
    }
    let inserted = sqlx::query(
        "INSERT INTO votes (round_id, voter_id, target_id) VALUES (?, ?, ?) ON CONFLICT DO NOTHING",
    )
    .bind(round_id)
    .bind(voter_id)
    .bind(target_id)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(AppError::Conflict("already_voted"));
    }

    crate::rounds::tripot::on_vote(&mut tx, group_id, voter_id).await?;
    let mut events = vec![Event::VoteCast { round_id, voter_id }];
    events.extend(close_if_done(&mut tx, group_id, round_id).await?);
    tx.commit().await?;
    Ok(events)
}

/// Drops a member's rank / choose / said ballot on a question (their seat ended, or a new ballot replaces it).
pub async fn clear_ballot(
    tx: &mut Transaction<'_, Sqlite>,
    round_id: i64,
    voter_id: i64,
) -> AppResult<()> {
    for sql in [
        "DELETE FROM rankings WHERE round_id = ? AND voter_id = ?",
        "DELETE FROM choices WHERE round_id = ? AND voter_id = ?",
        "DELETE FROM said_guesses WHERE round_id = ? AND voter_id = ?",
    ] {
        sqlx::query(sql)
            .bind(round_id)
            .bind(voter_id)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

/// Was this member in the party when the question opened?
pub(crate) async fn party_player(
    tx: &mut Transaction<'_, Sqlite>,
    round_id: i64,
    member_id: i64,
) -> AppResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM party_players WHERE round_id = ? AND member_id = ?)",
    )
    .bind(round_id)
    .bind(member_id)
    .fetch_one(&mut **tx)
    .await?)
}

/// Agreements needed to skip or remove someone: a majority of the other active members
/// (the target doesn't count), and never one person alone.
pub fn majority(active: i64) -> i64 {
    ((active - 1) / 2 + 1).max(2)
}

/// Majority skips and removals need at least three people: with two, one would decide alone.
pub fn votes_possible(active: i64) -> bool {
    active >= 3
}

/// A room with active members always has an owner: when none is left, the longest-standing
/// active member takes over. `true` when someone was promoted.
pub async fn ensure_owner(tx: &mut Transaction<'_, Sqlite>, group_id: i64) -> AppResult<bool> {
    let promoted = sqlx::query(
        "UPDATE members SET role = 'owner' WHERE id = \
         (SELECT id FROM members WHERE group_id = ?1 AND removed_at IS NULL ORDER BY id LIMIT 1) \
         AND NOT EXISTS (SELECT 1 FROM members WHERE group_id = ?1 AND removed_at IS NULL AND role = 'owner')",
    )
    .bind(group_id)
    .execute(&mut **tx)
    .await?;
    Ok(promoted.rows_affected() > 0)
}

/// Startup repair: every live room with members but no active owner gets one. Idempotent.
pub async fn repair_owners(db: &SqlitePool) -> AppResult<u64> {
    let done = sqlx::query(
        "UPDATE members SET role = 'owner' WHERE id IN \
         (SELECT MIN(m.id) FROM members m JOIN groups g ON g.id = m.group_id AND g.deleted_at IS NULL \
          WHERE m.removed_at IS NULL GROUP BY m.group_id HAVING SUM(m.role = 'owner') = 0)",
    )
    .execute(db)
    .await?;
    Ok(done.rows_affected())
}

/// Startup repair: a choose question copied into a room without its items (a share or a
/// challenge, before they copied them) gets the items of the question it was copied from (same
/// text). Its open round, which nobody could vote on, becomes playable again. Idempotent.
/// Returns (repaired, left without items).
pub async fn repair_choose_items(db: &SqlitePool) -> AppResult<(u64, i64)> {
    let done = sqlx::query(
        "UPDATE questions SET items = \
           (SELECT s.items FROM questions s WHERE s.kind = 'choose' AND s.items IS NOT NULL \
            AND lower(s.text) = lower(questions.text) ORDER BY s.group_id IS NOT NULL, s.id LIMIT 1) \
         WHERE kind = 'choose' AND items IS NULL AND EXISTS \
           (SELECT 1 FROM questions s WHERE s.kind = 'choose' AND s.items IS NOT NULL \
            AND lower(s.text) = lower(questions.text))",
    )
    .execute(db)
    .await?;
    let left: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM questions WHERE kind = 'choose' AND items IS NULL",
    )
    .fetch_one(db)
    .await?;
    Ok((done.rows_affected(), left))
}

pub async fn active_count(tx: &mut Transaction<'_, Sqlite>, group_id: i64) -> AppResult<i64> {
    Ok(
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM members WHERE group_id = ? AND removed_at IS NULL",
        )
        .bind(group_id)
        .fetch_one(&mut **tx)
        .await?,
    )
}

/// Members the group agreed not to wait for on this question (and who still have not voted; on a
/// said question still writing, not written).
pub async fn skipped(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: i64,
    round_id: i64,
) -> AppResult<Vec<i64>> {
    let needed = majority(active_count(tx, group_id).await?);
    Ok(sqlx::query_scalar(concat!(
        "SELECT s.target_id FROM skips s JOIN members m ON m.id = s.target_id AND m.removed_at IS NULL \
         JOIN members f ON f.id = s.from_id AND f.removed_at IS NULL \
         WHERE s.round_id = ? AND NOT ",
        crate::rounds::said::done!("s.round_id", "s.target_id"),
        " GROUP BY s.target_id HAVING COUNT(*) >= ?"
    ))
    .bind(round_id)
    .bind(needed)
    .fetch_all(&mut **tx)
    .await?)
}

/// Questions in a row someone can miss before the room stops waiting for them ("away").
pub const AWAY_AFTER: i64 = 3;

/// Hours after which an open question closes once at least half of the people it waits for voted.
pub const QUORUM_HOURS: i64 = 24;

/// SQL: the seat `m` (a `members` alias) is away (see [`away`]); `$after` is the parameter bound to
/// [`AWAY_AFTER`]. One rule for one room ([`away`]) and for all of an account's rooms (`today`).
macro_rules! away_sql {
    ($after:literal) => {
        concat!(
            "((SELECT kind FROM groups WHERE id = m.group_id) = 'room' \
             AND (SELECT COUNT(*) FROM rounds r WHERE r.group_id = m.group_id AND r.closed_at IS NOT NULL \
                  AND r.closed_at >= m.created_at AND EXISTS (SELECT 1 FROM votes x WHERE x.round_id = r.id) \
                  AND r.number > COALESCE((SELECT MAX(r2.number) FROM votes v JOIN rounds r2 ON r2.id = v.round_id \
                                           WHERE v.voter_id = m.id AND r2.group_id = m.group_id), 0)) >= ",
            $after,
            ")"
        )
    };
}
pub(crate) use away_sql;

/// Members who are away: the last [`AWAY_AFTER`] questions of the room closed without their vote
/// (closed since they joined; a removed question nobody could answer doesn't count). The room
/// stops waiting for them until they vote again. Rooms only.
pub async fn away(tx: &mut Transaction<'_, Sqlite>, group_id: i64) -> AppResult<Vec<i64>> {
    Ok(sqlx::query_scalar(concat!(
        "SELECT m.id FROM members m WHERE m.group_id = ?1 AND m.removed_at IS NULL AND ",
        away_sql!("?2"),
        " ORDER BY m.id"
    ))
    .bind(group_id)
    .bind(AWAY_AFTER)
    .fetch_all(&mut **tx)
    .await?)
}

/// Who an open question no longer waits for: skipped by the room, or away. Sorted, no duplicates.
pub async fn not_waited(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: i64,
    round_id: i64,
) -> AppResult<(Vec<i64>, Vec<i64>)> {
    let skipped = skipped(tx, group_id, round_id).await?;
    let away = away(tx, group_id).await?;
    Ok((skipped, away))
}

fn union(a: &[i64], b: &[i64]) -> usize {
    let mut all: Vec<i64> = a.iter().chain(b).copied().collect();
    all.sort_unstable();
    all.dedup();
    all.len()
}

/// How many people a room's open question waits for: its active members minus the skipped and the away.
pub(crate) async fn waiting_for(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: i64,
    round_id: i64,
) -> AppResult<i64> {
    let (skipped, away) = not_waited(tx, group_id, round_id).await?;
    Ok(active_count(tx, group_id).await? - union(&skipped, &away) as i64)
}

/// Open for [`QUORUM_HOURS`] (a said question's guessing: counted from its start), so half of the
/// people it waits for is enough.
pub(crate) async fn late(tx: &mut Transaction<'_, Sqlite>, round_id: i64) -> AppResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT COALESCE(said_guess_at, opened_at) <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?) \
         FROM rounds WHERE id = ?",
    )
    .bind(format!("-{QUORUM_HOURS} hours"))
    .bind(round_id)
    .fetch_one(&mut **tx)
    .await?)
}

/// Closes the round once every active member has voted or been skipped, and opens the next one
/// (unless the room is paced: then it waits for the owner or the delay, see `pacing`).
/// A party question closes once everyone it was opened for and still there has voted, and never
/// opens the next one by itself (the reveal waits for the taps, see `party`).
/// A said question still writing moves on to its guessing instead (see [`crate::rounds::said::advance`]).
pub async fn close_if_done(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: i64,
    round_id: i64,
) -> AppResult<Vec<Event>> {
    let party = party(&mut **tx, group_id).await?;
    let mut events = Vec::new();
    match crate::rounds::said::advance(tx, group_id, round_id).await? {
        crate::rounds::said::Step::Writing => return Ok(events),
        crate::rounds::said::Step::TooFew if party.is_some() => {
            return close_party_round(tx, group_id, round_id, None).await;
        }
        crate::rounds::said::Step::TooFew => return close_room_round(tx, group_id, round_id).await,
        crate::rounds::said::Step::Guessing => events.push(Event::RoundUpdated { round_id }),
        crate::rounds::said::Step::NotWriting => {}
    }
    if party.is_some() {
        let (eligible, voted): (i64, i64) = sqlx::query_as(
            "SELECT COUNT(*), COALESCE(SUM(EXISTS (SELECT 1 FROM votes v WHERE v.round_id = p.round_id \
                                                   AND v.voter_id = p.member_id)), 0) \
             FROM party_players p JOIN members m ON m.id = p.member_id AND m.removed_at IS NULL \
             WHERE p.round_id = ?",
        )
        .bind(round_id)
        .fetch_one(&mut **tx)
        .await?;
        if voted >= eligible {
            events.extend(close_party_round(tx, group_id, round_id, None).await?);
        }
        return Ok(events);
    }
    let votes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM votes v JOIN members m ON m.id = v.voter_id AND m.removed_at IS NULL WHERE v.round_id = ?",
    )
    .bind(round_id)
    .fetch_one(&mut **tx)
    .await?;
    let waiting_for = waiting_for(tx, group_id, round_id).await?;
    // Open for a day: half of the people it waits for is enough (a holiday doesn't stop the room).
    let late = late(tx, round_id).await?;
    if votes > 0 && (votes >= waiting_for || late && 2 * votes >= waiting_for) {
        events.extend(close_room_round(tx, group_id, round_id).await?);
    }
    Ok(events)
}

/// Closes a room question now, settles its bets and opens the next one (unless the room is paced).
async fn close_room_round(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: i64,
    round_id: i64,
) -> AppResult<Vec<Event>> {
    let done = sqlx::query("UPDATE rounds SET closed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND closed_at IS NULL")
        .bind(round_id)
        .execute(&mut **tx)
        .await?;
    // Already closed (by someone else just before): nothing to announce twice.
    if done.rows_affected() == 0 {
        return Ok(Vec::new());
    }
    crate::rounds::tripot::settle(tx, round_id).await?;
    let mut events = vec![Event::RoundClosed { round_id }];
    if crate::rooms::pacing::pause(tx, group_id).await? {
        events.push(Event::RoomUpdated);
    } else if let Some(next) = ensure_open_round(tx, group_id).await? {
        events.push(Event::RoundOpened { round_id: next });
    }
    Ok(events)
}

/// Closes an open question with no result (its question was removed): every vote, ballot, said
/// answer and skip on it goes, then the next question opens (a party pauses for its reveal, as at a deadline).
/// A room never counts such a question as missed (see [`away`]).
pub async fn drop_round(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: i64,
    round_id: i64,
) -> AppResult<Vec<Event>> {
    for sql in [
        "DELETE FROM votes WHERE round_id = ?",
        "DELETE FROM rankings WHERE round_id = ?",
        "DELETE FROM choices WHERE round_id = ?",
        "DELETE FROM said_guesses WHERE round_id = ?",
        "DELETE FROM said_answers WHERE round_id = ?",
        "DELETE FROM skips WHERE round_id = ?",
    ] {
        sqlx::query(sql).bind(round_id).execute(&mut **tx).await?;
    }
    if party(&mut **tx, group_id).await?.is_some() {
        return close_party_round(tx, group_id, round_id, None).await;
    }
    let done = sqlx::query(
        "UPDATE rounds SET closed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND closed_at IS NULL",
    )
    .bind(round_id)
    .execute(&mut **tx)
    .await?;
    if done.rows_affected() == 0 {
        return Ok(Vec::new());
    }
    crate::rounds::tripot::refund(tx, round_id).await?;
    let mut events = vec![Event::RoundClosed { round_id }];
    if let Some(next) = ensure_open_round(tx, group_id).await? {
        events.push(Event::RoundOpened { round_id: next });
    }
    Ok(events)
}

/// Closes a party question (everyone voted, its deadline, or the end of the party) at `now`
/// (database clock when `None`) and pauses the party for the reveal. Idempotent: a closed question
/// gives no events.
pub async fn close_party_round(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: i64,
    round_id: i64,
    now: Option<&str>,
) -> AppResult<Vec<Event>> {
    let done = sqlx::query(
        "UPDATE rounds SET closed_at = COALESCE(?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
         WHERE id = ? AND closed_at IS NULL",
    )
    .bind(now)
    .bind(round_id)
    .execute(&mut **tx)
    .await?;
    if done.rows_affected() == 0 {
        return Ok(Vec::new());
    }
    sqlx::query(
        "UPDATE groups SET paused_at = COALESCE(?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), next_open_at = NULL \
         WHERE id = ?",
    )
    .bind(now)
    .bind(group_id)
    .execute(&mut **tx)
    .await?;
    Ok(vec![Event::RoundClosed { round_id }, Event::RoomUpdated])
}

/// Votes per person, most voted first. An anonymous question's tallies carry no voter ids: this is
/// the one place results read who voted for whom (`anonymous`).
pub async fn tallies(db: &SqlitePool, round_id: i64) -> AppResult<Vec<Tally>> {
    let anonymous = crate::rooms::anonymous::is_anonymous(db, round_id).await?;
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT target_id, voter_id FROM votes WHERE round_id = ? ORDER BY created_at",
    )
    .bind(round_id)
    .fetch_all(db)
    .await?;
    let mut tallies: Vec<Tally> = Vec::new();
    for (target_id, voter_id) in rows {
        match tallies.iter_mut().find(|t| t.target_id == target_id) {
            Some(t) => {
                t.count += 1;
                t.voter_ids.push(voter_id);
            }
            None => tallies.push(Tally {
                target_id,
                count: 1,
                voter_ids: vec![voter_id],
            }),
        }
    }
    tallies.sort_by(|a, b| b.count.cmp(&a.count).then(a.target_id.cmp(&b.target_id)));
    if anonymous {
        for t in &mut tallies {
            t.voter_ids.clear();
        }
    }
    Ok(tallies)
}

pub async fn view(db: &SqlitePool, round: RoundRow, me: i64) -> AppResult<RoundView> {
    let voter_ids: Vec<i64> =
        sqlx::query_scalar("SELECT voter_id FROM votes WHERE round_id = ? ORDER BY created_at")
            .bind(round.id)
            .fetch_all(db)
            .await?;
    let my_vote: Option<i64> =
        sqlx::query_scalar("SELECT target_id FROM votes WHERE round_id = ? AND voter_id = ?")
            .bind(round.id)
            .bind(me)
            .fetch_optional(db)
            .await?;
    let mut tx = db.begin().await?;
    let active = active_count(&mut tx, round.group_id).await?;
    let (skips, skipped_ids, away_ids) = if round.closed_at.is_none() {
        let rows: Vec<(i64, i64, bool)> = sqlx::query_as(
            "SELECT target_id, COUNT(*), MAX(from_id = ?) FROM skips WHERE round_id = ? GROUP BY target_id",
        )
        .bind(me)
        .bind(round.id)
        .fetch_all(&mut *tx)
        .await?;
        let skips = rows
            .into_iter()
            .map(|(target_id, count, mine)| SkipState {
                target_id,
                count,
                mine,
            })
            .collect();
        let (skipped, away) = not_waited(&mut tx, round.group_id, round.id).await?;
        (skips, skipped, away)
    } else {
        (Vec::new(), Vec::new(), Vec::new())
    };
    tx.commit().await?;
    // Votes needed to close: everyone still in the group, minus those the group decided not to
    // wait for and those who are away.
    let member_count = active - union(&skipped_ids, &away_ids) as i64;
    let result = match round.closed_at {
        Some(_) => Some(tallies(db, round.id).await?),
        None => None,
    };
    let pinned = is_pinned(db, round.id).await?;
    let (ranking, my_ranking) = if round.kind == "rank" {
        let mine: Vec<i64> = sqlx::query_scalar(
            "SELECT member_id FROM rankings WHERE round_id = ? AND voter_id = ? ORDER BY position",
        )
        .bind(round.id)
        .bind(me)
        .fetch_all(db)
        .await?;
        let ranking = match round.closed_at {
            Some(_) => Some(borda(db, round.id).await?),
            None => None,
        };
        (ranking, if mine.is_empty() { None } else { Some(mine) })
    } else {
        (None, None)
    };
    let (items, my_choices, choose) = if round.kind == "choose" {
        let mine: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT member_id, item FROM choices WHERE round_id = ? AND voter_id = ? ORDER BY member_id",
        )
        .bind(round.id)
        .bind(me)
        .fetch_all(db)
        .await?;
        let choose = match round.closed_at {
            Some(_) => Some(choose_result(db, round.id).await?),
            None => None,
        };
        (
            crate::rounds::model::parse_items(round.items.as_deref()),
            (!mine.is_empty()).then(|| {
                mine.into_iter()
                    .map(|(member_id, item)| Choice { member_id, item })
                    .collect()
            }),
            choose,
        )
    } else {
        (None, None, None)
    };
    let suggested_by: Option<String> = sqlx::query_scalar(
        "SELECT u.name FROM questions q JOIN users u ON u.id = q.suggested_by WHERE q.id = ?",
    )
    .bind(round.question_id)
    .fetch_optional(db)
    .await?;
    let tripot = crate::rounds::tripot::view(db, &round, me).await?;
    let said = crate::rounds::said::view(
        db,
        round.id,
        &round.kind,
        round.closed_at.is_some(),
        Some(me),
    )
    .await?;
    Ok(RoundView {
        said,
        tripot,
        anonymous: round.anonymous,
        id: round.id,
        number: round.number,
        question: round.text,
        custom: round.custom,
        kind: round.kind,
        opened_at: round.opened_at,
        closed_at: round.closed_at,
        member_count,
        voter_ids,
        my_vote,
        result,
        pinned,
        skips,
        skip_needed: majority(active),
        skipped_ids,
        away_ids,
        ranking,
        my_ranking,
        suggested_by,
        items,
        my_choices,
        choose,
    })
}

/// A choose question's result: per person (member order) their own wire, the others' guesses and
/// the most guessed item(s); per voter how many guesses matched a real choice, best first.
pub async fn choose_result(db: &SqlitePool, round_id: i64) -> AppResult<ChooseResult> {
    let rows: Vec<(i64, i64, i64)> = sqlx::query_as(
        "SELECT voter_id, member_id, item FROM choices WHERE round_id = ? ORDER BY member_id, voter_id",
    )
    .bind(round_id)
    .fetch_all(db)
    .await?;
    Ok(compute_choose(&rows))
}

/// [`choose_result`] from `(voter, member, item)` wires.
pub fn compute_choose(rows: &[(i64, i64, i64)]) -> ChooseResult {
    use std::collections::BTreeMap;
    let real: BTreeMap<i64, i64> = rows
        .iter()
        .filter(|(v, m, _)| v == m)
        .map(|(_, m, i)| (*m, *i))
        .collect();
    let mut people: BTreeMap<i64, ChoosePerson> = BTreeMap::new();
    let mut scores: BTreeMap<i64, ChooseScore> = BTreeMap::new();
    for &(voter_id, member_id, item) in rows {
        let person = people.entry(member_id).or_insert_with(|| ChoosePerson {
            member_id,
            real: real.get(&member_id).copied(),
            top: Vec::new(),
            guesses: Vec::new(),
        });
        if voter_id == member_id {
            continue;
        }
        person.guesses.push(ChooseGuess { voter_id, item });
        if let Some(&truth) = real.get(&member_id) {
            let score = scores.entry(voter_id).or_insert(ChooseScore {
                member_id: voter_id,
                hits: 0,
                total: 0,
            });
            score.total += 1;
            score.hits += i64::from(item == truth);
        }
    }
    // Voters whose guesses could not be checked (nobody else answered) still get a line.
    for &(voter_id, _, _) in rows {
        scores.entry(voter_id).or_insert(ChooseScore {
            member_id: voter_id,
            hits: 0,
            total: 0,
        });
    }
    for person in people.values_mut() {
        let mut counts: BTreeMap<i64, i64> = BTreeMap::new();
        for g in &person.guesses {
            *counts.entry(g.item).or_default() += 1;
        }
        let best = counts.values().copied().max().unwrap_or(0);
        person.top = counts
            .into_iter()
            .filter(|&(_, n)| n == best && best > 0)
            .map(|(i, _)| i)
            .collect();
    }
    let mut scores: Vec<ChooseScore> = scores.into_values().collect();
    // Best first: most hits, then the better ratio (fewer misses), then member order.
    // Nothing checkable (0/0) goes after any real score, 0/3 included.
    scores.sort_by(|a, b| {
        b.hits
            .cmp(&a.hits)
            .then((b.total > 0).cmp(&(a.total > 0)))
            .then((a.total - a.hits).cmp(&(b.total - b.hits)))
            .then(a.member_id.cmp(&b.member_id))
    });
    ChooseResult {
        people: people.into_values().collect(),
        scores,
    }
}

/// Points per member for a ranking question: with N members ranked, 1st gets N points, last gets 1.
pub async fn borda(db: &SqlitePool, round_id: i64) -> AppResult<Vec<RankEntry>> {
    let rows: Vec<(i64, i64, f64, i64)> = sqlx::query_as(
        "SELECT r.member_id, \
                SUM(n.size - r.position + 1), AVG(CAST(r.position AS REAL)), SUM(r.position = 1) \
         FROM rankings r \
         JOIN (SELECT voter_id, COUNT(*) AS size FROM rankings WHERE round_id = ?1 GROUP BY voter_id) n \
           ON n.voter_id = r.voter_id \
         WHERE r.round_id = ?1 GROUP BY r.member_id \
         ORDER BY 2 DESC, 4 DESC, 3 ASC, r.member_id",
    )
    .bind(round_id)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(member_id, points, average, firsts)| RankEntry {
            member_id,
            points,
            average,
            firsts,
        })
        .collect())
}

pub async fn is_pinned(db: &SqlitePool, round_id: i64) -> AppResult<bool> {
    let pinned: Option<i64> = sqlx::query_scalar("SELECT 1 FROM pins WHERE round_id = ?")
        .bind(round_id)
        .fetch_optional(db)
        .await?;
    Ok(pinned.is_some())
}
