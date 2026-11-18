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

fn build_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty() && v != "dev")
}

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

fn git_sha(dir: &Path) -> Option<String> {
    let sha = git(dir, &["rev-parse", "HEAD"])?;
    if sha.len() != 40 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
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
    if let Some(head) = head.and_then(|p| std::fs::read_to_string(p).ok())
        && let Some(name) = head.trim().strip_prefix("ref:")
        && let Some(branch) = git_path(name.trim())
        && let Some(refs) = git_path("refs")
        && let Some(existing) = branch
            .ancestors()
            .find(|p| p.starts_with(&refs) && p.exists())
    {
        println!("cargo:rerun-if-changed={}", existing.display());
    }
    Some(sha)
}

fn expo_version(json: &str) -> Option<String> {
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
