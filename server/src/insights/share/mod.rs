use crate::platform::ids::{MemberId, RoomId, RoundId, UserId};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::Me,
    insights::stats,
    platform::error::{AppError, AppResult},
    platform::i18n::{self, Lang},
    rounds::game,
};

const W: u32 = 1200;
const H: u32 = 630;
macro_rules! live_sql {
    () => {
        "(s.expires_at IS NULL OR s.expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))"
    };
}
pub(crate) use live_sql;
macro_rules! expires_sql {
    () => {
        "strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+90 days')"
    };
}

mod pages;
mod render;
mod svg;
pub(crate) use pages::*;
pub(crate) use render::*;
use svg::{esc, pack_svg, svg};

#[derive(Deserialize)]
pub(crate) struct ShareBody {
    #[serde(default)]
    result: bool,
}

pub(crate) async fn create(
    State(state): State<AppState>,
    Me(me): Me,
    Path(round_id): Path<RoundId>,
    Json(body): Json<ShareBody>,
) -> AppResult<Json<Value>> {
    let round = game::round(&state.db, me.group_id, round_id).await?;
    let with_result = body.result && round.closed_at.is_some();
    let lang = i18n::of_member(&state.db, me.id).await?;
    let existing: Option<String> = sqlx::query_scalar(concat!(
        "SELECT token FROM shares s WHERE round_id = ? AND member_id = ? AND with_result = ? AND lang = ? AND ",
        live_sql!()
    ))
    .bind(round_id)
    .bind(me.id)
    .bind(with_result)
    .bind(lang.as_str())
    .fetch_optional(&state.db)
    .await?;
    let token = if let Some(t) = existing {
        t
    } else {
        let t = crate::accounts::auth::new_token()[..12].to_owned();
        sqlx::query(concat!(
            "INSERT INTO shares (token, round_id, member_id, with_result, lang, expires_at) \
             VALUES (?, ?, ?, ?, ?, ",
            expires_sql!(),
            ")"
        ))
        .bind(&t)
        .bind(round_id)
        .bind(me.id)
        .bind(with_result)
        .bind(lang.as_str())
        .execute(&state.db)
        .await?;
        t
    };
    Ok(Json(
        json!({ "token": token, "path": format!("/s/{token}") }),
    ))
}

#[derive(Deserialize)]
pub(crate) struct StatShareBody {
    #[serde(default)]
    period: Option<String>,
    key: String,
    #[serde(default)]
    tz: Option<i64>,
}

pub(crate) async fn create_stat(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<StatShareBody>,
) -> AppResult<Json<Value>> {
    let period = stats::Period::parse(body.period.as_deref())?;
    let tz = stats::clamp_tz(body.tz.unwrap_or(0));
    let lang = i18n::of_member(&state.db, me.id).await?;
    let s = stats::for_group_in(&state.db, me.group_id, period, tz, lang).await?;
    let story = s
        .stories
        .into_iter()
        .find(|st| st.key == body.key)
        .ok_or(AppError::NotFound)?;
    let mut people: Vec<(String, String, Option<String>)> = Vec::new();
    for id in &story.member_ids {
        let (name, color, avatar, user_id): (String, String, Option<String>, Option<UserId>) =
            sqlx::query_as(
                "SELECT name, color, avatar, user_id FROM members WHERE id = ? AND group_id = ?",
            )
            .bind(id)
            .bind(me.group_id)
            .fetch_one(&state.db)
            .await?;
        let first = if i18n::is_deleted(&name, user_id) {
            i18n::deleted_name(lang).to_owned()
        } else {
            name.split_whitespace().next().unwrap_or("?").to_owned()
        };
        people.push((first, color, avatar));
    }
    let token = freeze_stat(
        &state.db,
        Frozen {
            group_id: me.group_id,
            member_id: me.id,
            key: story.key,
            title: story.title,
            big: &story.big,
            line: &story.line,
            people: &json!(people).to_string(),
            lang,
            member_ids: &json!(story.member_ids).to_string(),
            label: None,
        },
    )
    .await?;
    Ok(Json(
        json!({ "token": token, "path": format!("/s/{token}") }),
    ))
}

pub(crate) struct Frozen<'a> {
    pub group_id: RoomId,
    pub member_id: MemberId,
    pub key: &'a str,
    pub title: &'a str,
    pub big: &'a str,
    pub line: &'a str,
    pub people: &'a str,
    pub lang: Lang,
    pub member_ids: &'a str,
    pub label: Option<&'a str>,
}

pub(crate) async fn freeze_stat(db: &sqlx::SqlitePool, f: Frozen<'_>) -> AppResult<String> {
    let existing: Option<String> = sqlx::query_scalar(concat!(
        "SELECT token FROM stat_shares s WHERE group_id = ? AND member_id = ? AND key = ? AND big = ? \
         AND line = ? AND people = ? AND lang = ? AND label IS ? AND ",
        live_sql!(),
        " ORDER BY created_at DESC LIMIT 1"
    ))
    .bind(f.group_id)
    .bind(f.member_id)
    .bind(f.key)
    .bind(f.big)
    .bind(f.line)
    .bind(f.people)
    .bind(f.lang.as_str())
    .bind(f.label)
    .fetch_optional(db)
    .await?;
    if let Some(token) = existing {
        return Ok(token);
    }
    let token = crate::accounts::auth::new_token()[..12].to_owned();
    sqlx::query(concat!(
        "INSERT INTO stat_shares (token, group_id, member_id, key, title, big, line, people, lang, member_ids, label, expires_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ",
        expires_sql!(),
        ")"
    ))
    .bind(&token)
    .bind(f.group_id)
    .bind(f.member_id)
    .bind(f.key)
    .bind(f.title)
    .bind(f.big)
    .bind(f.line)
    .bind(f.people)
    .bind(f.lang.as_str())
    .bind(f.member_ids)
    .bind(f.label)
    .execute(db)
    .await?;
    Ok(token)
}

pub(crate) async fn mine(
    State(state): State<AppState>,
    crate::accounts::auth::SignedIn(me): crate::accounts::auth::SignedIn,
) -> AppResult<Json<Value>> {
    #[allow(clippy::type_complexity)]
    let rows: Vec<(String, String, String, String, bool, String, Option<String>)> =
        sqlx::query_as(concat!(
            "SELECT * FROM (\
               SELECT s.token, 'question' AS kind, q.text AS title, g.name AS room, s.with_result, \
                      s.created_at, s.expires_at \
               FROM shares s JOIN members m ON m.id = s.member_id JOIN rounds r ON r.id = s.round_id \
               JOIN questions q ON q.id = r.question_id JOIN groups g ON g.id = r.group_id AND g.deleted_at IS NULL \
               WHERE m.user_id = ?1 AND ",
            live_sql!(),
            " UNION ALL \
               SELECT s.token, 'stat', s.title || ' · ' || s.big, COALESCE(s.label, g.name), 1, s.created_at, s.expires_at \
               FROM stat_shares s JOIN members m ON m.id = s.member_id \
               JOIN groups g ON g.id = s.group_id AND g.deleted_at IS NULL \
               WHERE m.user_id = ?1 AND ",
            live_sql!(),
            ") ORDER BY created_at DESC LIMIT 100"
        ))
        .bind(me)
        .fetch_all(&state.db)
        .await?;
    let out: Vec<Value> = rows
        .into_iter()
        .map(
            |(token, kind, title, room, with_result, created_at, expires_at)| {
                json!({
                    "token": token, "path": format!("/s/{token}"), "kind": kind, "title": title,
                    "room": room, "with_result": with_result, "created_at": created_at,
                    "expires_at": expires_at,
                })
            },
        )
        .collect();
    Ok(Json(json!(out)))
}

pub(crate) async fn revoke(
    State(state): State<AppState>,
    crate::accounts::auth::SignedIn(me): crate::accounts::auth::SignedIn,
    Path(token): Path<String>,
) -> AppResult<Json<Value>> {
    macro_rules! revoke_sql {
        ($table:literal) => {
            concat!(
                "DELETE FROM ", $table, " WHERE token = ?1 AND member_id IN (SELECT m.id FROM members m \
                 WHERE m.user_id = ?2 OR EXISTS (SELECT 1 FROM members o WHERE o.group_id = m.group_id \
                 AND o.user_id = ?2 AND o.role = 'owner' AND o.removed_at IS NULL))"
            )
        };
    }
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let mut gone = 0;
    for sql in [revoke_sql!("shares"), revoke_sql!("stat_shares")] {
        gone += sqlx::query(sql)
            .bind(&token)
            .bind(me)
            .execute(&mut *tx)
            .await?
            .rows_affected();
    }
    tx.commit().await?;
    if gone == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(json!({ "ok": true })))
}

enum Card {
    Question(QuestionCard),
    Stat(StatCard),
}

impl Card {
    fn lang(&self) -> Lang {
        match self {
            Card::Question(c) => c.lang,
            Card::Stat(c) => c.lang,
        }
    }
}

struct StatCard {
    lang: Lang,
    room: String,
    label: Option<String>,
    title: String,
    big: String,
    line: String,
    people: Vec<Person>,
}

struct Person {
    name: String,
    color: String,
    avatar: Option<String>,
}

struct QuestionCard {
    lang: Lang,
    room: String,
    number: i64,
    question: String,
    rank: bool,
    podium: Vec<(Person, String)>,
}

async fn load(state: &AppState, token: &str) -> AppResult<Card> {
    let question: Option<(RoundId, RoomId, bool, String)> = sqlx::query_as(concat!(
        "SELECT s.round_id, r.group_id, s.with_result, s.lang FROM shares s JOIN rounds r ON r.id = s.round_id \
         JOIN groups g ON g.id = r.group_id AND g.deleted_at IS NULL WHERE s.token = ? AND ",
        live_sql!()
    ))
    .bind(token)
    .fetch_optional(&state.db)
    .await?;
    let Some((round_id, group_id, with_result, lang)) = question else {
        return load_stat(state, token).await.map(Card::Stat);
    };
    let lang = Lang::from_db(Some(&lang));
    let round = game::round(&state.db, group_id, round_id).await?;
    let room: String = sqlx::query_scalar("SELECT name FROM groups WHERE id = ?")
        .bind(group_id)
        .fetch_one(&state.db)
        .await?;
    let member = |id: MemberId| {
        let db = state.db.clone();
        async move {
            let row = sqlx::query_as::<_, (String, String, Option<String>, Option<UserId>)>(
                "SELECT name, color, avatar, user_id FROM members WHERE id = ?",
            )
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap_or_else(|_| ("?".into(), "#8D8D99".into(), None, None));
            let name = if i18n::is_deleted(&row.0, row.3) {
                i18n::deleted_name(lang).to_owned()
            } else {
                row.0.split_whitespace().next().unwrap_or("?").to_owned()
            };
            Person {
                name,
                color: row.1,
                avatar: row.2,
            }
        }
    };
    let mut podium = Vec::new();
    if with_result && round.closed_at.is_some() {
        if round.kind == "said" {
            let result = crate::rounds::said::result(&state.db, round_id).await?;
            for s in result
                .into_iter()
                .flat_map(|r| r.scores)
                .filter(|s| s.points > 0)
                .take(3)
            {
                podium.push((member(s.member_id).await, i18n::points(s.points)));
            }
        } else if round.kind == "choose" {
            let scores = game::choose_result(&state.db, round_id).await?.scores;
            for s in scores.into_iter().filter(|s| s.hits > 0).take(3) {
                podium.push((member(s.member_id).await, format!("{}/{}", s.hits, s.total)));
            }
        } else if round.kind == "rank" {
            for r in game::borda(&state.db, round_id).await?.into_iter().take(3) {
                podium.push((member(r.member_id).await, i18n::points(r.points)));
            }
        } else {
            let tallies = game::tallies(&state.db, round_id).await?;
            let total: i64 = tallies.iter().map(|t| t.count).sum::<i64>().max(1);
            for t in tallies.into_iter().take(3) {
                podium.push((
                    member(t.target_id).await,
                    i18n::percent(lang, t.count * 100 / total),
                ));
            }
        }
    }
    Ok(Card::Question(QuestionCard {
        lang,
        room,
        number: round.number,
        question: round.text,
        rank: round.kind == "rank",
        podium,
    }))
}

async fn load_stat(state: &AppState, token: &str) -> AppResult<StatCard> {
    #[allow(clippy::type_complexity)]
    let (room, title, big, line, people, lang, label): (
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
    ) = sqlx::query_as(concat!(
        "SELECT g.name, s.title, s.big, s.line, s.people, s.lang, s.label FROM stat_shares s \
             JOIN groups g ON g.id = s.group_id AND g.deleted_at IS NULL WHERE s.token = ? AND ",
        live_sql!()
    ))
    .bind(token)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(StatCard {
        lang: Lang::from_db(Some(&lang)),
        room,
        label,
        title,
        big,
        line,
        people: parse_people(&people),
    })
}

fn parse_people(json: &str) -> Vec<Person> {
    let rows: Vec<Vec<Value>> = serde_json::from_str(json).unwrap_or_default();
    rows.into_iter()
        .filter_map(|row| {
            Some(Person {
                name: row.first()?.as_str()?.to_owned(),
                color: row.get(1)?.as_str()?.to_owned(),
                avatar: row.get(2).and_then(Value::as_str).map(str::to_owned),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::svg::label_year;
    use super::*;

    #[test]
    fn stat_people_old_and_new_forms() {
        let old = parse_people(r##"[["Ana","#FF7A45"],["Bob","#5BC8FF"]]"##);
        assert_eq!(old.len(), 2);
        assert_eq!(
            (old[1].name.as_str(), old[1].color.as_str()),
            ("Bob", "#5BC8FF")
        );
        assert!(old.iter().all(|p| p.avatar.is_none()));
        let new = parse_people(r##"[["Ana","#FF7A45","0.1"],["Bob","#5BC8FF",null]]"##);
        assert_eq!(new[0].avatar.as_deref(), Some("0.1"));
        assert!(new[1].avatar.is_none());
        assert!(parse_people("nope").is_empty());
        assert_eq!(parse_people(r##"[["Ana"],[1,2],["Bob","#000"]]"##).len(), 1);
    }

    #[test]
    fn cards_with_avatars_render() {
        let spec = crate::accounts::avatar::presets()[0].clone();
        let people = |n: usize| -> Vec<Person> {
            (0..n)
                .map(|i| Person {
                    name: format!("P{i}"),
                    color: "#FF7A45".into(),
                    avatar: match i % 3 {
                        0 => Some(spec.clone()),
                        1 => None,
                        _ => Some("9.9".into()),
                    },
                })
                .collect()
        };
        let stat = Card::Stat(StatCard {
            lang: Lang::En,
            room: "Salon".into(),
            label: None,
            title: "Titre".into(),
            big: "42".into(),
            line: "Une ligne".into(),
            people: people(3),
        });
        let wrapped = Card::Stat(StatCard {
            lang: Lang::Fr,
            room: "Salon".into(),
            label: Some("Wrapped 2026".into()),
            title: "Ton crush de l'année".into(),
            big: "Léa".into(),
            line: "Tu as voté 27 fois pour Léa. On ne dira rien.".into(),
            people: people(3),
        });
        let wrapped_svg = svg(&wrapped);
        assert!(wrapped_svg.contains(">2026</text>") && wrapped_svg.contains("Wrapped 2026"));
        assert!(!wrapped_svg.contains(">Salon<"));
        assert_eq!(label_year("Salon · 2026"), Some("2026"));
        assert_eq!(label_year("Salon"), None);
        let question = Card::Question(QuestionCard {
            lang: Lang::Fr,
            room: "Salon".into(),
            number: 3,
            question: "Qui ?".into(),
            rank: false,
            podium: people(3).into_iter().map(|p| (p, "50 %".into())).collect(),
        });
        for card in [stat, wrapped, question] {
            let svg = svg(&card);
            assert_eq!(svg.matches("clipPath id=").count(), 1, "{svg}");
            assert!(render(&card).is_some());
        }
    }
}
