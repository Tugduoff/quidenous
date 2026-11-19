use crate::platform::ids::RoomId;
use sqlx::{Sqlite, Transaction};

use crate::platform::error::{AppError, AppResult};

pub const THEMES: [&str; 12] = [
    "lime", "rose", "bleu", "orange", "violet", "menthe", "apero", "feu", "karaoke", "pyjama",
    "arcade", "piscine",
];

pub(crate) fn clean_theme(raw: &str) -> AppResult<Option<&'static str>> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    THEMES
        .iter()
        .find(|t| **t == raw)
        .copied()
        .map(Some)
        .ok_or(AppError::BadRequest("bad_setting"))
}

pub(crate) async fn set(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: RoomId,
    theme: Option<&str>,
) -> AppResult<()> {
    sqlx::query("UPDATE groups SET theme = ? WHERE id = ?")
        .bind(theme)
        .bind(group_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themes() {
        assert_eq!(THEMES.len(), 12);
        assert_eq!(clean_theme("").unwrap(), None);
        assert_eq!(clean_theme("  ").unwrap(), None);
        assert_eq!(clean_theme("piscine").unwrap(), Some("piscine"));
        assert_eq!(clean_theme("lime").unwrap(), Some("lime"));
        for bad in ["Lime", "noir", "winter", "lime-2"] {
            assert!(clean_theme(bad).is_err(), "{bad}");
        }
    }
}
