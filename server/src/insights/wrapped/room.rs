use std::collections::HashMap;

use crate::{
    AppState,
    insights::stats,
    insights::streaks::{self, Room},
    platform::error::AppResult,
    platform::i18n::Lang,
    rooms::model::MemberRow,
};

use super::*;

#[allow(clippy::too_many_lines)]
pub(super) async fn room(
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
