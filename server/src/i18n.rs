//! The two languages the server writes in (French, the source, and English), and every
//! user-facing text it sends: push notifications, inbox entries, account e-mails, share cards.
//! A person's language is `users.locale` (null = French, the language of accounts made before
//! this existed). Contract: `docs/I18N.md`.

use axum::{Json, extract::State};
use serde::Serialize;
use sqlx::SqliteExecutor;

use crate::{
    AppState,
    auth::UserId,
    error::{AppError, AppResult},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Lang {
    #[default]
    Fr,
    En,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::Fr, Lang::En];

    pub fn as_str(self) -> &'static str {
        match self {
            Lang::Fr => "fr",
            Lang::En => "en",
        }
    }

    /// `"fr"` / `"en"`; anything else is not a language we speak.
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.as_str() == s)
    }

    /// A stored language: missing or unknown means French.
    pub fn from_db(s: Option<&str>) -> Self {
        s.and_then(Self::parse).unwrap_or_default()
    }

    /// The French or the English one.
    pub fn pick<T>(self, fr: T, en: T) -> T {
        match self {
            Lang::Fr => fr,
            Lang::En => en,
        }
    }
}

/// `GET/PUT /api/me/locale`: the account's language, and the Profile setting it came from.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Locale {
    /// `fr` | `en`: the language the app shows, the one the server writes in. Null = never set (French).
    pub locale: Option<String>,
    /// `auto` | `fr` | `en`. Null = never set.
    pub pref: Option<String>,
}

pub async fn get_locale(
    State(state): State<AppState>,
    UserId(user): UserId,
) -> AppResult<Json<Locale>> {
    let row: Locale = sqlx::query_as("SELECT locale, locale_pref AS pref FROM users WHERE id = ?")
        .bind(user)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}

pub async fn put_locale(
    State(state): State<AppState>,
    UserId(user): UserId,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<Locale>> {
    let locale = body["locale"].as_str().and_then(Lang::parse);
    let pref = body["pref"]
        .as_str()
        .filter(|p| *p == "auto" || Lang::parse(p).is_some());
    let (Some(locale), Some(pref)) = (locale, pref) else {
        return Err(AppError::BadRequest("bad_locale"));
    };
    sqlx::query("UPDATE users SET locale = ?, locale_pref = ? WHERE id = ?")
        .bind(locale.as_str())
        .bind(pref)
        .bind(user)
        .execute(&state.db)
        .await?;
    Ok(Json(Locale {
        locale: Some(locale.as_str().to_owned()),
        pref: Some(pref.to_owned()),
    }))
}

/// An account's language.
pub async fn of_user<'e, E: SqliteExecutor<'e>>(db: E, user_id: i64) -> sqlx::Result<Lang> {
    let locale: Option<Option<String>> =
        sqlx::query_scalar("SELECT locale FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(db)
            .await?;
    Ok(Lang::from_db(locale.flatten().as_deref()))
}

/// The language of the account behind a room seat.
pub async fn of_member<'e, E: SqliteExecutor<'e>>(db: E, member_id: i64) -> sqlx::Result<Lang> {
    let locale: Option<Option<String>> = sqlx::query_scalar(
        "SELECT u.locale FROM members m JOIN users u ON u.id = m.user_id WHERE m.id = ?",
    )
    .bind(member_id)
    .fetch_optional(db)
    .await?;
    Ok(Lang::from_db(locale.flatten().as_deref()))
}

/// A text in both languages; the reader's language picks one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    pub fr: String,
    pub en: String,
}

impl Text {
    pub fn new(fr: impl Into<String>, en: impl Into<String>) -> Self {
        Self {
            fr: fr.into(),
            en: en.into(),
        }
    }

    /// Words of the users themselves (a question, a message): the same in every language.
    pub fn same(s: impl Into<String>) -> Self {
        let s = s.into();
        Self {
            fr: s.clone(),
            en: s,
        }
    }

    /// Built per language.
    pub fn by(f: impl Fn(Lang) -> String) -> Self {
        Self {
            fr: f(Lang::Fr),
            en: f(Lang::En),
        }
    }

    pub fn get(&self, lang: Lang) -> &str {
        match lang {
            Lang::Fr => &self.fr,
            Lang::En => &self.en,
        }
    }
}

// ---------- push and inbox ----------

pub fn result_and_question(lang: Lang) -> &'static str {
    lang.pick(
        "Résultat dispo · nouvelle question",
        "Results are in · new question",
    )
}

pub fn result_ready(lang: Lang) -> &'static str {
    lang.pick("Résultat dispo", "Results are in")
}

pub fn everyone_voted(lang: Lang) -> &'static str {
    lang.pick("Tout le monde a voté.", "Everyone has voted.")
}

pub fn new_question(lang: Lang) -> &'static str {
    lang.pick("Nouvelle question", "New question")
}

/// Chat push title: `Ana · « Who…? »`.
pub fn chat_title(lang: Lang, name: &str, question: &str) -> String {
    match lang {
        Lang::Fr => format!("{name} · « {question} »"),
        Lang::En => format!("{name} · “{question}”"),
    }
}

pub fn mention_title(lang: Lang, name: &str, question: &str) -> String {
    match lang {
        Lang::Fr => format!("{name} t'a mentionné · « {question} »"),
        Lang::En => format!("{name} mentioned you · “{question}”"),
    }
}

pub fn nudge(lang: Lang, name: &str) -> String {
    match lang {
        Lang::Fr => format!("{name} attend ton vote"),
        Lang::En => format!("{name} is waiting for your vote"),
    }
}

pub fn ask_questions(lang: Lang, name: &str) -> String {
    match lang {
        Lang::Fr => format!("{name} demande des questions"),
        Lang::En => format!("{name} is asking for questions"),
    }
}

pub fn out_of_questions(lang: Lang, room: &str) -> String {
    match lang {
        Lang::Fr => format!("Plus de questions dans « {room} »."),
        Lang::En => format!("No questions left in “{room}”."),
    }
}

pub fn new_room(lang: Lang) -> &'static str {
    lang.pick("Nouveau salon", "New room")
}

pub fn added_to_room(lang: Lang, name: &str, room: &str) -> String {
    match lang {
        Lang::Fr => format!("{name} t'a ajouté à « {room} »"),
        Lang::En => format!("{name} added you to “{room}”"),
    }
}

// ---------- member names ----------

/// What a deleted account is called. The database keeps the French placeholder
/// (`accounts::delete::DELETED_NAME`); texts for a reader use their language.
pub fn deleted_name(lang: Lang) -> &'static str {
    lang.pick(crate::accounts::delete::DELETED_NAME, "Deleted account")
}

/// Is this seat the leftover of a deleted account? (anonymised: no account, the placeholder name)
pub fn is_deleted(name: &str, user_id: Option<i64>) -> bool {
    user_id.is_none() && name == crate::accounts::delete::DELETED_NAME
}

/// A member's name as shown to a reader: a deleted account's placeholder in the reader's language.
pub fn member_name(lang: Lang, name: &str, user_id: Option<i64>) -> String {
    if is_deleted(name, user_id) {
        deleted_name(lang).to_owned()
    } else {
        name.to_owned()
    }
}

// ---------- party ----------

/// The name of a party created without one.
pub fn party_default_name(lang: Lang, host: &str) -> String {
    match lang {
        Lang::Fr => format!("Soirée de {host}"),
        Lang::En => format!("{host}'s party"),
    }
}

// ---------- history export (CSV) ----------

pub fn csv_header(lang: Lang) -> &'static str {
    lang.pick(
        "numéro,date,question,gagnant(s),score du gagnant,votes,détail",
        "number,date,question,winner(s),winner's score,votes,details",
    )
}

/// `Ana (2) : Bob, Carl`: who got how many votes, from whom.
pub fn csv_votes(lang: Lang, name: &str, count: i64, voters: &str) -> String {
    match lang {
        Lang::Fr => format!("{name} ({count}) : {voters}"),
        Lang::En => format!("{name} ({count}): {voters}"),
    }
}

pub fn csv_disposition(lang: Lang) -> &'static str {
    lang.pick(
        "attachment; filename=\"qui-de-nous-historique.csv\"",
        "attachment; filename=\"qui-de-nous-history.csv\"",
    )
}

// ---------- account e-mails ----------

/// Subject, heading, intro and outro of a code e-mail.
pub struct CodeMail {
    pub subject: String,
    pub heading: &'static str,
    pub intro: &'static str,
    pub outro: &'static str,
}

pub fn verify_email(lang: Lang, code: &str) -> CodeMail {
    match lang {
        Lang::Fr => CodeMail {
            subject: format!("{code} · ton code Qui de nous ?"),
            heading: "Vérifie ton adresse",
            intro: "Voici ton code pour vérifier ton adresse e-mail sur Qui de nous ?",
            outro: "Il expire dans 15 minutes. Si tu n'as rien demandé, ignore cet e-mail.",
        },
        Lang::En => CodeMail {
            subject: format!("{code} · your Qui de nous ? code"),
            heading: "Verify your e-mail",
            intro: "Here is your code to verify your e-mail address on Qui de nous ?",
            outro: "It expires in 15 minutes. If you didn't ask for it, ignore this e-mail.",
        },
    }
}

pub fn reset_password(lang: Lang, code: &str) -> CodeMail {
    match lang {
        Lang::Fr => CodeMail {
            subject: format!("{code} · nouveau mot de passe Qui de nous ?"),
            heading: "Mot de passe oublié",
            intro: "Voici ton code pour choisir un nouveau mot de passe sur Qui de nous ?",
            outro: "Il expire dans 30 minutes. Si tu n'as rien demandé, ignore cet e-mail : ton mot de passe ne change pas.",
        },
        Lang::En => CodeMail {
            subject: format!("{code} · new Qui de nous ? password"),
            heading: "Forgot your password",
            intro: "Here is your code to choose a new password on Qui de nous ?",
            outro: "It expires in 30 minutes. If you didn't ask for it, ignore this e-mail: your password stays the same.",
        },
    }
}

pub fn delete_account(lang: Lang, code: &str) -> CodeMail {
    match lang {
        Lang::Fr => CodeMail {
            subject: format!("{code} · supprimer ton compte Qui de nous ?"),
            heading: "Supprimer ton compte",
            intro: "Voici ton code pour supprimer ton compte Qui de nous ?",
            outro: "Il expire dans 30 minutes. La suppression est définitive. Si tu n'as rien demandé, ignore cet e-mail : ton compte reste tel quel.",
        },
        Lang::En => CodeMail {
            subject: format!("{code} · delete your Qui de nous ? account"),
            heading: "Delete your account",
            intro: "Here is your code to delete your Qui de nous ? account.",
            outro: "It expires in 30 minutes. Deletion is permanent. If you didn't ask for it, ignore this e-mail: your account stays as it is.",
        },
    }
}

// ---------- share cards and pages ----------

pub fn card_tag_rank(lang: Lang, number: i64) -> String {
    format!("{} · #{number}", lang.pick("CLASSEMENT", "RANKING"))
}

pub fn card_tag_question(lang: Lang, number: i64) -> String {
    format!("{} · #{number}", lang.pick("QUESTION", "QUESTION"))
}

pub fn card_tag_result(lang: Lang, number: i64) -> String {
    format!("{} · #{number}", lang.pick("RÉSULTAT", "RESULT"))
}

pub fn card_footer(lang: Lang) -> &'static str {
    lang.pick(
        "Joue avec tes potes sur qui de nous ?",
        "Play with your friends on qui de nous ?",
    )
}

/// A ranking score on the podium (`pts` in both languages).
pub fn points(points: i64) -> String {
    format!("{points} pts")
}

/// A share of the votes on the podium.
pub fn percent(lang: Lang, pc: i64) -> String {
    match lang {
        Lang::Fr => format!("{pc} %"),
        Lang::En => format!("{pc}%"),
    }
}

pub fn share_question_desc(lang: Lang, number: i64, room: &str) -> String {
    match lang {
        Lang::Fr => format!("Question #{number} dans « {room} ». Et toi, tu votes pour qui ?"),
        Lang::En => format!("Question #{number} in “{room}”. Who would you vote for?"),
    }
}

pub fn share_result_desc(lang: Lang, names: &str) -> String {
    match lang {
        Lang::Fr => format!("Résultat : {names}"),
        Lang::En => format!("Result: {names}"),
    }
}

pub fn share_open(lang: Lang) -> &'static str {
    lang.pick("Ouvrir qui de nous ?", "Open qui de nous ?")
}

pub fn share_create(lang: Lang) -> &'static str {
    lang.pick(
        "Pas encore de salon ? Crée le tien, c'est gratuit",
        "No room yet? Create yours, it's free",
    )
}

pub fn share_expired(lang: Lang) -> &'static str {
    lang.pick("Lien expiré", "Link expired")
}

/// Primary button of a shared question's page (`/play?share=`).
pub fn share_play(lang: Lang) -> &'static str {
    lang.pick(
        "Jouer cette question avec tes potes",
        "Play this question with your friends",
    )
}

// ---------- pack pages (`/p/{id}`) ----------

pub fn card_tag_pack(lang: Lang, count: i64) -> String {
    format!("PACK · {count} {}", lang.pick("QUESTIONS", "QUESTIONS"))
}

pub fn pack_by(lang: Lang, author: &str) -> String {
    match lang {
        Lang::Fr => format!("par {author}"),
        Lang::En => format!("by {author}"),
    }
}

pub fn pack_desc(lang: Lang, count: i64, author: &str) -> String {
    match lang {
        Lang::Fr => format!("{count} questions par {author}"),
        Lang::En => format!("{count} questions by {author}"),
    }
}

pub fn pack_install(lang: Lang) -> &'static str {
    lang.pick("Installer dans un salon", "Install in a room")
}

pub fn pack_missing(lang: Lang) -> &'static str {
    lang.pick("Pack introuvable", "Pack not found")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_languages_default_to_french() {
        assert_eq!(Lang::from_db(None), Lang::Fr);
        assert_eq!(Lang::from_db(Some("en")), Lang::En);
        assert_eq!(Lang::from_db(Some("de")), Lang::Fr);
        assert_eq!(Lang::parse("EN"), None);
        let t = Text::by(|l| nudge(l, "Ana"));
        assert_eq!(t.get(Lang::Fr), "Ana attend ton vote");
        assert_eq!(t.get(Lang::En), "Ana is waiting for your vote");
    }
}
