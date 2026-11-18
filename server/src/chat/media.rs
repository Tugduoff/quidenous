//! Images in the result chat (docs/CHAT_MEDIA.md): uploaded as multipart, sniffed by their first
//! bytes, re-encoded (which drops EXIF, GPS and every other metadata), stored under
//! `DATA_DIR/chat/<room>/` and served only through short-lived signed URLs (`/api/media/<token>`)
//! the room's members get with the messages. Files go with their message, the author's account
//! or the room.

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

/// Largest image accepted (`413 too_large` above).
pub const IMAGE_MAX: usize = 8 * 1024 * 1024;
/// The whole upload: the image plus a caption and mentions.
pub const UPLOAD_BODY_MAX: usize = IMAGE_MAX + 64 * 1024;
/// Longest side of a stored photo (larger ones are scaled down).
const MAX_SIDE: u32 = 1600;
const JPEG_QUALITY: u8 = 85;
/// Pixels on either side, and memory, any image decode may take (decompression bombs). The app
/// scales photos down before sending them (1600 px in the chat, less for a profile photo).
const DECODE_MAX_SIDE: u32 = 4096;
const DECODE_MAX_ALLOC: u64 = 64 * 1024 * 1024;
/// Image decodes (chat and profile photos) running at once, server-wide: each may take
/// [`DECODE_MAX_ALLOC`] and its resize, so a burst of uploads can't run the host out of memory.
const DECODES_AT_ONCE: usize = 2;
/// Signed URLs expire on a 6-hour grid, 6 to 12 hours ahead: the same URL all along a stretch
/// of time, so the app's image cache keeps working.
const URL_PERIOD: u64 = 6 * 3600;
/// Hex characters of HMAC kept in a token (128 bits).
const MAC_LEN: usize = 32;
/// Types an image may be stored and served as.
const MIMES: [&str; 3] = ["image/jpeg", "image/png", "image/gif"];

/// What older apps, the inbox, pushes and reports show for an image or a GIF (the same in every
/// language: it is stored once, in `messages.text`).
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

/// The format by magic bytes (not the file name or the declared type): JPEG, PNG, WebP, GIF only.
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

/// A stored image, ready to write.
struct Processed {
    bytes: Vec<u8>,
    ext: &'static str,
    mime: &'static str,
    width: u32,
    height: u32,
}

/// Checks and re-encodes an upload (CPU-heavy: call it off the async threads). JPEG and WebP
/// become a JPEG (a PNG for a WebP with transparency), PNG stays PNG, both turned upright (EXIF
/// orientation) and at most [`MAX_SIDE`] wide and high; nothing of the original file but its
/// pixels survives. A GIF is kept as it is (animated; GIF has no EXIF) once its first frame decodes.
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

/// The limits every decode of an uploaded image runs with.
pub(crate) fn decode_limits() -> image::Limits {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(DECODE_MAX_SIDE);
    limits.max_image_height = Some(DECODE_MAX_SIDE);
    limits.max_alloc = Some(DECODE_MAX_ALLOC);
    limits
}

/// Runs an image decode (and its resize, encode) on a blocking thread, [`DECODES_AT_ONCE`] at most
/// server-wide; the others wait their turn. The slot stays taken until the work is done, even if
/// the request is dropped meanwhile.
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

/// A body over the route's limit is `413 too_large`; any other broken upload `400 bad_upload`.
fn upload_error(e: MultipartError) -> AppError {
    if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
        AppError::TooLarge("too_large")
    } else {
        AppError::BadRequest("bad_upload")
    }
}

/// A room's image directory.
fn room_dir(data_dir: &FsPath, room: i64) -> PathBuf {
    data_dir.join("chat").join(room.to_string())
}

/// The file behind an image message's stored media (server-made names only; never a path).
pub fn file_path(data_dir: &FsPath, room: i64, media: Option<&str>) -> Option<PathBuf> {
    let media: Value = serde_json::from_str(media?).ok()?;
    let file = media["file"].as_str()?;
    let safe = !file.is_empty()
        && !file.starts_with('.')
        && file.chars().all(|c| c.is_ascii_alphanumeric() || c == '.');
    safe.then(|| room_dir(data_dir, room).join(file))
}

/// Removes image files (best effort: a file already gone is fine).
pub async fn remove_files(paths: Vec<PathBuf>) {
    for path in paths {
        if let Err(e) = tokio::fs::remove_file(&path).await
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(path = %path.display(), error = %e, "chat image not removed");
        }
    }
}

/// A deleted room's images, all at once (nobody can open its chat any more).
pub async fn remove_room(state: &AppState, room: i64) {
    let dir = room_dir(&state.config.data_dir, room);
    if let Err(e) = tokio::fs::remove_dir_all(&dir).await
        && e.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(room, error = %e, "chat images of a deleted room not removed");
    }
}

/// `POST /rounds/{id}/messages/image` (multipart): `image` (the file), optional `caption`,
/// `mentions` (a JSON array of member ids), `everyone` (`1`) and `client_key`. Same rules as a text
/// message (finished question, chat limits, retries, inbox, pushes, live), plus the image limits.
/// Answers the message.
pub(crate) async fn upload(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
    mut multipart: Multipart,
) -> AppResult<Json<MessageRow>> {
    let round = crate::chat::messages::chat_round(&state, &me, id).await?;
    // Charged before the body is read (a retry gives it back below): a flood of uploads costs
    // its sender, not the server's memory.
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
    while let Some(field) = multipart.next_field().await.map_err(upload_error)? {
        match field.name() {
            Some("image") => image = Some(field.bytes().await.map_err(upload_error)?),
            Some("caption") => caption = Some(field.text().await.map_err(upload_error)?),
            Some("mentions") => {
                let text = field.text().await.map_err(upload_error)?;
                mentions = serde_json::from_str(&text).unwrap_or_default();
            }
            Some("everyone") => {
                let text = field.text().await.map_err(upload_error)?;
                everyone = matches!(text.trim(), "1" | "true");
            }
            Some("client_key") => key = Some(field.text().await.map_err(upload_error)?),
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
    // A retry of an upload whose answer was lost: the image already posted.
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
    // A 128-bit random name in the room's directory.
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
            // The same upload landed twice at once: the first one keeps its file.
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

/// When a URL signed at `now` expires: the next 6-hour boundary, plus 6 hours.
pub fn expiry(now: u64) -> u64 {
    now.div_ceil(URL_PERIOD) * URL_PERIOD + URL_PERIOD
}

/// `base64url(payload).<HMAC>`, keyed by the server's secret (`AUTH_SECRET` / `auth.key`).
pub fn sign(state: &AppState, purpose: &str, payload: &str) -> String {
    let mac = state.auth.keys.mac(&[purpose, payload]);
    format!("{}.{}", URL_SAFE_NO_PAD.encode(payload), &mac[..MAC_LEN])
}

/// The payload of a token [`sign`] made for this purpose, or `None`.
pub fn verify(state: &AppState, purpose: &str, token: &str) -> Option<String> {
    let (payload, mac) = token.split_once('.')?;
    let payload = String::from_utf8(URL_SAFE_NO_PAD.decode(payload).ok()?).ok()?;
    let expected = state.auth.keys.mac(&[purpose, &payload]);
    constant_eq(mac, &expected[..MAC_LEN]).then_some(payload)
}

const PURPOSE: &str = "chat media";

/// The URL of an image message for the app (`review`: an admin reviewing a report, which also
/// shows it while hidden).
pub fn image_url(state: &AppState, message_id: i64, review: bool) -> String {
    let exp = expiry(now_unix());
    let payload = if review {
        format!("{message_id}.{exp}.r")
    } else {
        format!("{message_id}.{exp}")
    };
    format!("/api/media/{}", sign(state, PURPOSE, &payload))
}

/// A message's `media` as the app gets it: fresh URLs, the size, the type (`None` for text).
pub fn payload(
    state: &AppState,
    message_id: i64,
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

/// `GET /media/{token}` (no sign-in: image tags can't send one; the token is the key). A valid,
/// unexpired token for an image message that is still there, not hidden by reports (unless the
/// token is an admin's), in a room that wasn't deleted. `404` otherwise.
pub async fn serve(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> AppResult<Response> {
    let payload = verify(&state, PURPOSE, &token).ok_or(AppError::NotFound)?;
    let mut parts = payload.split('.');
    let (Some(id), Some(exp), review) = (
        parts.next().and_then(|p| p.parse::<i64>().ok()),
        parts.next().and_then(|p| p.parse::<u64>().ok()),
        parts.next() == Some("r"),
    ) else {
        return Err(AppError::NotFound);
    };
    let now = now_unix();
    if exp <= now {
        return Err(AppError::NotFound);
    }
    let (room, media): (i64, Option<String>) = sqlx::query_as(
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
            file_path(dir, 3, Some(r#"{"file":"ab12.jpg"}"#)),
            Some(PathBuf::from("/data/chat/3/ab12.jpg"))
        );
        for bad in [
            r#"{"file":"../x.jpg"}"#,
            r#"{"file":"a/b.jpg"}"#,
            r#"{"file":""}"#,
            "{}",
        ] {
            assert_eq!(file_path(dir, 3, Some(bad)), None, "{bad}");
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
