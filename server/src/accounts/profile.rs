use axum::{
    Json,
    extract::{Multipart, State},
};
use serde::Deserialize;

use crate::{
    AppState,
    accounts::auth::{Me, new_token},
    platform::error::{AppError, AppResult},
    realtime::live::Event,
    rooms::model::{Member, MemberRow},
};

const PHOTO_SIZE: u32 = 256;

pub(crate) const PHOTO_UPLOAD_MAX: usize = 12 * 1024 * 1024;

pub(crate) async fn me(Me(me): Me) -> Json<Member> {
    Json(Member::from(&me))
}

#[derive(Deserialize)]
pub(crate) struct UpdateMe {
    name: String,
}

pub(crate) async fn update_me(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<UpdateMe>,
) -> AppResult<Json<Member>> {
    let name = crate::platform::text::clean_name(&body.name)?;
    // One person, one name: the account and every room they are in.
    sqlx::query("UPDATE users SET name = ? WHERE id = ?")
        .bind(&name)
        .bind(me.user_id)
        .execute(&state.db)
        .await?;
    sqlx::query("UPDATE members SET name = ?1 WHERE user_id = ?2 OR id = ?3")
        .bind(&name)
        .bind(me.user_id)
        .bind(me.id)
        .execute(&state.db)
        .await?;
    profile_changed(&state, &me).await
}

pub(crate) async fn upload_photo(
    State(state): State<AppState>,
    Me(me): Me,
    mut multipart: Multipart,
) -> AppResult<Json<Member>> {
    // Charged before the body is read: a flood of uploads costs its sender, not the server's memory.
    if let Some(user) = me.user_id {
        crate::platform::quotas::hit_user(&state, &crate::platform::quotas::PHOTO_USER, user)?;
    }
    let mut bytes = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::BadRequest("bad_upload"))?
    {
        if field.name() == Some("photo") {
            bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|_| AppError::BadRequest("bad_upload"))?,
            );
        }
    }
    let bytes = bytes.ok_or(AppError::BadRequest("missing_photo"))?;
    // A 128-bit random name: photos are public to whoever has the link.
    let file = format!("{}-{}.jpg", me.id, &new_token()[..32]);
    let dir = state.config.data_dir.join("photos");
    tokio::fs::create_dir_all(&dir).await?;
    let path = dir.join(&file);
    // Decoding and resizing are CPU- and memory-heavy: off the async threads, within size limits,
    // a couple at a time server-wide.
    state
        .decodes
        .run(move || -> AppResult<()> {
            let mut reader =
                image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
            reader.limits(crate::chat::media::decode_limits());
            let image = reader.decode()?.resize_to_fill(
                PHOTO_SIZE,
                PHOTO_SIZE,
                image::imageops::FilterType::Lanczos3,
            );
            image.to_rgb8().save(path)?;
            Ok(())
        })
        .await?;
    // A photo replaces the drawn avatar.
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    sqlx::query("UPDATE users SET photo = ?, avatar = NULL WHERE id = ?")
        .bind(&file)
        .bind(me.user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE members SET photo = ?1, avatar = NULL WHERE user_id = ?2 OR id = ?3")
        .bind(&file)
        .bind(me.user_id)
        .bind(me.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    // The old file goes once nothing points at it any more (a failed update keeps it).
    if let Some(old) = &me.photo {
        let _ = tokio::fs::remove_file(dir.join(old)).await;
    }
    profile_changed(&state, &me).await
}

/// After a name, photo or avatar change (account-wide): every room I sit in hears it live; my seat here comes back.
async fn profile_changed(state: &AppState, me: &MemberRow) -> AppResult<Json<Member>> {
    let seats: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT id, group_id FROM members WHERE (user_id = ?1 OR id = ?2) AND removed_at IS NULL",
    )
    .bind(me.user_id)
    .bind(me.id)
    .fetch_all(&state.db)
    .await?;
    for (member_id, group_id) in seats {
        state
            .live
            .send(group_id, Event::MemberUpdated { member_id });
    }
    let member = sqlx::query_as::<_, MemberRow>("SELECT * FROM members WHERE id = ?")
        .bind(me.id)
        .fetch_one(&state.db)
        .await?;
    Ok(Json(Member::from(&member)))
}

#[derive(Deserialize)]
pub(crate) struct AvatarBody {
    /// A spec (see `avatar`), or null to go back to the initial.
    #[serde(default)]
    avatar: Option<String>,
}

/// Pick a drawn avatar (it replaces the photo), or drop it with `null`. Account-wide, like the photo.
pub(crate) async fn set_avatar(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<AvatarBody>,
) -> AppResult<Json<Member>> {
    let avatar = body.avatar;
    if avatar
        .as_deref()
        .is_some_and(|a| !crate::accounts::avatar::valid(a))
    {
        return Err(AppError::BadRequest("bad_avatar"));
    }
    if let Some(avatar) = &avatar {
        let photos: Vec<String> = sqlx::query_scalar(
            "SELECT photo FROM users WHERE id = ?1 AND photo IS NOT NULL \
             UNION SELECT photo FROM members WHERE (user_id = ?1 OR id = ?2) AND photo IS NOT NULL",
        )
        .bind(me.user_id)
        .bind(me.id)
        .fetch_all(&state.db)
        .await?;
        sqlx::query("UPDATE users SET avatar = ?, photo = NULL WHERE id = ?")
            .bind(avatar)
            .bind(me.user_id)
            .execute(&state.db)
            .await?;
        sqlx::query("UPDATE members SET avatar = ?1, photo = NULL WHERE user_id = ?2 OR id = ?3")
            .bind(avatar)
            .bind(me.user_id)
            .bind(me.id)
            .execute(&state.db)
            .await?;
        let dir = state.config.data_dir.join("photos");
        for photo in photos {
            // Server-made names only (`<id>-<hex>.jpg`); never a path.
            if !photo.is_empty() && !photo.contains(['/', '\\']) && !photo.starts_with('.') {
                let _ = tokio::fs::remove_file(dir.join(photo)).await;
            }
        }
    } else {
        sqlx::query("UPDATE users SET avatar = NULL WHERE id = ?")
            .bind(me.user_id)
            .execute(&state.db)
            .await?;
        sqlx::query("UPDATE members SET avatar = NULL WHERE user_id = ?1 OR id = ?2")
            .bind(me.user_id)
            .bind(me.id)
            .execute(&state.db)
            .await?;
    }
    profile_changed(&state, &me).await
}
