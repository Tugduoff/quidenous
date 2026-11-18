use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{
    Router,
    http::{HeaderMap, header},
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    AppState,
    accounts::crypto::constant_eq,
    platform::error::{AppError, AppResult},
};

mod publish;
mod serve;
use publish::*;
use serve::*;

pub const PUBLISH_MAX: usize = 100 * 1024 * 1024;
const UNPACKED_MAX: u64 = 200 * 1024 * 1024;
const JSON_MAX: u64 = 8 * 1024 * 1024;
const KEEP: usize = 5;
const TMP_PREFIX: &str = ".tmp-";
const STALE_TMP: Duration = Duration::from_secs(3600);
const RUNTIME_IDLE: Duration = Duration::from_secs(120 * 86_400);
const KEEP_RUNTIMES: usize = 4;
const JSON_UTF8: &str = "application/json; charset=utf-8";

#[derive(Clone, Default)]
pub struct Publisher {
    one_at_a_time: std::sync::Arc<tokio::sync::Mutex<()>>,
    clock: PublishClock,
}

#[derive(Clone, Default)]
struct PublishClock(std::sync::Arc<AtomicU64>);

impl PublishClock {
    fn next(&self) -> u64 {
        let mut last = self.0.load(Ordering::SeqCst);
        loop {
            let next = now_millis().max(last + 1);
            match self
                .0
                .compare_exchange(last, next, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => return next,
                Err(actual) => last = actual,
            }
        }
    }
}

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

fn valid_runtime(s: &str) -> bool {
    (1..=128).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        && s != "."
        && s != ".."
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

fn iso_millis(ms: u64) -> String {
    let secs = ms / 1000;
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
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
    let digest = |s: &str| hex::encode(Sha256::digest(s.as_bytes()));
    if !given.is_empty() && constant_eq(&digest(given), &digest(expected)) {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
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
