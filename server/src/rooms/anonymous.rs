//! Anonymous votes (docs/ANONYMOUS.md): a result shows counts only, never who voted for whom.
//!
//! The room owner picks a rule (`groups.anonymous_votes`): `never`, `per_question` (a question's own
//! `questions.anonymous` decides) or `always`. What applies to a question is decided when it opens
//! and kept on the round (`rounds.anonymous`): changing the rule later never reveals a past or open
//! question's votes; a new rule can make the open question anonymous (only that way round).
//! Pick and ranking questions only, and never in a party: a choose question's reveal *is* who took
//! what, and a party plays its reveal on a shared screen.
//!
//! Every per-voter read of `votes` / `rankings` for a result goes through [`crate::rounds::game::tallies`]
//! (voter ids dropped), [`crate::rounds::game::borda`] (points only) or the stats (anonymous questions
//! count as votes cast and received, never as who-for-whom). Features that must hide themselves
//! on an anonymous question read [`is_anonymous`] (or `rounds.anonymous` in SQL).

use sqlx::{Sqlite, Transaction};

use crate::platform::error::{AppError, AppResult};

/// The room rules, as stored and sent.
pub const MODES: [&str; 3] = ["never", "per_question", "always"];

/// A setting value from the app (`400 bad_setting` otherwise).
pub fn clean_mode(mode: &str) -> AppResult<&'static str> {
    MODES
        .iter()
        .find(|m| **m == mode)
        .copied()
        .ok_or(AppError::BadRequest("bad_setting"))
}

/// The SQL deciding a round's anonymity from its room and question (`rounds` row in scope).
macro_rules! decided {
    () => {
        "(SELECT g.kind = 'room' AND q.kind IN ('pick', 'rank') \
           AND (g.anonymous_votes = 'always' OR (g.anonymous_votes = 'per_question' AND q.anonymous)) \
          FROM groups g, questions q WHERE g.id = rounds.group_id AND q.id = rounds.question_id)"
    };
}

/// Called as a question opens: records whether its votes are anonymous.
pub async fn decide(tx: &mut Transaction<'_, Sqlite>, round_id: i64) -> AppResult<()> {
    sqlx::query(concat!(
        "UPDATE rounds SET anonymous = COALESCE(",
        decided!(),
        ", 0) WHERE id = ?"
    ))
    .bind(round_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// The owner changed the rule (a room, not a party: checked by the caller). The open question
/// can only become anonymous (`always`, or `per_question` with a question that asks for it):
/// nobody saw who voted for whom yet, so it is safe; nothing ever goes the other way.
pub async fn set_mode(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: i64,
    mode: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE groups SET anonymous_votes = ? WHERE id = ?")
        .bind(mode)
        .bind(group_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(concat!(
        "UPDATE rounds SET anonymous = 1 WHERE group_id = ? AND closed_at IS NULL AND NOT anonymous \
         AND COALESCE(",
        decided!(),
        ", 0)"
    ))
    .bind(group_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// The room's rule (`per_question` for a room that is gone).
pub async fn mode<'e, E: sqlx::SqliteExecutor<'e>>(db: E, group_id: i64) -> AppResult<String> {
    Ok(
        sqlx::query_scalar("SELECT anonymous_votes FROM groups WHERE id = ?")
            .bind(group_id)
            .fetch_optional(db)
            .await?
            .unwrap_or_else(|| "per_question".into()),
    )
}

/// Is this round's result anonymous (`rounds.anonymous`, decided when it opened)? The one check
/// for any feature that must not show who voted for whom (a bet on a result, a reveal, a recap).
/// `false` for a round that doesn't exist.
pub async fn is_anonymous<'e, E: sqlx::SqliteExecutor<'e>>(
    db: E,
    round_id: i64,
) -> AppResult<bool> {
    Ok(
        sqlx::query_scalar("SELECT anonymous FROM rounds WHERE id = ?")
            .bind(round_id)
            .fetch_optional(db)
            .await?
            .unwrap_or(false),
    )
}

/// A question as added (or imported, or installed from a pack) asks to be anonymous: only pick
/// and ranking questions can.
pub fn wanted(kind: &str, anonymous: bool) -> bool {
    anonymous && matches!(kind, "pick" | "rank")
}
