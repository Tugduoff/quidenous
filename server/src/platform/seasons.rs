use axum::{Json, extract::State, http::header, response::IntoResponse};
use serde::Serialize;

use crate::{
    AppState,
    insights::{
        streaks::{DAY, now_secs},
        wrapped::civil,
    },
    platform::config::parse_month_day,
};

pub(crate) const DEFAULT_SEASONS: &str = "winter:12-01..01-06";
const OFF: &str = "off";
const ID_MAX: usize = 20;
const CACHED_FOR_AN_HOUR: &str = "public, max-age=3600";
const MALFORMED: &str = "is not id:MM-DD..MM-DD (id a-z, 20 letters at most)";
const REPEATED: &str = "names a skin already listed";

type MonthDay = (u32, u32);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Season {
    pub id: String,
    pub from: MonthDay,
    pub until: MonthDay,
}

impl Season {
    fn contains(&self, month: u32, day: u32) -> bool {
        in_window(self.from, self.until, (month, day))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skipped {
    pub entry: String,
    pub why: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seasons {
    pub skins: Vec<Season>,
    pub skipped: Vec<Skipped>,
}

impl Default for Seasons {
    fn default() -> Self {
        Self::parse(DEFAULT_SEASONS)
    }
}

impl Seasons {
    pub fn parse(raw: &str) -> Self {
        let mut seasons = Self {
            skins: Vec::new(),
            skipped: Vec::new(),
        };
        let raw = raw.trim();
        if raw.eq_ignore_ascii_case(OFF) {
            return seasons;
        }
        for entry in raw.split(',').map(str::trim).filter(|e| !e.is_empty()) {
            let why = match parse_entry(entry) {
                Some(s) if seasons.skins.iter().any(|known| known.id == s.id) => REPEATED,
                Some(s) => {
                    seasons.skins.push(s);
                    continue;
                }
                None => MALFORMED,
            };
            seasons.skipped.push(Skipped {
                entry: entry.to_owned(),
                why,
            });
        }
        seasons
    }
}

fn in_window(from: MonthDay, until: MonthDay, date: MonthDay) -> bool {
    if from <= until {
        from <= date && date <= until
    } else {
        date >= from || date <= until
    }
}

fn active(skins: &[Season], month: u32, day: u32) -> Option<&Season> {
    skins.iter().find(|s| s.contains(month, day))
}

fn parse_entry(entry: &str) -> Option<Season> {
    let (id, window) = entry.split_once(':')?;
    let id = id.trim();
    if id.is_empty() || id.len() > ID_MAX || !id.bytes().all(|b| b.is_ascii_lowercase()) {
        return None;
    }
    let (from, until) = window.split_once("..")?;
    Some(Season {
        id: id.to_owned(),
        from: parse_md(from.trim())?,
        until: parse_md(until.trim())?,
    })
}

fn parse_md(s: &str) -> Option<MonthDay> {
    let (m, d) = s.split_once('-')?;
    let two_digits = |part: &str| part.len() == 2 && part.bytes().all(|b| b.is_ascii_digit());
    if !two_digits(m) || !two_digits(d) {
        return None;
    }
    parse_month_day(s)
}

fn md((m, d): MonthDay) -> String {
    format!("{m:02}-{d:02}")
}

fn month_day(days: i64) -> MonthDay {
    let (_, month, day) = civil(days);
    (month, day)
}

#[derive(Serialize)]
struct Skin {
    id: String,
    from: String,
    until: String,
}

impl From<&Season> for Skin {
    fn from(s: &Season) -> Self {
        Self {
            id: s.id.clone(),
            from: md(s.from),
            until: md(s.until),
        }
    }
}

#[derive(Serialize)]
struct SeasonView {
    active: Option<String>,
    skins: Vec<Skin>,
}

pub(crate) async fn season(State(state): State<AppState>) -> impl IntoResponse {
    let skins = &state.config.seasons.skins;
    let (month, day) = month_day(now_secs().div_euclid(DAY));
    let view = SeasonView {
        active: active(skins, month, day).map(|s| s.id.clone()),
        skins: skins.iter().map(Skin::from).collect(),
    };
    ([(header::CACHE_CONTROL, CACHED_FOR_AN_HOUR)], Json(view))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(id: &str, from: MonthDay, until: MonthDay) -> Season {
        Season {
            id: id.into(),
            from,
            until,
        }
    }

    fn parse_seasons(raw: &str) -> Vec<Season> {
        Seasons::parse(raw).skins
    }

    #[test]
    fn default_and_off() {
        assert_eq!(Seasons::default().skins, vec![s("winter", (12, 1), (1, 6))]);
        assert!(parse_seasons("off").is_empty());
        assert!(parse_seasons("OFF").is_empty());
        assert!(parse_seasons("").is_empty());
        assert!(parse_seasons("  ").is_empty());
    }

    #[test]
    fn several_entries() {
        assert_eq!(
            parse_seasons(" winter:12-01..01-06, summer:07-01..08-31 ,leap:02-29..02-29"),
            vec![
                s("winter", (12, 1), (1, 6)),
                s("summer", (7, 1), (8, 31)),
                s("leap", (2, 29), (2, 29)),
            ]
        );
    }

    #[test]
    fn bad_entries_are_skipped() {
        let raw = "Winter:12-01..01-06,x1:01-01..01-02,abcdefghijklmnopqrstu:01-01..01-02,\
                   feb:02-30..03-01,nov:11-31..12-01,mon:13-01..01-01,zero:00-10..01-01,\
                   short:1-01..01-02,nodots:01-01-01-02,:01-01..01-02,nocolon,\
                   ok:10-31..11-02,ok:01-01..01-02";
        let seasons = Seasons::parse(raw);
        assert_eq!(seasons.skins, vec![s("ok", (10, 31), (11, 2))]);
        assert_eq!(seasons.skipped.len(), 12);
        assert_eq!(
            seasons.skipped.last(),
            Some(&Skipped {
                entry: "ok:01-01..01-02".into(),
                why: REPEATED
            })
        );
        assert_eq!(parse_md("02-29"), Some((2, 29)));
        assert_eq!(parse_md("02-30"), None);
        assert_eq!(parse_md("04-31"), None);
        assert_eq!(parse_md("12-31"), Some((12, 31)));
    }

    #[test]
    fn windows() {
        let winter = ((12, 1), (1, 6));
        assert!(in_window(winter.0, winter.1, (12, 1)));
        assert!(in_window(winter.0, winter.1, (12, 31)));
        assert!(in_window(winter.0, winter.1, (1, 1)));
        assert!(in_window(winter.0, winter.1, (1, 6)));
        assert!(!in_window(winter.0, winter.1, (1, 7)));
        assert!(!in_window(winter.0, winter.1, (11, 30)));
        assert!(!in_window(winter.0, winter.1, (7, 15)));
        let summer = ((7, 1), (8, 31));
        assert!(in_window(summer.0, summer.1, (7, 1)));
        assert!(in_window(summer.0, summer.1, (8, 15)));
        assert!(in_window(summer.0, summer.1, (8, 31)));
        assert!(!in_window(summer.0, summer.1, (6, 30)));
        assert!(!in_window(summer.0, summer.1, (9, 1)));
        assert!(!in_window(summer.0, summer.1, (1, 3)));
    }

    #[test]
    fn first_matching_skin() {
        let skins = vec![s("winter", (12, 1), (1, 6)), s("noel", (12, 20), (12, 26))];
        assert_eq!(
            active(&skins, 12, 24).map(|s| s.id.as_str()),
            Some("winter")
        );
        assert_eq!(
            active(&skins[1..], 12, 24).map(|s| s.id.as_str()),
            Some("noel")
        );
        assert!(active(&skins, 6, 1).is_none());
    }

    #[test]
    fn utc_dates() {
        let (leap_day, late_november, millennium_march) = (19_782, 20_775, 11_017);
        assert_eq!(month_day(0), (1, 1));
        assert_eq!(month_day(leap_day), (2, 29));
        assert_eq!(month_day(late_november), (11, 18));
        assert_eq!(month_day(millennium_march), (3, 1));
        assert_eq!(month_day(-1), (12, 31));
    }
}
