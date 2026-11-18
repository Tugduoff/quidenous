use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io::Read,
    path::{Path, PathBuf},
};

use axum::{
    Json,
    body::Bytes,
    extract::{Path as UrlPath, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    AppState,
    accounts::crypto::random_hex,
    platform::error::{AppError, AppResult},
};

use super::*;

pub(super) async fn publish(State(state): State<AppState>, req: Request) -> AppResult<Response> {
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
    let _guard = state.updates.one_at_a_time.lock().await;
    let job = updates.clone();
    let clock = state.updates.clock.clone();
    let meta =
        tokio::task::spawn_blocking(move || store(&job, archive, certificate.as_deref(), &clock))
            .await
            .map_err(|e| AppError::Internal(format!("publish task: {e}")))??;
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

pub(super) struct TmpDir(Option<PathBuf>);

impl Drop for TmpDir {
    fn drop(&mut self) {
        if let Some(dir) = self.0.take() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

#[derive(Default)]
pub(super) struct Unpacked {
    descriptor: Option<Vec<u8>>,
    body: Option<Vec<u8>>,
    hashes: HashMap<String, String>,
}

pub(super) fn entry_path(raw: &[u8]) -> AppResult<String> {
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

pub(super) fn unpack(archive: &[u8], assets_dir: &Path) -> AppResult<Unpacked> {
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

pub(super) fn check_asset(
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

pub(super) fn signature_value(header: &str) -> Option<&str> {
    header.split(',').find_map(|part| {
        part.trim()
            .strip_prefix("sig=\"")
            .and_then(|v| v.strip_suffix('"'))
    })
}

pub(super) fn verify_signature(certificate: &[u8], signature: &str, body: &[u8]) -> AppResult<()> {
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

pub(super) fn parse_iso_millis(s: &str) -> Option<u64> {
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
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = u64::try_from(era * 146_097 + doe - 719_468).ok()?;
    Some(((days * 86_400 + hh * 3600 + mm * 60 + ss) * 1000) + ms)
}

pub(super) fn newest_update_time(
    runtime_dir: &Path,
    platform: &str,
) -> std::io::Result<Option<u64>> {
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

pub(super) fn store(
    updates: &Path,
    archive: Bytes,
    certificate: Option<&[u8]>,
    clock: &PublishClock,
) -> AppResult<Meta> {
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

    let published_at = clock.next();
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

pub(super) async fn retain(runtime_dir: &Path, platform: &str) -> std::io::Result<()> {
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

pub(super) async fn prune_runtimes(updates: &Path) -> std::io::Result<()> {
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

pub(super) async fn sweep_tmp(updates: &Path) {
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

pub(super) async fn remove(
    State(state): State<AppState>,
    UrlPath((runtime, id)): UrlPath<(String, String)>,
    headers: HeaderMap,
) -> AppResult<StatusCode> {
    authorize(&state, &headers)?;
    if !valid_runtime(&runtime) || !valid_id(&id) {
        return Err(AppError::NotFound);
    }
    let dir = updates_dir(&state).join(&runtime).join(&id);
    let _guard = state.updates.one_at_a_time.lock().await;
    match tokio::fs::remove_dir_all(&dir).await {
        Ok(()) => {
            tracing::info!(%id, %runtime, "update deleted");
            Ok(StatusCode::NO_CONTENT)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(AppError::NotFound),
        Err(e) => Err(e.into()),
    }
}
