use std::collections::HashMap;

use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sqlx::SqlitePool;

use crate::{
    AppState,
    accounts::auth::{Me, UserId},
    platform::error::{AppError, AppResult},
    platform::i18n::Lang,
};

use super::*;

#[derive(Debug, Clone, Serialize)]
pub struct Badge {
    pub key: &'static str,
    pub icon: &'static str,
    pub mascot: &'static str,
    pub name: &'static str,
    pub hint: String,
    pub tier: i64,
    pub tiers: [i64; 3],
    pub progress: i64,
    pub earned_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TitleChip {
    pub key: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub tier: i64,
}

impl TitleChip {
    pub fn new(key: &str, tier: i64, lang: Lang) -> Option<Self> {
        let def = find(key)?;
        (1..=3).contains(&tier).then(|| TitleChip {
            key: def.key,
            name: def.name(lang),
            icon: def.icon,
            tier,
        })
    }
}

/// An account's earned badges: key → (highest tier, when it was earned).
pub async fn earned(db: &SqlitePool, user_id: i64) -> AppResult<HashMap<String, (i64, String)>> {
    let rows: Vec<(String, i64, String)> = sqlx::query_as(
        "SELECT key, tier, earned_at FROM user_badges b WHERE user_id = ? \
         AND tier = (SELECT MAX(tier) FROM user_badges x WHERE x.user_id = b.user_id AND x.key = b.key)",
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(key, tier, at)| (key, (tier, at)))
        .collect())
}

/// `GET /badges`: my collection, in catalogue order (evaluated first, so a fresh threshold shows).
pub async fn list(State(state): State<AppState>, UserId(user): UserId) -> AppResult<Json<Value>> {
    let lang = crate::platform::i18n::of_user(&state.db, user).await?;
    let values = metrics(&state.db, user, Streaks::All, &mut RoomCache::new()).await?;
    if let Err(e) = award(&state, user, &values, None).await {
        tracing::warn!(user, error = ?e, "badges not awarded");
    }
    let held = earned(&state.db, user).await?;
    let title: Option<String> = sqlx::query_scalar("SELECT title FROM users WHERE id = ?")
        .bind(user)
        .fetch_optional(&state.db)
        .await?
        .flatten();
    let badges: Vec<Badge> = CATALOGUE
        .iter()
        .zip(values)
        .map(|(def, progress)| {
            let (tier, earned_at) = held
                .get(def.key)
                .map(|(t, at)| (*t, Some(at.clone())))
                .unwrap_or((0, None));
            let maxed = tier >= 3;
            let goal = def.tiers[(tier.clamp(0, 2)) as usize];
            Badge {
                key: def.key,
                icon: def.icon,
                mascot: def.mascot,
                name: def.name(lang),
                hint: hint(def.key, goal, lang, maxed),
                tier,
                tiers: def.tiers,
                progress,
                earned_at,
            }
        })
        .collect();
    let earned_count = badges.iter().filter(|b| b.tier > 0).count();
    Ok(Json(json!({
        "title": title.filter(|t| held.contains_key(t)),
        "earned": earned_count,
        "total": CATALOGUE.len(),
        "badges": badges,
    })))
}

#[derive(Deserialize)]
pub struct TitleBody {
    #[serde(default)]
    key: Option<Value>,
}

/// `PUT /me/title {key}`: display one of my titles (`null` clears). `400 bad_title`, `403 not_earned`.
pub async fn set_title(
    State(state): State<AppState>,
    UserId(user): UserId,
    Json(body): Json<TitleBody>,
) -> AppResult<Json<Value>> {
    let key = match body.key {
        None | Some(Value::Null) => None,
        Some(Value::String(k)) => Some(find(&k).ok_or(AppError::BadRequest("bad_title"))?.key),
        Some(_) => return Err(AppError::BadRequest("bad_title")),
    };
    if let Some(key) = key {
        let held: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM user_badges WHERE user_id = ? AND key = ?)",
        )
        .bind(user)
        .bind(key)
        .fetch_one(&state.db)
        .await?;
        if !held {
            return Err(AppError::Forbidden("not_earned"));
        }
    }
    sqlx::query("UPDATE users SET title = ? WHERE id = ?")
        .bind(key)
        .bind(user)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "title": key })))
}

/// `GET /titles`: `{members: {"<member id>": TitleChip}}`, current members of my room who display a
/// title they hold. A party answers `{members: {}}`.
pub async fn titles(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    let lang = crate::platform::i18n::of_member(&state.db, me.id).await?;
    let rows: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT m.id, u.title, MAX(b.tier) FROM members m \
         JOIN groups g ON g.id = m.group_id AND g.kind = 'room' \
         JOIN users u ON u.id = m.user_id AND u.title IS NOT NULL AND u.deleted_at IS NULL \
         JOIN user_badges b ON b.user_id = u.id AND b.key = u.title \
         WHERE m.group_id = ? AND m.removed_at IS NULL GROUP BY m.id, u.title",
    )
    .bind(me.group_id)
    .fetch_all(&state.db)
    .await?;
    let members: Map<String, Value> = rows
        .into_iter()
        .filter_map(|(id, key, tier)| {
            TitleChip::new(&key, tier, lang).map(|chip| (id.to_string(), json!(chip)))
        })
        .collect();
    Ok(Json(json!({ "members": members })))
}

/// `GET /members/{id}/badges`: a member of my room's earned badges (highest tier each) and the
/// title they display. `404` unless they are in my room.
pub async fn member(
    State(state): State<AppState>,
    Me(me): Me,
    Path(member_id): Path<i64>,
) -> AppResult<Json<Value>> {
    let user: i64 = sqlx::query_scalar(
        "SELECT m.user_id FROM members m JOIN users u ON u.id = m.user_id AND u.deleted_at IS NULL \
         WHERE m.id = ? AND m.group_id = ? AND m.removed_at IS NULL",
    )
    .bind(member_id)
    .bind(me.group_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let lang = crate::platform::i18n::of_member(&state.db, me.id).await?;
    let held = earned(&state.db, user).await?;
    let title: Option<String> = sqlx::query_scalar("SELECT title FROM users WHERE id = ?")
        .bind(user)
        .fetch_one(&state.db)
        .await?;
    let title = title.and_then(|t| TitleChip::new(&t, held.get(&t)?.0, lang));
    let badges: Vec<Value> = CATALOGUE
        .iter()
        .filter_map(|def| {
            let (tier, _) = held.get(def.key)?;
            Some(json!({
                "key": def.key, "name": def.name(lang), "icon": def.icon, "mascot": def.mascot, "tier": tier,
            }))
        })
        .collect();
    Ok(Json(json!({ "title": title, "badges": badges })))
}
