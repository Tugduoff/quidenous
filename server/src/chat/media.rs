use crate::platform::ids::{MessageId, RoomId, RoundId};
use std::{
    io::Cursor,
    path::{Path as FsPath, PathBuf},
};

use axum::{
    Json,
    body::Body,
    extract::{Multipart, Path, State, multipart::MultipartError},
    http::{HeaderValue, StatusCode, header},
    response::Response,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use image::{
    DynamicImage, ImageDecoder, ImageFormat, ImageReader, codecs::jpeg::JpegEncoder,
    imageops::FilterType, metadata::Orientation,
};
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::Me,
    accounts::crypto::{constant_eq, random_hex},
    chat::messages::MessageRow,
    chat::messages::NewMessage,
    platform::error::{AppError, AppResult},
};

pub const IMAGE_MAX: usize = 8 * 1024 * 1024;
pub const UPLOAD_BODY_MAX: usize = IMAGE_MAX + 64 * 1024;
const MAX_SIDE: u32 = 1600;
const JPEG_QUALITY: u8 = 85;
const DECODE_MAX_SIDE: u32 = 4096;
const DECODE_MAX_ALLOC: u64 = 64 * 1024 * 1024;
const DECODES_AT_ONCE: usize = 2;
const URL_PERIOD: u64 = 6 * 3600;
const MAC_LEN: usize = 32;
const MIMES: [&str; 3] = ["image/jpeg", "image/png", "image/gif"];

pub fn fallback(kind: &str, caption: Option<&str>) -> String {
    let label = if kind == "gif" { "GIF" } else { "📷 Photo" };
    match caption {
        Some(c) => format!("{label} · {c}"),
        None => label.to_owned(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Jpeg,
    Png,
    Webp,
    Gif,
}

fn sniff(bytes: &[u8]) -> Option<Format> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(Format::Jpeg)
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(Format::Png)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(Format::Gif)
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(Format::Webp)
    } else {
        None
    }
}

struct Processed {
    bytes: Vec<u8>,
    ext: &'static str,
    mime: &'static str,
    width: u32,
    height: u32,
}

fn process(bytes: Vec<u8>, format: Format) -> AppResult<Processed> {
    let mut reader = ImageReader::with_format(
        Cursor::new(bytes.as_slice()),
        match format {
            Format::Jpeg => ImageFormat::Jpeg,
            Format::Png => ImageFormat::Png,
            Format::Webp => ImageFormat::WebP,
            Format::Gif => ImageFormat::Gif,
        },
    );
    reader.limits(decode_limits());
    if format == Format::Gif {
        let first = reader.decode()?;
        let (width, height) = (first.width(), first.height());
        return Ok(Processed {
            bytes,
            ext: "gif",
            mime: "image/gif",
            width,
            height,
        });
    }
    let mut decoder = reader.into_decoder()?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);
    if image.width() > MAX_SIDE || image.height() > MAX_SIDE {
        image = image.resize(MAX_SIDE, MAX_SIDE, FilterType::CatmullRom);
    }
    let mut out = Vec::new();
    let png = format == Format::Png || (format == Format::Webp && image.color().has_alpha());
    let (ext, mime) = if png {
        image.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)?;
        ("png", "image/png")
    } else {
        JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY).encode_image(&image.to_rgb8())?;
        ("jpg", "image/jpeg")
    };
    Ok(Processed {
        bytes: out,
        ext,
        mime,
        width: image.width(),
        height: image.height(),
    })
}

pub(crate) fn decode_limits() -> image::Limits {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(DECODE_MAX_SIDE);
    limits.max_image_height = Some(DECODE_MAX_SIDE);
    limits.max_alloc = Some(DECODE_MAX_ALLOC);
    limits
}

#[derive(Clone)]
pub struct Decodes(std::sync::Arc<tokio::sync::Semaphore>);

impl Default for Decodes {
    fn default() -> Self {
        Self(std::sync::Arc::new(tokio::sync::Semaphore::new(
            DECODES_AT_ONCE,
        )))
    }
}

impl Decodes {
    pub(crate) async fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> AppResult<T> + Send + 'static,
    ) -> AppResult<T> {
        let slot = self
            .0
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        tokio::task::spawn_blocking(move || {
            let _slot = slot;
            work()
        })
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
    }
}

fn upload_error(e: &MultipartError) -> AppError {
    if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
        AppError::TooLarge("too_large")
    } else {
        AppError::BadRequest("bad_upload")
    }
}

fn room_dir(data_dir: &FsPath, room: RoomId) -> PathBuf {
    data_dir.join("chat").join(room.to_string())
}

pub fn file_path(data_dir: &FsPath, room: RoomId, media: Option<&str>) -> Option<PathBuf> {
    let media: Value = serde_json::from_str(media?).ok()?;
    let file = media["file"].as_str()?;
    let safe = !file.is_empty()
        && !file.starts_with('.')
        && file.chars().all(|c| c.is_ascii_alphanumeric() || c == '.');
    safe.then(|| room_dir(data_dir, room).join(file))
}

pub async fn remove_files(paths: Vec<PathBuf>) {
    for path in paths {
        if let Err(e) = tokio::fs::remove_file(&path).await
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(path = %path.display(), error = %e, "chat image not removed");
        }
    }
}

pub async fn remove_room(state: &AppState, room: RoomId) {
    let dir = room_dir(&state.config.data_dir, room);
    if let Err(e) = tokio::fs::remove_dir_all(&dir).await
        && e.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(%room, error = %e, "chat images of a deleted room not removed");
    }
}

pub(crate) async fn upload(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<RoundId>,
    mut multipart: Multipart,
) -> AppResult<Json<MessageRow>> {
    let round = crate::chat::messages::chat_round(&state, &me, id).await?;
    let user = me.user_id;
    if let Some(user) = user {
        crate::platform::quotas::hit_user(&state, &crate::platform::quotas::CHAT_IMAGE_USER, user)?;
        if let Err(e) = crate::platform::quotas::hit_user(
            &state,
            &crate::platform::quotas::CHAT_IMAGE_DAY_USER,
            user,
        ) {
            crate::platform::quotas::refund_user(
                &state,
                &crate::platform::quotas::CHAT_IMAGE_USER,
                user,
            );
            return Err(e);
        }
    }
    let (mut image, mut caption, mut mentions, mut everyone) = (None, None, Vec::new(), false);
    let mut key = None;
    while let Some(field) = multipart.next_field().await.map_err(|e| upload_error(&e))? {
        match field.name() {
            Some("image") => image = Some(field.bytes().await.map_err(|e| upload_error(&e))?),
            Some("caption") => caption = Some(field.text().await.map_err(|e| upload_error(&e))?),
            Some("mentions") => {
                let text = field.text().await.map_err(|e| upload_error(&e))?;
                mentions = serde_json::from_str(&text).unwrap_or_default();
            }
            Some("everyone") => {
                let text = field.text().await.map_err(|e| upload_error(&e))?;
                everyone = matches!(text.trim(), "1" | "true");
            }
            Some("client_key") => key = Some(field.text().await.map_err(|e| upload_error(&e))?),
            _ => {}
        }
    }
    let image = image.ok_or(AppError::BadRequest("missing_image"))?;
    if image.len() > IMAGE_MAX {
        return Err(AppError::TooLarge("too_large"));
    }
    let format = sniff(&image).ok_or(AppError::BadRequest("bad_image"))?;
    let caption = crate::chat::messages::clean_caption(caption.as_deref())?;
    let key = crate::chat::messages::client_key(key.as_deref())?;
    if let Some(posted) = crate::chat::messages::retried(&state, &me, key.as_deref()).await? {
        if let Some(user) = user {
            crate::platform::quotas::refund_user(
                &state,
                &crate::platform::quotas::CHAT_IMAGE_USER,
                user,
            );
            crate::platform::quotas::refund_user(
                &state,
                &crate::platform::quotas::CHAT_IMAGE_DAY_USER,
                user,
            );
        }
        return Ok(Json(posted));
    }
    crate::chat::messages::chat_quota(&state, &me)?;
    let image = state
        .decodes
        .run(move || process(image.to_vec(), format))
        .await?;
    let file = format!("{}.{}", random_hex(16), image.ext);
    let dir = room_dir(&state.config.data_dir, me.group_id);
    tokio::fs::create_dir_all(&dir).await?;
    let path = dir.join(&file);
    tokio::fs::write(&path, &image.bytes).await?;
    let message = NewMessage {
        kind: "image",
        text: fallback("image", caption.as_deref()),
        caption,
        media: Some(json!({
            "file": file,
            "width": image.width,
            "height": image.height,
            "mime": image.mime,
        })),
        mentions,
        everyone,
        client_key: key,
    };
    match crate::chat::messages::post_chat(&state, &me, &round, message).await {
        Ok(posted) => {
            if !posted.fresh {
                remove_files(vec![path]).await;
            }
            Ok(Json(posted.row))
        }
        Err(e) => {
            remove_files(vec![path]).await;
            Err(e)
        }
    }
}

fn now_unix() -> u64 {
    crate::accounts::crypto::now_unix()
}

pub fn expiry(now: u64) -> u64 {
    now.div_ceil(URL_PERIOD) * URL_PERIOD + URL_PERIOD
}

pub fn sign(state: &AppState, purpose: &str, payload: &str) -> String {
    let mac = state.auth.keys.mac(&[purpose, payload]);
    format!("{}.{}", URL_SAFE_NO_PAD.encode(payload), &mac[..MAC_LEN])
}

pub fn verify(state: &AppState, purpose: &str, token: &str) -> Option<String> {
    let (payload, mac) = token.split_once('.')?;
    let payload = String::from_utf8(URL_SAFE_NO_PAD.decode(payload).ok()?).ok()?;
    let expected = state.auth.keys.mac(&[purpose, &payload]);
    constant_eq(mac, &expected[..MAC_LEN]).then_some(payload)
}

const PURPOSE: &str = "chat media";

pub fn image_url(state: &AppState, message_id: MessageId, review: bool) -> String {
    let exp = expiry(now_unix());
    let payload = if review {
        format!("{message_id}.{exp}.r")
    } else {
        format!("{message_id}.{exp}")
    };
    format!("/api/media/{}", sign(state, PURPOSE, &payload))
}

pub fn payload(
    state: &AppState,
    message_id: MessageId,
    kind: &str,
    stored: Option<&str>,
) -> Option<Value> {
    let media: Value = serde_json::from_str(stored?).ok()?;
    match kind {
        "image" => Some(json!({
            "url": image_url(state, message_id, false),
            "width": media["width"],
            "height": media["height"],
            "mime": media["mime"],
        })),
        "gif" => crate::chat::gifs::payload(state, &media),
        _ => None,
    }
}

pub async fn serve(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> AppResult<Response> {
    let payload = verify(&state, PURPOSE, &token).ok_or(AppError::NotFound)?;
    let mut parts = payload.split('.');
    let (Some(id), Some(exp), review) = (
        parts
            .next()
            .and_then(|p| p.parse::<i64>().ok())
            .map(MessageId::from_raw),
        parts.next().and_then(|p| p.parse::<u64>().ok()),
        parts.next() == Some("r"),
    ) else {
        return Err(AppError::NotFound);
    };
    let now = now_unix();
    if exp <= now {
        return Err(AppError::NotFound);
    }
    let (room, media): (RoomId, Option<String>) = sqlx::query_as(
        "SELECT r.group_id, x.media FROM messages x JOIN rounds r ON r.id = x.round_id \
         JOIN groups g ON g.id = r.group_id \
         WHERE x.id = ?1 AND x.kind = 'image' AND (x.hidden_at IS NULL OR ?2) AND g.deleted_at IS NULL",
    )
    .bind(id)
    .bind(review)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let mime = media
        .as_deref()
        .and_then(|m| serde_json::from_str::<Value>(m).ok())
        .and_then(|m| m["mime"].as_str().map(str::to_owned))
        .filter(|m| MIMES.contains(&m.as_str()))
        .ok_or(AppError::NotFound)?;
    let path =
        file_path(&state.config.data_dir, room, media.as_deref()).ok_or(AppError::NotFound)?;
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(AppError::NotFound),
        Err(e) => return Err(e.into()),
    };
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("jpg");
    let mut res = Response::new(Body::from(bytes));
    let h = res.headers_mut();
    let value = |s: String| HeaderValue::from_str(&s).unwrap_or(HeaderValue::from_static(""));
    h.insert(header::CONTENT_TYPE, value(mime));
    h.insert(
        header::CACHE_CONTROL,
        value(format!("private, max-age={}", exp - now)),
    );
    h.insert(
        header::CONTENT_DISPOSITION,
        value(format!("inline; filename=\"quidenous-{id}.{ext}\"")),
    );
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; sandbox"),
    );
    Ok(res)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[test]
    fn expiry_is_on_the_grid_6_to_12_hours_ahead() {
        for now in [
            0,
            1,
            URL_PERIOD - 1,
            URL_PERIOD,
            URL_PERIOD + 1,
            1_763_300_000,
        ] {
            let exp = expiry(now);
            assert_eq!(exp % URL_PERIOD, 0);
            assert!(
                exp - now >= URL_PERIOD && exp - now <= 2 * URL_PERIOD,
                "{now}"
            );
        }
        assert_eq!(
            expiry(10),
            expiry(URL_PERIOD - 10),
            "stable within a period"
        );
    }

    #[test]
    fn sniffs_by_magic_bytes() {
        assert_eq!(sniff(b"\xFF\xD8\xFF\xE0rest"), Some(Format::Jpeg));
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\nrest"), Some(Format::Png));
        assert_eq!(sniff(b"GIF89a..."), Some(Format::Gif));
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some(Format::Webp));
        assert_eq!(sniff(b"<svg xmlns="), None);
        assert_eq!(sniff(b"hello"), None);
    }

    #[test]
    fn file_paths_are_server_names_only() {
        let dir = FsPath::new("/data");
        assert_eq!(
            file_path(dir, RoomId::from_raw(3), Some(r#"{"file":"ab12.jpg"}"#)),
            Some(PathBuf::from("/data/chat/3/ab12.jpg"))
        );
        for bad in [
            r#"{"file":"../x.jpg"}"#,
            r#"{"file":"a/b.jpg"}"#,
            r#"{"file":""}"#,
            "{}",
        ] {
            assert_eq!(
                file_path(dir, RoomId::from_raw(3), Some(bad)),
                None,
                "{bad}"
            );
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn decodes_run_a_couple_at_a_time() {
        static RUNNING: AtomicUsize = AtomicUsize::new(0);
        static MOST: AtomicUsize = AtomicUsize::new(0);
        let decodes = Decodes::default();
        let tasks: Vec<_> = (0..8)
            .map(|_| {
                let decodes = decodes.clone();
                tokio::spawn(async move {
                    decodes
                        .run(|| {
                            let now = RUNNING.fetch_add(1, Ordering::SeqCst) + 1;
                            MOST.fetch_max(now, Ordering::SeqCst);
                            std::thread::sleep(std::time::Duration::from_millis(30));
                            RUNNING.fetch_sub(1, Ordering::SeqCst);
                            Ok(())
                        })
                        .await
                })
            })
            .collect();
        for task in tasks {
            task.await.unwrap().unwrap();
        }
        assert_eq!(MOST.load(Ordering::SeqCst), DECODES_AT_ONCE);
    }
}
