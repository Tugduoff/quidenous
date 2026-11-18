use super::*;

pub(super) fn people_drafts(w: &Writer, s: &Stats, d: &mut Vec<Draft>) {
    if let Some(a) = &s.comeback {
        let (who, n) = (w.name(a.member_id), a.value);
        d.push(draft(
            "comeback",
            vec![a.member_id],
            format!("{n} questions"),
            w.pick(
                w.l(
                    vec![
                        format!("{n} questions sans rien, puis {who} gagne. Le retour que personne n'attendait."),
                        format!("Après {n} questions dans l'ombre, {who} remonte sur le trône."),
                        format!("{who} n'avait rien gagné depuis {n} questions. C'est réparé."),
                    ],
                    vec![
                        format!("{n} questions with nothing, then {who} wins. Nobody saw that coming."),
                        format!("After {n} questions in the shadows, {who} is back on the throne."),
                        format!("{who} hadn't won anything in {n} questions. Fixed."),
                    ],
                ),
                w.l(
                    vec![
                        format!("{n} questions dans l'ombre, puis {who} revient. Comme un ex à 2 h du matin."),
                        format!("{who} revient après {n} questions d'absence. Le retour de flamme."),
                    ],
                    vec![
                        format!("{n} questions in the shadows, then {who} is back. Like an ex at 2 a.m."),
                        format!("{who} is back after {n} questions away. The old flame rekindles."),
                    ],
                ),
            ),
            n,
        ));
    }
    if let Some(a) = s.narcissist.as_ref().filter(|a| a.value >= 2) {
        let (who, n) = (w.name(a.member_id), a.value);
        d.push(draft(
            "narcissist",
            vec![a.member_id],
            n.to_string(),
            w.pick(
                w.l(
                    vec![
                        format!("{who} s'est donné sa propre voix {n} fois. La confiance, ça ne s'achète pas."),
                        format!("{n} votes de {who} pour {who}. Au moins une personne y croit."),
                        format!("{who} vote pour {who}, {n} fois. On n'est jamais mieux servi que par soi-même."),
                    ],
                    vec![
                        format!("{who} voted for themselves {n} times. You can't buy that confidence."),
                        format!("{n} votes from {who} for {who}. At least someone believes."),
                        format!("{who} votes for {who}, {n} times. If you want it done right…"),
                    ],
                ),
                w.l(
                    vec![
                        format!("Le crush numéro un de {who} ? {who}. {n} votes pour soi."),
                        format!("{n} votes pour soi. {who} s'aime, et l'assume pleinement."),
                    ],
                    vec![
                        format!("{who}'s number one crush? {who}. {n} votes for themselves."),
                        format!("{n} votes for themselves. {who} is in love, and owns it."),
                    ],
                ),
            ),
            n,
        ));
    }
    if let Some(p) = &s.duo {
        let (a, b, n) = (w.name(p.voter_id), w.name(p.target_id), p.value);
        d.push(draft(
            "duo",
            vec![p.voter_id, p.target_id],
            n.to_string(),
            w.pick(
                w.l(
                    vec![
                        format!("{a} a voté {n} fois pour {b}. On dit ça, on dit rien."),
                        format!(
                            "{n} votes de {a} pour {b}. Ce n'est plus un hasard, c'est un signal."
                        ),
                        format!(
                            "{a} trouve toujours une raison de choisir {b}. {n} fois, pour être précis."
                        ),
                    ],
                    vec![
                        format!("{a} voted for {b} {n} times. Just saying."),
                        format!("{n} votes from {a} for {b}. That's not chance, that's a signal."),
                        format!("{a} always finds a reason to pick {b}. {n} times, to be exact."),
                    ],
                ),
                w.l(
                    vec![
                        format!(
                            "{a} a choisi {b} {n} fois. À ce stade, il faut un deuxième rendez-vous."
                        ),
                        format!(
                            "{n} votes de {a} pour {b}. Quelqu'un leur réserve une table pour deux ?"
                        ),
                        format!("{a} et ses {n} votes pour {b}. Le groupe a vu, le groupe sait."),
                    ],
                    vec![
                        format!("{a} picked {b} {n} times. At this point, it's time for a second date."),
                        format!("{n} votes from {a} for {b}. Can someone book them a table for two?"),
                        format!("{a} and their {n} votes for {b}. The group saw. The group knows."),
                    ],
                ),
            ),
            n,
        ));
    }
    if let Some(p) = &s.twins {
        let (a, b, n, t) = (w.name(p.a_id), w.name(p.b_id), p.value, p.total);
        let pc = pct(n, t);
        d.push(draft(
            "twins",
            vec![p.a_id, p.b_id],
            w.percent(pc),
            w.pick(
                w.l(
                    vec![
                        format!("{a} et {b} votent pareil {pc} % du temps. Télépathie ou triche ?"),
                        format!("Même choix {n} fois sur {t}. {a} et {b} partagent un cerveau."),
                        format!(
                            "{a} et {b} : d'accord {n} fois sur {t}. Ça se concerte en douce ?"
                        ),
                    ],
                    vec![
                        format!(
                            "{a} and {b} vote the same {pc}% of the time. Telepathy or cheating?"
                        ),
                        format!("Same pick {n} times out of {t}. {a} and {b} share a brain."),
                        format!("{a} and {b}: agreed {n} times out of {t}. Conferring in secret?"),
                    ],
                ),
                w.l(
                    vec![
                        format!("{a} et {b} : {pc} % de votes identiques. Ça se fiance quand ?"),
                        format!(
                            "{a} et {b} en phase {n} fois sur {t}. Il se passe quoi en coulisses ?"
                        ),
                    ],
                    vec![
                        format!("{a} and {b}: {pc}% identical votes. When's the engagement?"),
                        format!(
                            "{a} and {b} in sync {n} times out of {t}. What's going on backstage?"
                        ),
                    ],
                ),
            ),
            n,
        ));
    }
    if let Some(a) = &s.contrarian {
        let (who, n, t) = (w.name(a.member_id), a.value, a.total.unwrap_or(0));
        let pc = pct(n, t);
        d.push(draft(
            "contrarian",
            vec![a.member_id],
            w.percent(pc),
            w.l(
                vec![
                    format!(
                        "{who} vote à contre-courant {pc} % du temps. Toujours un avis à part."
                    ),
                    format!("{n} votes sur {t} contre l'avis du groupe. {who} fait bande à part."),
                    format!("Quand tout le monde va à gauche, {who} va à droite. {pc} % du temps."),
                ],
                vec![
                    format!(
                        "{who} votes against the tide {pc}% of the time. Always their own take."
                    ),
                    format!("{n} votes out of {t} against the group. {who} goes it alone."),
                    format!("When everyone goes left, {who} goes right. {pc}% of the time."),
                ],
            ),
            n,
        ));
    }
    if let Some(a) = &s.kingmaker {
        let (who, n) = (w.name(a.member_id), a.value);
        d.push(draft(
            "kingmaker",
            vec![a.member_id],
            n.to_string(),
            w.pick(
                w.l(
                    vec![
                        format!(
                            "{n} victoires à une voix près, et {who} était dedans. Craignez son vote."
                        ),
                        format!("{who} a fait basculer {n} votes serrés. Le vrai pouvoir est là."),
                        format!("Gagné à une voix près, {n} fois : c'était celle de {who}."),
                    ],
                    vec![
                        format!("{n} wins by a single vote, and {who} was in on it. Fear their vote."),
                        format!("{who} tipped {n} close votes. That's where the real power is."),
                        format!("Won by one vote, {n} times: it was {who}'s."),
                    ],
                ),
                w.l(
                    vec![
                        format!(
                            "{n} victoires serrées décidées par {who}. On fait la cour à qui, déjà ?"
                        ),
                        format!(
                            "{who} a fait basculer {n} votes serrés. Tout le monde veut ses faveurs."
                        ),
                    ],
                    vec![
                        format!("{n} close wins decided by {who}. Remind us who to flirt with?"),
                        format!("{who} tipped {n} close votes. Everyone wants their favour."),
                    ],
                ),
            ),
            n,
        ));
    }
    if let Some(a) = &s.night_owl {
        let (who, n) = (w.name(a.member_id), a.value);
        d.push(draft(
            "night_owl",
            vec![a.member_id],
            n.to_string(),
            w.pick(
                w.l(
                    vec![
                        format!("{n} votes entre 23 h et 5 h. {who} ne dort jamais, apparemment."),
                        format!("{who} vote quand tout le monde dort. {n} fois après 23 h."),
                        format!("{n} votes nocturnes pour {who}. Les meilleures idées viennent la nuit."),
                    ],
                    vec![
                        format!("{n} votes between 11 p.m. and 5 a.m. {who} never sleeps, apparently."),
                        format!("{who} votes while everyone sleeps. {n} times after 11 p.m."),
                        format!("{n} late-night votes for {who}. The best ideas come at night."),
                    ],
                ),
                w.l(
                    vec![
                        format!("{n} votes après 23 h. On ne demandera pas à {who} ce qui se passait à côté."),
                        format!("{who} vote la nuit, {n} fois. Insomnie ou soirée qui s'éternise ?"),
                    ],
                    vec![
                        format!("{n} votes after 11 p.m. We won't ask {who} what else was going on."),
                        format!("{who} votes at night, {n} times. Insomnia or a party that won't end?"),
                    ],
                ),
            ),
            n,
        ));
    }
    if let Some(a) = &s.slowest {
        let (who, t) = (w.name(a.member_id), duration(a.value, w.lang));
        d.push(draft(
            "slowest",
            vec![a.member_id],
            t.clone(),
            w.pick(
                w.l(
                    vec![
                        format!("{who} met {t} à voter, en général. Le suspense, c'est un art."),
                        format!("{t} avant de voter. {who} a le temps de se faire un café."),
                        format!("{who} vote en {t}, en médiane. Rien ne presse, visiblement."),
                    ],
                    vec![
                        format!("{who} usually takes {t} to vote. Suspense is an art."),
                        format!("{t} before voting. {who} has time to make a coffee."),
                        format!("{who} votes in {t}, median. No rush, clearly."),
                    ],
                ),
                w.l(
                    vec![
                        format!("{t} pour se décider. {who} aime faire monter la tension."),
                        format!(
                            "{who} fait attendre tout le monde {t}. L'art de se faire désirer."
                        ),
                    ],
                    vec![
                        format!("{t} to make up their mind. {who} likes to build the tension."),
                        format!("{who} keeps everyone waiting {t}. Playing hard to get."),
                    ],
                ),
            ),
            a.value / 60,
        ));
    }
}
