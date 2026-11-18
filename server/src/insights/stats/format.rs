use std::{
    collections::{HashMap, HashSet},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::platform::i18n::Lang;

use super::*;

pub(super) fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or_default()
}

pub(super) fn local_secs(t: f64, tz: i64) -> i64 {
    t.floor() as i64 + tz
}

pub(super) fn local_day(t: f64, tz: i64) -> i64 {
    local_secs(t, tz).div_euclid(DAY)
}

pub(super) fn local_hour(t: f64, tz: i64) -> usize {
    (local_secs(t, tz).rem_euclid(DAY) / 3600) as usize
}

/// Monday = 0 (1970-01-01 was a Thursday).
pub(super) fn weekday(day: i64) -> usize {
    (day + 3).rem_euclid(7) as usize
}

/// `YYYY-MM-DD` of a day number since 1970-01-01.
pub(super) fn date(day: i64) -> String {
    let z = day + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

pub(super) fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.total_cmp(b));
    let mid = values.len() / 2;
    Some(if values.len().is_multiple_of(2) {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    })
}

pub(super) fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// Highest value (>= `min`), lowest member id on ties, current members only.
pub(super) fn top(counts: &HashMap<i64, i64>, current: &HashSet<i64>, min: i64) -> Option<Award> {
    counts
        .iter()
        .filter(|(m, v)| **v >= min.max(1) && current.contains(m))
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
        .map(|(&member_id, &value)| Award {
            member_id,
            value,
            total: None,
        })
}

/// `a/b > c/d` without floats.
pub(super) fn ratio_cmp(a: (i64, i64), b: (i64, i64)) -> std::cmp::Ordering {
    (i128::from(a.0) * i128::from(b.1)).cmp(&(i128::from(b.0) * i128::from(a.1)))
}

/// Biggest share `value / total`, then higher value, then lower id.
pub(super) fn top_ratio<K: Ord + Copy>(
    items: impl Iterator<Item = (K, i64, i64)>,
) -> Option<(K, i64, i64)> {
    items.filter(|(_, v, t)| *v > 0 && *t > 0).max_by(|a, b| {
        ratio_cmp((a.1, a.2), (b.1, b.2))
            .then(a.1.cmp(&b.1))
            .then(b.0.cmp(&a.0))
    })
}

pub(crate) fn pct(value: i64, total: i64) -> i64 {
    if total <= 0 {
        0
    } else {
        (value * 100 + total / 2) / total
    }
}

/// "42 s", "12 min", "3 h 12", "2 j 4 h" (English: "2 d 4 h").
pub(super) fn duration(secs: i64, lang: Lang) -> String {
    let secs = secs.max(0);
    match secs {
        s if s < 60 => format!("{s} s"),
        s if s < 3600 => format!("{} min", s / 60),
        s if s < DAY => format!("{} h {:02}", s / 3600, (s % 3600) / 60),
        s => format!("{} {} {} h", s / DAY, lang.pick("j", "d"), (s % DAY) / 3600),
    }
}

/// "2,1": French decimal comma; "2.1" in English.
pub(super) fn decimal(v: f64, lang: Lang) -> String {
    let s = format!("{v:.1}");
    match lang {
        Lang::Fr => s.replace('.', ","),
        Lang::En => s,
    }
}

pub(crate) fn first_name(name: &str) -> String {
    name.split_whitespace().next().unwrap_or("?").to_owned()
}

/// Award keys each member holds.
pub(super) fn titles(s: &Stats) -> HashMap<i64, Vec<&'static str>> {
    let mut t: HashMap<i64, Vec<&'static str>> = HashMap::new();
    let mut add = |key: &'static str, ids: &[i64]| {
        for id in ids {
            let list = t.entry(*id).or_default();
            if !list.contains(&key) {
                list.push(key);
            }
        }
    };
    let awards: [(&'static str, &Option<Award>); 19] = [
        ("most_chosen", &s.most_chosen),
        ("most_crowned", &s.most_crowned),
        ("narcissist", &s.narcissist),
        ("fastest", &s.fastest),
        ("slowest", &s.slowest),
        ("last_voter", &s.last_voter),
        ("nudger", &s.nudger),
        ("most_nudged", &s.most_nudged),
        ("author", &s.author),
        ("contrarian", &s.contrarian),
        ("sheep", &s.sheep),
        ("kingmaker", &s.kingmaker),
        ("streak", &s.streak),
        ("longest_streak", &s.longest_streak),
        ("comeback", &s.comeback),
        ("night_owl", &s.night_owl),
        ("early_bird", &s.early_bird),
        ("chatterbox", &s.chatterbox),
        ("silent", &s.silent),
    ];
    for (key, award) in awards {
        if let Some(a) = award {
            add(key, &[a.member_id]);
        }
    }
    if let Some(d) = &s.duo {
        add("duo", &[d.voter_id, d.target_id]);
    }
    if let Some(d) = &s.loyal {
        add("loyal", &[d.voter_id, d.target_id]);
    }
    if let Some(p) = &s.twins {
        add("twins", &[p.a_id, p.b_id]);
    }
    if let Some(r) = &s.best_rank {
        add("best_rank", &[r.member_id]);
    }
    if let Some(r) = &s.worst_rank {
        add("worst_rank", &[r.member_id]);
    }
    add("never_chosen", &s.never_chosen);
    t
}

/// (title in `lang`, lucide icon) of an award key.
pub fn meta(key: &str, lang: Lang) -> (&'static str, &'static str) {
    let (fr, en, icon) = match key {
        "most_chosen" => ("La star", "The star", "star"),
        "most_crowned" => ("Collection de couronnes", "Crown collector", "crown"),
        "narcissist" => ("Narcisse", "Narcissus", "flower-2"),
        "fastest" => ("Gâchette facile", "Quick draw", "zap"),
        "slowest" => ("Prend son temps", "Takes their time", "snail"),
        "last_voter" => ("Tout le monde attend", "Everyone's waiting", "hourglass"),
        "nudger" => ("Relou officiel", "Official pest", "bell-ring"),
        "most_nudged" => ("Toujours relancé", "Always nudged", "alarm-clock"),
        "author" => ("La plume", "The pen", "feather"),
        "duo" => ("Le crush", "The crush", "heart"),
        "never_chosen" => ("Les invisibles", "The invisibles", "ghost"),
        "contrarian" => ("Le contrarian", "The contrarian", "shuffle"),
        "sheep" => ("Le mouton", "The sheep", "users"),
        "loyal" => ("Le fidèle", "The loyal one", "anchor"),
        "twins" => ("Les jumeaux de vote", "Vote twins", "copy"),
        "kingmaker" => ("Le faiseur de rois", "The kingmaker", "gavel"),
        "streak" => ("Série en cours", "Current streak", "flame"),
        "longest_streak" => ("Record de série", "Longest streak", "trophy"),
        "comeback" => ("Le revenant", "The comeback", "rotate-ccw"),
        "night_owl" => ("Couche-tard", "Night owl", "moon"),
        "early_bird" => ("Lève-tôt", "Early bird", "sunrise"),
        "chatterbox" => ("Le bavard", "The chatterbox", "message-circle"),
        "silent" => ("Le roi du silence", "Silent king", "volume-x"),
        "best_rank" => ("Toujours sur le podium", "Always on the podium", "medal"),
        "worst_rank" => ("Lanterne rouge", "Bringing up the rear", "turtle"),
        "most_split" => ("La plus clivante", "Most divisive", "split"),
        "most_unanimous" => ("La plus unanime", "Most unanimous", "handshake"),
        "hottest" => ("Le moment le plus chaud", "Hottest moment", "flame"),
        _ => ("Stat", "Stat", "sparkles"),
    };
    (lang.pick(fr, en), icon)
}
