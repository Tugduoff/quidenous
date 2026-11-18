use crate::platform::ids::{MemberId, RoomId, RoundId, UserId};
use std::collections::HashMap;

use serde_json::json;
use sqlx::SqlitePool;

use crate::{
    AppState,
    insights::streaks::{self, Room},
    platform::error::AppResult,
    platform::i18n::Lang,
    realtime::live::Event,
};

use super::{CATALOGUE, Metrics, find, inbox_text};

pub(super) struct Seats {
    rooms: Vec<(MemberId, RoomId)>,
    parties: Vec<MemberId>,
}

impl Seats {
    async fn load(db: &SqlitePool, user_id: UserId) -> AppResult<Self> {
        let rows: Vec<(MemberId, RoomId, bool)> = sqlx::query_as(
            "SELECT m.id, m.group_id, g.kind = 'party' FROM members m JOIN groups g ON g.id = m.group_id \
             WHERE m.user_id = ? ORDER BY m.id",
        )
        .bind(user_id)
        .fetch_all(db)
        .await?;
        let mut seats = Seats {
            rooms: Vec::new(),
            parties: Vec::new(),
        };
        for (id, group, party) in rows {
            if party {
                seats.parties.push(id);
            } else {
                seats.rooms.push((id, group));
            }
        }
        Ok(seats)
    }

    fn room_json(&self) -> String {
        json!(self.rooms.iter().map(|s| s.0).collect::<Vec<_>>()).to_string()
    }
}

#[derive(Clone, Copy)]
pub enum Streaks {
    All,
    Only(RoomId),
}

pub type RoomCache = HashMap<RoomId, Option<Room>>;

pub async fn metrics(
    db: &SqlitePool,
    user_id: UserId,
    streak_rooms: Streaks,
    cache: &mut RoomCache,
) -> AppResult<Metrics> {
    let seats = Seats::load(db, user_id).await?;
    let mut m: Metrics = [0; CATALOGUE.len()];
    let tz: i64 = sqlx::query_scalar("SELECT tz_offset FROM notification_prefs WHERE user_id = ?")
        .bind(user_id)
        .fetch_optional(db)
        .await?
        .unwrap_or(0);
    let rooms = seats.room_json();
    let parties = json!(seats.parties).to_string();
    if !seats.rooms.is_empty() {
        let (cast, selfish, eligible, night, first): (i64, i64, i64, i64, i64) =
            sqlx::query_as(concat!(
                "SELECT COUNT(*), \
                   COALESCE(SUM(q.kind <> 'choose' AND NOT r.anonymous AND v.voter_id = v.target_id), 0), \
                   COALESCE(SUM(q.kind <> 'choose' AND NOT r.anonymous), 0), \
                   COALESCE(SUM(CAST(strftime('%H', v.created_at, ?2) AS INTEGER) NOT BETWEEN 5 AND 22), 0), \
                   COALESCE(SUM(r.closed_at IS NOT NULL \
                     AND EXISTS (SELECT 1 FROM votes o WHERE o.round_id = v.round_id AND o.voter_id <> v.voter_id) \
                     AND NOT EXISTS (SELECT 1 FROM votes o WHERE o.round_id = v.round_id \
                       AND (o.created_at < v.created_at OR (o.created_at = v.created_at AND o.voter_id < v.voter_id)))), 0) \
                 FROM votes v JOIN rounds r ON r.id = v.round_id JOIN questions q ON q.id = r.question_id \
                 WHERE v.voter_id IN ",
                mine!()
            ))
            .bind(&rooms)
            .bind(format!("{tz:+} minutes"))
            .fetch_one(db)
            .await?;
        let (pick_wins, rank_wins, evidence, flair): (i64, i64, i64, i64) = sqlx::query_as(concat!(
            "WITH s AS (SELECT id, group_id FROM members WHERE id IN ",
            mine!(),
            "), r AS (SELECT r.id, s.id AS me, q.kind FROM rounds r JOIN s ON s.group_id = r.group_id \
                JOIN questions q ON q.id = r.question_id \
                WHERE r.closed_at IS NOT NULL AND NOT r.anonymous AND q.kind IN ('pick', 'rank')), \
             t AS (SELECT v.round_id, v.target_id, COUNT(*) AS n FROM votes v \
                JOIN r ON r.id = v.round_id AND r.kind = 'pick' GROUP BY v.round_id, v.target_id), \
             tot AS (SELECT round_id, MAX(n) AS best, SUM(n) AS total FROM t GROUP BY round_id), \
             b AS (SELECT k.round_id, k.voter_id, COUNT(*) AS size FROM rankings k \
                JOIN r ON r.id = k.round_id AND r.kind = 'rank' GROUP BY k.round_id, k.voter_id), \
             p AS (SELECT k.round_id, k.member_id, SUM(b.size - k.position + 1) AS pts FROM rankings k \
                JOIN b ON b.round_id = k.round_id AND b.voter_id = k.voter_id GROUP BY k.round_id, k.member_id), \
             pb AS (SELECT round_id, MAX(pts) AS best FROM p GROUP BY round_id) \
             SELECT \
               (SELECT COUNT(*) FROM r JOIN t ON t.round_id = r.id AND t.target_id = r.me \
                  JOIN tot ON tot.round_id = r.id WHERE t.n = tot.best), \
               (SELECT COUNT(*) FROM r JOIN p ON p.round_id = r.id AND p.member_id = r.me \
                  JOIN pb ON pb.round_id = r.id WHERE p.pts = pb.best), \
               (SELECT COUNT(*) FROM r JOIN t ON t.round_id = r.id AND t.target_id = r.me \
                  JOIN tot ON tot.round_id = r.id WHERE tot.total >= 3 AND t.n = tot.total), \
               (SELECT COUNT(*) FROM r JOIN votes v ON v.round_id = r.id AND v.voter_id = r.me \
                  JOIN tot ON tot.round_id = r.id JOIN t ON t.round_id = r.id AND t.target_id = v.target_id \
                  WHERE tot.total >= 3 AND v.target_id <> r.me AND t.n = tot.best)"
        ))
        .bind(&rooms)
        .fetch_one(db)
        .await?;
        let (messages, nudges, bets, written): (i64, i64, i64, i64) = sqlx::query_as(concat!(
            "SELECT (SELECT COUNT(*) FROM messages WHERE member_id IN ",
            mine!(),
            "), (SELECT COUNT(*) FROM nudges WHERE from_id IN ",
            mine!(),
            "), (SELECT COUNT(*) FROM tripot_bets WHERE member_id IN ",
            mine!(),
            " AND payout > 0 AND NOT refunded), \
             (SELECT COUNT(*) FROM questions q WHERE q.group_id IN \
                (SELECT group_id FROM members WHERE id IN ",
            mine!(),
            ") AND q.author_id IN ",
            mine!(),
            " AND q.origin = 'written' AND EXISTS (SELECT 1 FROM rounds r \
                WHERE r.group_id = q.group_id AND r.question_id = q.id AND r.closed_at IS NOT NULL \
                AND EXISTS (SELECT 1 FROM votes v WHERE v.round_id = r.id)))"
        ))
        .bind(&rooms)
        .fetch_one(db)
        .await?;
        m[0] = cast;
        m[1] = pick_wins + rank_wins;
        m[2] = evidence;
        m[3] = bets;
        m[4] = flair;
        m[5] = if selfish == 0 { eligible } else { 0 };
        m[6] = selfish;
        m[8] = first;
        m[9] = written;
        m[10] = messages;
        m[11] = nudges;
        m[12] = night;
        let now = streaks::now_secs();
        for &(seat, group) in &seats.rooms {
            if let Streaks::Only(only) = streak_rooms
                && only != group
            {
                continue;
            }
            if let std::collections::hash_map::Entry::Vacant(e) = cache.entry(group) {
                e.insert(Room::load_history(db, group).await?);
            }
            if let Some(Some(room)) = cache.get(&group) {
                let days = room.days(seat, tz, now);
                let best = streaks::streak(&days, streaks::local_day(now, tz)).best;
                m[7] = m[7].max(best);
            }
        }
    }
    if !seats.parties.is_empty() {
        m[13] = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT p.group_id) FROM members p WHERE p.id IN (SELECT value FROM json_each(?)) \
             AND EXISTS (SELECT 1 FROM votes v WHERE v.voter_id = p.id)",
        )
        .bind(&parties)
        .fetch_one(db)
        .await?;
    }
    Ok(m)
}

pub async fn award(
    state: &AppState,
    user_id: UserId,
    values: &Metrics,
    group_id: Option<RoomId>,
) -> AppResult<Vec<(&'static str, i64)>> {
    let (evaluated, held): (bool, String) = sqlx::query_as(
        "SELECT badges_at IS NOT NULL, \
           COALESCE((SELECT json_group_array(key || ':' || tier) FROM user_badges WHERE user_id = ?1), '[]') \
         FROM users WHERE id = ?1 AND deleted_at IS NULL",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .unwrap_or((true, "[]".into()));
    let held: Vec<String> = serde_json::from_str(&held).unwrap_or_default();
    let missing = CATALOGUE.iter().zip(values).any(|(def, v)| {
        (1..=def.tier_for(*v)).any(|t| !held.contains(&format!("{}:{t}", def.key)))
    });
    if evaluated && !missing {
        return Ok(Vec::new());
    }
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let user: Option<(Option<String>, Option<String>)> =
        sqlx::query_as("SELECT badges_at, locale FROM users WHERE id = ? AND deleted_at IS NULL")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some((badges_at, locale)) = user else {
        return Ok(Vec::new());
    };
    let silent = badges_at.is_none();
    let mut news = Vec::new();
    for (def, value) in CATALOGUE.iter().zip(values) {
        let mut highest = 0;
        for tier in 1..=def.tier_for(*value) {
            let added = sqlx::query(
                "INSERT OR IGNORE INTO user_badges (user_id, key, tier, group_id) VALUES (?, ?, ?, ?)",
            )
            .bind(user_id)
            .bind(def.key)
            .bind(tier)
            .bind(group_id)
            .execute(&mut *tx)
            .await?;
            if added.rows_affected() > 0 {
                highest = tier;
            }
        }
        if highest > 0 {
            news.push((def.key, highest));
        }
    }
    sqlx::query("UPDATE users SET badges_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    let mut told = None;
    if !silent && !news.is_empty() {
        told = inbox_seat(&mut tx, user_id, group_id).await?;
        if let Some((seat, _)) = told {
            let lang = Lang::from_db(locale.as_deref());
            for (key, tier) in &news {
                let Some(def) = find(key) else { continue };
                sqlx::query("INSERT INTO inbox (member_id, kind, text) VALUES (?, 'badge', ?)")
                    .bind(seat)
                    .bind(inbox_text(def, *tier, lang))
                    .execute(&mut *tx)
                    .await?;
            }
        }
    }
    tx.commit().await?;
    if let Some((_, room)) = told {
        state.live.send(room, Event::InboxChanged);
    }
    Ok(news)
}

pub(crate) async fn inbox_seat(
    tx: &mut sqlx::SqliteConnection,
    user_id: UserId,
    group_id: Option<RoomId>,
) -> AppResult<Option<(MemberId, RoomId)>> {
    Ok(sqlx::query_as(
        "SELECT m.id, m.group_id FROM members m \
         JOIN groups g ON g.id = m.group_id AND g.kind = 'room' AND g.deleted_at IS NULL \
         WHERE m.user_id = ?1 AND m.removed_at IS NULL \
         ORDER BY m.group_id IS ?2 DESC, \
           m.group_id IS (SELECT last_group_id FROM users WHERE id = ?1) DESC, \
           COALESCE(m.active_at, m.created_at) DESC, m.id DESC LIMIT 1",
    )
    .bind(user_id)
    .bind(group_id)
    .fetch_optional(&mut *tx)
    .await?)
}

pub async fn evaluate(
    state: &AppState,
    user_ids: &[UserId],
    group_id: Option<RoomId>,
) -> AppResult<Vec<(UserId, &'static str, i64)>> {
    let mut cache = RoomCache::new();
    let streak_rooms = group_id.map_or(Streaks::All, Streaks::Only);
    let mut out = Vec::new();
    for &user in user_ids {
        let values = metrics(&state.db, user, streak_rooms, &mut cache).await?;
        for (key, tier) in award(state, user, &values, group_id).await? {
            out.push((user, key, tier));
        }
    }
    Ok(out)
}

pub async fn after_close_now(
    state: &AppState,
    group_id: RoomId,
) -> AppResult<Vec<(UserId, &'static str, i64)>> {
    let round: Option<RoundId> = sqlx::query_scalar(
        "SELECT id FROM rounds WHERE group_id = ? AND closed_at IS NOT NULL ORDER BY closed_at DESC, id DESC LIMIT 1",
    )
    .bind(group_id)
    .fetch_optional(&state.db)
    .await?;
    let Some(round) = round else {
        return Ok(Vec::new());
    };
    let users: Vec<UserId> = sqlx::query_scalar(
        "SELECT DISTINCT m.user_id FROM members m WHERE m.user_id IS NOT NULL AND m.id IN ( \
           SELECT voter_id FROM votes WHERE round_id = ?1 UNION SELECT target_id FROM votes WHERE round_id = ?1 \
           UNION SELECT member_id FROM rankings WHERE round_id = ?1 \
           UNION SELECT q.author_id FROM rounds r JOIN questions q ON q.id = r.question_id WHERE r.id = ?1) \
         ORDER BY m.user_id",
    )
    .bind(round)
    .fetch_all(&state.db)
    .await?;
    evaluate(state, &users, Some(group_id)).await
}

pub fn after_close(state: &AppState, group_id: RoomId) {
    let state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = after_close_now(&state, group_id).await {
            tracing::warn!(%group_id, error = ?e, "badges after a question closed");
        }
    });
}

pub(super) const SWEEP_EVERY: std::time::Duration = std::time::Duration::from_secs(3600);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Swept {
    pub evaluated: usize,
    pub earned: usize,
    pub wrapped_notices: usize,
}

pub async fn sweep_at(state: &AppState, now: i64) -> AppResult<Swept> {
    let users: Vec<UserId> = sqlx::query_scalar(
        "SELECT DISTINCT m.user_id FROM members m JOIN users u ON u.id = m.user_id AND u.deleted_at IS NULL \
         WHERE m.active_at >= strftime('%Y-%m-%dT%H:%M:%fZ', ?, 'unixepoch', '-2 hours') ORDER BY m.user_id",
    )
    .bind(now)
    .fetch_all(&state.db)
    .await?;
    let mut swept = Swept {
        evaluated: users.len(),
        ..Default::default()
    };
    let mut cache = RoomCache::new();
    for user in users {
        let earned = async {
            let values = metrics(&state.db, user, Streaks::All, &mut cache).await?;
            award(state, user, &values, None).await
        }
        .await;
        match earned {
            Ok(news) => swept.earned += news.len(),
            Err(e) => tracing::warn!(%user, error = ?e, "badge sweep"),
        }
    }
    swept.wrapped_notices = crate::insights::wrapped::notify_at(state, now).await?;
    Ok(swept)
}

pub fn spawn_sweeper(state: AppState) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(SWEEP_EVERY);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            match sweep_at(&state, streaks::now_secs()).await {
                Ok(s) if s.earned == 0 && s.wrapped_notices == 0 => {}
                Ok(s) => tracing::info!(
                    evaluated = s.evaluated,
                    earned = s.earned,
                    wrapped = s.wrapped_notices,
                    "badge sweep"
                ),
                Err(e) => tracing::warn!(error = ?e, "badge sweep"),
            }
        }
    });
}
