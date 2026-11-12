//! Room vibes: the tone a room picks when it is created. Each vibe draws the built-in deck
//! questions tagged for it (`[famille] Qui…` in `deck/fr.txt`), in the order of its `draws`
//! list. The list lives in `deck/vibes.json`, so adding a vibe is a data change.

use std::sync::LazyLock;

use axum::Json;
use serde::{Deserialize, Serialize};

/// Untagged deck lines, and rooms without a vibe, are "between friends".
pub const DEFAULT_TAG: &str = "potes";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vibe {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub description: String,
    /// Deck tags this vibe asks from, favourite first. Not sent to the app.
    #[serde(skip_serializing)]
    pub draws: Vec<String>,
}

pub static VIBES: LazyLock<Vec<Vibe>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../deck/vibes.json")).expect("deck/vibes.json is valid")
});

pub fn find(id: &str) -> Option<&'static Vibe> {
    VIBES.iter().find(|v| v.id == id)
}

/// A tag a deck line may carry: every vibe id and every tag a vibe draws from.
pub fn is_tag(tag: &str) -> bool {
    VIBES
        .iter()
        .any(|v| v.id == tag || v.draws.iter().any(|d| d == tag))
}

/// Deck tags a room asks from, favourite first.
pub fn draws(vibe: Option<&str>) -> Vec<String> {
    vibe.and_then(find)
        .map(|v| v.draws.clone())
        .unwrap_or_else(|| vec![DEFAULT_TAG.to_owned()])
}

/// The tags stored on a deck question (`potes,famille`); none stored means the default.
pub fn has_tag(stored: Option<&str>, tag: &str) -> bool {
    match stored {
        Some(s) if !s.is_empty() => s.split(',').any(|t| t == tag),
        _ => tag == DEFAULT_TAG,
    }
}

pub async fn list() -> Json<&'static [Vibe]> {
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
