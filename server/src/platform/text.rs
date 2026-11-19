use crate::platform::error::{AppError, AppResult};

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

fn blank(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\u{200C}' | '\u{200D}' | '\u{FE00}'..='\u{FE0F}')
}

pub(crate) fn line(s: &str) -> String {
    let kept: String = s.chars().filter(|c| !hidden(*c)).collect();
    let out = kept.split_whitespace().collect::<Vec<_>>().join(" ");
    if out.chars().all(blank) {
        String::new()
    } else {
        out
    }
}

pub(crate) fn lines(s: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for l in s.lines() {
        let l = line(l);
        if l.is_empty() && out.last().is_none_or(std::string::String::is_empty) {
            continue;
        }
        out.push(l);
    }
    while out.last().is_some_and(std::string::String::is_empty) {
        out.pop();
    }
    out.join("\n")
}

pub(crate) fn key(s: &str) -> String {
    line(s).to_lowercase()
}

const NAME_MAX: usize = 24;

pub(crate) fn clean_name(name: &str) -> AppResult<String> {
    let name = crate::platform::text::line(name);
    if name.is_empty() || name.chars().count() > NAME_MAX {
        return Err(AppError::BadRequest("bad_name"));
    }
    Ok(name)
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
