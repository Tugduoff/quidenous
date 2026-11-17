//! Bakes what `GET /api/version` reports into the binary, so every build path (Docker, a plain
//! `cargo build` on the production host, CI) says which release and commit it is:
//! - version: `QUIDENOUS_VERSION` at build time, else `expo.version` in `../app/app.json` (a tag's
//!   app.json holds the released version);
//! - sha: `QUIDENOUS_GIT_SHA` at build time, else `git rev-parse HEAD` in this checkout.
//!
//! Missing ones are left out (`ops.rs` falls back to the runtime environment, then `dev`).

use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("set by cargo"));
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=QUIDENOUS_VERSION");
    println!("cargo:rerun-if-env-changed=QUIDENOUS_GIT_SHA");

    let app_json = manifest.join("../app/app.json");
    // Only existing paths: cargo reruns the script on every build for a missing one.
    if app_json.exists() {
        println!("cargo:rerun-if-changed={}", app_json.display());
    }
    let version = build_env("QUIDENOUS_VERSION").or_else(|| {
        std::fs::read_to_string(&app_json)
            .ok()
            .and_then(|json| expo_version(&json))
    });
    if let Some(v) = version {
        let v = v.trim().trim_start_matches('v');
        if !v.is_empty() {
            println!("cargo:rustc-env=QDN_BUILD_VERSION={v}");
        }
    }

    let sha = build_env("QUIDENOUS_GIT_SHA").or_else(|| git_sha(&manifest));
    if let Some(sha) = sha {
        println!("cargo:rustc-env=QDN_BUILD_SHA={sha}");
    }
}

/// Set, not blank and not the `dev` placeholder.
fn build_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty() && v != "dev")
}

/// `git <args>` in `dir`, its trimmed output when it succeeds. The checkout may belong to another
/// user (a root build of a deploy checkout): we already run its build script, so it is trusted.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(["-c", "safe.directory=*"])
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?.trim().to_owned();
    (!s.is_empty()).then_some(s)
}

/// HEAD's commit; also tells cargo to rerun when HEAD moves (checkout, commit, pull), so an
/// incremental build never reports a stale commit. No git or not a repository: `None`.
fn git_sha(dir: &Path) -> Option<String> {
    let sha = git(dir, &["rev-parse", "HEAD"])?;
    if sha.len() != 40 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    // `--git-path` resolves worktrees (HEAD is per worktree, refs and packed-refs are shared).
    let git_path = |name: &str| {
        git(dir, &["rev-parse", "--git-path", name]).map(|p| {
            let p = PathBuf::from(p);
            if p.is_absolute() { p } else { dir.join(p) }
        })
    };
    let head = git_path("HEAD");
    for path in [&head, &git_path("packed-refs")].into_iter().flatten() {
        if path.exists() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    // On a branch, the branch's ref moves on commit while HEAD's file doesn't change.
    if let Some(head) = head.and_then(|p| std::fs::read_to_string(p).ok())
        && let Some(name) = head.trim().strip_prefix("ref:")
        && let Some(branch) = git_path(name.trim())
        && let Some(refs) = git_path("refs")
    {
        // A ref only in packed-refs has no file yet: watch the closest folder under refs/ where
        // it will appear (never the whole git folder, which changes all the time).
        if let Some(existing) = branch
            .ancestors()
            .find(|p| p.starts_with(&refs) && p.exists())
        {
            println!("cargo:rerun-if-changed={}", existing.display());
        }
    }
    Some(sha)
}

/// `expo.version` in app.json: a small scan of strings and nesting, enough for a JSON file
/// without pulling a JSON crate into the build.
fn expo_version(json: &str) -> Option<String> {
    // Key that opened each open object or array (`None` for the root and array items).
    let mut path: Vec<Option<String>> = Vec::new();
    let mut key: Option<String> = None;
    let mut after_colon = false;
    let mut chars = json.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                let s = read_string(&mut chars)?;
                if after_colon {
                    let in_expo = path.len() == 2 && path[1].as_deref() == Some("expo");
                    if in_expo && key.as_deref() == Some("version") {
                        return Some(s);
                    }
                    after_colon = false;
                } else {
                    key = Some(s);
                }
            }
            ':' => after_colon = true,
            '{' | '[' => {
                path.push(if after_colon { key.take() } else { None });
                after_colon = false;
            }
            '}' | ']' => {
                path.pop();
                after_colon = false;
            }
            ',' => after_colon = false,
            _ => {}
        }
    }
    None
}

/// The rest of a string whose opening quote was read (escapes kept as written but `\"`).
fn read_string(chars: &mut std::str::Chars) -> Option<String> {
    let mut s = String::new();
    loop {
        match chars.next()? {
            '"' => return Some(s),
            '\\' => {
                let c = chars.next()?;
                if c != '"' && c != '\\' {
                    s.push('\\');
                }
                s.push(c);
            }
            c => s.push(c),
        }
    }
}
