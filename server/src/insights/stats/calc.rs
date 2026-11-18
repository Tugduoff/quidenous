use std::collections::{HashMap, HashSet};

use crate::platform::i18n::{self, Lang};

use super::*;

/// Everything loaded, ready to be turned into [`Stats`].
pub(super) struct Calc {
    period: Period,
    tz: i64,
    today: i64,
    info: HashMap<i64, MemberInfo>,
    /// Current members, by id.
    current_ids: Vec<i64>,
    current: HashSet<i64>,
    rounds: Vec<RoundCalc>,
    messages_by: HashMap<i64, i64>,
    messages: i64,
}

impl Calc {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        period: Period,
        tz: i64,
        today: i64,
        member_rows: Vec<MemberRowT>,
        round_rows: Vec<RoundRow>,
        vote_rows: Vec<(i64, i64, i64, f64)>,
        point_rows: Vec<(i64, i64, i64, f64)>,
        message_rows: Vec<(i64, i64)>,
    ) -> Self {
        let mut info = HashMap::new();
        let mut current_ids = Vec::new();
        for (id, name, joined, active, deleted) in member_rows {
            if active {
                current_ids.push(id);
            }
            info.insert(
                id,
                MemberInfo {
                    name,
                    joined,
                    deleted,
                },
            );
        }
        let current: HashSet<i64> = current_ids.iter().copied().collect();
        let mut rounds: Vec<RoundCalc> = round_rows
            .into_iter()
            .map(
                |(id, number, text, kind, opened, closed, anonymous)| RoundCalc {
                    anonymous,
                    id,
                    number,
                    text,
                    rank: kind == "rank",
                    opened,
                    closed,
                    votes: Vec::new(),
                    tally: Vec::new(),
                    points: Vec::new(),
                    winners: Vec::new(),
                    messages: 0,
                    chatters: HashSet::new(),
                },
            )
            .collect();
        let index: HashMap<i64, usize> =
            rounds.iter().enumerate().map(|(i, r)| (r.id, i)).collect();
        for (round_id, voter, target, at) in vote_rows {
            if let Some(&i) = index.get(&round_id) {
                rounds[i].votes.push((voter, target, at));
            }
        }
        for (round_id, member, points, avg) in point_rows {
            if let Some(&i) = index.get(&round_id) {
                rounds[i].points.push((member, points, avg));
            }
        }
        let mut messages_by: HashMap<i64, i64> = HashMap::new();
        let messages = message_rows.len() as i64;
        for (round_id, member) in message_rows {
            *messages_by.entry(member).or_default() += 1;
            if let Some(&i) = index.get(&round_id) {
                rounds[i].messages += 1;
                rounds[i].chatters.insert(member);
            }
        }
        for r in &mut rounds {
            let mut tally: HashMap<i64, i64> = HashMap::new();
            for &(_, target, _) in &r.votes {
                *tally.entry(target).or_default() += 1;
            }
            r.tally = tally.into_iter().collect();
            r.tally.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            if r.anonymous {
                for v in &mut r.votes {
                    v.1 = HIDDEN;
                }
            }
            // Ranking questions are won on points; ties count for everyone tied.
            r.winners = if r.rank && !r.points.is_empty() {
                let best = r.points.iter().map(|p| p.1).max().unwrap_or(0);
                r.points
                    .iter()
                    .filter(|p| p.1 == best)
                    .map(|p| p.0)
                    .collect()
            } else {
                let best = r.tally.first().map(|t| t.1).unwrap_or(0);
                r.tally
                    .iter()
                    .filter(|t| t.1 == best && best > 0)
                    .map(|t| t.0)
                    .collect()
            };
        }
        Self {
            period,
            tz,
            today,
            info,
            current_ids,
            current,
            rounds,
            messages_by,
            messages,
        }
    }

    fn present(&self, member: i64, r: &RoundCalc) -> bool {
        self.info.get(&member).is_some_and(|m| m.joined <= r.opened)
            || r.votes.iter().any(|v| v.0 == member)
    }

    pub(crate) fn finish(
        self,
        nudges: Vec<(i64, i64)>,
        authors: Vec<(i64, i64)>,
        spicy: bool,
        lang: Lang,
    ) -> Stats {
        let cur = &self.current;
        let votes_total: i64 = self.rounds.iter().map(|r| r.votes.len() as i64).sum();
        let mut s = Stats {
            period: self.period,
            members: self.current_ids.len() as i64,
            questions: self.rounds.len() as i64,
            votes: votes_total,
            nudges: nudges.len() as i64,
            seconds_voting: votes_total * SECONDS_PER_VOTE,
            messages: self.messages,
            ..Default::default()
        };
        s.median_round_seconds = median(
            &mut self
                .rounds
                .iter()
                .map(|r| r.closed - r.opened)
                .collect::<Vec<_>>(),
        )
        .map(|m| m.round() as i64);

        let mut received: HashMap<i64, i64> = HashMap::new();
        let mut cast: HashMap<i64, i64> = HashMap::new();
        let mut wins: HashMap<i64, i64> = HashMap::new();
        let mut selfish: HashMap<i64, i64> = HashMap::new();
        let mut last: HashMap<i64, i64> = HashMap::new();
        let mut pairs: HashMap<(i64, i64), i64> = HashMap::new();
        let mut delays: HashMap<i64, Vec<f64>> = HashMap::new();
        let mut night: HashMap<i64, i64> = HashMap::new();
        let mut early: HashMap<i64, i64> = HashMap::new();
        let mut heatmap = vec![vec![0i64; 24]; 7];
        // Contrarian / sheep: (eligible, against) per voter.
        let mut against: HashMap<i64, (i64, i64)> = HashMap::new();
        let mut kingmaker: HashMap<i64, i64> = HashMap::new();
        // Twins: (shared, same) per pair of voters.
        let mut twins: HashMap<(i64, i64), (i64, i64)> = HashMap::new();

        for r in &self.rounds {
            for w in &r.winners {
                *wins.entry(*w).or_default() += 1;
            }
            if r.winners.len() > 1 {
                s.ties += 1;
            }
            if r.votes.len() > 1 && r.tally.len() == 1 {
                s.unanimous += 1;
            }
            for &(target, n) in &r.tally {
                *received.entry(target).or_default() += n;
            }
            for &(voter, target, at) in &r.votes {
                *cast.entry(voter).or_default() += 1;
                if !r.anonymous {
                    if voter == target {
                        *selfish.entry(voter).or_default() += 1;
                    }
                    *pairs.entry((voter, target)).or_default() += 1;
                }
                delays.entry(voter).or_default().push(at - r.opened);
                let hour = local_hour(at, self.tz);
                heatmap[weekday(local_day(at, self.tz))][hour] += 1;
                if !(5..23).contains(&hour) {
                    *night.entry(voter).or_default() += 1;
                } else if hour < 9 {
                    *early.entry(voter).or_default() += 1;
                }
            }
            if let [.., before, last_vote] = r.votes.as_slice() {
                *last.entry(last_vote.0).or_default() += 1;
                s.seconds_waiting_last += (last_vote.2 - before.2).max(0.0).round() as i64;
            }
            if !r.rank && !r.anonymous && r.votes.len() >= 3 {
                for &(voter, target, _) in &r.votes {
                    let e = against.entry(voter).or_default();
                    e.0 += 1;
                    if !r.winners.contains(&target) {
                        e.1 += 1;
                    }
                }
                let top = r.tally.first().map(|t| t.1).unwrap_or(0);
                let second = r.tally.get(1).map(|t| t.1).unwrap_or(0);
                if let Some(winner) = r.sole_winner()
                    && top - second == 1
                {
                    for &(voter, target, _) in &r.votes {
                        if target == winner && voter != winner {
                            *kingmaker.entry(voter).or_default() += 1;
                        }
                    }
                }
            }
            let voters: Vec<(i64, i64)> = r
                .votes
                .iter()
                .filter(|v| !r.anonymous && cur.contains(&v.0))
                .map(|v| (v.0, v.1))
                .collect();
            for (i, a) in voters.iter().enumerate() {
                for b in &voters[i + 1..] {
                    let key = (a.0.min(b.0), a.0.max(b.0));
                    let e = twins.entry(key).or_default();
                    e.0 += 1;
                    if a.1 == b.1 {
                        e.1 += 1;
                    }
                }
            }
        }

        s.heatmap = heatmap;
        s.most_chosen = top(&received, cur, 1);
        s.most_crowned = top(&wins, cur, 1);
        s.narcissist = top(&selfish, cur, 1);
        s.last_voter = top(&last, cur, 1);
        let mut nudged_by: HashMap<i64, i64> = HashMap::new();
        let mut nudged: HashMap<i64, i64> = HashMap::new();
        for (from, to) in &nudges {
            *nudged_by.entry(*from).or_default() += 1;
            *nudged.entry(*to).or_default() += 1;
        }
        s.nudger = top(&nudged_by, cur, 1);
        s.most_nudged = top(&nudged, cur, 1);
        s.author = top(&authors.into_iter().collect(), cur, 1);
        s.duo = pairs
            .iter()
            .filter(|((v, t), n)| v != t && **n > 1 && cur.contains(v) && cur.contains(t))
            .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
            .map(|(&(voter_id, target_id), &value)| Duo {
                voter_id,
                target_id,
                value,
                total: None,
            });
        if votes_total > 0 {
            s.never_chosen = self
                .current_ids
                .iter()
                .copied()
                .filter(|m| !received.contains_key(m))
                .collect();
        }

        let mut medians: HashMap<i64, i64> = HashMap::new();
        for (m, d) in delays.iter_mut() {
            if d.len() >= MIN_VOTES_FOR_SPEED
                && let Some(v) = median(d)
            {
                medians.insert(*m, v.round() as i64);
            }
        }
        let mut speeds: Vec<(i64, i64)> = medians
            .iter()
            .filter(|(m, _)| cur.contains(m))
            .map(|(&m, &v)| (m, v))
            .collect();
        speeds.sort_by_key(|&(m, v)| (v, m));
        if speeds.len() >= 2 {
            let (m, v) = speeds[0];
            s.fastest = Some(Award {
                member_id: m,
                value: v,
                total: None,
            });
            let (m, v) = speeds[speeds.len() - 1];
            s.slowest = Some(Award {
                member_id: m,
                value: v,
                total: None,
            });
        }

        let enough_people = self.current_ids.len() >= 3;
        if enough_people {
            let pool: Vec<(i64, i64, i64)> = against
                .iter()
                .filter(|(m, (eligible, _))| cur.contains(m) && *eligible >= 3)
                .map(|(&m, &(eligible, a))| (m, a, eligible))
                .collect();
            if pool.len() >= 2 {
                s.contrarian = top_ratio(pool.iter().copied()).map(award_ratio);
                let contrarian = s.contrarian.as_ref().map(|a| a.member_id);
                s.sheep = top_ratio(
                    pool.iter()
                        .filter(|p| Some(p.0) != contrarian)
                        .map(|&(m, a, e)| (m, e - a, e)),
                )
                .map(award_ratio);
            }
            s.twins = top_ratio(
                twins
                    .iter()
                    .filter(|(_, (shared, same))| *shared >= 3 && *same >= 2)
                    .map(|(&k, &(shared, same))| (k, same, shared)),
            )
            .map(|((a_id, b_id), value, total)| Pair {
                a_id,
                b_id,
                value,
                total,
            });
        }
        s.kingmaker = top(&kingmaker, cur, 2);

        // Loyal: the biggest share of one's (non-self) votes going to one person.
        let mut given: HashMap<i64, i64> = HashMap::new();
        for (&(v, t), &n) in &pairs {
            if v != t {
                *given.entry(v).or_default() += n;
            }
        }
        s.loyal = top_ratio(
            pairs
                .iter()
                .filter(|((v, t), n)| v != t && **n >= 3 && cur.contains(v) && cur.contains(t))
                .filter(|((v, _), _)| given.get(v).copied().unwrap_or(0) >= 4)
                .map(|(&k, &n)| (k, n, given[&k.0])),
        )
        .map(|((voter_id, target_id), value, total)| Duo {
            voter_id,
            target_id,
            value,
            total: Some(total),
        });

        // Streaks and droughts, question by question.
        let mut streak_now: HashMap<i64, i64> = HashMap::new();
        let mut streak_best: HashMap<i64, i64> = HashMap::new();
        let mut comeback: HashMap<i64, i64> = HashMap::new();
        for &m in &self.current_ids {
            let (mut run, mut best, mut dry, mut back) = (0i64, 0i64, 0i64, 0i64);
            for r in &self.rounds {
                if r.winners.contains(&m) {
                    run += 1;
                    best = best.max(run);
                    back = back.max(dry);
                    dry = 0;
                } else {
                    run = 0;
                    if self.present(m, r) {
                        dry += 1;
                    }
                }
            }
            streak_now.insert(m, run);
            streak_best.insert(m, best);
            comeback.insert(m, back);
        }
        s.streak = top(&streak_now, cur, 2);
        s.longest_streak = top(&streak_best, cur, 3);
        s.comeback = top(&comeback, cur, 5);
        s.night_owl = top(&night, cur, 2);
        s.early_bird = top(&early, cur, 2);
        s.chatterbox = top(&self.messages_by, cur, 1);
        if enough_people && self.messages >= 10 {
            let chatterbox = s.chatterbox.as_ref().map(|a| a.member_id);
            s.silent = self
                .current_ids
                .iter()
                .filter(|m| cast.get(m).copied().unwrap_or(0) > 0 && Some(**m) != chatterbox)
                .map(|&m| (m, self.messages_by.get(&m).copied().unwrap_or(0), cast[&m]))
                .min_by(|a, b| a.1.cmp(&b.1).then(b.2.cmp(&a.2)).then(a.0.cmp(&b.0)))
                .map(|(member_id, value, _)| Award {
                    member_id,
                    value,
                    total: None,
                });
        }

        // Average position on ranking questions: mean, over questions, of the average given by voters.
        let mut positions: HashMap<i64, Vec<f64>> = HashMap::new();
        for r in self.rounds.iter().filter(|r| r.rank) {
            for &(m, _, avg) in &r.points {
                positions.entry(m).or_default().push(avg);
            }
        }
        let avg_rank: HashMap<i64, (f64, i64)> = positions
            .iter()
            .filter(|(m, p)| p.len() >= 2 && cur.contains(m))
            .map(|(&m, p)| (m, (p.iter().sum::<f64>() / p.len() as f64, p.len() as i64)))
            .collect();
        if avg_rank.len() >= 2 {
            let best = avg_rank
                .iter()
                .min_by(|a, b| {
                    a.1.0
                        .total_cmp(&b.1.0)
                        .then(b.1.1.cmp(&a.1.1))
                        .then(a.0.cmp(b.0))
                })
                .map(|(&m, &(avg, count))| RankAward {
                    member_id: m,
                    avg: round1(avg),
                    count,
                });
            let best_id = best.as_ref().map(|b| b.member_id);
            s.worst_rank = avg_rank
                .iter()
                .filter(|(m, _)| Some(**m) != best_id)
                .max_by(|a, b| {
                    a.1.0
                        .total_cmp(&b.1.0)
                        .then(a.1.1.cmp(&b.1.1))
                        .then(b.0.cmp(a.0))
                })
                .map(|(&m, &(avg, count))| RankAward {
                    member_id: m,
                    avg: round1(avg),
                    count,
                });
            s.best_rank = best;
        }

        // Question awards: ties go to the most recent question.
        s.most_split = self
            .rounds
            .iter()
            .filter(|r| !r.rank && r.votes.len() >= 4 && r.tally.len() >= 3)
            .max_by(|a, b| {
                a.tally
                    .len()
                    .cmp(&b.tally.len())
                    .then(b.tally[0].1.cmp(&a.tally[0].1))
                    .then(a.number.cmp(&b.number))
            })
            .map(|r| r.reference(r.tally.len() as i64, r.votes.len() as i64));
        s.most_unanimous = self
            .rounds
            .iter()
            .filter(|r| r.votes.len() >= 3 && r.tally.len() == 1)
            .max_by(|a, b| {
                a.votes
                    .len()
                    .cmp(&b.votes.len())
                    .then(a.number.cmp(&b.number))
            })
            .map(|r| r.reference(r.votes.len() as i64, r.votes.len() as i64));
        s.hottest = self
            .rounds
            .iter()
            .filter(|r| r.messages >= 3)
            .max_by(|a, b| a.messages.cmp(&b.messages).then(a.number.cmp(&b.number)))
            .map(|r| r.reference(r.messages, r.chatters.len() as i64));

        let mut edges: Vec<Edge> = pairs
            .iter()
            .filter(|((v, t), _)| cur.contains(v) && cur.contains(t))
            .map(|(&(voter_id, target_id), &value)| Edge {
                voter_id,
                target_id,
                value,
            })
            .collect();
        edges.sort_by_key(|e| (e.voter_id, e.target_id));
        s.activity = self.activity();
        s.profiles = self.profiles(
            &s, &pairs, &received, &cast, &wins, &selfish, &medians, &avg_rank,
        );
        s.stories = self.stories(&s, spicy, lang);
        s.edges = edges;
        s
    }

    fn activity(&self) -> Activity {
        let (bucket, start, count) = match self.period.days() {
            Some(n) => ("day", self.today - n + 1, n),
            None => match self.rounds.first() {
                None => ("day", self.today, 0),
                Some(r) => {
                    let first = local_day(r.opened, self.tz).min(self.today);
                    let span = self.today - first + 1;
                    if span <= DAILY_MAX {
                        ("day", first, span)
                    } else {
                        let monday = first - weekday(first) as i64;
                        ("week", monday, (self.today - monday) / 7 + 1)
                    }
                }
            },
        };
        let width = if bucket == "week" { 7 } else { 1 };
        let slot = |t: f64| {
            let d = local_day(t, self.tz) - start;
            (d >= 0 && d / width < count).then_some((d / width) as usize)
        };
        let mut questions = vec![0; count as usize];
        let mut votes = vec![0; count as usize];
        for r in &self.rounds {
            if let Some(i) = slot(r.opened) {
                questions[i] += 1;
            }
            for v in &r.votes {
                if let Some(i) = slot(v.2) {
                    votes[i] += 1;
                }
            }
        }
        Activity {
            bucket,
            start: date(start),
            questions,
            votes,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn profiles(
        &self,
        s: &Stats,
        pairs: &HashMap<(i64, i64), i64>,
        received: &HashMap<i64, i64>,
        cast: &HashMap<i64, i64>,
        wins: &HashMap<i64, i64>,
        selfish: &HashMap<i64, i64>,
        medians: &HashMap<i64, i64>,
        avg_rank: &HashMap<i64, (f64, i64)>,
    ) -> Vec<MemberStats> {
        let titles = titles(s);
        let top3 = |mut list: Vec<Count>| {
            list.sort_by(|a, b| b.value.cmp(&a.value).then(a.member_id.cmp(&b.member_id)));
            list.truncate(3);
            list
        };
        self.current_ids
            .iter()
            .map(|&m| {
                let present = self.rounds.iter().filter(|r| self.present(m, r)).count() as i64;
                let won = wins.get(&m).copied().unwrap_or(0);
                let best = self
                    .rounds
                    .iter()
                    .filter(|r| !r.rank)
                    .filter_map(|r| {
                        let mine = r.tally.iter().find(|t| t.0 == m)?.1;
                        Some((r, mine, r.votes.len() as i64))
                    })
                    .max_by(|a, b| {
                        ratio_cmp((a.1, a.2), (b.1, b.2))
                            .then(a.1.cmp(&b.1))
                            .then(a.0.number.cmp(&b.0.number))
                    })
                    .map(|(r, mine, total)| r.reference(mine, total));
                let worst = self
                    .rounds
                    .iter()
                    .filter(|r| r.rank)
                    .filter_map(|r| {
                        let avg = r.points.iter().find(|p| p.0 == m)?.2;
                        Some((r, avg))
                    })
                    .max_by(|a, b| a.1.total_cmp(&b.1).then(a.0.number.cmp(&b.0.number)))
                    .map(|(r, avg)| r.reference(avg.round() as i64, r.points.len() as i64));
                MemberStats {
                    member_id: m,
                    present,
                    votes_cast: cast.get(&m).copied().unwrap_or(0),
                    received: received.get(&m).copied().unwrap_or(0),
                    wins: won,
                    win_rate: if present > 0 {
                        ((won as f64 / present as f64).min(1.0) * 1000.0).round() / 1000.0
                    } else {
                        0.0
                    },
                    messages: self.messages_by.get(&m).copied().unwrap_or(0),
                    self_votes: selfish.get(&m).copied().unwrap_or(0),
                    median_vote_seconds: medians.get(&m).copied(),
                    avg_rank: avg_rank.get(&m).map(|a| round1(a.0)),
                    top_voters: top3(
                        pairs
                            .iter()
                            .filter(|((v, t), _)| *t == m && *v != m && self.current.contains(v))
                            .map(|(&(v, _), &value)| Count {
                                member_id: v,
                                value,
                            })
                            .collect(),
                    ),
                    top_targets: top3(
                        pairs
                            .iter()
                            .filter(|((v, t), _)| *v == m && *t != m && self.current.contains(t))
                            .map(|(&(_, t), &value)| Count {
                                member_id: t,
                                value,
                            })
                            .collect(),
                    ),
                    best,
                    worst,
                    titles: titles.get(&m).cloned().unwrap_or_default(),
                }
            })
            .collect()
    }

    fn name(&self, id: i64, lang: Lang) -> String {
        self.info
            .get(&id)
            .map(|m| {
                if m.deleted {
                    i18n::deleted_name(lang).to_owned()
                } else {
                    first_name(&m.name)
                }
            })
            .unwrap_or_else(|| "?".into())
    }

    fn stories(&self, s: &Stats, spicy: bool, lang: Lang) -> Vec<Story> {
        let drafts = drafts(s, &|id| self.name(id, lang), &self.rounds, spicy, lang);
        let mut used: HashMap<i64, usize> = HashMap::new();
        let mut keys: HashSet<&'static str> = HashSet::new();
        let mut out = Vec::new();
        for d in drafts {
            if out.len() >= MAX_STORIES {
                break;
            }
            if keys.contains(d.key)
                || d.members
                    .iter()
                    .any(|m| used.get(m).copied().unwrap_or(0) >= STORIES_PER_MEMBER)
            {
                continue;
            }
            for m in &d.members {
                *used.entry(*m).or_default() += 1;
            }
            keys.insert(d.key);
            let (title, icon) = meta(d.key, lang);
            out.push(Story {
                key: d.key,
                icon,
                title,
                big: d.big,
                line: pick_line(&d.lines, d.seed),
                member_ids: d.members,
                round_id: d.round_id,
            });
        }
        out
    }
}

pub(super) fn award_ratio((member_id, value, total): (i64, i64, i64)) -> Award {
    Award {
        member_id,
        value,
        total: Some(total),
    }
}
