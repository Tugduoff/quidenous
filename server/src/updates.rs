//! Self-hosted expo-updates (protocol v1).
//!
//! CI builds and signs the update, then publishes a tar.gz to `/api/updates/publish`; the
//! server only stores the exact manifest (or directive) bytes and the precomputed
//! `expo-signature` value, and serves them verbatim (the signing key never reaches it).
//!
//! Layout: `DATA_DIR/updates/<runtime>/<id>/{body.json, meta.json, assets/<name>}`.

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
    let bytes = tokio::fs::read(dir.join("assets").join(&name))
        .await
        .map_err(|_| AppError::NotFound)?;
    let content_type = meta
        .assets
        .get(&name)
        .and_then(|t| HeaderValue::from_str(t).ok())
        .unwrap_or(HeaderValue::from_static("application/octet-stream"));
    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=31536000, immutable"),
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
    let updates = updates_dir(&state);
    let _guard = PUBLISH.lock().await;
    let job = updates.clone();
    let meta = tokio::task::spawn_blocking(move || store(&job, archive))
        .await
        .map_err(|e| AppError::Internal(format!("publish task: {e}")))??;
    retain(&updates.join(&meta.runtime_version), &meta.platform).await?;
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

/// Unpacks, validates and moves the archive into place. Runs on a blocking thread, under
/// the publish lock.
fn store(updates: &Path, archive: Bytes) -> AppResult<Meta> {
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
