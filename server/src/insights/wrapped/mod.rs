//! The Wrapped (docs/WRAPPED.md): « Ton année qui de nous » (personal, across the rooms I'm in) and
//! « L'année de <salon> » (one room), a story of ~10 cards about a calendar year, shareable as
//! server-drawn cards. Shown from `WRAPPED_FROM` (Dec 1) to `WRAPPED_UNTIL` (Jan 31): a window that
//! spans New Year shows the year it started in. Admins (and `WRAPPED_PREVIEW=1` servers) preview it
//! any time.
//!
//! Privacy: only finished questions of the year; only rooms where the reader has a seat now; only
//! people who share one of those rooms with them now, by first name; an anonymous question never
//! says who voted for whom (it still counts in votes received and in who won).

use crate::platform::ids::{MemberId, UserId};
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
    insights::streaks::{self, DAY},
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

mod cards;
mod personal;
mod room;
use crate::insights::stats::{first_name, pct};
use cards::*;
use personal::*;
use room::*;

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
    pub member_id: MemberId,
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

// ---------- personal ----------

// ---------- room ----------

// ---------- API ----------

/// Who asks: the account, and the seat (scope `room`).
struct Reader {
    user: UserId,
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
            let room = crate::accounts::auth::x_room(headers);
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
    let member_ids: Vec<MemberId> = card.people.iter().map(|p| p.member_id).collect();
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
    let users: Vec<(UserId, Option<String>)> = sqlx::query_as(
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
