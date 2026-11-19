use super::{Draft, RoundCalc, Stats, Writer, decimal, draft, duration, pct, round_draft};

pub(super) fn other_drafts(w: &Writer, s: &Stats, rounds: &[RoundCalc], d: &mut Vec<Draft>) {
    if let Some(p) = &s.loyal {
        let same_as_duo = s
            .duo
            .as_ref()
            .is_some_and(|c| (c.voter_id, c.target_id) == (p.voter_id, p.target_id));
        if !same_as_duo {
            let (a, b, n, t) = (
                w.name(p.voter_id),
                w.name(p.target_id),
                p.value,
                p.total.unwrap_or(0),
            );
            let pc = pct(n, t);
            d.push(draft(
                "loyal",
                vec![p.voter_id, p.target_id],
                w.percent(pc),
                w.pick(
                    w.l(
                        vec![
                            format!("{pc} % des votes de {a} vont à {b}. La fidélité, ça existe."),
                            format!("{a} vote pour {b} {n} fois sur {t}. Inébranlable."),
                        ],
                        vec![
                            format!("{pc}% of {a}'s votes go to {b}. Loyalty is real."),
                            format!("{a} votes for {b} {n} times out of {t}. Unshakeable."),
                        ],
                    ),
                    w.l(
                        vec![
                            format!("{a} réserve {pc} % de ses voix à {b}. Exclusif, apparemment."),
                            format!("{n} votes sur {t} pour {b}. {a} ne regarde personne d'autre."),
                        ],
                        vec![
                            format!(
                                "{a} saves {pc}% of their votes for {b}. Exclusive, apparently."
                            ),
                            format!("{n} votes out of {t} for {b}. {a} has eyes for no one else."),
                        ],
                    ),
                ),
                n,
            ));
        }
    }
    if let Some(a) = &s.longest_streak {
        let same_as_streak = s
            .streak
            .as_ref()
            .is_some_and(|c| c.member_id == a.member_id && c.value == a.value);
        if !same_as_streak {
            let (who, n) = (w.name(a.member_id), a.value);
            d.push(draft(
                "longest_streak",
                vec![a.member_id],
                w.in_a_row(n),
                w.l(
                    vec![
                        format!(
                            "Record : {n} victoires de suite pour {who}. Personne n'a fait mieux."
                        ),
                        format!(
                            "{who} a aligné {n} victoires d'affilée. Le record tient toujours."
                        ),
                    ],
                    vec![
                        format!("Record: {n} wins in a row for {who}. Nobody has done better."),
                        format!(
                            "{who} strung together {n} wins in a row. The record still stands."
                        ),
                    ],
                ),
                n,
            ));
        }
    }
    if let Some(a) = &s.most_chosen {
        let (who, n) = (w.name(a.member_id), a.value);
        d.push(draft(
            "most_chosen",
            vec![a.member_id],
            n.to_string(),
            w.pick(
                w.l(
                    vec![
                        format!("{n} votes reçus. Le groupe ne voit que par {who}."),
                        format!("{who} a reçu {n} votes. Une vraie tête d'affiche."),
                    ],
                    vec![
                        format!("{n} votes received. The group only has eyes for {who}."),
                        format!("{who} got {n} votes. A true headliner."),
                    ],
                ),
                w.l(
                    vec![
                        format!("{n} votes pour {who}. Tout le monde y pense, personne ne le dit."),
                        format!("{who} attire {n} votes. Magnétique, c'est le mot."),
                    ],
                    vec![
                        format!("{n} votes for {who}. Everyone's thinking it, nobody's saying it."),
                        format!("{who} pulls in {n} votes. Magnetic is the word."),
                    ],
                ),
            ),
            n,
        ));
    }
    if let Some(a) = &s.most_crowned {
        let (who, n) = (w.name(a.member_id), a.value);
        d.push(draft(
            "most_crowned",
            vec![a.member_id],
            n.to_string(),
            w.l(
                vec![
                    format!("{who} a gagné {n} questions. Il va falloir une étagère."),
                    format!("{n} couronnes pour {who}. À ce rythme, on va manquer de place."),
                ],
                vec![
                    format!("{who} won {n} questions. Time to buy a trophy shelf."),
                    format!("{n} crowns for {who}. At this rate, we'll run out of room."),
                ],
            ),
            n,
        ));
    }
    if let Some(a) = &s.sheep {
        let (who, n, t) = (w.name(a.member_id), a.value, a.total.unwrap_or(0));
        let pc = pct(n, t);
        d.push(draft(
            "sheep",
            vec![a.member_id],
            w.percent(pc),
            w.l(
                vec![
                    format!(
                        "{who} vote avec la majorité {pc} % du temps. Du flair, ou le troupeau ?"
                    ),
                    format!("{n} fois sur {t}, {who} a suivi le groupe. Toujours du bon côté."),
                ],
                vec![
                    format!("{who} votes with the majority {pc}% of the time. Good instincts, or the herd?"),
                    format!("{n} times out of {t}, {who} went with the group. Always on the right side."),
                ],
            ),
            n,
        ));
    }
    if let Some(a) = &s.chatterbox {
        let (who, n) = (w.name(a.member_id), a.value);
        d.push(draft(
            "chatterbox",
            vec![a.member_id],
            n.to_string(),
            w.pick(
                w.l(
                    vec![
                        format!("{n} messages signés {who}. Le chat, c'est sa deuxième maison."),
                        format!("{who} a écrit {n} messages. Les autres votent, {who} commente."),
                    ],
                    vec![
                        format!("{n} messages from {who}. The chat is their second home."),
                        format!("{who} wrote {n} messages. The others vote, {who} comments."),
                    ],
                ),
                w.l(
                    vec![
                        format!(
                            "{n} messages de {who}. Toujours le dernier mot, et souvent le plus osé."
                        ),
                        format!(
                            "{who} a écrit {n} messages. Ça parle beaucoup, et pas toujours sagement."
                        ),
                    ],
                    vec![
                        format!("{n} messages from {who}. Always the last word, often the boldest."),
                        format!("{who} wrote {n} messages. Lots of talk, not always well-behaved."),
                    ],
                ),
            ),
            n,
        ));
    }
    if let Some(a) = &s.silent {
        let (who, n) = (w.name(a.member_id), a.value);
        let lines = if n == 0 {
            w.l(
                vec![
                    format!("Zéro message pour {who}. Ça vote, ça observe, ça ne dit rien."),
                    format!("{who} n'a jamais écrit un mot dans le chat. Le mystère reste entier."),
                ],
                vec![
                    format!("Zero messages from {who}. Votes, watches, says nothing."),
                    format!("{who} has never written a word in the chat. The mystery remains."),
                ],
            )
        } else {
            let pl = if n > 1 { "s" } else { "" };
            w.pick(
                w.l(
                    vec![
                        format!("{n} message{pl} en tout pour {who}. Rare, donc précieux."),
                        format!("{who} parle peu : {n} message{pl}. Mais ça vote, et ça observe."),
                    ],
                    vec![
                        format!("{n} message{pl} in total from {who}. Rare, so precious."),
                        format!("{who} says little: {n} message{pl}. But votes, and watches."),
                    ],
                ),
                w.l(
                    vec![format!(
                        "Discrétion totale pour {who} : {n} message{pl}. Les plus calmes cachent bien leur jeu."
                    )],
                    vec![format!(
                        "Total discretion from {who}: {n} message{pl}. It's always the quiet ones."
                    )],
                ),
            )
        };
        d.push(draft("silent", vec![a.member_id], n.to_string(), lines, n));
    }
    if let Some(a) = &s.early_bird {
        let (who, n) = (w.name(a.member_id), a.value);
        d.push(draft(
            "early_bird",
            vec![a.member_id],
            n.to_string(),
            w.pick(
                w.l(
                    vec![
                        format!("{n} votes avant 9 h. {who} vote avant même le café."),
                        format!("{who} vote au réveil, {n} fois avant 9 h. Respect."),
                    ],
                    vec![
                        format!("{n} votes before 9 a.m. {who} votes before their coffee."),
                        format!("{who} votes on waking up, {n} times before 9 a.m. Respect."),
                    ],
                ),
                w.l(
                    vec![format!(
                        "{n} votes avant 9 h. {who} vote encore au lit, visiblement."
                    )],
                    vec![format!(
                        "{n} votes before 9 a.m. {who} votes from bed, clearly."
                    )],
                ),
            ),
            n,
        ));
    }
    if let Some(a) = &s.fastest {
        let (who, t) = (w.name(a.member_id), duration(a.value, w.lang));
        d.push(draft(
            "fastest",
            vec![a.member_id],
            t.clone(),
            w.l(
                vec![
                    format!("{who} vote en {t}, en général. Ça ne traîne pas."),
                    format!("{t} chrono : {who} a déjà voté avant que tu lises la question."),
                ],
                vec![
                    format!("{who} usually votes in {t}. No hanging around."),
                    format!("{t} flat: {who} has voted before you've read the question."),
                ],
            ),
            a.value,
        ));
    }
    if let Some(a) = &s.last_voter {
        let (who, n) = (w.name(a.member_id), a.value);
        d.push(draft(
            "last_voter",
            vec![a.member_id],
            n.to_string(),
            w.pick(
                w.l(
                    vec![
                        format!("{n} fois, tout le monde attendait {who}. Mais ça valait le coup."),
                        format!(
                            "Dernier vote du groupe : {who}, {n} fois. L'art de se faire attendre."
                        ),
                    ],
                    vec![
                        format!("{n} times, everyone was waiting for {who}. Worth it, though."),
                        format!("Last vote in the group: {who}, {n} times. The art of the late entrance."),
                    ],
                ),
                w.l(
                    vec![format!(
                        "{who} vote en dernier, {n} fois. Toujours aimé se faire désirer."
                    )],
                    vec![format!(
                        "{who} votes last, {n} times. Always loved to keep them waiting."
                    )],
                ),
            ),
            n,
        ));
    }
    if let Some(q) = &s.most_unanimous {
        let n = q.value;
        let winner = rounds
            .iter()
            .find(|r| r.id == q.round_id)
            .and_then(|r| r.tally.first().map(|t| t.0));
        let (members, lines) = match winner {
            Some(top) => {
                let who = w.name(top);
                (
                    vec![top],
                    w.l(
                        vec![
                            format!(
                                "{n} votes sur {n} pour {who}. Pour une fois, tout le monde est d'accord."
                            ),
                            format!(
                                "Question #{} : {who}, à l'unanimité. Aucun débat.",
                                q.number
                            ),
                        ],
                        vec![
                            format!("{n} votes out of {n} for {who}. For once, everyone agrees."),
                            format!("Question #{}: {who}, unanimously. No debate.", q.number),
                        ],
                    ),
                )
            }
            None => (
                vec![],
                w.l(
                    vec![format!(
                        "{n} votes, un seul nom. Pour une fois, tout le monde est d'accord."
                    )],
                    vec![format!(
                        "{n} votes, a single name. For once, everyone agrees."
                    )],
                ),
            ),
        };
        d.push(round_draft(
            q,
            "most_unanimous",
            members,
            w.lang.pick(format!("{n} sur {n}"), format!("{n} of {n}")),
            lines,
        ));
    }
    if let Some(r) = &s.best_rank {
        let (who, avg, c) = (w.name(r.member_id), decimal(r.avg, w.lang), r.count);
        d.push(draft(
            "best_rank",
            vec![r.member_id],
            avg.clone(),
            w.l(
                vec![
                    format!(
                        "Place moyenne de {who} dans les classements : {avg}. Abonnement au podium."
                    ),
                    format!(
                        "Sur {c} classements, {who} est en moyenne à la place {avg}. Indéboulonnable."
                    ),
                ],
                vec![
                    format!("{who}'s average place in rankings: {avg}. A podium regular."),
                    format!("Over {c} rankings, {who} averages place {avg}. Unbudgeable."),
                ],
            ),
            c,
        ));
    }
    if let Some(a) = &s.nudger {
        let (who, n) = (w.name(a.member_id), a.value);
        d.push(draft(
            "nudger",
            vec![a.member_id],
            n.to_string(),
            w.l(
                vec![
                    format!(
                        "{who} a relancé les autres {n} fois. Quelqu'un doit bien s'en charger."
                    ),
                    format!("{n} relances signées {who}. Le réveil-matin du groupe."),
                ],
                vec![
                    format!("{who} nudged the others {n} times. Someone has to do it."),
                    format!("{n} nudges from {who}. The group's alarm clock."),
                ],
            ),
            n,
        ));
    }
    if let Some(a) = &s.most_nudged {
        let (who, n) = (w.name(a.member_id), a.value);
        d.push(draft(
            "most_nudged",
            vec![a.member_id],
            n.to_string(),
            w.l(
                vec![
                    format!("{n} relances pour {who}. Le groupe ne lâche rien."),
                    format!("On a relancé {who} {n} fois. Et ça finit toujours par voter."),
                ],
                vec![
                    format!("{n} nudges for {who}. The group never gives up."),
                    format!("{who} got nudged {n} times. And always votes in the end."),
                ],
            ),
            n,
        ));
    }
    if let Some(a) = &s.author {
        let (who, n) = (w.name(a.member_id), a.value);
        d.push(draft(
            "author",
            vec![a.member_id],
            n.to_string(),
            w.l(
                vec![
                    format!("{who} a écrit {n} questions. Le groupe lui doit pas mal de débats."),
                    format!("{n} questions signées {who}. La plume officielle du groupe."),
                ],
                vec![
                    format!("{who} wrote {n} questions. The group owes them quite a few debates."),
                    format!("{n} questions by {who}. The group's official pen."),
                ],
            ),
            n,
        ));
    }
    if let Some(r) = &s.worst_rank {
        let (who, avg) = (w.name(r.member_id), decimal(r.avg, w.lang));
        d.push(draft(
            "worst_rank",
            vec![r.member_id],
            avg.clone(),
            w.l(
                vec![
                    format!(
                        "Place moyenne de {who} : {avg}. Il faut bien quelqu'un pour fermer la marche."
                    ),
                    format!(
                        "{who} ferme la marche des classements, {avg} de moyenne. Avec panache."
                    ),
                ],
                vec![
                    format!("{who}'s average place: {avg}. Someone has to bring up the rear."),
                    format!("{who} brings up the rear in rankings, {avg} on average. With style."),
                ],
            ),
            r.count,
        ));
    }
}
