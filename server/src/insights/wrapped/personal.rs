use crate::platform::ids::{MemberId, RoomId, RoundId, UserId};
use std::collections::{HashMap, HashSet};

use serde_json::json;

use crate::{
    AppState,
    insights::streaks::{self, DAY, Room},
    platform::error::AppResult,
    platform::i18n::Lang,
};

use super::*;

#[allow(clippy::too_many_lines)]
pub(super) async fn personal(
    state: &AppState,
    user: UserId,
    year: i64,
    tz: i64,
    lang: Lang,
) -> AppResult<Vec<Card>> {
    let db = &state.db;
    let mine: Vec<(MemberId, RoomId, String)> = sqlx::query_as(
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
    let me_in: HashMap<RoomId, MemberId> = mine.iter().map(|(m, g, _)| (*g, *m)).collect();
    let my_seats: HashSet<MemberId> = mine.iter().map(|s| s.0).collect();
    let room_names: HashMap<RoomId, String> =
        mine.iter().map(|(_, g, n)| (*g, n.clone())).collect();
    let rooms = json!(mine.iter().map(|s| s.1).collect::<Vec<_>>()).to_string();
    let (since, until) = year_range(year, tz);

    #[allow(clippy::type_complexity)]
    let seat_rows: Vec<(
        MemberId,
        RoomId,
        Option<UserId>,
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
    let round_rows: Vec<(RoundId, RoomId, String, String, bool)> = sqlx::query_as(concat!(
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
    let vote_rows: Vec<(RoundId, MemberId, MemberId, f64)> = sqlx::query_as(concat!(
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
    let point_rows: Vec<(RoundId, MemberId, i64)> = sqlx::query_as(concat!(
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
    let message_rows: Vec<(RoundId, i64)> = sqlx::query_as(concat!(
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

    let mut seats: HashMap<MemberId, Seat> = HashMap::new();
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
    let mut faces: HashMap<UserId, MemberId> = HashMap::new();
    for (id, s) in &seats {
        if let Some(u) = s.user
            && s.current
            && u != user
        {
            let e = faces.entry(u).or_insert(*id);
            *e = (*e).max(*id);
        }
    }
    let person_of = |user: UserId| -> Option<Person> {
        let id = faces.get(&user)?;
        Some(seats.get(id)?.person(*id))
    };
    let user_of = |member: MemberId| seats.get(&member).and_then(|s| s.user);

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
    let index: HashMap<RoundId, usize> =
        rounds.iter().enumerate().map(|(i, r)| (r.id, i)).collect();
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
    let mut rooms_played: HashSet<RoomId> = HashSet::new();
    let mut days: HashSet<i64> = HashSet::new();
    let mut crush: HashMap<UserId, i64> = HashMap::new();
    let mut fans: HashMap<UserId, i64> = HashMap::new();
    let mut nemesis: HashMap<UserId, i64> = HashMap::new();
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
                n + u.raw(),
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
                n + u.raw(),
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
                n + u.raw(),
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
