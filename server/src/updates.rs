//! Self-hosted expo-updates (protocol v1).
//!
//! CI builds and signs the update, then publishes a tar.gz to `/api/updates/publish`; the
//! server only stores the exact manifest (or directive) bytes and the precomputed
//! `expo-signature` value, and serves them verbatim (the signing key never reaches it).
//!
//! Layout: `DATA_DIR/updates/<runtime>/<id>/{body.json, meta.json, assets/<name>}`.
//!
//! With `UPDATES_CERTIFICATE` (the code signing certificate the apps embed) the server also
//! refuses unsigned uploads and signatures that don't match it: a leaked publish token alone
//! can't make OTA stall on an update every phone rejects.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path as UrlPath, Query, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    AppState,
    accounts::crypto::{constant_eq, random_hex},
    error::{AppError, AppResult},
};

/// Largest accepted publish body (compressed).
pub const PUBLISH_MAX: usize = 100 * 1024 * 1024;
/// Largest accepted archive once decompressed.
const UNPACKED_MAX: u64 = 200 * 1024 * 1024;
/// Largest `update.json` / `body.json`.
const JSON_MAX: u64 = 8 * 1024 * 1024;
/// Entries kept per (runtime, platform).
const KEEP: usize = 5;
const TMP_PREFIX: &str = ".tmp-";
const STALE_TMP: Duration = Duration::from_secs(3600);
/// A runtime (native build) with nothing published for this long is removed, unless it is
/// one of the [`KEEP_RUNTIMES`] most recently published: phones still on that build keep the
/// JS they already downloaded, and need an APK update anyway.
const RUNTIME_IDLE: Duration = Duration::from_secs(120 * 86_400);
const KEEP_RUNTIMES: usize = 4;
const JSON_UTF8: &str = "application/json; charset=utf-8";

/// One publish (or delete) at a time: retention and duplicate checks stay simple.
static PUBLISH: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/updates", get(list))
        .route("/updates/manifest", get(manifest))
        .route("/updates/assets/{runtime}/{id}/{name}", get(asset))
        .route(
            "/updates/publish",
            post(publish).layer(axum::extract::DefaultBodyLimit::max(PUBLISH_MAX)),
        )
        .route("/updates/{runtime}/{id}", delete(remove))
}

// ---------- storage model ----------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Update,
    Directive,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Meta {
    id: String,
    kind: Kind,
    platform: String,
    runtime_version: String,
    created_at: String,
    published_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    signature: Option<String>,
    #[serde(default)]
    assets: BTreeMap<String, String>,
}

/// `update.json` in the archive.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Descriptor {
    kind: Kind,
    platform: String,
    runtime_version: String,
    #[serde(default)]
    signature: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    id: String,
    created_at: String,
    runtime_version: String,
    launch_asset: ManifestAsset,
    #[serde(default)]
    assets: Vec<ManifestAsset>,
    #[serde(default)]
    metadata: Option<Value>,
    #[serde(default)]
    extra: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestAsset {
    hash: String,
    key: String,
    content_type: String,
    url: String,
}

#[derive(Deserialize)]
struct Directive {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    parameters: Option<Value>,
}

// ---------- validation helpers ----------

fn valid_runtime(s: &str) -> bool {
    (1..=128).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        && s != "."
        && s != ".."
        // Would collide with the temporary extraction dirs next to the runtime dirs.
        && !s.starts_with(TMP_PREFIX)
}

fn valid_platform(s: &str) -> bool {
    matches!(s, "android" | "ios")
}

fn valid_id(s: &str) -> bool {
    (1..=64).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

fn valid_uuid(s: &str) -> bool {
    s.len() == 36
        && s.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
}

fn valid_name(s: &str) -> bool {
    (1..=200).contains(&s.len())
        && !s.starts_with('.')
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Server time in millis, strictly increasing within the process (so "newest" is never a tie).
fn next_published_at() -> u64 {
    static LAST: AtomicU64 = AtomicU64::new(0);
    let mut last = LAST.load(Ordering::SeqCst);
    loop {
        let next = now_millis().max(last + 1);
        match LAST.compare_exchange(last, next, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return next,
            Err(actual) => last = actual,
        }
    }
}

/// `2026-01-31T12:34:56.789Z` from unix millis.
fn iso_millis(ms: u64) -> String {
    let secs = ms / 1000;
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60,
        ms % 1000
    )
}

fn updates_dir(state: &AppState) -> PathBuf {
    state.config.data_dir.join("updates")
}

/// Bearer check for the publishing endpoints. No token configured: the feature is off (404).
fn authorize(state: &AppState, headers: &HeaderMap) -> AppResult<()> {
    let Some(expected) = state.config.updates_publish_token.as_deref() else {
        return Err(AppError::NotFound);
    };
    let given = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .unwrap_or_default();
    // Compare digests so the comparison doesn't depend on the token's length either.
    let digest = |s: &str| hex::encode(Sha256::digest(s.as_bytes()));
    if !given.is_empty() && constant_eq(&digest(given), &digest(expected)) {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
    }
}

// ---------- reading the store ----------

/// Every readable entry of `updates/<runtime>/`.
async fn entries(runtime_dir: &Path) -> std::io::Result<Vec<(PathBuf, Meta)>> {
    let mut out = Vec::new();
    let mut dir = match tokio::fs::read_dir(runtime_dir).await {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e),
    };
    while let Some(entry) = dir.next_entry().await? {
        let path = entry.path();
        let Ok(bytes) = tokio::fs::read(path.join("meta.json")).await else {
            continue;
        };
        if let Ok(meta) = serde_json::from_slice::<Meta>(&bytes) {
            out.push((path, meta));
        }
    }
    Ok(out)
}

/// The entry served for a runtime and platform: the latest published.
async fn newest(runtime_dir: &Path, platform: &str) -> std::io::Result<Option<(PathBuf, Meta)>> {
    Ok(entries(runtime_dir)
        .await?
        .into_iter()
        .filter(|(_, m)| m.platform == platform)
        .max_by_key(|(_, m)| m.published_at))
}

// ---------- GET /updates/manifest ----------

fn protocol_headers(res: &mut Response) {
    let h = res.headers_mut();
    h.insert("expo-protocol-version", HeaderValue::from_static("1"));
    h.insert("expo-sfv-version", HeaderValue::from_static("0"));
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=0"),
    );
}

async fn manifest(
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let mut res = serve_manifest(&state, &query, &headers)
        .await
        .unwrap_or_else(IntoResponse::into_response);
    protocol_headers(&mut res);
    res
}

fn header_or_query<'a>(
    headers: &'a HeaderMap,
    query: &'a HashMap<String, String>,
    header: &str,
    param: &str,
) -> Option<&'a str> {
    headers
        .get(header)
        .and_then(|v| v.to_str().ok())
        .or_else(|| query.get(param).map(String::as_str))
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

async fn serve_manifest(
    state: &AppState,
    query: &HashMap<String, String>,
    headers: &HeaderMap,
) -> AppResult<Response> {
    let platform = header_or_query(headers, query, "expo-platform", "platform")
        .ok_or(AppError::BadRequest("missing_platform"))?;
    if !valid_platform(platform) {
        return Err(AppError::BadRequest("bad_platform"));
    }
    let runtime = header_or_query(headers, query, "expo-runtime-version", "runtime-version")
        .ok_or(AppError::BadRequest("missing_runtime_version"))?;
    if !valid_runtime(runtime) {
        return Err(AppError::BadRequest("bad_runtime_version"));
    }
    let no_update = || StatusCode::NO_CONTENT.into_response();
    let Some((dir, meta)) = newest(&updates_dir(state).join(runtime), platform).await? else {
        return Ok(no_update());
    };
    let body = tokio::fs::read(dir.join("body.json")).await?;

    if meta.kind == Kind::Directive {
        let directive: Directive = serde_json::from_slice(&body)
            .map_err(|_| AppError::Internal("stored directive unreadable".into()))?;
        if directive.kind != "rollBackToEmbedded" {
            return Ok(no_update());
        }
        let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
        let (current, embedded) = (
            header("expo-current-update-id"),
            header("expo-embedded-update-id"),
        );
        if current.is_some() && current == embedded {
            // Already running the embedded bundle.
            return Ok(no_update());
        }
    }

    let accept = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("*/*");
    let multipart =
        accept.trim().is_empty() || accept.contains("*/*") || accept.contains("multipart/mixed");
    if !multipart {
        if meta.kind == Kind::Directive {
            // Directives only travel in multipart responses.
            return Ok(no_update());
        }
        let mut res = (
            [(header::CONTENT_TYPE, HeaderValue::from_static(JSON_UTF8))],
            body,
        )
            .into_response();
        if let Some(sig) = meta.signature.as_deref().and_then(|s| s.parse().ok()) {
            res.headers_mut().insert("expo-signature", sig);
        }
        return Ok(res);
    }

    let boundary = random_hex(16);
    let part_name = match meta.kind {
        Kind::Update => "manifest",
        Kind::Directive => "directive",
    };
    let mut out = Vec::with_capacity(body.len() + 512);
    out.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{part_name}\"\r\nContent-Type: {JSON_UTF8}\r\n"
        )
        .as_bytes(),
    );
    if let Some(sig) = &meta.signature {
        out.extend_from_slice(format!("expo-signature: {sig}\r\n").as_bytes());
    }
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(&body);
    out.extend_from_slice(b"\r\n");
    if meta.kind == Kind::Update {
        out.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"extensions\"\r\nContent-Type: {JSON_UTF8}\r\n\r\n{{\"assetRequestHeaders\":{{}}}}\r\n"
            )
            .as_bytes(),
        );
    }
    out.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    Ok((
        [(
            header::CONTENT_TYPE,
            format!("multipart/mixed; boundary={boundary}"),
        )],
        out,
    )
        .into_response())
}

// ---------- GET /updates/assets/{runtime}/{id}/{name} ----------

/// The Content-Type an asset is served with, from its extension (never the publisher's own
/// value: an HTML or SVG "asset" would otherwise run as a page on our domain). Phones don't
/// read it (they use the manifest's `contentType`), so anything unexpected is a plain download.
fn asset_content_type(name: &str) -> &'static str {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("bundle" | "hbc" | "js") => "application/javascript",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("mp3") => "audio/mpeg",
        Some("mp4") => "video/mp4",
        _ => "application/octet-stream",
    }
}

async fn asset(
    State(state): State<AppState>,
    UrlPath((runtime, id, name)): UrlPath<(String, String, String)>,
) -> AppResult<Response> {
    if !valid_runtime(&runtime) || !valid_id(&id) || !valid_name(&name) {
        return Err(AppError::NotFound);
    }
    let dir = updates_dir(&state).join(&runtime).join(&id);
    let meta: Meta = match tokio::fs::read(dir.join("meta.json")).await {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| AppError::NotFound)?,
        Err(_) => return Err(AppError::NotFound),
    };
    if !meta.assets.contains_key(&name) {
        return Err(AppError::NotFound);
    }
    let bytes = tokio::fs::read(dir.join("assets").join(&name))
        .await
        .map_err(|_| AppError::NotFound)?;
    Ok((
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static(asset_content_type(&name)),
            ),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=31536000, immutable"),
            ),
            (
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ),
        ],
        bytes,
    )
        .into_response())
}

// ---------- POST /updates/publish ----------

async fn publish(State(state): State<AppState>, req: Request) -> AppResult<Response> {
    // Before reading up to 100 MB of body.
    authorize(&state, req.headers())?;
    let archive = match axum::body::to_bytes(req.into_body(), PUBLISH_MAX).await {
        Ok(b) => b,
        Err(_) => {
            return Ok((
                StatusCode::PAYLOAD_TOO_LARGE,
                Json(json!({ "error": "archive_too_large" })),
            )
                .into_response());
        }
    };
    let certificate = match &state.config.updates_certificate {
        Some(path) => Some(tokio::fs::read(path).await.map_err(|e| {
            AppError::Internal(format!("UPDATES_CERTIFICATE {}: {e}", path.display()))
        })?),
        None => None,
    };
    let updates = updates_dir(&state);
    let _guard = PUBLISH.lock().await;
    let job = updates.clone();
    let meta = tokio::task::spawn_blocking(move || store(&job, archive, certificate.as_deref()))
        .await
        .map_err(|e| AppError::Internal(format!("publish task: {e}")))??;
    // The update is live: a failed clean-up is logged (the next publish retries it), never
    // reported as a failed publish.
    if let Err(e) = retain(&updates.join(&meta.runtime_version), &meta.platform).await {
        tracing::warn!(error = ?e, "pruning old updates");
    }
    if let Err(e) = prune_runtimes(&updates).await {
        tracing::warn!(error = ?e, "pruning old runtime versions");
    }
    sweep_tmp(&updates).await;
    tracing::info!(
        id = %meta.id,
        runtime = %meta.runtime_version,
        platform = %meta.platform,
        kind = ?meta.kind,
        "update published"
    );
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": meta.id,
            "runtimeVersion": meta.runtime_version,
            "platform": meta.platform,
            "kind": meta.kind,
        })),
    )
        .into_response())
}

/// Removes the temporary dir unless the update made it into place.
struct TmpDir(Option<PathBuf>);

impl Drop for TmpDir {
    fn drop(&mut self) {
        if let Some(dir) = self.0.take() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

/// What came out of the archive: the two JSON files in memory, assets written to disk.
#[derive(Default)]
struct Unpacked {
    descriptor: Option<Vec<u8>>,
    body: Option<Vec<u8>>,
    /// Asset name -> base64url(SHA-256).
    hashes: HashMap<String, String>,
}

/// Normalized relative path of an archive entry; `..`, absolute paths and oddities refused.
fn entry_path(raw: &[u8]) -> AppResult<String> {
    let unsafe_path = AppError::BadRequest("unsafe_path_in_archive");
    let s = std::str::from_utf8(raw).map_err(|_| AppError::BadRequest("unsafe_path_in_archive"))?;
    if s.starts_with('/') || s.contains('\\') || s.contains('\0') {
        return Err(unsafe_path);
    }
    let mut parts = Vec::new();
    for part in s.split('/') {
        match part {
            "" | "." => {}
            ".." => return Err(unsafe_path),
            p => parts.push(p),
        }
    }
    Ok(parts.join("/"))
}

fn unpack(archive: &[u8], assets_dir: &Path) -> AppResult<Unpacked> {
    let bad = |_| AppError::BadRequest("bad_archive");
    let decoder = flate2::read::GzDecoder::new(archive).take(UNPACKED_MAX + 1024 * 1024);
    let mut tar = tar::Archive::new(decoder);
    let mut out = Unpacked::default();
    let mut seen = HashSet::new();
    let mut total = 0u64;
    for entry in tar.entries().map_err(bad)? {
        let mut entry = entry.map_err(bad)?;
        let path = entry_path(&entry.path_bytes())?;
        match entry.header().entry_type() {
            tar::EntryType::Directory | tar::EntryType::XGlobalHeader => continue,
            tar::EntryType::Regular | tar::EntryType::Continuous => {}
            tar::EntryType::Symlink | tar::EntryType::Link => {
                return Err(AppError::BadRequest("link_in_archive"));
            }
            _ => return Err(AppError::BadRequest("unsupported_archive_entry")),
        }
        let size = entry.size();
        total += size;
        if total > UNPACKED_MAX {
            return Err(AppError::BadRequest("archive_too_large"));
        }
        let wanted = path == "update.json"
            || path == "body.json"
            || path
                .strip_prefix("assets/")
                .is_some_and(|n| !n.contains('/') && valid_name(n));
        if !wanted {
            continue;
        }
        if !seen.insert(path.clone()) {
            return Err(AppError::BadRequest("duplicate_archive_entry"));
        }
        if let Some(name) = path.strip_prefix("assets/") {
            let mut file = std::fs::File::create_new(assets_dir.join(name))?;
            let mut hasher = Sha256::new();
            let mut buf = vec![0u8; 64 * 1024];
            loop {
                let n = entry.read(&mut buf).map_err(bad)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
                std::io::Write::write_all(&mut file, &buf[..n])?;
            }
            out.hashes
                .insert(name.to_owned(), URL_SAFE_NO_PAD.encode(hasher.finalize()));
        } else {
            if size > JSON_MAX {
                return Err(AppError::BadRequest("json_too_large"));
            }
            let mut bytes = Vec::with_capacity(size as usize);
            entry.read_to_end(&mut bytes).map_err(bad)?;
            if path == "update.json" {
                out.descriptor = Some(bytes);
            } else {
                out.body = Some(bytes);
            }
        }
    }
    Ok(out)
}

/// Checks one manifest asset against the archive; returns its file name.
fn check_asset(
    a: &ManifestAsset,
    runtime: &str,
    id: &str,
    hashes: &HashMap<String, String>,
) -> AppResult<String> {
    if a.key.is_empty() {
        return Err(AppError::BadRequest("bad_manifest_asset_key"));
    }
    if a.content_type.is_empty() || HeaderValue::from_str(&a.content_type).is_err() {
        return Err(AppError::BadRequest("bad_manifest_asset_content_type"));
    }
    let prefix = format!("/api/updates/assets/{runtime}/{id}/");
    let name = a
        .url
        .rfind(&prefix)
        .map(|i| &a.url[i + prefix.len()..])
        .filter(|n| valid_name(n))
        .ok_or(AppError::BadRequest("bad_asset_url"))?;
    let actual = hashes
        .get(name)
        .ok_or(AppError::BadRequest("missing_asset"))?;
    if *actual != a.hash {
        return Err(AppError::BadRequest("asset_hash_mismatch"));
    }
    Ok(name.to_owned())
}

/// The base64 signature of an `expo-signature` value (`sig="…", keyid="main"`).
fn signature_value(header: &str) -> Option<&str> {
    header.split(',').find_map(|part| {
        part.trim()
            .strip_prefix("sig=\"")
            .and_then(|v| v.strip_suffix('"'))
    })
}

/// Checks an `expo-signature` (RSA PKCS#1 v1.5 over SHA-256 of the exact body bytes, as
/// ota-publish.mjs signs and expo-updates verifies) against the configured certificate.
fn verify_signature(certificate: &[u8], signature: &str, body: &[u8]) -> AppResult<()> {
    let key = jsonwebtoken::DecodingKey::from_rsa_pem(certificate)
        .map_err(|e| AppError::Internal(format!("UPDATES_CERTIFICATE unreadable: {e}")))?;
    let raw = signature_value(signature)
        .and_then(|s| base64::engine::general_purpose::STANDARD.decode(s).ok())
        .ok_or(AppError::BadRequest("bad_signature"))?;
    let ok = jsonwebtoken::crypto::verify(
        &URL_SAFE_NO_PAD.encode(raw),
        body,
        &key,
        jsonwebtoken::Algorithm::RS256,
    )
    .unwrap_or(false);
    if ok {
        Ok(())
    } else {
        Err(AppError::BadRequest("bad_signature"))
    }
}

/// Unix millis of an ISO 8601 UTC date (`2026-01-31T12:34:56.789Z`, fraction optional).
fn parse_iso_millis(s: &str) -> Option<u64> {
    let s = s.strip_suffix('Z').or_else(|| s.strip_suffix("+00:00"))?;
    let (date, time) = s.split_once('T')?;
    let mut d = date.splitn(3, '-').map(|p| p.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let (hms, frac) = time.split_once('.').unwrap_or((time, ""));
    let mut t = hms.splitn(3, ':').map(|p| p.parse::<u64>().ok());
    let (hh, mm, ss) = (t.next()??, t.next()??, t.next()??);
    if !(1..=12).contains(&m) || !(1..=31).contains(&day) || hh > 23 || mm > 59 || ss > 60 {
        return None;
    }
    if !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let ms = format!("{frac:0<3}")[..3].parse::<u64>().ok()?;
    // Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = u64::try_from(era * 146_097 + doe - 719_468).ok()?;
    Some(((days * 86_400 + hh * 3600 + mm * 60 + ss) * 1000) + ms)
}

/// `createdAt` of the newest stored update (directives aside) of a runtime and platform.
fn newest_update_time(runtime_dir: &Path, platform: &str) -> std::io::Result<Option<u64>> {
    let dir = match std::fs::read_dir(runtime_dir) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut newest = None;
    for entry in dir.flatten() {
        let Ok(bytes) = std::fs::read(entry.path().join("meta.json")) else {
            continue;
        };
        let Ok(meta) = serde_json::from_slice::<Meta>(&bytes) else {
            continue;
        };
        if meta.kind == Kind::Update && meta.platform == platform {
            newest = newest.max(parse_iso_millis(&meta.created_at));
        }
    }
    Ok(newest)
}

/// Unpacks, validates and moves the archive into place. Runs on a blocking thread, under
/// the publish lock. `certificate`: signatures are required and checked against it.
fn store(updates: &Path, archive: Bytes, certificate: Option<&[u8]>) -> AppResult<Meta> {
    std::fs::create_dir_all(updates)?;
    let tmp_path = updates.join(format!("{TMP_PREFIX}{}", random_hex(8)));
    let mut tmp = TmpDir(Some(tmp_path.clone()));
    let assets_dir = tmp_path.join("assets");
    std::fs::create_dir_all(&assets_dir)?;

    let unpacked = unpack(&archive, &assets_dir)?;
    drop(archive);
    let descriptor: Descriptor = serde_json::from_slice(
        unpacked
            .descriptor
            .as_deref()
            .ok_or(AppError::BadRequest("missing_update_json"))?,
    )
    .map_err(|_| AppError::BadRequest("bad_update_json"))?;
    let body = unpacked
        .body
        .ok_or(AppError::BadRequest("missing_body_json"))?;
    let runtime = descriptor.runtime_version;
    if !valid_runtime(&runtime) {
        return Err(AppError::BadRequest("bad_runtime_version"));
    }
    if !valid_platform(&descriptor.platform) {
        return Err(AppError::BadRequest("bad_platform"));
    }
    if let Some(sig) = &descriptor.signature
        && (sig.is_empty() || HeaderValue::from_str(sig).is_err())
    {
        return Err(AppError::BadRequest("bad_signature"));
    }
    if let Some(cert) = certificate {
        let sig = descriptor
            .signature
            .as_deref()
            .ok_or(AppError::BadRequest("unsigned_update"))?;
        verify_signature(cert, sig, &body)?;
    }

    let published_at = next_published_at();
    let (id, created_at, assets) = match descriptor.kind {
        Kind::Update => {
            let m: Manifest =
                serde_json::from_slice(&body).map_err(|_| AppError::BadRequest("bad_manifest"))?;
            if !valid_uuid(&m.id) {
                return Err(AppError::BadRequest("bad_manifest_id"));
            }
            if m.runtime_version != runtime {
                return Err(AppError::BadRequest("runtime_version_mismatch"));
            }
            // Two CI runs finishing out of order must not serve the older JS as the newest:
            // phones that already run the newer one would get nothing more until the next
            // publish, and the others would get the older code. Same time: a re-run, fine.
            let created =
                parse_iso_millis(&m.created_at).ok_or(AppError::BadRequest("bad_created_at"))?;
            let newest = newest_update_time(&updates.join(&runtime), &descriptor.platform)?;
            if newest.is_some_and(|n| created < n) {
                return Err(AppError::Conflict("older_than_current"));
            }
            if [&m.metadata, &m.extra]
                .iter()
                .any(|v| v.as_ref().is_some_and(|v| !v.is_object()))
            {
                return Err(AppError::BadRequest("bad_manifest"));
            }
            let mut assets = BTreeMap::new();
            for a in std::iter::once(&m.launch_asset).chain(&m.assets) {
                let name = check_asset(a, &runtime, &m.id, &unpacked.hashes)?;
                assets.insert(name, a.content_type.clone());
            }
            // Only what the manifest references is kept.
            for name in unpacked.hashes.keys() {
                if !assets.contains_key(name) {
                    std::fs::remove_file(assets_dir.join(name))?;
                }
            }
            (m.id, m.created_at, assets)
        }
        Kind::Directive => {
            let d: Directive =
                serde_json::from_slice(&body).map_err(|_| AppError::BadRequest("bad_directive"))?;
            let created_at = match d.kind.as_str() {
                "rollBackToEmbedded" => d
                    .parameters
                    .as_ref()
                    .and_then(|p| p.get("commitTime"))
                    .and_then(Value::as_str)
                    .ok_or(AppError::BadRequest("missing_commit_time"))?
                    .to_owned(),
                "noUpdateAvailable" => d
                    .parameters
                    .as_ref()
                    .and_then(|p| p.get("commitTime"))
                    .and_then(Value::as_str)
                    .map_or_else(|| iso_millis(published_at), str::to_owned),
                _ => return Err(AppError::BadRequest("bad_directive_type")),
            };
            if d.parameters.as_ref().is_some_and(|p| !p.is_object()) {
                return Err(AppError::BadRequest("bad_directive"));
            }
            std::fs::remove_dir_all(&assets_dir)?;
            (
                format!("directive-{published_at}"),
                created_at,
                BTreeMap::new(),
            )
        }
    };

    let meta = Meta {
        id,
        kind: descriptor.kind,
        platform: descriptor.platform,
        runtime_version: runtime,
        created_at,
        published_at,
        signature: descriptor.signature,
        assets,
    };
    std::fs::write(tmp_path.join("body.json"), &body)?;
    std::fs::write(
        tmp_path.join("meta.json"),
        serde_json::to_vec_pretty(&meta).map_err(|e| AppError::Internal(e.to_string()))?,
    )?;
    let runtime_dir = updates.join(&meta.runtime_version);
    std::fs::create_dir_all(&runtime_dir)?;
    let target = runtime_dir.join(&meta.id);
    if target.exists() {
        return Err(AppError::Conflict("already_published"));
    }
    if let Err(e) = std::fs::rename(&tmp_path, &target) {
        return Err(if target.exists() {
            AppError::Conflict("already_published")
        } else {
            e.into()
        });
    }
    tmp.0 = None;
    Ok(meta)
}

/// Keeps the newest [`KEEP`] entries of a runtime and platform.
async fn retain(runtime_dir: &Path, platform: &str) -> std::io::Result<()> {
    let mut list: Vec<_> = entries(runtime_dir)
        .await?
        .into_iter()
        .filter(|(_, m)| m.platform == platform)
        .collect();
    list.sort_by_key(|(_, m)| std::cmp::Reverse(m.published_at));
    for (dir, meta) in list.into_iter().skip(KEEP) {
        tracing::info!(id = %meta.id, runtime = %meta.runtime_version, platform, "old update removed");
        tokio::fs::remove_dir_all(dir).await?;
    }
    Ok(())
}

/// Removes the runtime dirs of old native builds (see [`RUNTIME_IDLE`]).
async fn prune_runtimes(updates: &Path) -> std::io::Result<()> {
    let mut runtimes = Vec::new();
    let mut dir = tokio::fs::read_dir(updates).await?;
    while let Some(entry) = dir.next_entry().await? {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !valid_runtime(&name) || !entry.file_type().await?.is_dir() {
            continue;
        }
        let last = entries(&entry.path())
            .await?
            .iter()
            .map(|(_, m)| m.published_at)
            .max()
            .unwrap_or(0);
        runtimes.push((last, entry.path()));
    }
    runtimes.sort_by_key(|(last, _)| std::cmp::Reverse(*last));
    let cutoff = now_millis().saturating_sub(RUNTIME_IDLE.as_millis() as u64);
    for (last, path) in runtimes.into_iter().skip(KEEP_RUNTIMES) {
        if last < cutoff {
            tracing::info!(runtime = %path.display(), "old runtime removed");
            tokio::fs::remove_dir_all(path).await?;
        }
    }
    Ok(())
}

/// Removes extraction dirs left behind by a crash (older than an hour).
async fn sweep_tmp(updates: &Path) {
    let Ok(mut dir) = tokio::fs::read_dir(updates).await else {
        return;
    };
    while let Ok(Some(entry)) = dir.next_entry().await {
        if !entry.file_name().to_string_lossy().starts_with(TMP_PREFIX) {
            continue;
        }
        let stale = entry
            .metadata()
            .await
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > STALE_TMP);
        if stale {
            let _ = tokio::fs::remove_dir_all(entry.path()).await;
        }
    }
}

// ---------- admin ----------

async fn list(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Vec<Value>>> {
    authorize(&state, &headers)?;
    let updates = updates_dir(&state);
    let mut all = Vec::new();
    if let Ok(mut dir) = tokio::fs::read_dir(&updates).await {
        while let Some(entry) = dir.next_entry().await? {
            let name = entry.file_name();
            if valid_runtime(&name.to_string_lossy()) && entry.file_type().await?.is_dir() {
                all.extend(entries(&entry.path()).await?.into_iter().map(|(_, m)| m));
            }
        }
    }
    let mut current: HashMap<(String, String), u64> = HashMap::new();
    for m in &all {
        let key = (m.runtime_version.clone(), m.platform.clone());
        let best = current.entry(key).or_default();
        *best = (*best).max(m.published_at);
    }
    all.sort_by_key(|m| std::cmp::Reverse(m.published_at));
    Ok(Json(
        all.into_iter()
            .map(|m| {
                let is_current = current
                    .get(&(m.runtime_version.clone(), m.platform.clone()))
                    .is_some_and(|&p| p == m.published_at);
                json!({
                    "runtimeVersion": m.runtime_version,
                    "platform": m.platform,
                    "id": m.id,
                    "kind": m.kind,
                    "createdAt": m.created_at,
                    "publishedAt": m.published_at,
                    "current": is_current,
                })
            })
            .collect(),
    ))
}

async fn remove(
    State(state): State<AppState>,
    UrlPath((runtime, id)): UrlPath<(String, String)>,
    headers: HeaderMap,
) -> AppResult<StatusCode> {
    authorize(&state, &headers)?;
    if !valid_runtime(&runtime) || !valid_id(&id) {
        return Err(AppError::NotFound);
    }
    let dir = updates_dir(&state).join(&runtime).join(&id);
    let _guard = PUBLISH.lock().await;
    match tokio::fs::remove_dir_all(&dir).await {
        Ok(()) => {
            tracing::info!(%id, %runtime, "update deleted");
            Ok(StatusCode::NO_CONTENT)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(AppError::NotFound),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_dates() {
        assert_eq!(iso_millis(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso_millis(1_709_210_096_789), "2024-02-29T12:34:56.789Z");
        for ms in [0, 1_709_210_096_789, 1_763_200_000_123, 951_782_400_000] {
            assert_eq!(parse_iso_millis(&iso_millis(ms)), Some(ms));
        }
        assert_eq!(
            parse_iso_millis("2024-02-29T12:34:56Z"),
            Some(1_709_210_096_000)
        );
        assert_eq!(
            parse_iso_millis("2024-02-29T12:34:56.7+00:00"),
            Some(1_709_210_096_700)
        );
        for bad in [
            "",
            "2024-02-29",
            "2024-02-29T12:34:56",
            "2024-13-01T00:00:00Z",
            "x",
        ] {
            assert_eq!(parse_iso_millis(bad), None, "{bad}");
        }
    }

    #[test]
    fn content_types_come_from_the_extension() {
        assert_eq!(asset_content_type("a.hbc"), "application/javascript");
        assert_eq!(asset_content_type("a.PNG"), "image/png");
        for name in ["a.html", "a.svg", "a.xml", "noext"] {
            assert_eq!(asset_content_type(name), "application/octet-stream");
        }
    }

    #[test]
    fn archive_paths() {
        assert_eq!(entry_path(b"./assets/a.js").unwrap(), "assets/a.js");
        assert_eq!(entry_path(b"body.json").unwrap(), "body.json");
        assert_eq!(entry_path(b"./").unwrap(), "");
        assert!(entry_path(b"../x").is_err());
        assert!(entry_path(b"assets/../../x").is_err());
        assert!(entry_path(b"/etc/passwd").is_err());
    }

    #[test]
    fn names() {
        assert!(valid_runtime("1.0.0") && valid_runtime("exposdk_54-0"));
        assert!(!valid_runtime("..") && !valid_runtime("a/b") && !valid_runtime(""));
        assert!(valid_name("bundle-abc.hbc") && !valid_name(".hidden") && !valid_name("a b"));
        assert!(valid_uuid("0f8fad5b-d9cb-469f-a165-70867728950e"));
        assert!(!valid_uuid("0f8fad5b-d9cb-469f-a165-70867728950"));
    }
}
