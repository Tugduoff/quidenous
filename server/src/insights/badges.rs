//! Titles & badges (docs/BADGES.md): a catalogue of badges earned from real play, 3 tiers each
//! (bronze, silver, gold), computed per account over all its seats. Each earned badge is a *title*
//! the account may display next to its name.
//!
//! Awarding is idempotent ([`evaluate`]): `INSERT OR IGNORE` one row per tier reached, and one inbox
//! entry per badge for its highest new tier, except on an account's very first evaluation
//! (`users.badges_at` null), which backfills silently. It runs after a question closes (spawned,
//! post-commit: [`after_close`]), when someone opens their collection (`GET /badges`), and hourly for
//! the accounts active lately ([`spawn_sweeper`], which also posts the Wrapped notice in December).
//! Badges are never taken back.

use std::collections::HashMap;

use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sqlx::SqlitePool;

use crate::{
    AppState,
    accounts::auth::{Me, UserId},
    insights::streaks::{self, Room},
    platform::error::{AppError, AppResult},
    platform::i18n::Lang,
    realtime::live::Event,
};

/// One badge of the catalogue. Names are the titles (French is the source).
pub struct BadgeDef {
    pub key: &'static str,
    pub fr: &'static str,
    pub en: &'static str,
    /// Lucide icon, kebab-case.
    pub icon: &'static str,
    /// The mascot of its family (docs/MASCOTS.md).
    pub mascot: &'static str,
    /// Bronze, silver and gold thresholds of its metric.
    pub tiers: [i64; 3],
}

impl BadgeDef {
    pub fn name(&self, lang: Lang) -> &'static str {
        lang.pick(self.fr, self.en)
    }

    /// The highest tier a metric value reaches (0 = none).
    pub fn tier_for(&self, value: i64) -> i64 {
        self.tiers.iter().filter(|t| value >= **t).count() as i64
    }
}

const fn def(
    key: &'static str,
    fr: &'static str,
    en: &'static str,
    icon: &'static str,
    mascot: &'static str,
    tiers: [i64; 3],
) -> BadgeDef {
    BadgeDef {
        key,
        fr,
        en,
        icon,
        mascot,
        tiers,
    }
}

/// Every badge, in display order. Indexes match [`Metrics`].
pub const CATALOGUE: [BadgeDef; 14] = [
    def(
        "pilier",
        "Le pilier",
        "The pillar",
        "landmark",
        "pave",
        [50, 250, 1000],
    ),
    def(
        "suspect",
        "Le suspect habituel",
        "The usual suspect",
        "fingerprint",
        "bulle",
        [5, 25, 100],
    ),
    def(
        "evidence",
        "L'évidence",
        "The obvious one",
        "target",
        "bulle",
        [1, 5, 20],
    ),
    def(
        "devin",
        "Le devin",
        "The oracle",
        "eye",
        "pave",
        [3, 15, 50],
    ),
    def(
        "flair",
        "Le flair",
        "The nose",
        "radar",
        "zeste",
        [25, 100, 400],
    ),
    def(
        "incorruptible",
        "L'incorruptible",
        "The incorruptible",
        "shield-check",
        "pave",
        [30, 150, 500],
    ),
    def(
        "narcisse",
        "Narcisse",
        "Narcissus",
        "flower-2",
        "paillette",
        [5, 20, 50],
    ),
    def(
        "marathon",
        "Le marathonien",
        "The marathoner",
        "flame",
        "paillette",
        [7, 30, 100],
    ),
    def(
        "gachette",
        "La gâchette",
        "Quick draw",
        "zap",
        "zeste",
        [10, 50, 200],
    ),
    def(
        "plume",
        "La plume",
        "The quill",
        "feather",
        "pave",
        [3, 15, 50],
    ),
    def(
        "bavard",
        "Le bavard",
        "The chatterbox",
        "message-circle",
        "paillette",
        [50, 250, 1000],
    ),
    def(
        "relou",
        "Le relou",
        "The nag",
        "bell-ring",
        "zeste",
        [10, 50, 200],
    ),
    def(
        "couche_tard",
        "Le couche-tard",
        "The night owl",
        "moon",
        "roupi",
        [10, 50, 200],
    ),
    def(
        "fetard",
        "Le fêtard",
        "The party animal",
        "party-popper",
        "paillette",
        [1, 5, 20],
    ),
];

/// One value per badge of [`CATALOGUE`], same order.
pub type Metrics = [i64; CATALOGUE.len()];

/// The catalogue entry of a key.
pub fn find(key: &str) -> Option<&'static BadgeDef> {
    CATALOGUE.iter().find(|b| b.key == key)
}

/// "bronze" / "argent" / "or" (English: bronze, silver, gold).
pub fn tier_name(tier: i64, lang: Lang) -> &'static str {
    match tier {
        3 => lang.pick("or", "gold"),
        2 => lang.pick("argent", "silver"),
        _ => "bronze",
    }
}

/// How to reach `n` (the next tier), or how it was earned once maxed (`done`).
pub fn hint(key: &str, n: i64, lang: Lang, done: bool) -> String {
    let s = if n > 1 { "s" } else { "" };
    let fr = |next: String, got: String| if done { got } else { next };
    match lang {
        Lang::Fr => match key {
            "pilier" => fr(format!("Vote {n} fois"), format!("{n} votes et plus")),
            "suspect" => fr(
                format!("Gagne {n} questions"),
                format!("{n} questions gagnées et plus"),
            ),
            "evidence" => fr(
                format!("Récolte toutes les voix (3 ou plus) {n} fois"),
                format!("Toutes les voix pour toi, {n} fois et plus"),
            ),
            "devin" => fr(
                format!("Gagne {n} paris au Tripot"),
                format!("{n} paris gagnés au Tripot et plus"),
            ),
            "flair" => fr(
                format!("Vote {n} fois pour le gagnant (pas pour toi)"),
                format!("{n} votes pour le gagnant et plus"),
            ),
            "incorruptible" => fr(
                format!("Vote {n} fois sans jamais voter pour toi"),
                format!("{n} votes, jamais pour toi"),
            ),
            "narcisse" => fr(
                format!("Vote {n} fois pour toi-même"),
                format!("{n} votes pour toi-même et plus"),
            ),
            "marathon" => fr(
                format!("Vote {n} jours d'affilée dans un salon"),
                format!("{n} jours d'affilée et plus"),
            ),
            "gachette" => fr(
                format!("Vote en premier sur {n} questions"),
                format!("Premier à voter, {n} fois et plus"),
            ),
            "plume" => fr(
                format!("Écris {n} questions qui sont posées"),
                format!("{n} questions écrites et posées et plus"),
            ),
            "bavard" => fr(
                format!("Envoie {n} messages dans le chat"),
                format!("{n} messages et plus"),
            ),
            "relou" => fr(
                format!("Relance les autres {n} fois"),
                format!("{n} relances et plus"),
            ),
            "couche_tard" => fr(
                format!("Vote {n} fois entre 23 h et 5 h"),
                format!("{n} votes nocturnes et plus"),
            ),
            "fetard" => fr(
                format!("Vote dans {n} soirée{s}"),
                format!("{n} soirée{s} et plus"),
            ),
            _ => String::new(),
        },
        Lang::En => match key {
            "pilier" => fr(format!("Vote {n} times"), format!("{n}+ votes")),
            "suspect" => fr(format!("Win {n} questions"), format!("{n}+ questions won")),
            "evidence" => fr(
                format!("Get every vote (3 or more) {n} time{s}"),
                format!("Every vote for you, {n}+ times"),
            ),
            "devin" => fr(
                format!("Win {n} Tripot bets"),
                format!("{n}+ Tripot bets won"),
            ),
            "flair" => fr(
                format!("Vote for the winner {n} times (not yourself)"),
                format!("{n}+ votes for the winner"),
            ),
            "incorruptible" => fr(
                format!("Vote {n} times, never for yourself"),
                format!("{n}+ votes, never for yourself"),
            ),
            "narcisse" => fr(
                format!("Vote for yourself {n} times"),
                format!("{n}+ votes for yourself"),
            ),
            "marathon" => fr(
                format!("Vote {n} days in a row in a room"),
                format!("{n}+ days in a row"),
            ),
            "gachette" => fr(
                format!("Vote first on {n} questions"),
                format!("First to vote, {n}+ times"),
            ),
            "plume" => fr(
                format!("Write {n} questions that get asked"),
                format!("{n}+ questions written and asked"),
            ),
            "bavard" => fr(format!("Send {n} chat messages"), format!("{n}+ messages")),
            "relou" => fr(
                format!("Nudge the others {n} times"),
                format!("{n}+ nudges"),
            ),
            "couche_tard" => fr(
                format!("Vote {n} times between 11 pm and 5 am"),
                format!("{n}+ late-night votes"),
            ),
            "fetard" => fr(
                format!("Vote at {n} part{}", if n > 1 { "ies" } else { "y" }),
                format!("{n}+ parties"),
            ),
            _ => String::new(),
        },
    }
}

/// « Nouveau titre : Le pilier (or) » / "New title: The pillar (gold)".
pub fn inbox_text(def: &BadgeDef, tier: i64, lang: Lang) -> String {
    match lang {
        Lang::Fr => format!(
            "Nouveau titre : {} ({})",
            def.name(lang),
            tier_name(tier, lang)
        ),
        Lang::En => format!("New title: {} ({})", def.name(lang), tier_name(tier, lang)),
    }
}

// ---------- metrics ----------

/// Room seats of an account (any state, deleted rooms included) and its party seats.
struct Seats {
    rooms: Vec<(i64, i64)>,
    parties: Vec<i64>,
}

impl Seats {
    async fn load(db: &SqlitePool, user_id: i64) -> AppResult<Self> {
        let rows: Vec<(i64, i64, bool)> = sqlx::query_as(
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

/// Which rooms the day streaks (`marathon`) are replayed for.
#[derive(Clone, Copy)]
pub enum Streaks {
    /// Every room of the account (the collection, the sweep).
    All,
    /// Only this one (right after one of its questions closed: the others haven't changed).
    Only(i64),
}

/// Rooms loaded once per evaluation run (several accounts share them).
pub type RoomCache = HashMap<i64, Option<Room>>;

/// SQL: my room seats (`?1`, a JSON array of member ids).
macro_rules! mine {
    () => {
        "(SELECT value FROM json_each(?1))"
    };
}

/// Every metric of an account. A few aggregate queries over its seats (indexed: `votes_voter`,
/// `messages_member`, `nudges_from`, `tripot_bets_member`, rounds by room) and the streak replay.
pub async fn metrics(
    db: &SqlitePool,
    user_id: i64,
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
        // My votes: cast, self votes and the eligible ones (pick and ranking, not anonymous), late at
        // night in my time zone, and the closed questions I voted on first (2 voters or more).
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
        // Questions won (pick: most votes, ties count; ranking: most points), all mine (3 votes or
        // more), and my votes for a winner other than me (3 votes or more). Closed, not anonymous.
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
        // Messages, nudges, Tripot bets won, questions written and asked.
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
        // Best day streak, in my time zone.
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

// ---------- awarding ----------

/// Records what `values` reach: every tier as a row (`INSERT OR IGNORE`, so evaluating twice adds
/// nothing), then one inbox entry per badge for its highest new tier, unless this is the account's
/// first evaluation (silent backfill). `group_id`: the room where it happened (its seat gets the
/// entry). Returns the badges newly earned, `(key, highest new tier)`.
pub async fn award(
    state: &AppState,
    user_id: i64,
    values: &Metrics,
    group_id: Option<i64>,
) -> AppResult<Vec<(&'static str, i64)>> {
    // Nothing new and already evaluated once: no write at all (spares the SD card).
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

/// The seat that hears about it: mine in `group_id` when it is a room I'm still in, else the room I
/// used last, else my most recently active room. `(member, room)`.
pub(crate) async fn inbox_seat(
    tx: &mut sqlx::SqliteConnection,
    user_id: i64,
    group_id: Option<i64>,
) -> AppResult<Option<(i64, i64)>> {
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

/// Evaluates these accounts: their metrics, then [`award`]. With `group_id` (a question of that
/// room just closed), the day streaks are only replayed for that room. Returns what was earned.
pub async fn evaluate(
    state: &AppState,
    user_ids: &[i64],
    group_id: Option<i64>,
) -> AppResult<Vec<(i64, &'static str, i64)>> {
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

/// The accounts a question that just closed in `group_id` may have changed: its voters, the people
/// voted for or ranked, and its author.
pub async fn after_close_now(
    state: &AppState,
    group_id: i64,
) -> AppResult<Vec<(i64, &'static str, i64)>> {
    let round: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM rounds WHERE group_id = ? AND closed_at IS NOT NULL ORDER BY closed_at DESC, id DESC LIMIT 1",
    )
    .bind(group_id)
    .fetch_optional(&state.db)
    .await?;
    let Some(round) = round else {
        return Ok(Vec::new());
    };
    let users: Vec<i64> = sqlx::query_scalar(
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

/// After a question closed (committed): [`after_close_now`] in the background. Errors are logged,
/// never the request's.
pub fn after_close(state: &AppState, group_id: i64) {
    let state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = after_close_now(&state, group_id).await {
            tracing::warn!(group_id, error = ?e, "badges after a question closed");
        }
    });
}

/// How often the sweep runs (first run at startup).
const SWEEP_EVERY: std::time::Duration = std::time::Duration::from_secs(3600);

/// What a sweep did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Swept {
    pub evaluated: usize,
    pub earned: usize,
    pub wrapped_notices: usize,
}

/// The hourly sweep at `now` (unix seconds): accounts used in the last 2 hours are evaluated (Tripot
/// settles, party deadlines, nudges and chat don't go through a question closing in their room), and
/// in the Wrapped window the "your year is ready" notice goes out (docs/WRAPPED.md).
pub async fn sweep_at(state: &AppState, now: i64) -> AppResult<Swept> {
    let users: Vec<i64> = sqlx::query_scalar(
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
            Err(e) => tracing::warn!(user, error = ?e, "badge sweep"),
        }
    }
    swept.wrapped_notices = crate::insights::wrapped::notify_at(state, now).await?;
    Ok(swept)
}

/// The background task for [`sweep_at`] (errors are logged; the next run tries again).
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

// ---------- API ----------

#[derive(Debug, Clone, Serialize)]
pub struct Badge {
    pub key: &'static str,
    pub icon: &'static str,
    pub mascot: &'static str,
    pub name: &'static str,
    pub hint: String,
    pub tier: i64,
    pub tiers: [i64; 3],
    pub progress: i64,
    pub earned_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TitleChip {
    pub key: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub tier: i64,
}

impl TitleChip {
    pub fn new(key: &str, tier: i64, lang: Lang) -> Option<Self> {
        let def = find(key)?;
        (1..=3).contains(&tier).then(|| TitleChip {
            key: def.key,
            name: def.name(lang),
            icon: def.icon,
            tier,
        })
    }
}

/// An account's earned badges: key → (highest tier, when it was earned).
pub async fn earned(db: &SqlitePool, user_id: i64) -> AppResult<HashMap<String, (i64, String)>> {
    let rows: Vec<(String, i64, String)> = sqlx::query_as(
        "SELECT key, tier, earned_at FROM user_badges b WHERE user_id = ? \
         AND tier = (SELECT MAX(tier) FROM user_badges x WHERE x.user_id = b.user_id AND x.key = b.key)",
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(key, tier, at)| (key, (tier, at)))
        .collect())
}

/// `GET /badges`: my collection, in catalogue order (evaluated first, so a fresh threshold shows).
pub async fn list(State(state): State<AppState>, UserId(user): UserId) -> AppResult<Json<Value>> {
    let lang = crate::platform::i18n::of_user(&state.db, user).await?;
    let values = metrics(&state.db, user, Streaks::All, &mut RoomCache::new()).await?;
    if let Err(e) = award(&state, user, &values, None).await {
        tracing::warn!(user, error = ?e, "badges not awarded");
    }
    let held = earned(&state.db, user).await?;
    let title: Option<String> = sqlx::query_scalar("SELECT title FROM users WHERE id = ?")
        .bind(user)
        .fetch_optional(&state.db)
        .await?
        .flatten();
    let badges: Vec<Badge> = CATALOGUE
        .iter()
        .zip(values)
        .map(|(def, progress)| {
            let (tier, earned_at) = held
                .get(def.key)
                .map(|(t, at)| (*t, Some(at.clone())))
                .unwrap_or((0, None));
            let maxed = tier >= 3;
            let goal = def.tiers[(tier.clamp(0, 2)) as usize];
            Badge {
                key: def.key,
                icon: def.icon,
                mascot: def.mascot,
                name: def.name(lang),
                hint: hint(def.key, goal, lang, maxed),
                tier,
                tiers: def.tiers,
                progress,
                earned_at,
            }
        })
        .collect();
    let earned_count = badges.iter().filter(|b| b.tier > 0).count();
    Ok(Json(json!({
        "title": title.filter(|t| held.contains_key(t)),
        "earned": earned_count,
        "total": CATALOGUE.len(),
        "badges": badges,
    })))
}

#[derive(Deserialize)]
pub struct TitleBody {
    #[serde(default)]
    key: Option<Value>,
}

/// `PUT /me/title {key}`: display one of my titles (`null` clears). `400 bad_title`, `403 not_earned`.
pub async fn set_title(
    State(state): State<AppState>,
    UserId(user): UserId,
    Json(body): Json<TitleBody>,
) -> AppResult<Json<Value>> {
    let key = match body.key {
        None | Some(Value::Null) => None,
        Some(Value::String(k)) => Some(find(&k).ok_or(AppError::BadRequest("bad_title"))?.key),
        Some(_) => return Err(AppError::BadRequest("bad_title")),
    };
    if let Some(key) = key {
        let held: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM user_badges WHERE user_id = ? AND key = ?)",
        )
        .bind(user)
        .bind(key)
        .fetch_one(&state.db)
        .await?;
        if !held {
            return Err(AppError::Forbidden("not_earned"));
        }
    }
    sqlx::query("UPDATE users SET title = ? WHERE id = ?")
        .bind(key)
        .bind(user)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "title": key })))
}

/// `GET /titles`: `{members: {"<member id>": TitleChip}}`, current members of my room who display a
/// title they hold. A party answers `{members: {}}`.
pub async fn titles(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    let lang = crate::platform::i18n::of_member(&state.db, me.id).await?;
    let rows: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT m.id, u.title, MAX(b.tier) FROM members m \
         JOIN groups g ON g.id = m.group_id AND g.kind = 'room' \
         JOIN users u ON u.id = m.user_id AND u.title IS NOT NULL AND u.deleted_at IS NULL \
         JOIN user_badges b ON b.user_id = u.id AND b.key = u.title \
         WHERE m.group_id = ? AND m.removed_at IS NULL GROUP BY m.id, u.title",
    )
    .bind(me.group_id)
    .fetch_all(&state.db)
    .await?;
    let members: Map<String, Value> = rows
        .into_iter()
        .filter_map(|(id, key, tier)| {
            TitleChip::new(&key, tier, lang).map(|chip| (id.to_string(), json!(chip)))
        })
        .collect();
    Ok(Json(json!({ "members": members })))
}

/// `GET /members/{id}/badges`: a member of my room's earned badges (highest tier each) and the
/// title they display. `404` unless they are in my room.
pub async fn member(
    State(state): State<AppState>,
    Me(me): Me,
    Path(member_id): Path<i64>,
) -> AppResult<Json<Value>> {
    let user: i64 = sqlx::query_scalar(
        "SELECT m.user_id FROM members m JOIN users u ON u.id = m.user_id AND u.deleted_at IS NULL \
         WHERE m.id = ? AND m.group_id = ? AND m.removed_at IS NULL",
    )
    .bind(member_id)
    .bind(me.group_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let lang = crate::platform::i18n::of_member(&state.db, me.id).await?;
    let held = earned(&state.db, user).await?;
    let title: Option<String> = sqlx::query_scalar("SELECT title FROM users WHERE id = ?")
        .bind(user)
        .fetch_one(&state.db)
        .await?;
    let title = title.and_then(|t| TitleChip::new(&t, held.get(&t)?.0, lang));
    let badges: Vec<Value> = CATALOGUE
        .iter()
        .filter_map(|def| {
            let (tier, _) = held.get(def.key)?;
            Some(json!({
                "key": def.key, "name": def.name(lang), "icon": def.icon, "mascot": def.mascot, "tier": tier,
            }))
        })
        .collect();
    Ok(Json(json!({ "title": title, "badges": badges })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_is_consistent() {
        let mut keys: Vec<&str> = CATALOGUE.iter().map(|b| b.key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), CATALOGUE.len());
        for b in &CATALOGUE {
            assert!(
                b.tiers[0] < b.tiers[1] && b.tiers[1] < b.tiers[2],
                "{}",
                b.key
            );
            assert!(
                ["zeste", "bulle", "pave", "paillette", "roupi"].contains(&b.mascot),
                "{}",
                b.key
            );
            for lang in Lang::ALL {
                for n in [1, 2, b.tiers[2]] {
                    assert!(!hint(b.key, n, lang, false).is_empty(), "{}", b.key);
                    assert!(!hint(b.key, n, lang, true).is_empty(), "{}", b.key);
                }
            }
        }
    }

    #[test]
    fn tiers_and_texts() {
        let pilier = find("pilier").unwrap();
        assert_eq!(pilier.tier_for(0), 0);
        assert_eq!(pilier.tier_for(49), 0);
        assert_eq!(pilier.tier_for(50), 1);
        assert_eq!(pilier.tier_for(250), 2);
        assert_eq!(pilier.tier_for(5000), 3);
        assert_eq!(
            inbox_text(pilier, 3, Lang::Fr),
            "Nouveau titre : Le pilier (or)"
        );
        assert_eq!(
            inbox_text(pilier, 3, Lang::En),
            "New title: The pillar (gold)"
        );
        assert_eq!(hint("pilier", 250, Lang::Fr, false), "Vote 250 fois");
        assert_eq!(hint("pilier", 250, Lang::En, false), "Vote 250 times");
        assert_eq!(hint("fetard", 1, Lang::En, false), "Vote at 1 party");
        assert!(TitleChip::new("nope", 1, Lang::Fr).is_none());
        assert!(TitleChip::new("pilier", 0, Lang::Fr).is_none());
    }
}
