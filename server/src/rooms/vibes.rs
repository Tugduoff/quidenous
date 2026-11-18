use std::sync::LazyLock;

use axum::Json;
use serde::{Deserialize, Serialize};

pub(crate) const DEFAULT_TAG: &str = "potes";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Vibe {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub description: String,
    #[serde(skip_serializing)]
    pub draws: Vec<String>,
}

#[allow(clippy::expect_used)]
pub(crate) static VIBES: LazyLock<Vec<Vibe>> = LazyLock::new(|| {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/deck/vibes.json"
    )))
    .expect("deck/vibes.json is valid")
});

pub(crate) fn find(id: &str) -> Option<&'static Vibe> {
    VIBES.iter().find(|v| v.id == id)
}

pub(crate) fn is_tag(tag: &str) -> bool {
    VIBES
        .iter()
        .any(|v| v.id == tag || v.draws.iter().any(|d| d == tag))
}

pub(crate) fn draws(vibe: Option<&str>) -> Vec<String> {
    vibe.and_then(find)
        .map_or_else(|| vec![DEFAULT_TAG.to_owned()], |v| v.draws.clone())
}

pub(crate) fn has_tag(stored: Option<&str>, tag: &str) -> bool {
    match stored {
        Some(s) if !s.is_empty() => s.split(',').any(|t| t == tag),
        _ => tag == DEFAULT_TAG,
    }
}

pub(crate) async fn list() -> Json<&'static [Vibe]> {
    Json(VIBES.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vibes_parse_and_draw_known_tags() {
        assert!(VIBES.len() >= 3);
        for v in VIBES.iter() {
            assert!(!v.draws.is_empty(), "{} draws nothing", v.id);
        }
        assert_eq!(draws(None), vec!["potes"]);
        assert_eq!(draws(Some("nope")), vec!["potes"]);
        assert!(has_tag(None, "potes"));
        assert!(!has_tag(None, "famille"));
        assert!(has_tag(Some("potes,famille"), "famille"));
    }
}
