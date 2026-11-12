//! Self-hosted expo-updates: publishing (CI) and serving (protocol v1).

use std::path::PathBuf;

use axum::{
    Router,
    body::{Body, Bytes},
    http::{HeaderMap, Request, StatusCode},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use http_body_util::BodyExt;
use quidenous_server::{app, config::Config, state};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tower::ServiceExt;

const TOKEN: &str = "secret";
const RV: &str = "1.0.0";
const SIG: &str = "sig=\"c2lnbmF0dXJl\", keyid=\"main\"";

async fn test_app(token: Option<&str>) -> (Router, PathBuf) {
    let dir = std::env::temp_dir().join(format!("qdn-ota-test-{}", rand_suffix()));
    let config = Config {
        port: 0,
        database_url: format!("sqlite://{}?mode=rwc", dir.join("test.db").display()),
        data_dir: dir.clone(),
        web_dir: None,
        group_name: "Test".into(),
        join_code: "CODE-1".into(),
        vapid_private_key: None,
        vapid_subject: "mailto:test@example.com".into(),
        fcm_service_account: None,
        updates_publish_token: token.map(Into::into),
        ..Config::default()
    };
    std::fs::create_dir_all(&dir).unwrap();
    (app(state(config).await.unwrap()), dir)
}

/// Unique per test app (tests run in parallel, timestamps alone can collide).
fn rand_suffix() -> String {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{}-{nanos}-{n}", std::process::id())
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, HeaderMap, Bytes) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, headers, bytes)
}

fn json_of(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap_or(Value::Null)
}

async fn publish(app: &Router, token: Option<&str>, archive: Vec<u8>) -> (StatusCode, Value) {
    let mut req = Request::post("/api/updates/publish").header("content-type", "application/gzip");
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    let (status, _, body) = send(app, req.body(Body::from(archive)).unwrap()).await;
    (status, json_of(&body))
}

/// GET /api/updates/manifest with the given headers.
async fn manifest(
    app: &Router,
    uri: &str,
    headers: &[(&str, &str)],
) -> (StatusCode, HeaderMap, Bytes) {
    let mut req = Request::get(uri);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    send(app, req.body(Body::empty()).unwrap()).await
}

async fn manifest_for(
    app: &Router,
    runtime: &str,
    platform: &str,
) -> (StatusCode, HeaderMap, Bytes) {
    manifest(
        app,
        "/api/updates/manifest",
        &[
            ("expo-runtime-version", runtime),
            ("expo-platform", platform),
        ],
    )
    .await
}

async fn admin(app: &Router, method: &str, path: &str, token: Option<&str>) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(format!("/api{path}"));
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    let (status, _, body) = send(app, req.body(Body::empty()).unwrap()).await;
    (status, json_of(&body))
}

// ---------- archives ----------

enum Entry<'a> {
    File(&'a str, Vec<u8>),
    Symlink(&'a str, &'a str),
}

/// A tar.gz with raw entry names (so tests can write `..` or absolute paths).
fn tarball(entries: Vec<Entry>) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for entry in entries {
        let mut header = tar::Header::new_gnu();
        let (name, data) = match entry {
            Entry::File(name, data) => {
                header.set_entry_type(tar::EntryType::Regular);
                (name, data)
            }
            Entry::Symlink(name, target) => {
                header.set_entry_type(tar::EntryType::Symlink);
                header.set_link_name(target).unwrap();
                (name, Vec::new())
            }
        };
        let raw = &mut header.as_old_mut().name;
        raw.fill(0);
        raw[..name.len()].copy_from_slice(name.as_bytes());
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, data.as_slice()).unwrap();
    }
    let tar = builder.into_inner().unwrap();
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    std::io::Write::write_all(&mut gz, &tar).unwrap();
    gz.finish().unwrap()
}

fn hash(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(bytes))
}

fn uuid(n: u32) -> String {
    format!("0f8fad5b-d9cb-469f-a165-{n:012x}")
}

fn asset_json(runtime: &str, id: &str, name: &str, bytes: &[u8], content_type: &str) -> Value {
    json!({
        "hash": hash(bytes),
        "key": name.split('.').next().unwrap(),
        "contentType": content_type,
        "fileExtension": format!(".{}", name.rsplit('.').next().unwrap()),
        "url": format!("https://ota.example.com/api/updates/assets/{runtime}/{id}/{name}"),
    })
}

const BUNDLE: &[u8] = b"var x = 1; // hermes bytecode stand-in";
const ICON: &[u8] = b"\x89PNG fake image";

fn manifest_body(runtime: &str, id: &str) -> Value {
    json!({
        "id": id,
        "createdAt": "2026-11-12T10:00:00.000Z",
        "runtimeVersion": runtime,
        "launchAsset": asset_json(runtime, id, "bundle-abc.hbc", BUNDLE, "application/javascript"),
        "assets": [asset_json(runtime, id, "icon-def.png", ICON, "image/png")],
        "metadata": {},
        "extra": { "expoClient": { "name": "Qui de nous ?" } },
    })
}

fn descriptor(kind: &str, platform: &str, runtime: &str) -> Value {
    json!({ "kind": kind, "platform": platform, "runtimeVersion": runtime, "signature": SIG })
}

/// update.json + body.json (pretty-printed, to check bytes are kept) + the two assets.
fn update_archive(runtime: &str, platform: &str, body: &Value) -> (Vec<u8>, Vec<u8>) {
    let body = serde_json::to_vec_pretty(body).unwrap();
    let archive = tarball(vec![
        Entry::File(
            "./update.json",
            descriptor("update", platform, runtime)
                .to_string()
                .into_bytes(),
        ),
        Entry::File("./body.json", body.clone()),
        Entry::File("./assets/bundle-abc.hbc", BUNDLE.to_vec()),
        Entry::File("./assets/icon-def.png", ICON.to_vec()),
        Entry::File("./assets/unused.txt", b"not referenced".to_vec()),
    ]);
    (archive, body)
}

fn directive_archive(runtime: &str, platform: &str, directive: &Value) -> Vec<u8> {
    tarball(vec![
        Entry::File(
            "update.json",
            descriptor("directive", platform, runtime)
                .to_string()
                .into_bytes(),
        ),
        Entry::File("body.json", directive.to_string().into_bytes()),
    ])
}

fn boundary(headers: &HeaderMap) -> String {
    let ct = headers["content-type"].to_str().unwrap();
    ct.strip_prefix("multipart/mixed; boundary=")
        .unwrap_or_else(|| panic!("not multipart: {ct}"))
        .to_owned()
}

fn expected_multipart(boundary: &str, name: &str, body: &[u8], signature: bool) -> Vec<u8> {
    let mut out = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\nContent-Type: application/json; charset=utf-8\r\n"
    )
    .into_bytes();
    if signature {
        out.extend_from_slice(format!("expo-signature: {SIG}\r\n").as_bytes());
    }
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(body);
    out.extend_from_slice(b"\r\n");
    if name == "manifest" {
        out.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"extensions\"\r\nContent-Type: application/json; charset=utf-8\r\n\r\n{{\"assetRequestHeaders\":{{}}}}\r\n"
            )
            .as_bytes(),
        );
    }
    out.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    out
}

fn assert_protocol_headers(headers: &HeaderMap) {
    assert_eq!(headers["expo-protocol-version"], "1");
    assert_eq!(headers["expo-sfv-version"], "0");
    assert_eq!(headers["cache-control"], "private, max-age=0");
}

/// The id the manifest endpoint serves (multipart), or None on 204.
async fn served(app: &Router, runtime: &str, platform: &str) -> Option<String> {
    let (status, headers, body) = manifest_for(app, runtime, platform).await;
    if status == StatusCode::NO_CONTENT {
        return None;
    }
    assert_eq!(status, StatusCode::OK);
    let b = boundary(&headers);
    let text = String::from_utf8(body.to_vec()).unwrap();
    let part = text.split(&format!("--{b}")).nth(1).unwrap();
    let json = part.split("\r\n\r\n").nth(1).unwrap().trim_end();
    let v: Value = serde_json::from_str(json).unwrap();
    Some(
        v["id"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| v["type"].as_str().unwrap().to_owned()),
    )
}

fn leftover_tmp(dir: &std::path::Path) -> bool {
    std::fs::read_dir(dir.join("updates"))
        .map(|d| {
            d.flatten()
                .any(|e| e.file_name().to_string_lossy().starts_with(".tmp-"))
        })
        .unwrap_or(false)
}

// ---------- tests ----------

#[tokio::test]
async fn publishing_needs_the_token_and_is_off_without_it() {
    let (app, _) = test_app(Some(TOKEN)).await;
    let id = uuid(1);
    let (archive, _) = update_archive(RV, "android", &manifest_body(RV, &id));
    assert_eq!(
        publish(&app, None, archive.clone()).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        publish(&app, Some("nope"), archive.clone()).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        admin(&app, "GET", "/updates", None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        admin(&app, "DELETE", &format!("/updates/{RV}/{id}"), Some("nope"))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(served(&app, RV, "android").await, None);

    let (off, _) = test_app(None).await;
    assert_eq!(
        publish(&off, Some(TOKEN), archive).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        admin(&off, "GET", "/updates", Some(TOKEN)).await.0,
        StatusCode::NOT_FOUND
    );
    // Serving stays on (nothing to serve).
    let (status, headers, _) = manifest_for(&off, RV, "android").await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_protocol_headers(&headers);
    let (_, f) = admin(&off, "GET", "/features", None).await;
    assert!(f.as_array().unwrap().contains(&json!("ota")));
}

#[tokio::test]
async fn a_published_update_is_served_byte_for_byte() {
    let (app, dir) = test_app(Some(TOKEN)).await;
    let id = uuid(1);
    let (archive, body) = update_archive(RV, "android", &manifest_body(RV, &id));
    let (status, res) = publish(&app, Some(TOKEN), archive).await;
    assert_eq!(status, StatusCode::CREATED, "{res}");
    assert_eq!(
        res,
        json!({ "id": id, "runtimeVersion": RV, "platform": "android", "kind": "update" })
    );
    assert!(!leftover_tmp(&dir));
    // Only referenced assets are kept.
    let stored = dir.join("updates").join(RV).join(&id);
    assert!(stored.join("assets/bundle-abc.hbc").exists());
    assert!(!stored.join("assets/unused.txt").exists());
    assert_eq!(std::fs::read(stored.join("body.json")).unwrap(), body);

    // Multipart (default accept), with the signature on the manifest part.
    for accept in [
        None,
        Some("*/*"),
        Some("multipart/mixed,application/expo+json,application/json"),
    ] {
        let mut headers = vec![("expo-runtime-version", RV), ("expo-platform", "android")];
        if let Some(a) = accept {
            headers.push(("accept", a));
        }
        let (status, h, bytes) = manifest(&app, "/api/updates/manifest", &headers).await;
        assert_eq!(status, StatusCode::OK);
        assert_protocol_headers(&h);
        let b = boundary(&h);
        assert!(b.chars().all(|c| c.is_ascii_alphanumeric()) && !b.is_empty());
        assert_eq!(
            bytes.to_vec(),
            expected_multipart(&b, "manifest", &body, true)
        );
    }

    // Query parameters work as well as headers.
    let (status, h, bytes) = manifest(
        &app,
        &format!("/api/updates/manifest?platform=android&runtime-version={RV}"),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        bytes.to_vec(),
        expected_multipart(&boundary(&h), "manifest", &body, true)
    );

    // Plain JSON: the manifest itself, signature in a header.
    for accept in [
        "application/expo+json",
        "application/json",
        "application/expo+json, application/json",
    ] {
        let (status, h, bytes) = manifest(
            &app,
            "/api/updates/manifest",
            &[
                ("expo-runtime-version", RV),
                ("expo-platform", "android"),
                ("accept", accept),
            ],
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_protocol_headers(&h);
        assert_eq!(h["content-type"], "application/json; charset=utf-8");
        assert_eq!(h["expo-signature"], SIG);
        assert_eq!(bytes.to_vec(), body);
    }

    // Other runtime, other platform: nothing.
    for (rv, platform) in [("2.0.0", "android"), (RV, "ios")] {
        let (status, h, bytes) = manifest_for(&app, rv, platform).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_protocol_headers(&h);
        assert!(bytes.is_empty());
    }

    // Missing or invalid parameters.
    for headers in [
        vec![("expo-platform", "android")],
        vec![("expo-runtime-version", RV)],
        vec![("expo-runtime-version", RV), ("expo-platform", "web")],
        vec![("expo-runtime-version", ".."), ("expo-platform", "android")],
    ] {
        let (status, h, _) = manifest(&app, "/api/updates/manifest", &headers).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{headers:?}");
        assert_protocol_headers(&h);
    }

    // Assets.
    for (name, bytes, ct) in [
        ("bundle-abc.hbc", BUNDLE, "application/javascript"),
        ("icon-def.png", ICON, "image/png"),
    ] {
        let (status, h, got) = send(
            &app,
            Request::get(format!("/api/updates/assets/{RV}/{id}/{name}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(h["content-type"], ct);
        assert_eq!(h["cache-control"], "public, max-age=31536000, immutable");
        assert_eq!(got.as_ref(), bytes);
    }
    for path in [
        format!("{RV}/{id}/unused.txt"),
        format!("{RV}/{id}/meta.json"),
        format!("{RV}/{id}/.hidden"),
        format!("{RV}/{id}/..%2Fbody.json"),
        format!("..%2F..%2F/{id}/bundle-abc.hbc"),
        format!("2.0.0/{id}/bundle-abc.hbc"),
        format!("{RV}/not_an_id/bundle-abc.hbc"),
    ] {
        let (status, _, _) = send(
            &app,
            Request::get(format!("/api/updates/assets/{path}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }

    // Same id again: conflict.
    let (archive, _) = update_archive(RV, "android", &manifest_body(RV, &id));
    let (status, _) = publish(&app, Some(TOKEN), archive).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(!leftover_tmp(&dir));
}

#[tokio::test]
async fn bad_archives_are_refused() {
    let (app, dir) = test_app(Some(TOKEN)).await;
    let id = uuid(7);
    let good = manifest_body(RV, &id);
    let update_json = || descriptor("update", "android", RV).to_string().into_bytes();
    let body_json = |v: &Value| v.to_string().into_bytes();
    let assets = || {
        vec![
            Entry::File("assets/bundle-abc.hbc", BUNDLE.to_vec()),
            Entry::File("assets/icon-def.png", ICON.to_vec()),
        ]
    };
    let with = |update: Vec<u8>, body: Vec<u8>| {
        let mut entries = vec![
            Entry::File("update.json", update),
            Entry::File("body.json", body),
        ];
        entries.extend(assets());
        tarball(entries)
    };

    let mut cases: Vec<(&str, Vec<u8>, &str)> = Vec::new();
    let mut m = good.clone();
    m["launchAsset"]["hash"] = json!(hash(b"something else"));
    cases.push((
        "hash mismatch",
        with(update_json(), body_json(&m)),
        "asset_hash_mismatch",
    ));
    let mut m = good.clone();
    m["assets"][0]["url"] = json!(format!(
        "https://x/api/updates/assets/{RV}/{}/icon-def.png",
        uuid(8)
    ));
    cases.push((
        "url with another id",
        with(update_json(), body_json(&m)),
        "bad_asset_url",
    ));
    let mut m = good.clone();
    m["assets"][0]["url"] = json!(format!(
        "https://x/api/updates/assets/9.9.9/{id}/icon-def.png"
    ));
    cases.push((
        "url with another runtime",
        with(update_json(), body_json(&m)),
        "bad_asset_url",
    ));
    let mut m = good.clone();
    m["assets"][0]["url"] = json!(format!(
        "https://x/api/updates/assets/{RV}/{id}/../icon-def.png"
    ));
    cases.push((
        "url with a bad name",
        with(update_json(), body_json(&m)),
        "bad_asset_url",
    ));
    let mut m = good.clone();
    m["assets"][0]["url"] = json!(format!(
        "https://x/api/updates/assets/{RV}/{id}/missing.png"
    ));
    cases.push((
        "asset not in the archive",
        with(update_json(), body_json(&m)),
        "missing_asset",
    ));
    let mut m = good.clone();
    m["runtimeVersion"] = json!("2.0.0");
    cases.push((
        "runtime mismatch",
        with(update_json(), body_json(&m)),
        "runtime_version_mismatch",
    ));
    let mut m = good.clone();
    m["id"] = json!("not-a-uuid");
    cases.push((
        "bad id",
        with(update_json(), body_json(&m)),
        "bad_manifest_id",
    ));
    let mut m = good.clone();
    m.as_object_mut().unwrap().remove("launchAsset");
    cases.push((
        "no launch asset",
        with(update_json(), body_json(&m)),
        "bad_manifest",
    ));
    cases.push((
        "bad runtime",
        with(
            descriptor("update", "android", "../etc")
                .to_string()
                .into_bytes(),
            body_json(&good),
        ),
        "bad_runtime_version",
    ));
    cases.push((
        "bad platform",
        with(
            descriptor("update", "web", RV).to_string().into_bytes(),
            body_json(&good),
        ),
        "bad_platform",
    ));
    cases.push((
        "signature with a newline",
        with(
            json!({ "kind": "update", "platform": "android", "runtimeVersion": RV, "signature": "a\nb" })
                .to_string()
                .into_bytes(),
            body_json(&good),
        ),
        "bad_signature",
    ));
    cases.push((
        "no update.json",
        tarball(vec![Entry::File("body.json", body_json(&good))]),
        "missing_update_json",
    ));
    cases.push((
        "no body.json",
        tarball(vec![Entry::File("update.json", update_json())]),
        "missing_body_json",
    ));
    let mut traversal = vec![
        Entry::File("update.json", update_json()),
        Entry::File("body.json", body_json(&good)),
        Entry::File("assets/../../escape.txt", b"x".to_vec()),
    ];
    traversal.extend(assets());
    cases.push((
        "path traversal",
        tarball(traversal),
        "unsafe_path_in_archive",
    ));
    cases.push((
        "absolute path",
        tarball(vec![Entry::File("/tmp/qdn-escape.txt", b"x".to_vec())]),
        "unsafe_path_in_archive",
    ));
    cases.push((
        "symlink",
        tarball(vec![Entry::Symlink("assets/bundle-abc.hbc", "/etc/passwd")]),
        "link_in_archive",
    ));
    cases.push(("not gzip", b"plain bytes".to_vec(), "bad_archive"));
    cases.push((
        "rollback without commitTime",
        directive_archive(RV, "android", &json!({ "type": "rollBackToEmbedded" })),
        "missing_commit_time",
    ));
    cases.push((
        "unknown directive",
        directive_archive(RV, "android", &json!({ "type": "explode" })),
        "bad_directive_type",
    ));

    for (what, archive, code) in cases {
        let (status, body) = publish(&app, Some(TOKEN), archive).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{what}: {body}");
        assert_eq!(body["error"], code, "{what}");
        assert!(!leftover_tmp(&dir), "{what}");
    }
    assert!(!std::env::temp_dir().join("qdn-escape.txt").exists());
    assert_eq!(served(&app, RV, "android").await, None);
    assert_eq!(
        admin(&app, "GET", "/updates", Some(TOKEN)).await.1,
        json!([])
    );

    // The good one still goes through.
    let (status, _) = publish(&app, Some(TOKEN), with(update_json(), body_json(&good))).await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn newest_wins_and_only_five_are_kept() {
    let (app, dir) = test_app(Some(TOKEN)).await;
    // An iOS update in the same runtime is counted apart.
    let (archive, _) = update_archive(RV, "ios", &manifest_body(RV, &uuid(100)));
    assert_eq!(
        publish(&app, Some(TOKEN), archive).await.0,
        StatusCode::CREATED
    );
    for n in 1..=7 {
        let (archive, _) = update_archive(RV, "android", &manifest_body(RV, &uuid(n)));
        assert_eq!(
            publish(&app, Some(TOKEN), archive).await.0,
            StatusCode::CREATED
        );
        assert_eq!(served(&app, RV, "android").await, Some(uuid(n)));
    }
    assert_eq!(served(&app, RV, "ios").await, Some(uuid(100)));
    let runtime_dir = dir.join("updates").join(RV);
    for n in 1..=2 {
        assert!(!runtime_dir.join(uuid(n)).exists());
    }
    for n in 3..=7 {
        assert!(runtime_dir.join(uuid(n)).exists());
    }

    let (status, list) = admin(&app, "GET", "/updates", Some(TOKEN)).await;
    assert_eq!(status, StatusCode::OK);
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 6);
    let ids: Vec<&str> = list.iter().map(|e| e["id"].as_str().unwrap()).collect();
    let mut expected: Vec<String> = (3..=7).rev().map(uuid).collect();
    expected.push(uuid(100));
    assert_eq!(ids, expected);
    let current: Vec<&str> = list
        .iter()
        .filter(|e| e["current"] == true)
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert_eq!(current, [uuid(7), uuid(100)]);
    let first = &list[0];
    assert_eq!(first["runtimeVersion"], RV);
    assert_eq!(first["platform"], "android");
    assert_eq!(first["kind"], "update");
    assert_eq!(first["createdAt"], "2026-11-12T10:00:00.000Z");
    assert!(first["publishedAt"].as_u64().unwrap() > list[1]["publishedAt"].as_u64().unwrap());
}

#[tokio::test]
async fn rollback_directive_then_update_then_delete() {
    let (app, _) = test_app(Some(TOKEN)).await;
    let a = uuid(1);
    let (archive, _) = update_archive(RV, "android", &manifest_body(RV, &a));
    publish(&app, Some(TOKEN), archive).await;

    let directive = json!({ "type": "rollBackToEmbedded", "parameters": { "commitTime": "2026-11-12T11:00:00.000Z" } });
    let (status, res) = publish(
        &app,
        Some(TOKEN),
        directive_archive(RV, "android", &directive),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{res}");
    assert_eq!(res["kind"], "directive");
    let directive_id = res["id"].as_str().unwrap().to_owned();
    assert!(directive_id.starts_with("directive-"));

    // Served as a directive part (signed), no extensions part.
    let (status, h, bytes) = manifest(
        &app,
        "/api/updates/manifest",
        &[
            ("expo-runtime-version", RV),
            ("expo-platform", "android"),
            ("expo-current-update-id", &a),
            ("expo-embedded-update-id", "embedded-1"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_protocol_headers(&h);
    let body = directive.to_string().into_bytes();
    assert_eq!(
        bytes.to_vec(),
        expected_multipart(&boundary(&h), "directive", &body, true)
    );
    // Already on the embedded bundle: nothing to do.
    let (status, h, _) = manifest(
        &app,
        "/api/updates/manifest",
        &[
            ("expo-runtime-version", RV),
            ("expo-platform", "android"),
            ("expo-current-update-id", "embedded-1"),
            ("expo-embedded-update-id", "embedded-1"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_protocol_headers(&h);
    // A directive can't be served as plain JSON.
    let (status, _, _) = manifest(
        &app,
        "/api/updates/manifest",
        &[
            ("expo-runtime-version", RV),
            ("expo-platform", "android"),
            ("accept", "application/expo+json"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    // Other platform unaffected.
    assert_eq!(served(&app, RV, "ios").await, None);

    let (_, list) = admin(&app, "GET", "/updates", Some(TOKEN)).await;
    assert_eq!(list[0]["id"], directive_id.as_str());
    assert_eq!(list[0]["kind"], "directive");
    assert_eq!(list[0]["createdAt"], "2026-11-12T11:00:00.000Z");
    assert_eq!(list[0]["current"], true);
    assert_eq!(list[1]["current"], false);

    // A later update supersedes the directive.
    let b = uuid(2);
    let (archive, _) = update_archive(RV, "android", &manifest_body(RV, &b));
    publish(&app, Some(TOKEN), archive).await;
    assert_eq!(served(&app, RV, "android").await, Some(b.clone()));

    // Deleting brings back the previous entry each time.
    let (status, _) = admin(&app, "DELETE", &format!("/updates/{RV}/{b}"), Some(TOKEN)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        served(&app, RV, "android").await,
        Some("rollBackToEmbedded".into())
    );
    let (status, _) = admin(
        &app,
        "DELETE",
        &format!("/updates/{RV}/{directive_id}"),
        Some(TOKEN),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(served(&app, RV, "android").await, Some(a.clone()));
    let (status, _) = admin(&app, "DELETE", &format!("/updates/{RV}/{b}"), Some(TOKEN)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = admin(&app, "DELETE", &format!("/updates/../{b}"), Some(TOKEN)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // noUpdateAvailable: nothing is served.
    let (status, res) = publish(
        &app,
        Some(TOKEN),
        directive_archive(RV, "android", &json!({ "type": "noUpdateAvailable" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(served(&app, RV, "android").await, None);
    let (_, list) = admin(&app, "GET", "/updates", Some(TOKEN)).await;
    assert_eq!(list[0]["id"], res["id"]);
    assert!(list[0]["createdAt"].as_str().unwrap().ends_with('Z'));
}
