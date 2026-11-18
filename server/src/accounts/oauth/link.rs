use rand::RngExt;

use crate::accounts::notify;
use crate::{AppState, platform::error::AppError, platform::i18n::Notice, rooms::members::COLORS};

use super::*;

pub(super) struct Outcome {
    pub(crate) user_id: i64,
    pub(crate) new: bool,
    pub(crate) linked: bool,
}

pub(super) enum Refusal {
    /// Told to the app on redeem (`identity_taken`, `already_linked`, `email_taken`).
    Because(&'static str),
    Failed(AppError),
}

impl From<sqlx::Error> for Refusal {
    fn from(e: sqlx::Error) -> Self {
        Refusal::Failed(e.into())
    }
}

/// The account for this identity: already linked, the signed-in one (link mode), the one with the
/// same verified email (both sides verified), or a new one.
pub(super) async fn resolve(
    state: &AppState,
    provider: &str,
    id: &Identity,
    link_user: Option<i64>,
) -> Result<Outcome, Refusal> {
    let db = &state.db;
    let owner: Option<i64> = sqlx::query_scalar(
        "SELECT user_id FROM oauth_identities WHERE provider = ? AND subject = ?",
    )
    .bind(provider)
    .bind(&id.subject)
    .fetch_optional(db)
    .await?;
    let verified_email = id.email.as_deref().filter(|_| id.email_verified);

    if let Some(user) = link_user {
        match owner {
            Some(o) if o == user => {}
            Some(_) => return Err(Refusal::Because("identity_taken")),
            None => {
                let other: Option<String> = sqlx::query_scalar(
                    "SELECT subject FROM oauth_identities WHERE user_id = ? AND provider = ?",
                )
                .bind(user)
                .bind(provider)
                .fetch_optional(db)
                .await?;
                if other.is_some() {
                    return Err(Refusal::Because("already_linked"));
                }
                insert_identity(state, user, provider, id).await?;
            }
        }
        if let Some(email) = verified_email {
            // The provider vouches for this address: the account's own email if it had none, or verified.
            sqlx::query(
                "UPDATE users SET email = ?1, email_verified_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
                 WHERE id = ?2 AND email IS NULL AND NOT EXISTS (SELECT 1 FROM users WHERE email = ?1)",
            )
            .bind(email)
            .bind(user)
            .execute(db)
            .await?;
            sqlx::query(
                "UPDATE users SET email_verified_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
                 WHERE id = ? AND email = ? AND email_verified_at IS NULL",
            )
            .bind(user)
            .bind(email)
            .execute(db)
            .await?;
        }
        return Ok(Outcome {
            user_id: user,
            new: false,
            linked: true,
        });
    }

    if let Some(user) = owner {
        sqlx::query("UPDATE oauth_identities SET email = ? WHERE provider = ? AND subject = ?")
            .bind(&id.email)
            .bind(provider)
            .bind(&id.subject)
            .execute(db)
            .await?;
        return Ok(Outcome {
            user_id: user,
            new: false,
            linked: false,
        });
    }

    if let Some(email) = verified_email {
        let existing: Option<(i64, Option<String>)> =
            sqlx::query_as("SELECT id, email_verified_at FROM users WHERE email = ?")
                .bind(email)
                .fetch_optional(db)
                .await?;
        match existing {
            // Both sides verified: same person.
            Some((user, Some(_))) => {
                insert_identity(state, user, provider, id).await?;
                notify(state, user, Notice::ProviderLinked(provider_name(provider)));
                return Ok(Outcome {
                    user_id: user,
                    new: false,
                    linked: false,
                });
            }
            // Someone typed this address without proving it: never hand that account over, and
            // never detach it either (it may be this very person's password account, which would
            // be orphaned). The address's owner signs in with the password or resets it.
            Some((_, None)) => return Err(Refusal::Because("email_taken")),
            None => {}
        }
    }

    let name: String = id
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or("Ami")
        .chars()
        .take(24)
        .collect();
    let color = COLORS[rand::rng().random_range(0..COLORS.len())];
    let mut tx = crate::platform::db::write_tx(db).await?;
    let user: i64 = sqlx::query_scalar(
        "INSERT INTO users (name, color, friend_code, email, email_verified_at) \
         VALUES (?, ?, upper(hex(randomblob(5))), ?, CASE WHEN ? IS NULL THEN NULL ELSE strftime('%Y-%m-%dT%H:%M:%fZ', 'now') END) \
         RETURNING id",
    )
    .bind(&name)
    .bind(color)
    .bind(verified_email)
    .bind(verified_email)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO oauth_identities (user_id, provider, subject, email) VALUES (?, ?, ?, ?)",
    )
    .bind(user)
    .bind(provider)
    .bind(&id.subject)
    .bind(&id.email)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Outcome {
        user_id: user,
        new: true,
        linked: false,
    })
}

pub(super) async fn insert_identity(
    state: &AppState,
    user: i64,
    provider: &str,
    id: &Identity,
) -> Result<(), Refusal> {
    sqlx::query(
        "INSERT INTO oauth_identities (user_id, provider, subject, email) VALUES (?, ?, ?, ?)",
    )
    .bind(user)
    .bind(provider)
    .bind(&id.subject)
    .bind(&id.email)
    .execute(&state.db)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(d) if d.is_unique_violation() => Refusal::Because("identity_taken"),
        _ => e.into(),
    })?;
    Ok(())
}
