use crate::platform::i18n::Lang;
use crate::platform::ids::{MemberId, RoundId};

use super::*;

pub(super) struct Draft {
    pub(crate) key: &'static str,
    pub(crate) members: Vec<MemberId>,
    pub(crate) big: String,
    /// Candidate punchlines; one is picked from `seed`.
    pub(crate) lines: Vec<String>,
    pub(crate) seed: usize,
    pub(crate) round_id: Option<RoundId>,
}

pub(super) fn draft(
    key: &'static str,
    members: Vec<MemberId>,
    big: String,
    lines: Vec<String>,
    seed: i64,
) -> Draft {
    let seed = (seed.unsigned_abs() as usize).wrapping_add(
        members
            .iter()
            .map(|m| m.raw().unsigned_abs() as usize)
            .sum(),
    );
    Draft {
        key,
        members,
        big,
        lines,
        seed,
        round_id: None,
    }
}

/// The variant chosen by `seed`, or the next one that fits in [`LINE_MAX`] characters.
pub(super) fn pick_line(lines: &[String], seed: usize) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let n = lines.len();
    for i in 0..n {
        let l = &lines[(seed + i) % n];
        if l.chars().count() <= LINE_MAX {
            return l.clone();
        }
    }
    // Very long names: cut at a word boundary.
    let mut out = String::new();
    for word in lines[seed % n].split(' ') {
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

pub(super) struct Writer<'a> {
    name: &'a dyn Fn(MemberId) -> String,
    spicy: bool,
    pub(crate) lang: Lang,
}

impl Writer<'_> {
    pub(crate) fn name(&self, member: MemberId) -> String {
        (self.name)(member)
    }

    pub(crate) fn pick(&self, normal: Vec<String>, hot: Vec<String>) -> Vec<String> {
        if self.spicy && !hot.is_empty() {
            hot
        } else {
            normal
        }
    }

    pub(crate) fn l(&self, fr: Vec<String>, en: Vec<String>) -> Vec<String> {
        self.lang.pick(fr, en)
    }

    pub(crate) fn percent(&self, pc: i64) -> String {
        self.lang.pick(format!("{pc} %"), format!("{pc}%"))
    }

    pub(crate) fn in_a_row(&self, n: i64) -> String {
        self.lang
            .pick(format!("{n} d'affilée"), format!("{n} in a row"))
    }
}

/// Every possible story, funniest first. Spicy rooms (16+) get their own lines where written.
/// Each story has its lines in French and in English (same count, same tone).
pub(super) fn drafts(
    s: &Stats,
    name: &dyn Fn(MemberId) -> String,
    rounds: &[RoundCalc],
    spicy: bool,
    lang: Lang,
) -> Vec<Draft> {
    let w = Writer { name, spicy, lang };
    let mut d: Vec<Draft> = Vec::new();
    if let Some(a) = s.streak.as_ref().filter(|a| a.value >= 3) {
        d.push(streak(&w, a));
    }
    people_drafts(&w, s, &mut d);
    question_drafts(&w, s, &mut d);
    other_drafts(&w, s, rounds, &mut d);
    if let Some(a) = s.streak.as_ref().filter(|a| a.value < 3) {
        d.push(streak(&w, a));
    }
    d
}

pub(super) fn streak(w: &Writer, a: &Award) -> Draft {
    let (who, n) = (w.name(a.member_id), a.value);
    let lines = if n >= 3 {
        w.pick(
                w.l(
                    vec![
                        format!(
                            "{who} gagne depuis {n} questions d'affilée. Quelqu'un peut l'arrêter ?"
                        ),
                        format!(
                            "{n} victoires de suite pour {who}. Les autres font de la figuration."
                        ),
                        format!("{who} enchaîne {n} victoires. Ça devient une habitude."),
                    ],
                    vec![
                        format!("{who} has won {n} questions in a row. Can anyone stop them?"),
                        format!("{n} wins in a row for {who}. Everyone else is just an extra."),
                        format!("{who} racks up {n} wins. It's becoming a habit."),
                    ],
                ),
                w.l(
                    vec![
                        format!(
                            "{n} victoires d'affilée pour {who}. Le groupe a clairement un faible."
                        ),
                        format!(
                            "{who} gagne encore. {n} fois de suite, ce n'est plus du charme, c'est de l'hypnose."
                        ),
                    ],
                    vec![
                        format!("{n} wins in a row for {who}. The group clearly has a soft spot."),
                        format!(
                            "{who} wins again. {n} times running: that's not charm, that's hypnosis."
                        ),
                    ],
                ),
            )
    } else {
        w.l(
            vec![
                format!("{who} a gagné les {n} dernières questions. Ça commence à se voir."),
                format!("Deux victoires de suite pour {who}. Jamais deux sans trois ?"),
            ],
            vec![
                format!("{who} won the last {n} questions. People are starting to notice."),
                format!("Two wins in a row for {who}. Third time's the charm?"),
            ],
        )
    };
    draft("streak", vec![a.member_id], w.in_a_row(n), lines, n)
}

pub(super) fn round_draft(
    q: &QuestionRef,
    key: &'static str,
    members: Vec<MemberId>,
    big: String,
    lines: Vec<String>,
) -> Draft {
    let mut d = draft(key, members, big, lines, q.value + q.number);
    d.round_id = Some(q.round_id);
    d
}

pub(super) fn question_drafts(w: &Writer, s: &Stats, d: &mut Vec<Draft>) {
    if let Some(q) = &s.hottest {
        let (n, num) = (q.value, q.number);
        d.push(round_draft(
            q,
            "hottest",
            vec![],
            n.to_string(),
            w.pick(
                w.l(
                    vec![
                        format!(
                            "{n} messages sous la question #{num}. Le groupe avait des choses à dire."
                        ),
                        format!("Question #{num}, {n} messages. Ça a débattu sec."),
                        format!("{n} messages pour un seul résultat. Quelqu'un l'a mal pris."),
                    ],
                    vec![
                        format!("{n} messages under question #{num}. The group had things to say."),
                        format!("Question #{num}, {n} messages. That was a heated debate."),
                        format!("{n} messages for a single result. Someone took it badly."),
                    ],
                ),
                w.l(
                    vec![
                        format!("{n} messages sous la question #{num}. Ambiance fin de soirée."),
                        format!("Question #{num} : {n} messages. Ça a chauffé, et pas qu'un peu."),
                    ],
                    vec![
                        format!("{n} messages under question #{num}. Very end-of-the-night energy."),
                        format!("Question #{num}: {n} messages. Things got hot, and not just a little."),
                    ],
                ),
            ),
        ));
    }
    if let Some(q) = &s.most_split {
        let (n, t, num) = (q.value, q.total, q.number);
        d.push(round_draft(
            q,
            "most_split",
            vec![],
            n.to_string(),
            w.l(
                vec![
                    format!(
                        "Question #{num} : {n} noms différents pour {t} votes. Personne n'est d'accord."
                    ),
                    format!(
                        "{t} votes, {n} réponses différentes. Le consensus a pris des vacances."
                    ),
                    format!("La question #{num} a éparpillé le groupe : {n} noms pour {t} votes."),
                ],
                vec![
                    format!("Question #{num}: {n} different names for {t} votes. Nobody agrees."),
                    format!("{t} votes, {n} different answers. Consensus went on holiday."),
                    format!("Question #{num} scattered the group: {n} names for {t} votes."),
                ],
            ),
        ));
    }
    // The rest.
}
