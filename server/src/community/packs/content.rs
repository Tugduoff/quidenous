use crate::platform::ids::{PackId, UserId};
use serde::Deserialize;

use crate::{
    platform::error::{AppError, AppResult},
    platform::i18n::Lang,
};

use super::*;

#[derive(Deserialize)]
pub struct QuestionIn {
    text: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    items: Option<Vec<String>>,
    #[serde(default)]
    anonymous: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackQuestion {
    pub text: String,
    pub kind: String,
    pub items: Option<Vec<String>>,
    pub anonymous: bool,
}

pub(super) async fn stored_questions<'e, E: sqlx::SqliteExecutor<'e>>(
    db: E,
    id: PackId,
) -> AppResult<Vec<PackQuestion>> {
    let rows: Vec<(String, String, Option<String>, bool)> = sqlx::query_as(
        "SELECT text, kind, items, anonymous FROM pack_questions WHERE pack_id = ? ORDER BY position",
    )
    .bind(id)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(text, kind, items, anonymous)| PackQuestion {
            text,
            kind,
            items: items.and_then(|i| serde_json::from_str(&i).ok()),
            anonymous,
        })
        .collect())
}

#[derive(Deserialize)]
pub struct PackBody {
    pub(crate) name: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) text: Option<String>,
    pub(crate) language: Option<String>,
    pub(crate) tags: Option<Vec<String>>,
    pub(crate) spicy: Option<bool>,
    pub(crate) featured: Option<bool>,
    pub(crate) questions: Option<Vec<QuestionIn>>,
}

pub(super) fn clean_meta(name: &str, description: &str) -> AppResult<(String, String)> {
    let name = crate::platform::text::line(name);
    let description = crate::platform::text::line(description);
    if name.is_empty() || name.chars().count() > NAME_MAX || description.chars().count() > DESC_MAX
    {
        return Err(AppError::BadRequest("bad_pack"));
    }
    Ok((name, description))
}

pub(super) fn clean_language(language: &str) -> AppResult<Lang> {
    Lang::parse(language.trim()).ok_or(AppError::BadRequest("bad_language"))
}

pub(super) fn clean_tags(tags: &[String]) -> AppResult<Vec<&'static str>> {
    let mut out: Vec<&'static str> = Vec::new();
    for tag in tags {
        let tag = tag.trim();
        let known = CATEGORIES
            .iter()
            .find(|c| **c == tag)
            .ok_or(AppError::BadRequest("bad_tags"))?;
        if !out.contains(known) {
            out.push(known);
        }
    }
    if out.len() > MAX_TAGS {
        return Err(AppError::BadRequest("bad_tags"));
    }
    Ok(out)
}

pub(super) fn from_file(file: &crate::community::packfile::PackFile) -> Vec<PackQuestion> {
    file.lines
        .iter()
        .map(|l| PackQuestion {
            text: l.text.clone(),
            kind: l.kind.to_owned(),
            items: l.items.clone(),
            anonymous: l.anonymous,
        })
        .collect()
}

pub(super) fn from_list(list: Vec<QuestionIn>, known: &[PackQuestion]) -> Vec<PackQuestion> {
    list.into_iter()
        .map(|q| {
            let text = q.text.trim().to_owned();
            let kind = match q.kind.as_deref() {
                Some("rank") => "rank",
                Some("choose") => "choose",
                Some("said") => "said",
                _ => "pick",
            };
            let items = match (kind, q.items) {
                ("choose", Some(items)) => crate::community::deck::clean_items(items),
                ("choose", None) => known
                    .iter()
                    .find(|k| k.kind == "choose" && k.text.to_lowercase() == text.to_lowercase())
                    .and_then(|k| k.items.clone()),
                _ => None,
            };
            let anonymous = q.anonymous.unwrap_or_else(|| {
                known
                    .iter()
                    .any(|k| k.anonymous && k.text.to_lowercase() == text.to_lowercase())
            });
            PackQuestion {
                anonymous: crate::rooms::anonymous::wanted(kind, anonymous),
                text,
                kind: kind.to_owned(),
                items,
            }
        })
        .collect()
}

pub fn clean_questions(mut questions: Vec<PackQuestion>) -> AppResult<Vec<PackQuestion>> {
    for q in &mut questions {
        q.text = crate::platform::text::line(&q.text);
    }
    questions.retain(|q| {
        q.kind != "said"
            && (QUESTION_MIN..=QUESTION_MAX).contains(&q.text.chars().count())
            && (q.kind != "choose"
                || q.items
                    .clone()
                    .and_then(crate::community::deck::clean_items)
                    .is_some())
    });
    for q in &mut questions {
        if q.kind != "choose" {
            q.items = None;
        }
    }
    let mut seen = std::collections::HashSet::new();
    questions.retain(|q| seen.insert(crate::platform::text::key(&q.text)));
    if questions.len() < MIN_QUESTIONS || questions.len() > MAX_QUESTIONS {
        return Err(AppError::BadRequest("pack_size"));
    }
    Ok(questions)
}

pub(super) async fn insert_questions(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    id: PackId,
    questions: &[PackQuestion],
) -> AppResult<()> {
    for (i, q) in questions.iter().enumerate() {
        sqlx::query(
            "INSERT INTO pack_questions (pack_id, position, text, kind, items, anonymous) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(i as i64)
        .bind(&q.text)
        .bind(&q.kind)
        .bind(q.items.as_ref().map(|i| serde_json::to_string(i).unwrap_or_default()))
        .bind(q.anonymous)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub struct NewPack {
    pub name: String,
    pub description: String,
    pub language: Lang,
    pub tags: Vec<&'static str>,
    pub spicy: bool,
    pub featured: bool,
    pub status: &'static str,
    pub questions: Vec<PackQuestion>,
}

pub(super) async fn live_packs<'e, E: sqlx::SqliteExecutor<'e>>(
    db: E,
    author: UserId,
) -> AppResult<i64> {
    Ok(sqlx::query_scalar(
        "SELECT COUNT(*) FROM packs WHERE author_id = ? AND status IN ('public', 'review')",
    )
    .bind(author)
    .fetch_one(db)
    .await?)
}

pub async fn create_pack(
    db: &sqlx::SqlitePool,
    author: UserId,
    pack: &NewPack,
    capped: bool,
) -> AppResult<PackId> {
    let mut tx = crate::platform::db::write_tx(db).await?;
    if capped && live_packs(&mut *tx, author).await? >= PACKS_PER_USER {
        return Err(AppError::Conflict("too_many_packs"));
    }
    let id: PackId = sqlx::query_scalar(
        "INSERT INTO packs (name, description, author_id, language, tags, spicy, featured, status) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(&pack.name)
    .bind(&pack.description)
    .bind(author)
    .bind(pack.language.as_str())
    .bind(pack.tags.join(","))
    .bind(pack.spicy)
    .bind(pack.featured)
    .bind(pack.status)
    .fetch_one(&mut *tx)
    .await?;
    insert_questions(&mut tx, id, &pack.questions).await?;
    tx.commit().await?;
    Ok(id)
}
