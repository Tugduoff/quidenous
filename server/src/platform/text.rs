//! What people type, made safe to show: names, room names, questions and chat messages lose
//! invisible characters (zero-width spaces, bidi overrides, control characters) and runs of
//! blank space, so nothing shows up empty, reversed or a screen tall.

/// Characters that are never shown on their own and only serve to hide or flip text: zero-width
/// spaces and marks, bidi embeddings/overrides/isolates, word joiners, the BOM, soft hyphens,
/// Hangul fillers and the blank Braille pattern (both used for "empty" names). The zero-width
/// joiner and non-joiner stay (emoji sequences, Persian), as do variation selectors.
fn hidden(c: char) -> bool {
    matches!(c,
        '\u{00AD}' | '\u{061C}' | '\u{115F}' | '\u{1160}' | '\u{180E}'
        | '\u{200B}' | '\u{200E}' | '\u{200F}'
        | '\u{202A}'..='\u{202E}'
        | '\u{2060}'..='\u{2064}'
        | '\u{2066}'..='\u{2069}'
        | '\u{2800}' | '\u{3164}' | '\u{FEFF}' | '\u{FFA0}'
        | '\u{FFF9}'..='\u{FFFB}'
    ) || (c.is_control() && !c.is_whitespace())
}

/// Shows nothing by itself (joiners and variation selectors only make sense next to something).
fn blank(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\u{200C}' | '\u{200D}' | '\u{FE00}'..='\u{FE0F}')
}

/// One line: hidden characters dropped, any run of white space (line breaks included) becomes
/// one space, trimmed. Empty when nothing visible is left.
pub fn line(s: &str) -> String {
    let kept: String = s.chars().filter(|c| !hidden(*c)).collect();
    let out = kept.split_whitespace().collect::<Vec<_>>().join(" ");
    if out.chars().all(blank) {
        String::new()
    } else {
        out
    }
}

/// Several lines (a chat message): each line cleaned like [`line`], at most one blank line in a
/// row, trimmed. Empty when nothing visible is left.
pub fn lines(s: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for l in s.lines() {
        let l = line(l);
        if l.is_empty() && out.last().is_none_or(|p| p.is_empty()) {
            continue;
        }
        out.push(l);
    }
    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out.join("\n")
}

/// Compares two texts the way people read them: case-insensitive beyond ASCII ("ÉTÉ" = "été"),
/// invisible characters and extra spaces ignored.
pub fn key(s: &str) -> String {
    line(s).to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invisible_and_flipped_text_is_dropped() {
        assert_eq!(line("\u{200B}\u{200B}\u{200B}"), "");
        assert_eq!(line(" \u{3164} "), "");
        assert_eq!(line("\u{200D}\u{FE0F}"), "");
        assert_eq!(line("Lé\u{202E}a"), "Léa");
        assert_eq!(line("a\u{0007}b"), "ab");
        assert_eq!(
            line("  Qui   est\n\n\n le plus\tdrôle ?  "),
            "Qui est le plus drôle ?"
        );
        // Emoji sequences keep their joiners.
        assert_eq!(line("👨\u{200D}👩\u{200D}👧"), "👨\u{200D}👩\u{200D}👧");
    }

    #[test]
    fn messages_keep_one_blank_line_at_most() {
        assert_eq!(lines("\n\nsalut\n\n\n\n\nça va ?\n\n"), "salut\n\nça va ?");
        assert_eq!(lines("a\nb"), "a\nb");
        assert_eq!(lines("\u{200B}\n\u{202E}\n"), "");
    }

    #[test]
    fn keys_fold_case_beyond_ascii() {
        assert_eq!(key("ÉTÉ  à la PLAGE"), key("été à la plage"));
        assert_ne!(key("été"), key("ete"));
    }
}
