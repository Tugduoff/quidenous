//! Images and GIFs in the result chat (docs/CHAT_MEDIA.md): uploads (types, size, metadata
//! stripped), signed media URLs, the rules of the chat, deletions, the fallback text for older
//! apps, quotas, and GIFs through a mock Klipy.

mod common;

use std::sync::{Arc, Mutex};

use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Request, StatusCode, header},
};
use common::{call, join, room_id};
use http_body_util::BodyExt;
use quidenous_server::{AppState, app, config::Klipy, state};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn setup_with(klipy: Option<Klipy>) -> (Router, AppState) {
    let mut config = common::config(&common::temp_dir("qdn-media"));
    config.klipy = klipy;
    let state = state(config).await.unwrap();
    sqlx::query("DELETE FROM questions WHERE group_id IS NULL AND kind <> 'pick'")
        .execute(&state.db)
        .await
        .unwrap();
    (app(state.clone()), state)
}

/// Ana, Bob and Carl in the env room, the first question closed. (tokens, member ids, round, room)
async fn chat_in(app: &Router) -> ([String; 3], [i64; 3], i64, i64) {
    let (ana, ana_id) = join(app, "Ana").await;
    let (bob, bob_id) = join(app, "Bob").await;
    let (carl, carl_id) = join(app, "Carl").await;
    let room = room_id(app, &ana).await;
    let (_, open) = call(app, "GET", "/round/current", Some(&ana), Some(room), None).await;
    let round = open["id"].as_i64().unwrap();
    for t in [&ana, &bob, &carl] {
        let (s, b) = call(
            app,
            "POST",
            "/round/current/vote",
            Some(t),
            Some(room),
            Some(json!({ "target_id": bob_id })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    ([ana, bob, carl], [ana_id, bob_id, carl_id], round, room)
}

/// Any request, raw bytes back.
async fn raw(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    room: Option<i64>,
    content_type: Option<&str>,
    body: Vec<u8>,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut req = Request::builder().method(method).uri(uri);
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    if let Some(r) = room {
        req = req.header("x-room", r.to_string());
    }
    if let Some(c) = content_type {
        req = req.header("content-type", c);
    }
    let res = app
        .clone()
        .oneshot(req.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = res.into_body().collect().await.unwrap().to_bytes().to_vec();
    (status, headers, bytes)
}

/// A multipart body: (field name, file name if a file, content).
fn multipart(fields: &[(&str, Option<&str>, &[u8])]) -> Vec<u8> {
    let mut body = Vec::new();
    for (name, file, content) in fields {
        body.extend_from_slice(b"--XB\r\n");
        match file {
            Some(f) => body.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"{name}\"; filename=\"{f}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
                )
                .as_bytes(),
            ),
            None => body.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
            ),
        }
        body.extend_from_slice(content);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(b"--XB--\r\n");
    body
}

async fn post_image_with(
    app: &Router,
    token: &str,
    room: i64,
    round: i64,
    fields: &[(&str, Option<&str>, &[u8])],
) -> (StatusCode, Value) {
    let (s, _, b) = raw(
        app,
        "POST",
        &format!("/api/rounds/{round}/messages/image"),
        Some(token),
        Some(room),
        Some("multipart/form-data; boundary=XB"),
        multipart(fields),
    )
    .await;
    (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

async fn post_image(
    app: &Router,
    token: &str,
    room: i64,
    round: i64,
    image: &[u8],
) -> (StatusCode, Value) {
    post_image_with(app, token, room, round, &[("image", Some("p"), image)]).await
}

fn encode(image: image::DynamicImage, format: image::ImageFormat) -> Vec<u8> {
    let mut out = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut out), format)
        .unwrap();
    out
}

fn png(w: u32, h: u32) -> Vec<u8> {
    encode(
        image::RgbImage::from_pixel(w, h, image::Rgb([200, 80, 40])).into(),
        image::ImageFormat::Png,
    )
}

/// An APP1 Exif segment: orientation 6 (turned 90°) and a GPS IFD, plus a marker to look for.
fn exif_segment() -> Vec<u8> {
    let mut tiff = b"II*\0".to_vec();
    tiff.extend(8u32.to_le_bytes());
    // IFD0 at 8: two entries (orientation, GPS IFD pointer), then no next IFD.
    tiff.extend(2u16.to_le_bytes());
    tiff.extend(0x0112u16.to_le_bytes()); // Orientation, SHORT, 1, = 6
    tiff.extend(3u16.to_le_bytes());
    tiff.extend(1u32.to_le_bytes());
    tiff.extend([6, 0, 0, 0]);
    tiff.extend(0x8825u16.to_le_bytes()); // GPSInfo, LONG, 1, -> 38
    tiff.extend(4u16.to_le_bytes());
    tiff.extend(1u32.to_le_bytes());
    tiff.extend(38u32.to_le_bytes());
    tiff.extend(0u32.to_le_bytes());
    // GPS IFD at 38: GPSLatitudeRef "N".
    tiff.extend(1u16.to_le_bytes());
    tiff.extend(1u16.to_le_bytes());
    tiff.extend(2u16.to_le_bytes());
    tiff.extend(2u32.to_le_bytes());
    tiff.extend(b"N\0\0\0");
    tiff.extend(0u32.to_le_bytes());
    tiff.extend(b"GPSSECRET 48.8566 2.3522");
    let mut exif = b"Exif\0\0".to_vec();
    exif.extend(tiff);
    let mut segment = vec![0xFF, 0xE1];
    segment.extend(((exif.len() + 2) as u16).to_be_bytes());
    segment.extend(exif);
    segment
}

/// A 2000×1000 JPEG with that Exif segment right after its start marker.
fn jpeg_with_gps() -> Vec<u8> {
    let image = image::RgbImage::from_fn(2000, 1000, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 120])
    });
    let jpeg = encode(image.into(), image::ImageFormat::Jpeg);
    let mut out = jpeg[..2].to_vec();
    out.extend(exif_segment());
    out.extend(&jpeg[2..]);
    out
}

async fn messages(app: &Router, token: &str, room: i64, round: i64) -> Vec<Value> {
    let (s, list) = call(
        app,
        "GET",
        &format!("/rounds/{round}/messages"),
        Some(token),
        Some(room),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{list}");
    list.as_array().unwrap().clone()
}

fn find(list: &[Value], id: i64) -> Option<&Value> {
    list.iter().find(|m| m["id"] == id)
}

fn chat_files(state: &AppState, room: i64) -> Vec<std::path::PathBuf> {
    match std::fs::read_dir(state.config.data_dir.join("chat").join(room.to_string())) {
        Ok(dir) => dir.map(|e| e.unwrap().path()).collect(),
        Err(_) => Vec::new(),
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[tokio::test]
async fn images_are_stripped_resized_and_served_through_signed_urls() {
    let (app, state) = setup_with(None).await;
    let ([ana, bob, _], [_, bob_id, _], round, room) = chat_in(&app).await;
    let (_, features) = call(&app, "GET", "/features", None, None, None).await;
    let features = features.as_array().unwrap();
    assert!(features.contains(&json!("chat_images")));
    assert!(!features.contains(&json!("chat_gifs")));

    let mentions = format!("[{bob_id}]");
    let (s, m) = post_image_with(
        &app,
        &ana,
        room,
        round,
        &[
            ("image", Some("IMG_1.jpg"), &jpeg_with_gps()),
            ("caption", None, b"  Regarde @Bob  "),
            ("mentions", None, mentions.as_bytes()),
        ],
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{m}");
    assert_eq!(m["kind"], "image");
    assert_eq!(m["caption"], "Regarde @Bob");
    // Older apps show the fallback.
    assert_eq!(m["text"], "📷 Photo · Regarde @Bob");
    assert_eq!(m["mentions"], json!([bob_id]));
    // Turned upright (orientation 6), then at most 1600 px.
    assert_eq!(m["media"]["width"], 800);
    assert_eq!(m["media"]["height"], 1600);
    assert_eq!(m["media"]["mime"], "image/jpeg");
    let url = m["media"]["url"].as_str().unwrap().to_owned();
    assert!(url.starts_with("/api/media/"), "{url}");
    assert_eq!(m["reactions"], json!([]));

    // Served without a sign-in, metadata gone.
    let (s, h, bytes) = raw(&app, "GET", &url, None, None, None, Vec::new()).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(h[header::CONTENT_TYPE], "image/jpeg");
    assert_eq!(h[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    let cache = h[header::CACHE_CONTROL].to_str().unwrap().to_owned();
    let age: u64 = cache
        .strip_prefix("private, max-age=")
        .unwrap()
        .parse()
        .unwrap();
    assert!((6 * 3600..=12 * 3600).contains(&age), "{cache}");
    assert!(bytes.starts_with(&[0xFF, 0xD8]));
    assert!(!contains(&bytes, b"Exif"), "no Exif segment left");
    assert!(!contains(&bytes, b"GPSSECRET"));
    let decoded = image::load_from_memory(&bytes).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (800, 1600));
    // On disk, in the room's directory, as served.
    let files = chat_files(&state, room);
    assert_eq!(files.len(), 1);
    assert_eq!(std::fs::read(&files[0]).unwrap(), bytes);

    // Bob sees the same, with the same (stable) URL; his inbox has the fallback as a mention.
    let list = messages(&app, &bob, room, round).await;
    let seen = find(&list, m["id"].as_i64().unwrap()).unwrap();
    assert_eq!(seen["kind"], "image");
    assert_eq!(seen["media"]["url"], url.as_str());
    assert_eq!(seen["text"], "📷 Photo · Regarde @Bob");
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&bob), Some(room), None).await;
    assert!(
        inbox["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["kind"] == "mention" && i["text"] == "📷 Photo · Regarde @Bob"),
        "{inbox}"
    );
    // Text messages: kind text, no media.
    let (_, t) = call(
        &app,
        "POST",
        &format!("/rounds/{round}/messages"),
        Some(&bob),
        Some(room),
        Some(json!({ "text": "joli" })),
    )
    .await;
    assert_eq!(t["kind"], "text");
    assert_eq!(t["media"], Value::Null);
    assert_eq!(t["caption"], Value::Null);
}

#[tokio::test]
async fn image_types_and_sizes() {
    let (app, state) = setup_with(None).await;
    let ([ana, ..], _, round, room) = chat_in(&app).await;

    // PNG stays PNG; no caption = the bare fallback.
    let (s, m) = post_image(&app, &ana, room, round, &png(40, 30)).await;
    assert_eq!(s, StatusCode::OK, "{m}");
    assert_eq!(m["media"]["mime"], "image/png");
    assert_eq!(
        (m["media"]["width"].as_u64(), m["media"]["height"].as_u64()),
        (Some(40), Some(30))
    );
    assert_eq!(m["text"], "📷 Photo");
    assert_eq!(m["caption"], Value::Null);
    let (s, h, _) = raw(
        &app,
        "GET",
        m["media"]["url"].as_str().unwrap(),
        None,
        None,
        None,
        Vec::new(),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(h[header::CONTENT_TYPE], "image/png");

    // WebP (no transparency) becomes a JPEG.
    let webp = encode(
        image::RgbImage::from_pixel(20, 10, image::Rgb([1, 2, 3])).into(),
        image::ImageFormat::WebP,
    );
    let (s, m) = post_image(&app, &ana, room, round, &webp).await;
    assert_eq!(s, StatusCode::OK, "{m}");
    assert_eq!(m["media"]["mime"], "image/jpeg");

    // A GIF is kept byte for byte.
    let gif = encode(
        image::RgbaImage::from_pixel(12, 9, image::Rgba([9, 9, 9, 255])).into(),
        image::ImageFormat::Gif,
    );
    let (s, m) = post_image(&app, &ana, room, round, &gif).await;
    assert_eq!(s, StatusCode::OK, "{m}");
    assert_eq!(m["media"]["mime"], "image/gif");
    assert_eq!(m["media"]["width"], 12);
    let (s, h, bytes) = raw(
        &app,
        "GET",
        m["media"]["url"].as_str().unwrap(),
        None,
        None,
        None,
        Vec::new(),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(h[header::CONTENT_TYPE], "image/gif");
    assert_eq!(bytes, gif);

    // Not an image (by its bytes, whatever its name), a broken one, or none at all.
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#;
    for bad in [
        &b"hello, plain text"[..],
        &svg[..],
        &b"\xFF\xD8\xFF\xE0broken"[..],
    ] {
        let (s, b) = post_image(&app, &ana, room, round, bad).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
        assert_eq!(b["error"], "bad_image");
    }
    let (s, b) = post_image_with(&app, &ana, room, round, &[("caption", None, b"hi")]).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("missing_image"))
    );
    let long = "a".repeat(501);
    let (s, b) = post_image_with(
        &app,
        &ana,
        room,
        round,
        &[
            ("image", Some("p"), &png(4, 4)),
            ("caption", None, long.as_bytes()),
        ],
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_caption"))
    );

    // Over 8 MB: 413, whether the body fits the route's limit or not.
    let mut big = b"\xFF\xD8\xFF\xE0".to_vec();
    big.resize(8 * 1024 * 1024 + 1, 0);
    let (s, b) = post_image(&app, &ana, room, round, &big).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::PAYLOAD_TOO_LARGE, Some("too_large"))
    );
    big.resize(9 * 1024 * 1024, 0);
    let (s, b) = post_image(&app, &ana, room, round, &big).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::PAYLOAD_TOO_LARGE, Some("too_large"))
    );
    // Only the three accepted images are on disk.
    assert_eq!(chat_files(&state, room).len(), 3);
}

#[tokio::test]
async fn chat_rules_and_tokens() {
    let (app, state) = setup_with(None).await;
    let ([ana, bob, _], _, round, room) = chat_in(&app).await;
    let (_, m) = post_image(&app, &ana, room, round, &png(8, 8)).await;
    let id = m["id"].as_i64().unwrap();
    let url = m["media"]["url"].as_str().unwrap().to_owned();

    // The next question is open: no chat there yet.
    let (_, open) = call(&app, "GET", "/round/current", Some(&ana), Some(room), None).await;
    if let Some(next) = open["id"].as_i64().filter(|n| *n != round) {
        let (s, b) = post_image(&app, &ana, room, next, &png(8, 8)).await;
        assert_eq!(
            (s, b["error"].as_str()),
            (StatusCode::BAD_REQUEST, Some("round_open"))
        );
    }
    // Someone from another room can't post under this question.
    let (dan, _) = join(&app, "Dan").await;
    let (_, other) = call(
        &app,
        "POST",
        "/rooms",
        Some(&dan),
        None,
        Some(json!({ "name": "Ailleurs" })),
    )
    .await;
    let other = other["id"].as_i64().unwrap();
    let (s, _) = post_image(&app, &dan, other, round, &png(8, 8)).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = call(
        &app,
        "GET",
        &format!("/rounds/{round}/messages"),
        Some(&dan),
        Some(other),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // Tokens: tampered, made up, expired, for a text message.
    let token = url.strip_prefix("/api/media/").unwrap();
    let (payload, mac) = token.split_once('.').unwrap();
    let flipped = if mac.starts_with('0') { "1" } else { "0" };
    let forged = format!("/api/media/{payload}.{flipped}{}", &mac[1..]);
    let expired = format!(
        "/api/media/{}",
        quidenous_server::media::sign(&state, "chat media", &format!("{id}.1000"))
    );
    let (_, t) = call(
        &app,
        "POST",
        &format!("/rounds/{round}/messages"),
        Some(&bob),
        Some(room),
        Some(json!({ "text": "texte" })),
    )
    .await;
    let text_url = quidenous_server::media::image_url(&state, t["id"].as_i64().unwrap(), false);
    let other_id = format!(
        "/api/media/{}",
        quidenous_server::media::sign(&state, "klipy media", &format!("{id}.99999999999"))
    );
    for bad in [
        forged,
        expired,
        text_url,
        other_id,
        "/api/media/nope".into(),
        "/api/media/a.b".into(),
    ] {
        let (s, _, _) = raw(&app, "GET", &bad, None, None, None, Vec::new()).await;
        assert_eq!(s, StatusCode::NOT_FOUND, "{bad}");
    }
    let (s, _, _) = raw(&app, "GET", &url, None, None, None, Vec::new()).await;
    assert_eq!(s, StatusCode::OK);

    // Reactions work on images.
    let (s, r) = call(
        &app,
        "POST",
        &format!("/messages/{id}/reactions"),
        Some(&bob),
        Some(room),
        Some(json!({ "emoji": "🔥" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{r}");
    let list = messages(&app, &ana, room, round).await;
    assert_eq!(find(&list, id).unwrap()["reactions"][0]["emoji"], "🔥");
}

#[tokio::test]
async fn blocks_reports_and_deletions() {
    let (app, state) = setup_with(None).await;
    let ([ana, bob, carl], [ana_id, ..], round, room) = chat_in(&app).await;
    let (dan, _) = join(&app, "Dan").await;
    let ana_user: i64 = sqlx::query_scalar("SELECT user_id FROM members WHERE id = ?")
        .bind(ana_id)
        .fetch_one(&state.db)
        .await
        .unwrap();
    let (_, kept) = post_image(&app, &ana, room, round, &png(8, 8)).await;
    let (_, reported) = post_image(&app, &ana, room, round, &png(9, 9)).await;
    let (_, deleted) = post_image(&app, &ana, room, round, &png(10, 10)).await;
    assert_eq!(chat_files(&state, room).len(), 3);

    // Bob blocks Ana: her images leave his chat.
    let (s, _) = call(
        &app,
        "PUT",
        &format!("/blocks/{ana_user}"),
        Some(&bob),
        Some(room),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let list = messages(&app, &bob, room, round).await;
    assert!(find(&list, kept["id"].as_i64().unwrap()).is_none());
    assert!(
        find(
            &messages(&app, &carl, room, round).await,
            kept["id"].as_i64().unwrap()
        )
        .is_some()
    );

    // Three reports hide one: no longer served.
    let rid = reported["id"].as_i64().unwrap();
    for t in [&bob, &carl, &dan] {
        let (s, b) = call(
            &app,
            "POST",
            "/reports",
            Some(t),
            Some(room),
            Some(json!({ "kind": "message", "id": rid, "reason": "offensive" })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    let (s, _, _) = raw(
        &app,
        "GET",
        reported["media"]["url"].as_str().unwrap(),
        None,
        None,
        None,
        Vec::new(),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    // An admin reviewing the report still sees it.
    let review = quidenous_server::media::image_url(&state, rid, true);
    let (s, _, _) = raw(&app, "GET", &review, None, None, None, Vec::new()).await;
    assert_eq!(s, StatusCode::OK);

    // Deleting a message deletes its file, its inbox entries and its URL.
    let did = deleted["id"].as_i64().unwrap();
    let (s, b) = call(
        &app,
        "DELETE",
        &format!("/rounds/{round}/messages/{did}"),
        Some(&ana),
        Some(room),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(chat_files(&state, room).len(), 2);
    let (s, _, _) = raw(
        &app,
        "GET",
        deleted["media"]["url"].as_str().unwrap(),
        None,
        None,
        None,
        Vec::new(),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&carl), Some(room), None).await;
    assert_eq!(
        inbox["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["text"] == "📷 Photo")
            .count(),
        2,
        "{inbox}"
    );

    // Ana deletes her account: her images go.
    let (s, b) = call(&app, "DELETE", "/auth/account", Some(&ana), None, None).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert!(chat_files(&state, room).is_empty());
    let (s, _, _) = raw(
        &app,
        "GET",
        kept["media"]["url"].as_str().unwrap(),
        None,
        None,
        None,
        Vec::new(),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_deleted_room_takes_its_images() {
    let (app, state) = setup_with(None).await;
    let ([ana, bob, carl], _, round, room) = chat_in(&app).await;
    let (_, m) = post_image(&app, &bob, room, round, &png(8, 8)).await;
    assert_eq!(chat_files(&state, room).len(), 1);
    // Whoever owns the env room deletes it.
    let mut deleted = false;
    for t in [&ana, &bob, &carl] {
        let (s, _) = call(
            &app,
            "DELETE",
            &format!("/rooms/{room}"),
            Some(t),
            Some(room),
            None,
        )
        .await;
        deleted |= s == StatusCode::OK;
    }
    assert!(deleted, "the owner deletes the room");
    assert!(
        !state
            .config
            .data_dir
            .join("chat")
            .join(room.to_string())
            .exists()
    );
    let (s, _, _) = raw(
        &app,
        "GET",
        m["media"]["url"].as_str().unwrap(),
        None,
        None,
        None,
        Vec::new(),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn image_quota() {
    let (app, state) = setup_with(None).await;
    let ([ana, ..], [ana_id, ..], round, room) = chat_in(&app).await;
    let user: i64 = sqlx::query_scalar("SELECT user_id FROM members WHERE id = ?")
        .bind(ana_id)
        .fetch_one(&state.db)
        .await
        .unwrap();
    // The chat limit (15 a minute) comes first in a burst: use up the hourly image one directly.
    for _ in 0..quidenous_server::quotas::CHAT_IMAGE_USER.capacity {
        quidenous_server::quotas::hit_user(
            &state,
            &quidenous_server::quotas::CHAT_IMAGE_USER,
            user,
        )
        .unwrap();
    }
    let (s, b) = post_image(&app, &ana, room, round, &png(8, 8)).await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS, "{b}");
    assert!(chat_files(&state, room).is_empty());
    // Text still goes through.
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rounds/{round}/messages"),
        Some(&ana),
        Some(room),
        Some(json!({ "text": "toujours là" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
}

// ---------- GIFs ----------

#[tokio::test]
async fn gifs_are_off_without_a_key() {
    let (app, _) = setup_with(None).await;
    let ([ana, ..], _, round, room) = chat_in(&app).await;
    let (s, b) = call(&app, "GET", "/gifs?q=chat", Some(&ana), Some(room), None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::NOT_FOUND, Some("gifs_off"))
    );
    let (s, b) = call(
        &app,
        "POST",
        &format!("/rounds/{round}/messages/gif"),
        Some(&ana),
        Some(room),
        Some(json!({ "id": "1", "url": "/api/gifs/media/x.y", "width": 10, "height": 10 })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::NOT_FOUND, Some("gifs_off"))
    );
}

/// What the mock Klipy was asked: (path, query).
type Calls = Arc<Mutex<Vec<(String, String)>>>;

const WEBP: &[u8] = b"RIFF\x10\0\0\0WEBPmock animated webp";
const GIF: &[u8] = b"GIF89amock gif";

/// A Klipy look-alike on a local port: the search and trending endpoints (one fixture) and files.
async fn mock_klipy() -> (String, Calls) {
    use axum::{extract::OriginalUri, routing::get};
    let calls: Calls = Arc::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let fixture = json!({
        "result": true,
        "data": {
            "data": [
                { "id": 101, "title": "Chat", "type": "gif", "file": {
                    "md": { "webp": { "url": format!("{base}/files/md.webp"), "width": 220, "height": 180 },
                            "gif": { "url": format!("{base}/files/md.gif"), "width": 220, "height": 180 } },
                    "sm": { "webp": { "url": format!("{base}/files/sm.webp"), "width": 110, "height": 90 } } } },
                { "id": "ad-1", "type": "ad", "file": {
                    "md": { "webp": { "url": format!("{base}/files/md.webp"), "width": 1, "height": 1 } } } },
                { "id": 102, "title": "Ailleurs", "file": {
                    "md": { "gif": { "url": "https://evil.test/a.gif", "width": 1, "height": 1 } },
                    "sm": { "gif": { "url": "https://evil.test/a.gif", "width": 1, "height": 1 } } } },
                { "id": "103", "title": "Que du GIF", "file": {
                    "hd": { "gif": { "url": format!("{base}/files/hd.gif"), "width": 300, "height": 200 } },
                    "xs": { "gif": { "url": format!("{base}/files/xs.gif"), "width": 60, "height": 40 } } } }
            ],
            "current_page": 1, "per_page": 24, "has_next": true
        }
    });
    let api = {
        let calls = calls.clone();
        move |OriginalUri(uri): OriginalUri| {
            let calls = calls.clone();
            let fixture = fixture.clone();
            async move {
                calls
                    .lock()
                    .unwrap()
                    .push((uri.path().to_owned(), uri.query().unwrap_or("").to_owned()));
                axum::Json(fixture)
            }
        }
    };
    let file = |mime: &'static str, bytes: &'static [u8]| {
        get(move || async move { ([(header::CONTENT_TYPE, mime)], bytes) })
    };
    let router = Router::new()
        .route("/api/v1/{key}/gifs/search", get(api.clone()))
        .route("/api/v1/{key}/gifs/trending", get(api))
        .route("/files/md.webp", file("image/webp", WEBP))
        .route("/files/sm.webp", file("image/webp", WEBP))
        .route("/files/hd.gif", file("image/gif", GIF))
        .route("/files/xs.gif", file("image/gif", GIF))
        .route("/files/page.webp", file("text/html", b"<html>"));
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (base, calls)
}

#[tokio::test]
async fn gifs_through_klipy() {
    let (base, calls) = mock_klipy().await;
    let (app, state) = setup_with(Some(Klipy {
        api_key: "SECRET-KEY".into(),
        content_filter: "low".into(),
        api_base: base.clone(),
        media_hosts: vec![base.clone()],
    }))
    .await;
    let ([ana, bob, _], [ana_id, bob_id, _], round, room) = chat_in(&app).await;
    let (_, features) = call(&app, "GET", "/features", None, None, None).await;
    assert!(features.as_array().unwrap().contains(&json!("chat_gifs")));

    // Search: proxied URLs only, ads and foreign hosts left out.
    let (s, found) = call(
        &app,
        "GET",
        "/gifs?q=%20chat%20&page=2",
        Some(&ana),
        Some(room),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{found}");
    let text = found.to_string();
    assert!(
        !text.contains("127.0.0.1") && !text.contains("klipy.com") && !text.contains("evil"),
        "{text}"
    );
    assert_eq!(found["next"], true);
    let items = found["items"].as_array().unwrap();
    assert_eq!(items.len(), 2, "{found}");
    assert_eq!(items[0]["id"], "101");
    assert_eq!(items[0]["title"], "Chat");
    assert_eq!(
        (items[0]["width"].as_u64(), items[0]["height"].as_u64()),
        (Some(220), Some(180))
    );
    assert_eq!(items[1]["id"], "103");
    for item in items {
        for key in ["url", "preview_url"] {
            assert!(
                item[key].as_str().unwrap().starts_with("/api/gifs/media/"),
                "{item}"
            );
        }
    }
    {
        let calls = calls.lock().unwrap();
        let (path, query) = calls.last().unwrap();
        assert_eq!(path, "/api/v1/SECRET-KEY/gifs/search");
        for part in [
            "q=chat",
            "page=2",
            "per_page=24",
            "content_filter=low",
            "locale=FR",
            "customer_id=",
        ] {
            assert!(
                query.split('&').any(|p| p.starts_with(part)),
                "{part} in {query}"
            );
        }
        let customer = query
            .split('&')
            .find_map(|p| p.strip_prefix("customer_id="))
            .unwrap();
        // Opaque: an HMAC, not the account or seat id.
        assert_eq!(customer.len(), 32);
        assert!(
            customer.chars().all(|c| c.is_ascii_hexdigit()),
            "{customer}"
        );
        assert_ne!(customer, ana_id.to_string());
    }
    // Empty search: trending.
    let (s, _) = call(&app, "GET", "/gifs", Some(&ana), Some(room), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        calls.lock().unwrap().last().unwrap().0,
        "/api/v1/SECRET-KEY/gifs/trending"
    );

    // The proxy streams the file, publicly cacheable.
    let (s, h, bytes) = raw(
        &app,
        "GET",
        items[0]["url"].as_str().unwrap(),
        None,
        None,
        None,
        Vec::new(),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(h[header::CONTENT_TYPE], "image/webp");
    assert_eq!(h[header::CACHE_CONTROL], "public, max-age=86400");
    assert_eq!(bytes, WEBP);
    let (s, h, bytes) = raw(
        &app,
        "GET",
        items[1]["preview_url"].as_str().unwrap(),
        None,
        None,
        None,
        Vec::new(),
    )
    .await;
    assert_eq!(
        (s, h[header::CONTENT_TYPE].to_str().unwrap()),
        (StatusCode::OK, "image/gif")
    );
    assert_eq!(bytes, GIF);
    // ...but only our tokens, for allowed hosts, and images.
    let sign = |url: &str| {
        format!(
            "/api/gifs/media/{}",
            quidenous_server::media::sign(&state, "klipy media", url)
        )
    };
    for (bad, why) in [
        (sign("https://evil.test/a.gif"), "foreign host"),
        (sign("http://static.klipy.com/a.gif"), "not https"),
        (sign(&format!("{base}/files/page.webp")), "not an image"),
        (sign(&format!("{base}/files/missing.webp")), "missing"),
        (
            format!(
                "/api/gifs/media/{}",
                quidenous_server::media::sign(
                    &state,
                    "chat media",
                    &format!("{base}/files/md.webp")
                )
            ),
            "other purpose",
        ),
        ("/api/gifs/media/forged.0000".into(), "forged"),
    ] {
        let (s, _, _) = raw(&app, "GET", &bad, None, None, None, Vec::new()).await;
        assert_eq!(s, StatusCode::NOT_FOUND, "{why}");
    }

    // Posting one: the original URLs are stored, fresh proxy URLs come back.
    let (s, m) = call(
        &app,
        "POST",
        &format!("/rounds/{round}/messages/gif"),
        Some(&ana),
        Some(room),
        Some(json!({
            "id": items[0]["id"], "url": items[0]["url"], "preview_url": items[0]["preview_url"],
            "width": 220, "height": 180, "caption": "@Bob", "mentions": [bob_id],
        })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{m}");
    assert_eq!(m["kind"], "gif");
    assert_eq!(m["text"], "GIF · @Bob");
    assert_eq!(m["caption"], "@Bob");
    assert_eq!(m["mentions"], json!([bob_id]));
    assert_eq!(m["media"]["provider"], "klipy");
    assert_eq!(m["media"]["mime"], "image/webp");
    assert_eq!(m["media"]["width"], 220);
    assert_eq!(m["media"]["url"], items[0]["url"]);
    assert_eq!(m["media"]["preview_url"], items[0]["preview_url"]);
    let stored: String = sqlx::query_scalar("SELECT media FROM messages WHERE id = ?")
        .bind(m["id"].as_i64().unwrap())
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert!(
        stored.contains(&format!("{base}/files/md.webp")),
        "{stored}"
    );
    let list = messages(&app, &bob, room, round).await;
    let seen = find(&list, m["id"].as_i64().unwrap()).unwrap();
    assert_eq!(seen["kind"], "gif");
    assert_eq!(seen["media"]["url"], items[0]["url"]);
    // Reactions work on GIFs.
    let (s, _) = call(
        &app,
        "POST",
        &format!("/messages/{}/reactions", m["id"]),
        Some(&bob),
        Some(room),
        Some(json!({ "emoji": "😂" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    // Only URLs we gave out, of plausible sizes.
    for (body, why) in [
        (
            json!({ "id": "1", "url": format!("{base}/files/md.webp"), "width": 10, "height": 10 }),
            "raw url",
        ),
        (
            json!({ "id": "1", "url": "https://static.klipy.com/a.webp", "width": 10, "height": 10 }),
            "raw klipy url",
        ),
        (
            json!({ "id": "1", "url": sign("https://evil.test/a.webp"), "width": 10, "height": 10 }),
            "foreign host",
        ),
        (
            json!({ "id": "1", "url": items[0]["url"], "width": 0, "height": 10 }),
            "no width",
        ),
        (
            json!({ "id": "1", "url": items[0]["url"], "preview_url": "/api/gifs/media/x.y", "width": 10, "height": 10 }),
            "bad preview",
        ),
        (
            json!({ "id": "", "url": items[0]["url"], "width": 10, "height": 10 }),
            "no id",
        ),
    ] {
        let (s, b) = call(
            &app,
            "POST",
            &format!("/rounds/{round}/messages/gif"),
            Some(&ana),
            Some(room),
            Some(body),
        )
        .await;
        assert_eq!(
            (s, b["error"].as_str()),
            (StatusCode::BAD_REQUEST, Some("bad_gif")),
            "{why}"
        );
    }
}
