//! The Wrapped (docs/WRAPPED.md): « Ton année qui de nous » (personal, across the rooms I'm in) and
//! « L'année de <salon> » (one room), a story of ~10 cards about a calendar year, shareable as
//! server-drawn cards. Shown from `WRAPPED_FROM` (Dec 1) to `WRAPPED_UNTIL` (Jan 31): a window that
//! spans New Year shows the year it started in. Admins (and `WRAPPED_PREVIEW=1` servers) preview it
//! any time.
//!
//! Privacy: only finished questions of the year; only rooms where the reader has a seat now; only
//! people who share one of those rooms with them now, by first name; an anonymous question never
//! says who voted for whom (it still counts in votes received and in who won).

use std::collections::{HashMap, HashSet};

use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    AppState,
    insights::stats,
    insights::streaks::{self, DAY, Day, Days, Room},
    platform::config::WrappedWindow,
    platform::error::{AppError, AppResult},
    platform::i18n::Lang,
    realtime::live::Event,
    rooms::model::MemberRow,
};

/// Longest punchline.
const LINE_MAX: usize = 110;
/// Fewer data cards than this (intro and outro aside): nothing to show.
const MIN_CARDS: usize = 3;
/// The "your year is ready" notice goes to accounts that voted in the last this many days.
const ACTIVE_DAYS: i64 = 60;

/// Unix seconds of a stored timestamp (ISO 8601, UTC).
macro_rules! unix {
    ($col:literal) => {
        concat!("((julianday(", $col, ") - 2440587.5) * 86400.0)")
    };
}

/// Finished questions of the rooms `?1` (JSON array) opened in `[?2, ?3)` (unix seconds).
macro_rules! in_year {
    () => {
        concat!(
            "r.group_id IN (SELECT value FROM json_each(?1)) AND r.closed_at IS NOT NULL AND ",
            unix!("r.opened_at"),
            " >= ?2 AND ",
            unix!("r.opened_at"),
            " < ?3"
        )
    };
}

// ---------- dates and the window ----------

/// Days since 1970-01-01 of a civil date.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let (m, d) = (i64::from(m), i64::from(d));
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date of a day number since 1970-01-01.
pub fn civil(day: i64) -> (i64, u32, u32) {
    let z = day + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// The year whose Wrapped shows on this date, if any. A window that wraps past New Year
/// (`12-01` → `01-31`) shows year Y from Dec 1 of Y to Jan 31 of Y+1.
pub fn window_year(w: WrappedWindow, (y, m, d): (i64, u32, u32)) -> Option<i64> {
    let md = (m, d);
    if w.from <= w.until {
        (w.from <= md && md <= w.until).then_some(y)
    } else if md >= w.from {
        Some(y)
    } else if md <= w.until {
        Some(y - 1)
    } else {
        None
    }
}

/// `YYYY-MM-DD` of the first and last day of the window showing `year`.
pub fn bounds(w: WrappedWindow, year: i64) -> (String, String) {
    let until_year = if w.from <= w.until { year } else { year + 1 };
    (
        format!("{year:04}-{:02}-{:02}", w.from.0, w.from.1),
        format!("{until_year:04}-{:02}-{:02}", w.until.0, w.until.1),
    )
}

/// What a reader gets to see, decided from the date alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Access {
    pub year: i64,
    pub available: bool,
    pub preview: bool,
    pub can_preview: bool,
}

/// `today`: the reader's local date. `can_preview`: an admin, or a `WRAPPED_PREVIEW` server
/// (`always`, which previews without asking). Outside the window, without a preview: not available.
/// A `year` other than the window's is only accepted when previewing (`400 bad_year`).
pub fn access(
    w: WrappedWindow,
    today: (i64, u32, u32),
    year: Option<i64>,
    asked_preview: bool,
    can_preview: bool,
    always: bool,
) -> AppResult<Access> {
    let showing = window_year(w, today);
    let previewing = always || (asked_preview && can_preview);
    let year = match year {
        None => showing.unwrap_or(today.0),
        Some(y) if Some(y) == showing => y,
        Some(y) if previewing && (2000..=today.0 + 1).contains(&y) => y,
        Some(_) => return Err(AppError::BadRequest("bad_year")),
    };
    let in_window = showing == Some(year);
    Ok(Access {
        year,
        available: in_window || previewing,
        preview: !in_window && previewing,
        can_preview: can_preview || always,
    })
}

// ---------- the cards ----------

#[derive(Debug, Clone, Serialize)]
pub struct Person {
    pub name: String,
    pub color: String,
    pub photo_url: Option<String>,
    pub avatar: Option<String>,
    /// Their seat (share links list it, so erasing the account removes the card).
    #[serde(skip)]
    pub member_id: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MascotRef {
    pub who: &'static str,
    pub mood: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct Card {
    pub key: &'static str,
    pub icon: &'static str,
    pub mascot: Option<MascotRef>,
    pub title: String,
    pub big: String,
    pub line: String,
    pub people: Vec<Person>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    pub shareable: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Wrapped {
    pub year: i64,
    pub scope: &'static str,
    pub available: bool,
    pub preview: bool,
    pub can_preview: bool,
    pub from: String,
    pub until: String,
    pub room: Option<String>,
    pub cards: Vec<Card>,
}

fn card(
    key: &'static str,
    icon: &'static str,
    mascot: (&'static str, &'static str),
    title: String,
    big: String,
    line: String,
    people: Vec<Person>,
) -> Card {
    Card {
        key,
        icon,
        mascot: Some(MascotRef {
            who: mascot.0,
            mood: mascot.1,
        }),
        title,
        big,
        line,
        people,
        question: None,
        shareable: true,
    }
}

/// The variant chosen by `seed`, or the next one that fits [`LINE_MAX`]; cut at a word otherwise.
fn pick(seed: i64, lines: Vec<String>) -> String {
    let n = lines.len();
    if n == 0 {
        return String::new();
    }
    let start = seed.unsigned_abs() as usize;
    for i in 0..n {
        let l = &lines[(start + i) % n];
        if l.chars().count() <= LINE_MAX {
            return l.clone();
        }
    }
    let mut out = String::new();
    for word in lines[start % n].split(' ') {
        if out.chars().count() + word.chars().count() + 2 > LINE_MAX {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out.push('…');
    out
}

fn first_name(name: &str) -> String {
    name.split_whitespace().next().unwrap_or("?").to_owned()
}

fn percent(pc: i64, lang: Lang) -> String {
    lang.pick(format!("{pc} %"), format!("{pc}%"))
}

fn pct(value: i64, total: i64) -> i64 {
    if total <= 0 {
        0
    } else {
        (value * 100 + total / 2) / total
    }
}

fn days_text(n: i64, lang: Lang) -> String {
    lang.pick(format!("{n} jours"), format!("{n} days"))
}

fn plural(n: i64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn outro(year: i64, lang: Lang, room: Option<&str>) -> Card {
    let next = year + 1;
    Card {
        key: "outro",
        icon: "party-popper",
        mascot: Some(MascotRef {
            who: "roupi",
            mood: "sleepy",
        }),
        title: lang.pick("Et maintenant ?", "What now?").into(),
        big: lang.pick(
            format!("On remet ça en {next} ?"),
            format!("Same again in {next}?"),
        ),
        line: match room {
            Some(r) => lang.pick(
                format!("Merci « {r} ». À l'année prochaine."),
                format!("Thanks, “{r}”. See you next year."),
            ),
            None => lang
                .pick(
                    "Merci d'avoir joué. La bande t'attend.",
                    "Thanks for playing. The gang is waiting.",
                )
                .into(),
        },
        people: Vec::new(),
        question: None,
        shareable: false,
    }
}

/// Intro, the data cards, outro; or nothing when fewer than [`MIN_CARDS`] data cards.
fn assemble(intro: Option<Card>, data: Vec<Card>, outro: Card) -> Vec<Card> {
    match intro {
        Some(mut intro) if data.len() >= MIN_CARDS => {
            intro.shareable = false;
            let mut out = vec![intro];
            out.extend(data);
            out.push(outro);
            out
        }
        _ => Vec::new(),
    }
}

/// Unix seconds `[start, end)` of a calendar year in a time zone (minutes east of UTC).
fn year_range(year: i64, tz: i64) -> (i64, i64) {
    (
        days_from_civil(year, 1, 1) * DAY - tz * 60,
        days_from_civil(year + 1, 1, 1) * DAY - tz * 60,
    )
}

/// Best run of voted days within a year (local day numbers `[first, last]`), up to `today`.
fn best_in_year(days: &Days, first: i64, last: i64, today: i64) -> i64 {
    let year: Days = days.range(first..=last).map(|(d, s)| (*d, *s)).collect();
    if year.values().all(|s| *s != Day::Voted) {
        return 0;
    }
    streaks::streak(&year, today.min(last)).best
}

// ---------- personal ----------

/// A seat in one of the reader's rooms, as loaded.
struct Seat {
    user: Option<i64>,
    name: String,
    color: String,
    photo: Option<String>,
    avatar: Option<String>,
    current: bool,
}

impl Seat {
    fn person(&self, id: i64) -> Person {
        Person {
            name: first_name(&self.name),
            color: self.color.clone(),
            photo_url: self.photo.as_ref().map(|p| format!("/photos/{p}")),
            avatar: self.avatar.clone(),
            member_id: id,
        }
    }
}

struct YearRound {
    id: i64,
    group: i64,
    text: String,
    kind: String,
    anonymous: bool,
    /// (voter, target, unix time), in vote order.
    votes: Vec<(i64, i64, f64)>,
    /// Ranking points per member.
    points: HashMap<i64, i64>,
    messages: i64,
}

impl YearRound {
    fn tally(&self) -> HashMap<i64, i64> {
        let mut t = HashMap::new();
        for v in &self.votes {
            *t.entry(v.1).or_insert(0) += 1;
        }
        t
    }

    /// Pick: most votes (ties count); ranking: most points.
    fn winners(&self) -> HashSet<i64> {
        let scores = if self.kind == "rank" {
            self.points.clone()
        } else {
            self.tally()
        };
        let best = scores.values().copied().max().unwrap_or(0);
        if best <= 0 {
            return HashSet::new();
        }
        scores
            .into_iter()
            .filter(|(_, v)| *v == best)
            .map(|(m, _)| m)
            .collect()
    }
}

/// Highest count, then the lowest key (deterministic).
fn top<K: Ord + Copy + std::hash::Hash>(counts: &HashMap<K, i64>, min: i64) -> Option<(K, i64)> {
    counts
        .iter()
        .filter(|(_, v)| **v >= min)
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
        .map(|(k, v)| (*k, *v))
}

#[allow(clippy::too_many_lines)]
async fn personal(
    state: &AppState,
    user: i64,
    year: i64,
    tz: i64,
    lang: Lang,
) -> AppResult<Vec<Card>> {
    let db = &state.db;
    // My seats in the rooms I'm in now: group → my member id.
    let mine: Vec<(i64, i64, String)> = sqlx::query_as(
        "SELECT m.id, m.group_id, g.name FROM members m \
         JOIN groups g ON g.id = m.group_id AND g.kind = 'room' AND g.deleted_at IS NULL \
         WHERE m.user_id = ? AND m.removed_at IS NULL ORDER BY m.id",
    )
    .bind(user)
    .fetch_all(db)
    .await?;
    if mine.is_empty() {
        return Ok(Vec::new());
    }
    let me_in: HashMap<i64, i64> = mine.iter().map(|(m, g, _)| (*g, *m)).collect();
    let my_seats: HashSet<i64> = mine.iter().map(|s| s.0).collect();
    let room_names: HashMap<i64, String> = mine.iter().map(|(_, g, n)| (*g, n.clone())).collect();
    let rooms = json!(mine.iter().map(|s| s.1).collect::<Vec<_>>()).to_string();
    let (since, until) = year_range(year, tz);

    #[allow(clippy::type_complexity)]
    let seat_rows: Vec<(
        i64,
        i64,
        Option<i64>,
        String,
        String,
        Option<String>,
        Option<String>,
        bool,
    )> = sqlx::query_as(
        "SELECT id, group_id, user_id, name, color, photo, avatar, removed_at IS NULL FROM members \
             WHERE group_id IN (SELECT value FROM json_each(?)) ORDER BY id",
    )
    .bind(&rooms)
    .fetch_all(db)
    .await?;
    let round_rows: Vec<(i64, i64, String, String, bool)> = sqlx::query_as(concat!(
        "SELECT r.id, r.group_id, q.text, q.kind, r.anonymous FROM rounds r \
         JOIN questions q ON q.id = r.question_id WHERE ",
        in_year!(),
        " ORDER BY r.opened_at, r.id"
    ))
    .bind(&rooms)
    .bind(since as f64)
    .bind(until as f64)
    .fetch_all(db)
    .await?;
    let vote_rows: Vec<(i64, i64, i64, f64)> = sqlx::query_as(concat!(
        "SELECT v.round_id, v.voter_id, v.target_id, ",
        unix!("v.created_at"),
        " FROM votes v JOIN rounds r ON r.id = v.round_id WHERE ",
        in_year!(),
        " ORDER BY v.round_id, v.created_at, v.voter_id"
    ))
    .bind(&rooms)
    .bind(since as f64)
    .bind(until as f64)
    .fetch_all(db)
    .await?;
    let point_rows: Vec<(i64, i64, i64)> = sqlx::query_as(concat!(
        "WITH p AS (SELECT r.id FROM rounds r WHERE ",
        in_year!(),
        "), n AS (SELECT k.round_id, k.voter_id, COUNT(*) AS size FROM rankings k \
         WHERE k.round_id IN (SELECT id FROM p) GROUP BY k.round_id, k.voter_id) \
         SELECT k.round_id, k.member_id, SUM(n.size - k.position + 1) FROM rankings k \
         JOIN n ON n.round_id = k.round_id AND n.voter_id = k.voter_id GROUP BY k.round_id, k.member_id"
    ))
    .bind(&rooms)
    .bind(since as f64)
    .bind(until as f64)
    .fetch_all(db)
    .await?;
    let message_rows: Vec<(i64, i64)> = sqlx::query_as(concat!(
        "SELECT m.round_id, COUNT(*) FROM messages m JOIN rounds r ON r.id = m.round_id WHERE ",
        in_year!(),
        " GROUP BY m.round_id"
    ))
    .bind(&rooms)
    .bind(since as f64)
    .bind(until as f64)
    .fetch_all(db)
    .await?;
    let bets: i64 = sqlx::query_scalar(concat!(
        "SELECT COUNT(*) FROM tripot_bets b JOIN rounds r ON r.id = b.round_id WHERE ",
        in_year!(),
        " AND b.payout > 0 AND NOT b.refunded AND b.member_id IN (SELECT value FROM json_each(?4))"
    ))
    .bind(&rooms)
    .bind(since as f64)
    .bind(until as f64)
    .bind(json!(my_seats.iter().collect::<Vec<_>>()).to_string())
    .fetch_one(db)
    .await?;

    let mut seats: HashMap<i64, Seat> = HashMap::new();
    for (id, _group, user_id, name, color, photo, avatar, current) in seat_rows {
        seats.insert(
            id,
            Seat {
                user: user_id,
                name,
                color,
                photo,
                avatar,
                current,
            },
        );
    }
    // People I may name: someone else with a seat now in one of my rooms; shown with their latest one.
    let mut faces: HashMap<i64, i64> = HashMap::new();
    for (id, s) in &seats {
        if let Some(u) = s.user
            && s.current
            && u != user
        {
            let e = faces.entry(u).or_insert(*id);
            *e = (*e).max(*id);
        }
    }
    let person_of = |user: i64| -> Option<Person> {
        let id = faces.get(&user)?;
        Some(seats.get(id)?.person(*id))
    };
    let user_of = |member: i64| seats.get(&member).and_then(|s| s.user);

    let mut rounds: Vec<YearRound> = round_rows
        .into_iter()
        .map(|(id, group, text, kind, anonymous)| YearRound {
            id,
            group,
            text,
            kind,
            anonymous,
            votes: Vec::new(),
            points: HashMap::new(),
            messages: 0,
        })
        .collect();
    let index: HashMap<i64, usize> = rounds.iter().enumerate().map(|(i, r)| (r.id, i)).collect();
    for (round, voter, target, at) in vote_rows {
        if let Some(&i) = index.get(&round) {
            rounds[i].votes.push((voter, target, at));
        }
    }
    for (round, member, points) in point_rows {
        if let Some(&i) = index.get(&round) {
            rounds[i].points.insert(member, points);
        }
    }
    for (round, n) in message_rows {
        if let Some(&i) = index.get(&round) {
            rounds[i].messages = n;
        }
    }

    let mut answered = 0;
    let mut rooms_played: HashSet<i64> = HashSet::new();
    let mut days: HashSet<i64> = HashSet::new();
    let mut crush: HashMap<i64, i64> = HashMap::new();
    let mut fans: HashMap<i64, i64> = HashMap::new();
    let mut nemesis: HashMap<i64, i64> = HashMap::new();
    let (mut received, mut wins) = (0, 0);
    let mut best_q: Option<(i64, i64, usize)> = None;
    let (mut eligible, mut good) = (0, 0);
    let mut hottest: Option<usize> = None;
    for (i, r) in rounds.iter().enumerate() {
        let Some(&me) = me_in.get(&r.group) else {
            continue;
        };
        if r.messages >= 3 && hottest.is_none_or(|h| r.messages >= rounds[h].messages) {
            hottest = Some(i);
        }
        if let Some(v) = r.votes.iter().find(|v| v.0 == me) {
            answered += 1;
            rooms_played.insert(r.group);
            days.insert((v.2.floor() as i64 + tz * 60).div_euclid(DAY));
        }
        if r.kind == "choose" {
            continue;
        }
        let tally = r.tally();
        let winners = r.winners();
        let mine = tally.get(&me).copied().unwrap_or(0);
        let total = r.votes.len() as i64;
        received += mine;
        if winners.contains(&me) {
            wins += 1;
        }
        // Who voted for whom: never on an anonymous question.
        if !r.anonymous {
            for &(voter, target, _) in &r.votes {
                if voter == me && target != me {
                    if let Some(u) = user_of(target).filter(|u| faces.contains_key(u)) {
                        *crush.entry(u).or_default() += 1;
                    }
                } else if target == me
                    && voter != me
                    && let Some(u) = user_of(voter).filter(|u| faces.contains_key(u))
                {
                    *fans.entry(u).or_default() += 1;
                }
            }
        }
        if r.kind != "pick" {
            continue;
        }
        if mine > 0 && !winners.contains(&me) {
            for w in &winners {
                if let Some(u) = user_of(*w).filter(|u| faces.contains_key(u)) {
                    *nemesis.entry(u).or_default() += 1;
                }
            }
        }
        if total >= 3 && mine > 0 {
            // Biggest share, then more votes, then the latest.
            let better = best_q.is_none_or(|(v, t, _)| {
                (i128::from(mine) * i128::from(t))
                    .cmp(&(i128::from(v) * i128::from(total)))
                    .then(mine.cmp(&v))
                    .is_ge()
            });
            if better {
                best_q = Some((mine, total, i));
            }
        }
        if total >= 3
            && !r.anonymous
            && let Some(v) = r.votes.iter().find(|v| v.0 == me && v.1 != me)
        {
            eligible += 1;
            if winners.contains(&v.1) {
                good += 1;
            }
        }
    }

    let n_rooms = rooms_played.len() as i64;
    let n_days = days.len() as i64;
    let intro = (answered > 0).then(|| {
        let r = lang.pick(
            plural(n_rooms, "salon", "salons"),
            plural(n_rooms, "room", "rooms"),
        );
        let d = lang.pick(
            plural(n_days, "jour", "jours"),
            plural(n_days, "day", "days"),
        );
        card(
            "intro",
            "sparkles",
            ("zeste", "happy"),
            lang.pick(format!("Ton année {year}"), format!("Your {year}")),
            format!("{answered} questions"),
            pick(
                answered,
                lang.pick(
                    vec![
                        format!("{answered} questions répondues, {r}, {d} de jeu. Tu étais là."),
                        format!("{d} à voter dans {r}. Assiduité exemplaire."),
                        format!("{d} de jeu, {answered} réponses. Rien ne t'échappe."),
                    ],
                    vec![
                        format!("{answered} questions answered, {r}, {d} of play. You showed up."),
                        format!("{d} of voting across {r}. Model attendance."),
                        format!("{d} of play, {answered} answers. Nothing gets past you."),
                    ],
                ),
            ),
            Vec::new(),
        )
    });

    let mut data = Vec::new();
    if let Some((u, n)) = top(&crush, 2)
        && let Some(p) = person_of(u)
    {
        let who = p.name.clone();
        data.push(card(
            "crush",
            "heart",
            ("paillette", "happy"),
            lang.pick("Ton crush de l'année", "Your crush of the year")
                .into(),
            who.clone(),
            pick(
                n + u,
                lang.pick(
                    vec![
                        format!("Tu as voté {n} fois pour {who}. On ne dira rien."),
                        format!(
                            "{n} votes pour {who}. Ce n'est plus un vote, c'est une déclaration."
                        ),
                        format!("{who}, {n} fois. Ton cœur a parlé."),
                    ],
                    vec![
                        format!("You voted for {who} {n} times. Our lips are sealed."),
                        format!("{n} votes for {who}. That's not a vote, it's a confession."),
                        format!("{who}, {n} times. Your heart has spoken."),
                    ],
                ),
            ),
            vec![p],
        ));
    }
    if let Some((u, n)) = top(&fans, 2)
        && let Some(p) = person_of(u)
    {
        let who = p.name.clone();
        data.push(card(
            "fan",
            "heart-handshake",
            ("bulle", "happy"),
            lang.pick("Ton fan numéro un", "Your number one fan").into(),
            who.clone(),
            pick(
                n + u,
                lang.pick(
                    vec![
                        format!("{who} a voté {n} fois pour toi. Ça mérite un merci."),
                        format!("{n} votes de {who} pour toi. Fan-club officiel."),
                        format!("{who} pense à toi. {n} fois, précisément."),
                    ],
                    vec![
                        format!("{who} voted for you {n} times. Say thank you."),
                        format!("{n} votes from {who}. Official fan club."),
                        format!("{who} thinks of you. {n} times, to be precise."),
                    ],
                ),
            ),
            vec![p],
        ));
    }
    if let Some((u, n)) = top(&nemesis, 2)
        && let Some(p) = person_of(u)
    {
        let (who, next) = (p.name.clone(), year + 1);
        data.push(card(
            "nemesis",
            "swords",
            ("zeste", "accuse"),
            lang.pick("Ton rival", "Your nemesis").into(),
            who.clone(),
            pick(
                n + u,
                lang.pick(
                    vec![
                        format!("{who} t'a soufflé la victoire {n} fois. Revanche en {next}."),
                        format!("{n} fois, {who} a fini devant toi. Rien de personnel."),
                    ],
                    vec![
                        format!("{who} snatched the win from you {n} times. Rematch in {next}."),
                        format!("{n} times, {who} finished ahead of you. Nothing personal."),
                    ],
                ),
            ),
            vec![p],
        ));
    }
    if received > 0 {
        let lines = if wins > 0 {
            lang.pick(
                vec![
                    format!("{received} votes reçus, {wins} questions gagnées. On parle de toi."),
                    format!(
                        "Les autres ont pensé à toi {received} fois. {wins} victoires au compteur."
                    ),
                ],
                vec![
                    format!(
                        "{received} votes received, {wins} questions won. People talk about you."
                    ),
                    format!(
                        "The others thought of you {received} times. {wins} wins on the board."
                    ),
                ],
            )
        } else {
            lang.pick(
                vec![format!(
                    "{received} votes reçus. Pas encore de victoire, mais ça vient."
                )],
                vec![format!(
                    "{received} votes received. No win yet, but it's coming."
                )],
            )
        };
        data.push(card(
            "picked",
            "fingerprint",
            ("bulle", "caught"),
            lang.pick("Les votes pour toi", "Votes for you").into(),
            format!("{received} votes"),
            pick(received, lines),
            Vec::new(),
        ));
    }
    if let Some((v, t, i)) = best_q {
        let pc = pct(v, t);
        let mut c = card(
            "best_question",
            "target",
            ("bulle", "celebrate"),
            lang.pick("Ta question de l'année", "Your question of the year")
                .into(),
            percent(pc, lang),
            pick(
                pc,
                lang.pick(
                    vec![
                        format!("{pc} % des voix sur celle-là. Difficile de nier."),
                        format!("Celle-là, c'était toi. {pc} % des voix."),
                    ],
                    vec![
                        format!("{pc}% of the votes on this one. Hard to deny."),
                        format!("This one was you. {pc}% of the votes."),
                    ],
                ),
            ),
            Vec::new(),
        );
        c.question = Some(rounds[i].text.clone());
        data.push(c);
    }
    if eligible >= 5 {
        let pc = pct(good, eligible);
        let lines = if bets > 0 {
            lang.pick(
                vec![format!(
                    "{pc} % de bons choix et {bets} paris gagnés au Tripot. Le flair."
                )],
                vec![format!(
                    "{pc}% good calls and {bets} Tripot bets won. What a nose."
                )],
            )
        } else {
            lang.pick(
                vec![
                    format!("{pc} % de tes votes sont allés au gagnant. Tu sens les choses."),
                    format!(
                        "Tu as vu juste {pc} % du temps. Les voyantes n'ont qu'à bien se tenir."
                    ),
                ],
                vec![
                    format!("{pc}% of your votes went to the winner. You can feel it."),
                    format!("Right {pc}% of the time. Fortune tellers, watch out."),
                ],
            )
        };
        data.push(card(
            "flair",
            "radar",
            ("pave", "think"),
            lang.pick("Ton flair", "Your nose").into(),
            percent(pc, lang),
            pick(pc, lines),
            Vec::new(),
        ));
    }
    // Best day streak of the year, any of my rooms, in the reader's time zone.
    let now = streaks::now_secs();
    let first_day = days_from_civil(year, 1, 1);
    let last_day = days_from_civil(year + 1, 1, 1) - 1;
    let today = streaks::local_day(now, tz);
    let mut best = 0;
    for (&group, &seat) in &me_in {
        if let Some(room) = Room::load(db, group).await? {
            best = best.max(best_in_year(
                &room.days(seat, tz, now),
                first_day,
                last_day,
                today,
            ));
        }
    }
    if best >= 2 {
        data.push(card(
            "streak",
            "flame",
            ("paillette", "celebrate"),
            lang.pick("Ta meilleure série", "Your best streak").into(),
            days_text(best, lang),
            pick(
                best,
                lang.pick(
                    vec![
                        format!("{best} jours d'affilée sans rater un vote. Respect."),
                        format!("{best} jours de suite. La régularité, c'est toi."),
                    ],
                    vec![
                        format!("{best} days in a row without missing a vote. Respect."),
                        format!("{best} days straight. Consistency is your middle name."),
                    ],
                ),
            ),
            Vec::new(),
        ));
    }
    if let Some(c) = title_card(state, user, year, lang).await? {
        data.push(c);
    }
    if let Some(i) = hottest {
        let r = &rounds[i];
        let n = r.messages;
        let room = room_names.get(&r.group).cloned().unwrap_or_default();
        let mut c = card(
            "room_question",
            "message-circle",
            ("zeste", "think"),
            lang.pick(
                "La question qui a tout enflammé",
                "The question that set it all off",
            )
            .into(),
            format!("{n} messages"),
            pick(
                n,
                lang.pick(
                    vec![
                        format!("{n} messages dans « {room} ». Personne n'était d'accord."),
                        format!("{n} messages sur celle-là. Le chat s'en souvient."),
                    ],
                    vec![
                        format!("{n} messages in “{room}”. Nobody agreed."),
                        format!("{n} messages on this one. The chat remembers."),
                    ],
                ),
            ),
            Vec::new(),
        );
        c.question = Some(r.text.clone());
        data.push(c);
    }
    Ok(assemble(intro, data, outro(year, lang, None)))
}

/// The title of the year: the highest tier earned during it, else the title I display.
async fn title_card(state: &AppState, user: i64, year: i64, lang: Lang) -> AppResult<Option<Card>> {
    let earned: Vec<(String, i64)> = sqlx::query_as(
        "SELECT key, MAX(tier) FROM user_badges WHERE user_id = ? AND earned_at >= ? AND earned_at < ? \
         GROUP BY key",
    )
    .bind(user)
    .bind(format!("{year:04}-01-01"))
    .bind(format!("{:04}-01-01", year + 1))
    .fetch_all(&state.db)
    .await?;
    let in_year = crate::insights::badges::CATALOGUE
        .iter()
        .filter_map(|def| {
            let tier = earned.iter().find(|(k, _)| k == def.key)?.1;
            Some((def, tier))
        })
        // Highest tier, then catalogue order.
        .min_by_key(|(_, tier)| -tier);
    let (def, tier, of_year) = match in_year {
        Some((def, tier)) => (def, tier, true),
        None => {
            let shown: Option<(String, i64)> = sqlx::query_as(
                "SELECT u.title, (SELECT MAX(tier) FROM user_badges b WHERE b.user_id = u.id AND b.key = u.title) \
                 FROM users u WHERE u.id = ? AND u.title IS NOT NULL",
            )
            .bind(user)
            .fetch_optional(&state.db)
            .await?
            .and_then(|(k, t): (String, Option<i64>)| Some((k, t?)));
            let Some((key, tier)) = shown else {
                return Ok(None);
            };
            let Some(def) = crate::insights::badges::find(&key) else {
                return Ok(None);
            };
            (def, tier, false)
        }
    };
    let level = crate::insights::badges::tier_name(tier, lang);
    let lines = if of_year {
        lang.pick(
            vec![
                format!("Niveau {level}, décroché en {year}. Tu l'as bien mérité."),
                format!("Décroché en {year}, niveau {level}. Porte-le fièrement."),
            ],
            vec![
                format!("{level} level, earned in {year}. Well deserved."),
                format!("Earned in {year}, {level} level. Wear it proudly."),
            ],
        )
    } else {
        lang.pick(
            vec![format!(
                "Le titre que tu affiches, niveau {level}. Il te va bien."
            )],
            vec![format!("The title you wear, {level} level. It suits you.")],
        )
    };
    Ok(Some(card(
        "title",
        def.icon,
        (def.mascot, "celebrate"),
        lang.pick("Ton titre de l'année", "Your title of the year")
            .into(),
        def.name(lang).into(),
        pick(tier + year, lines),
        Vec::new(),
    )))
}

// ---------- room ----------

#[allow(clippy::too_many_lines)]
async fn room(
    state: &AppState,
    me: &MemberRow,
    room_name: &str,
    year: i64,
    tz: i64,
    lang: Lang,
) -> AppResult<Vec<Card>> {
    let db = &state.db;
    let group = me.group_id;
    let (since, until) = year_range(year, tz);
    let vibe: Option<String> = sqlx::query_scalar("SELECT vibe FROM groups WHERE id = ?")
        .bind(group)
        .fetch_optional(db)
        .await?
        .flatten();
    let s = stats::compute_range(
        db,
        group,
        stats::Period::All,
        (since as f64, until as f64),
        tz as i32,
        vibe.as_deref(),
        lang,
    )
    .await?;
    #[allow(clippy::type_complexity)]
    let rows: Vec<(i64, String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, name, color, photo, avatar FROM members WHERE group_id = ? AND removed_at IS NULL \
         AND user_id IS NOT NULL",
    )
    .bind(group)
    .fetch_all(db)
    .await?;
    let people: HashMap<i64, Person> = rows
        .into_iter()
        .map(|(id, name, color, photo, avatar)| {
            (
                id,
                Person {
                    name: first_name(&name),
                    color,
                    photo_url: photo.map(|p| format!("/photos/{p}")),
                    avatar,
                    member_id: id,
                },
            )
        })
        .collect();
    let face = |id: i64| people.get(&id).cloned();

    let intro = (s.questions > 0).then(|| {
        let (v, m, p) = (s.votes, s.messages, s.members);
        card(
            "intro",
            "sparkles",
            ("zeste", "happy"),
            lang.pick(
                format!("L'année de « {room_name} »"),
                format!("“{room_name}” in {year}"),
            ),
            format!("{} questions", s.questions),
            pick(
                s.questions,
                lang.pick(
                    vec![
                        format!("{v} votes, {m} messages, {p} membres. Quelle année."),
                        format!(
                            "{p} membres, {v} votes et {m} messages. Vous ne vous êtes pas ennuyés."
                        ),
                    ],
                    vec![
                        format!("{v} votes, {m} messages, {p} members. What a year."),
                        format!("{p} members, {v} votes and {m} messages. Never a dull moment."),
                    ],
                ),
            ),
            Vec::new(),
        )
    });

    let mut data = Vec::new();
    if let Some(a) = &s.most_chosen
        && let Some(p) = face(a.member_id)
    {
        let (who, n) = (p.name.clone(), a.value);
        data.push(card(
            "star",
            "star",
            ("bulle", "caught"),
            lang.pick("La star de l'année", "Star of the year").into(),
            who.clone(),
            pick(
                n,
                lang.pick(
                    vec![
                        format!("{who} a reçu {n} votes. Tout le monde pense à {who}."),
                        format!("{n} votes pour {who}. La bande a un faible."),
                    ],
                    vec![
                        format!("{who} got {n} votes. Everyone thinks of {who}."),
                        format!("{n} votes for {who}. The gang has a soft spot."),
                    ],
                ),
            ),
            vec![p],
        ));
    }
    if let Some(a) = &s.most_crowned
        && let Some(p) = face(a.member_id)
    {
        let (who, n) = (p.name.clone(), a.value);
        data.push(card(
            "crown",
            "crown",
            ("paillette", "celebrate"),
            lang.pick("Collection de couronnes", "Crown collector")
                .into(),
            who.clone(),
            pick(
                n,
                lang.pick(
                    vec![
                        format!(
                            "{who} a gagné {n} questions cette année. Le trône est confortable ?"
                        ),
                        format!("{n} victoires pour {who}. Les autres prennent des notes."),
                    ],
                    vec![
                        format!("{who} won {n} questions this year. Comfy throne?"),
                        format!("{n} wins for {who}. The others are taking notes."),
                    ],
                ),
            ),
            vec![p],
        ));
    }
    if let Some(d) = &s.duo
        && let (Some(a), Some(b)) = (face(d.voter_id), face(d.target_id))
    {
        let (x, y, n) = (a.name.clone(), b.name.clone(), d.value);
        data.push(card(
            "crush",
            "heart",
            ("paillette", "happy"),
            lang.pick("Le crush de l'année", "Crush of the year").into(),
            format!("{x} → {y}"),
            pick(
                n,
                lang.pick(
                    vec![
                        format!("{x} a voté {n} fois pour {y}. On dit ça, on dit rien."),
                        format!("{n} votes de {x} pour {y}. Il se passe quelque chose."),
                    ],
                    vec![
                        format!("{x} voted for {y} {n} times. Just saying."),
                        format!("{n} votes from {x} for {y}. Something's going on."),
                    ],
                ),
            ),
            vec![a, b],
        ));
    }
    if let Some(t) = &s.twins
        && let (Some(a), Some(b)) = (face(t.a_id), face(t.b_id))
    {
        let (x, y, pc) = (a.name.clone(), b.name.clone(), pct(t.value, t.total));
        data.push(card(
            "twins",
            "copy",
            ("zeste", "happy"),
            lang.pick("Les jumeaux de vote", "Vote twins").into(),
            percent(pc, lang),
            pick(
                pc,
                lang.pick(
                    vec![
                        format!("{x} et {y} ont voté pareil {pc} % du temps. Télépathie ?"),
                        format!(
                            "{pc} % de votes identiques pour {x} et {y}. Un seul cerveau pour deux."
                        ),
                    ],
                    vec![
                        format!("{x} and {y} voted the same {pc}% of the time. Telepathy?"),
                        format!("{pc}% identical votes for {x} and {y}. One brain, two people."),
                    ],
                ),
            ),
            vec![a, b],
        ));
    }
    // The forecaster: most chips won at the Tripot this year, else the one most often on the winning side.
    let chips: Vec<(i64, i64)> = sqlx::query_as(concat!(
        "SELECT b.member_id, SUM(b.payout - b.amount) FROM tripot_bets b JOIN rounds r ON r.id = b.round_id \
         WHERE r.group_id = ?1 AND r.closed_at IS NOT NULL AND ",
        unix!("r.opened_at"),
        " >= ?2 AND ",
        unix!("r.opened_at"),
        " < ?3 AND b.payout IS NOT NULL AND NOT b.refunded GROUP BY b.member_id"
    ))
    .bind(group)
    .bind(since as f64)
    .bind(until as f64)
    .fetch_all(db)
    .await?;
    let chips: HashMap<i64, i64> = chips
        .into_iter()
        .filter(|(m, _)| people.contains_key(m))
        .collect();
    if let Some((m, n)) = top(&chips, 1)
        && let Some(p) = face(m)
    {
        let who = p.name.clone();
        data.push(card(
            "predictor",
            "coins",
            ("pave", "think"),
            lang.pick("Le pronostiqueur", "The forecaster").into(),
            lang.pick(format!("+{n} jetons"), format!("+{n} chips")),
            pick(
                n,
                lang.pick(
                    vec![
                        format!("{who} a gagné {n} jetons au Tripot. Ils ne valent rien, mais quand même."),
                        format!("+{n} jetons pour {who}. Le Tripot n'a qu'à bien se tenir."),
                    ],
                    vec![
                        format!("{who} won {n} chips at the Tripot. Worthless, but still."),
                        format!("+{n} chips for {who}. The Tripot should be worried."),
                    ],
                ),
            ),
            vec![p],
        ));
    } else if let Some(a) = &s.sheep
        && let Some(p) = face(a.member_id)
    {
        let (who, pc) = (p.name.clone(), pct(a.value, a.total.unwrap_or(0)));
        data.push(card(
            "predictor",
            "users",
            ("pave", "think"),
            lang.pick("Toujours du bon côté", "Always on the winning side")
                .into(),
            percent(pc, lang),
            pick(
                pc,
                lang.pick(
                    vec![
                        format!("{who} a voté pour le gagnant {pc} % du temps. Flair ou suivisme ?"),
                        format!("{pc} % des votes de {who} pour le gagnant. Toujours dans le sens du vent."),
                    ],
                    vec![
                        format!("{who} backed the winner {pc}% of the time. Instinct or herd?"),
                        format!("{pc}% of {who}'s votes went to the winner. Always with the wind."),
                    ],
                ),
            ),
            vec![p],
        ));
    }
    // The longest day streak of the year among the members (the reader's time zone).
    if let Some(r) = Room::load(db, group).await? {
        let now = streaks::now_secs();
        let first = days_from_civil(year, 1, 1);
        let last = days_from_civil(year + 1, 1, 1) - 1;
        let today = streaks::local_day(now, tz);
        let best = r
            .members()
            .filter(|m| people.contains_key(m))
            .map(|m| (m, best_in_year(&r.days(m, tz, now), first, last, today)))
            .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)));
        if let Some((m, n)) = best.filter(|b| b.1 >= 2)
            && let Some(p) = face(m)
        {
            let who = p.name.clone();
            data.push(card(
                "streak",
                "flame",
                ("paillette", "celebrate"),
                lang.pick("Record de série", "Streak record").into(),
                days_text(n, lang),
                pick(
                    n,
                    lang.pick(
                        vec![
                            format!("{who} a voté {n} jours d'affilée. Personne n'a fait mieux."),
                            format!("{n} jours de suite pour {who}. La régularité incarnée."),
                        ],
                        vec![
                            format!("{who} voted {n} days in a row. Nobody did better."),
                            format!("{n} days straight for {who}. Consistency in person."),
                        ],
                    ),
                ),
                vec![p],
            ));
        }
    }
    if let Some(q) = &s.hottest {
        let (n, c) = (q.value, q.total);
        let mut x = card(
            "hottest",
            "message-circle",
            ("zeste", "accuse"),
            lang.pick("La question la plus chaude", "Hottest question")
                .into(),
            format!("{n} messages"),
            pick(
                n,
                lang.pick(
                    vec![
                        format!("{n} messages, {c} personnes dans le chat. Ça a chauffé."),
                        format!("{n} messages sur celle-là. Personne n'a lâché."),
                    ],
                    vec![
                        format!("{n} messages, {c} people in the chat. Things got heated."),
                        format!("{n} messages on this one. Nobody let it go."),
                    ],
                ),
            ),
            Vec::new(),
        );
        x.question = Some(q.question.clone());
        data.push(x);
    }
    if let Some(q) = &s.most_split {
        let (n, v) = (q.value, q.total);
        let mut x = card(
            "split",
            "split",
            ("pave", "think"),
            lang.pick("La plus clivante", "Most divisive").into(),
            lang.pick(format!("{n} avis"), format!("{n} picks")),
            pick(
                n,
                lang.pick(
                    vec![
                        format!(
                            "{n} personnes différentes désignées sur {v} votes. Aucun consensus."
                        ),
                        format!("{v} votes, {n} réponses différentes. La bande est partagée."),
                    ],
                    vec![
                        format!("{n} different people picked out of {v} votes. Zero consensus."),
                        format!("{v} votes, {n} different answers. The gang is split."),
                    ],
                ),
            ),
            Vec::new(),
        );
        x.question = Some(q.question.clone());
        data.push(x);
    }
    Ok(assemble(intro, data, outro(year, lang, Some(room_name))))
}

// ---------- API ----------

/// Who asks: the account, and the seat (scope `room`).
struct Reader {
    user: i64,
    seat: Option<MemberRow>,
}

async fn reader(state: &AppState, headers: &HeaderMap, scope: &str) -> AppResult<Reader> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?;
    match scope {
        "me" => Ok(Reader {
            user: crate::accounts::auth::user_for_token(state, token).await?,
            seat: None,
        }),
        "room" => {
            let room = headers
                .get("x-room")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<i64>().ok());
            let me = crate::accounts::auth::member_in_room(state, token, room).await?;
            if crate::rounds::game::party(&state.db, me.group_id)
                .await?
                .is_some()
            {
                return Err(AppError::BadRequest("bad_scope"));
            }
            let user = me.user_id.ok_or(AppError::Unauthorized)?;
            Ok(Reader {
                user,
                seat: Some(me),
            })
        }
        _ => Err(AppError::BadRequest("bad_scope")),
    }
}

/// The Wrapped a reader gets (empty cards when not available).
async fn build(
    state: &AppState,
    r: &Reader,
    year: Option<i64>,
    tz: i64,
    preview: bool,
) -> AppResult<Wrapped> {
    let lang = crate::platform::i18n::of_user(&state.db, r.user).await?;
    let today = civil(streaks::local_day(streaks::now_secs(), tz));
    let a = access(
        state.config.wrapped,
        today,
        year,
        preview,
        crate::community::packs::is_admin(state, r.user),
        state.config.wrapped_preview,
    )?;
    let (from, until) = bounds(state.config.wrapped, a.year);
    let room_name: Option<String> = match &r.seat {
        Some(me) => {
            sqlx::query_scalar("SELECT name FROM groups WHERE id = ?")
                .bind(me.group_id)
                .fetch_optional(&state.db)
                .await?
        }
        None => None,
    };
    let cards = match (a.available, &r.seat) {
        (false, _) => Vec::new(),
        (true, None) => personal(state, r.user, a.year, tz, lang).await?,
        (true, Some(me)) => {
            room(
                state,
                me,
                room_name.as_deref().unwrap_or_default(),
                a.year,
                tz,
                lang,
            )
            .await?
        }
    };
    Ok(Wrapped {
        year: a.year,
        scope: if r.seat.is_some() { "room" } else { "me" },
        available: a.available,
        preview: a.preview,
        can_preview: a.can_preview,
        from,
        until,
        room: room_name,
        cards,
    })
}

fn flag(v: Option<&str>) -> bool {
    matches!(v.map(str::trim), Some("1" | "true" | "yes"))
}

fn parse_year(v: Option<&str>) -> AppResult<Option<i64>> {
    match v.map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(None),
        Some(y) => y
            .parse()
            .map(Some)
            .map_err(|_| AppError::BadRequest("bad_year")),
    }
}

#[derive(Deserialize)]
pub struct WrappedQuery {
    scope: Option<String>,
    year: Option<String>,
    /// Minutes east of UTC.
    tz: Option<String>,
    preview: Option<String>,
}

/// `GET /wrapped?scope=me|room&year=&tz=&preview=1`.
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<WrappedQuery>,
) -> AppResult<Json<Wrapped>> {
    let r = reader(&state, &headers, q.scope.as_deref().unwrap_or("me")).await?;
    let tz = i64::from(stats::parse_tz(q.tz.as_deref()));
    let year = parse_year(q.year.as_deref())?;
    Ok(Json(
        build(&state, &r, year, tz, flag(q.preview.as_deref())).await?,
    ))
}

#[derive(Deserialize)]
pub struct ShareBody {
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    year: Option<i64>,
    key: String,
    #[serde(default)]
    tz: Option<i64>,
    #[serde(default)]
    preview: Option<bool>,
}

/// `POST /wrapped/share {scope, year, key, tz, preview?}` → `{token, path}`: one card of my current
/// Wrapped frozen into a public link (`stat_shares`, key `wrapped_<key>`). `404` if that card isn't
/// in it or isn't shareable.
pub async fn share(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ShareBody>,
) -> AppResult<Json<Value>> {
    let r = reader(&state, &headers, body.scope.as_deref().unwrap_or("me")).await?;
    let tz = i64::from(stats::clamp_tz(body.tz.unwrap_or(0)));
    let w = build(&state, &r, body.year, tz, body.preview.unwrap_or(false)).await?;
    let card = w
        .cards
        .into_iter()
        .find(|c| c.key == body.key && c.shareable)
        .ok_or(AppError::NotFound)?;
    let lang = crate::platform::i18n::of_user(&state.db, r.user).await?;
    // The link belongs to a seat: mine in the room, or (personal) the room I use most.
    let (seat, group) = match &r.seat {
        Some(me) => (me.id, me.group_id),
        None => {
            let mut c = state.db.acquire().await?;
            crate::insights::badges::inbox_seat(&mut c, r.user, None)
                .await?
                .ok_or(AppError::NotFound)?
        }
    };
    let label = match &w.room {
        Some(room) => format!("{room} · {}", w.year),
        None => format!("Wrapped {}", w.year),
    };
    let people: Vec<(String, String, Option<String>)> = card
        .people
        .iter()
        .map(|p| (p.name.clone(), p.color.clone(), p.avatar.clone()))
        .collect();
    let member_ids: Vec<i64> = card.people.iter().map(|p| p.member_id).collect();
    // On the picture, a question card shows its question rather than the punchline.
    let line = match &card.question {
        Some(q) => lang.pick(format!("« {q} »"), format!("“{q}”")),
        None => card.line.clone(),
    };
    let token = crate::insights::share::freeze_stat(
        &state.db,
        crate::insights::share::Frozen {
            group_id: group,
            member_id: seat,
            key: &format!("wrapped_{}", card.key),
            title: &card.title,
            big: &card.big,
            line: &line,
            people: &json!(people).to_string(),
            lang,
            member_ids: &json!(member_ids).to_string(),
            label: Some(&label),
        },
    )
    .await?;
    Ok(Json(
        json!({ "token": token, "path": format!("/s/{token}") }),
    ))
}

/// In the window (UTC date at `now`), the "your year is ready" inbox entry, once per account and
/// year (`users.wrapped_notified`), to every account that voted in the last [`ACTIVE_DAYS`] days, on
/// its current room seat. No push. Returns how many went out.
pub async fn notify_at(state: &AppState, now: i64) -> AppResult<usize> {
    let Some(year) = window_year(state.config.wrapped, civil(now.div_euclid(DAY))) else {
        return Ok(0);
    };
    let users: Vec<(i64, Option<String>)> = sqlx::query_as(
        "SELECT u.id, u.locale FROM users u WHERE u.deleted_at IS NULL AND u.wrapped_notified IS NOT ?1 \
         AND EXISTS (SELECT 1 FROM members m JOIN votes v ON v.voter_id = m.id WHERE m.user_id = u.id \
           AND v.created_at >= strftime('%Y-%m-%dT%H:%M:%fZ', ?2, 'unixepoch', ?3)) ORDER BY u.id",
    )
    .bind(year)
    .bind(now)
    .bind(format!("-{ACTIVE_DAYS} days"))
    .fetch_all(&state.db)
    .await?;
    let mut sent = 0;
    let mut rooms = Vec::new();
    for (user, locale) in users {
        let mut tx = crate::platform::db::write_tx(&state.db).await?;
        let marked = sqlx::query(
            "UPDATE users SET wrapped_notified = ?1 WHERE id = ?2 AND wrapped_notified IS NOT ?1",
        )
        .bind(year)
        .bind(user)
        .execute(&mut *tx)
        .await?;
        if marked.rows_affected() == 0 {
            continue;
        }
        if let Some((seat, room)) = crate::insights::badges::inbox_seat(&mut tx, user, None).await?
        {
            let lang = Lang::from_db(locale.as_deref());
            sqlx::query("INSERT INTO inbox (member_id, kind, text) VALUES (?, 'wrapped', ?)")
                .bind(seat)
                .bind(lang.pick(
                    "Ton année qui de nous est prête",
                    "Your year in Qui de nous is ready",
                ))
                .execute(&mut *tx)
                .await?;
            rooms.push(room);
            sent += 1;
        }
        tx.commit().await?;
    }
    rooms.sort_unstable();
    rooms.dedup();
    for room in rooms {
        state.live.send(room, Event::InboxChanged);
    }
    Ok(sent)
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: WrappedWindow = WrappedWindow {
        from: (12, 1),
        until: (1, 31),
    };

    #[test]
    fn civil_dates_round_trip() {
        for day in [-800_000, -1, 0, 1, 19_000, 20_819, 20_820, 2_000_000] {
            let (y, m, d) = civil(day);
            assert_eq!(days_from_civil(y, m, d), day);
        }
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(days_from_civil(2026, 12, 1), 20_788);
        assert_eq!(civil(days_from_civil(2024, 2, 29)), (2024, 2, 29));
    }

    #[test]
    fn the_window_spans_new_year() {
        assert_eq!(window_year(W, (2026, 11, 30)), None);
        assert_eq!(window_year(W, (2026, 12, 1)), Some(2026));
        assert_eq!(window_year(W, (2026, 12, 31)), Some(2026));
        assert_eq!(window_year(W, (2027, 1, 1)), Some(2026));
        assert_eq!(window_year(W, (2027, 1, 31)), Some(2026));
        assert_eq!(window_year(W, (2027, 2, 1)), None);
        assert_eq!(
            bounds(W, 2026),
            ("2026-12-01".to_owned(), "2027-01-31".to_owned())
        );
        // A window inside one year.
        let nov = WrappedWindow {
            from: (11, 1),
            until: (11, 30),
        };
        assert_eq!(window_year(nov, (2026, 11, 17)), Some(2026));
        assert_eq!(window_year(nov, (2026, 12, 1)), None);
        assert_eq!(
            bounds(nov, 2026),
            ("2026-11-01".to_owned(), "2026-11-30".to_owned())
        );
    }

    #[test]
    fn access_rules() {
        let a = |today, year, asked, can, always| access(W, today, year, asked, can, always);
        // Outside the window: not available, the year in progress.
        let out = a((2026, 11, 17), None, false, false, false).unwrap();
        assert_eq!(
            out,
            Access {
                year: 2026,
                available: false,
                preview: false,
                can_preview: false
            }
        );
        // Asking for a preview without the right: ignored.
        assert!(
            !a((2026, 11, 17), None, true, false, false)
                .unwrap()
                .available
        );
        // An admin previewing.
        let admin = a((2026, 11, 17), None, true, true, false).unwrap();
        assert!(admin.available && admin.preview && admin.can_preview);
        assert_eq!(
            a((2026, 11, 17), Some(2025), true, true, false)
                .unwrap()
                .year,
            2025
        );
        // A dev server: always, as a preview.
        let dev = a((2026, 6, 1), None, false, false, true).unwrap();
        assert!(dev.available && dev.preview && dev.can_preview);
        // In January, last year's Wrapped; another year only when previewing.
        let jan = a((2027, 1, 15), None, false, false, false).unwrap();
        assert_eq!((jan.year, jan.available, jan.preview), (2026, true, false));
        assert!(a((2027, 1, 15), Some(2026), false, false, false).is_ok());
        assert!(matches!(
            a((2027, 1, 15), Some(2027), false, false, false),
            Err(AppError::BadRequest("bad_year"))
        ));
        assert!(
            a((2027, 1, 15), Some(2027), true, true, false)
                .unwrap()
                .preview
        );
        assert!(a((2027, 1, 15), Some(1999), true, true, false).is_err());
    }

    #[test]
    fn year_ranges_follow_the_time_zone() {
        let (start, end) = year_range(2026, 60);
        assert_eq!(start, days_from_civil(2026, 1, 1) * DAY - 3600);
        assert_eq!(end - start, 365 * DAY);
    }

    #[test]
    fn lines_fit() {
        let long = "x".repeat(200);
        assert_eq!(pick(1, vec!["a".into(), "b".into()]), "b");
        assert_eq!(pick(0, vec![long.clone(), "ok".into()]), "ok");
        assert!(pick(0, vec![format!("{long} {long}")]).chars().count() <= LINE_MAX);
    }
}
