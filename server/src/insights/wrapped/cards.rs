use std::collections::{HashMap, HashSet};

use crate::{
    AppState,
    insights::streaks::{self, DAY, Day, Days},
    platform::error::AppResult,
    platform::i18n::Lang,
};

use super::*;

pub(super) fn card(
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
pub(super) fn pick(seed: i64, lines: Vec<String>) -> String {
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

pub(super) fn percent(pc: i64, lang: Lang) -> String {
    lang.pick(format!("{pc} %"), format!("{pc}%"))
}

pub(super) fn days_text(n: i64, lang: Lang) -> String {
    lang.pick(format!("{n} jours"), format!("{n} days"))
}

pub(super) fn plural(n: i64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

pub(super) fn outro(year: i64, lang: Lang, room: Option<&str>) -> Card {
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
pub(super) fn assemble(intro: Option<Card>, data: Vec<Card>, outro: Card) -> Vec<Card> {
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
pub(super) fn year_range(year: i64, tz: i64) -> (i64, i64) {
    (
        days_from_civil(year, 1, 1) * DAY - tz * 60,
        days_from_civil(year + 1, 1, 1) * DAY - tz * 60,
    )
}

/// Best run of voted days within a year (local day numbers `[first, last]`), up to `today`.
pub(super) fn best_in_year(days: &Days, first: i64, last: i64, today: i64) -> i64 {
    let year: Days = days.range(first..=last).map(|(d, s)| (*d, *s)).collect();
    if year.values().all(|s| *s != Day::Voted) {
        return 0;
    }
    streaks::streak(&year, today.min(last)).best
}

/// A seat in one of the reader's rooms, as loaded.
pub(super) struct Seat {
    pub(crate) user: Option<i64>,
    pub(crate) name: String,
    pub(crate) color: String,
    pub(crate) photo: Option<String>,
    pub(crate) avatar: Option<String>,
    pub(crate) current: bool,
}

impl Seat {
    pub(crate) fn person(&self, id: i64) -> Person {
        Person {
            name: first_name(&self.name),
            color: self.color.clone(),
            photo_url: self.photo.as_ref().map(|p| format!("/photos/{p}")),
            avatar: self.avatar.clone(),
            member_id: id,
        }
    }
}

pub(super) struct YearRound {
    pub(crate) id: i64,
    pub(crate) group: i64,
    pub(crate) text: String,
    pub(crate) kind: String,
    pub(crate) anonymous: bool,
    /// (voter, target, unix time), in vote order.
    pub(crate) votes: Vec<(i64, i64, f64)>,
    /// Ranking points per member.
    pub(crate) points: HashMap<i64, i64>,
    pub(crate) messages: i64,
}

impl YearRound {
    pub(crate) fn tally(&self) -> HashMap<i64, i64> {
        let mut t = HashMap::new();
        for v in &self.votes {
            *t.entry(v.1).or_insert(0) += 1;
        }
        t
    }

    /// Pick: most votes (ties count); ranking: most points.
    pub(crate) fn winners(&self) -> HashSet<i64> {
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
pub(super) fn top<K: Ord + Copy + std::hash::Hash>(
    counts: &HashMap<K, i64>,
    min: i64,
) -> Option<(K, i64)> {
    counts
        .iter()
        .filter(|(_, v)| **v >= min)
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
        .map(|(k, v)| (*k, *v))
}

/// The title of the year: the highest tier earned during it, else the title I display.
pub(super) async fn title_card(
    state: &AppState,
    user: i64,
    year: i64,
    lang: Lang,
) -> AppResult<Option<Card>> {
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
