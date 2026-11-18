use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use axum::{
    Json,
    extract::{Path as UrlPath, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::crypto::random_hex,
    platform::error::{AppError, AppResult},
};

use super::*;

/// Every readable entry of `updates/<runtime>/`.
pub(super) async fn entries(runtime_dir: &Path) -> std::io::Result<Vec<(PathBuf, Meta)>> {
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
pub(super) async fn newest(
    runtime_dir: &Path,
    platform: &str,
) -> std::io::Result<Option<(PathBuf, Meta)>> {
    Ok(entries(runtime_dir)
        .await?
        .into_iter()
        .filter(|(_, m)| m.platform == platform)
        .max_by_key(|(_, m)| m.published_at))
}

pub(super) fn protocol_headers(res: &mut Response) {
    let h = res.headers_mut();
    h.insert("expo-protocol-version", HeaderValue::from_static("1"));
    h.insert("expo-sfv-version", HeaderValue::from_static("0"));
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=0"),
    );
}

pub(super) async fn manifest(
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

pub(super) fn header_or_query<'a>(
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

pub(super) async fn serve_manifest(
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

/// The Content-Type an asset is served with, from its extension (never the publisher's own
/// value: an HTML or SVG "asset" would otherwise run as a page on our domain). Phones don't
/// read it (they use the manifest's `contentType`), so anything unexpected is a plain download.
pub(super) fn asset_content_type(name: &str) -> &'static str {
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

pub(super) async fn asset(
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

pub(super) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<Value>>> {
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
