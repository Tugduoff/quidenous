use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::Me,
    platform::error::{AppError, AppResult},
    platform::i18n::{self, Text},
    realtime::live::Event,
    realtime::push::{Kind, Notification},
    rooms::model::MemberRow,
    rounds::game,
};

/// The columns of a [`MessageDb`], for `SELECT`s on `messages x JOIN members m`.
macro_rules! message_columns {
    () => {
        "x.id, x.member_id, m.user_id, x.text, x.created_at, x.mentions, x.everyone, x.kind, x.caption, x.media, \
         x.hidden_at IS NOT NULL AS hidden"
    };
}

pub(crate) const MESSAGE_MAX: usize = 500;

/// More people than this named in one message counts as @tous.
const MENTIONS_MAX: usize = 5;

/// `@tous` pings the whole room at most this often per person (the message still goes out).
const EVERYONE_COOLDOWN: &str = "-10 minutes";

#[derive(sqlx::FromRow)]
pub(crate) struct MessageDb {
    id: i64,
    member_id: i64,
    user_id: Option<i64>,
    text: String,
    created_at: String,
    mentions: String,
    everyone: bool,
    kind: String,
    caption: Option<String>,
    /// As stored (`crate::chat::media`): turned into short-lived URLs for the reader.
    media: Option<String>,
    /// Hidden by reports (only its room owner gets it, to restore it).
    #[sqlx(default)]
    hidden: bool,
}

#[derive(Serialize)]
pub(crate) struct MessageRow {
    pub(crate) id: i64,
    member_id: i64,
    /// The author's account (null once deleted): the app hides people it blocked.
    user_id: Option<i64>,
    /// The message; for an image or a GIF, a fallback older apps show ("📷 Photo · caption").
    text: String,
    created_at: String,
    /// Members of the room this message mentions (`@Name` in the text or caption).
    mentions: Vec<i64>,
    /// `@tous`: the whole room.
    everyone: bool,
    /// `text`, `image` or `gif`.
    kind: String,
    /// Image and GIF: the author's words under it.
    caption: Option<String>,
    /// Image and GIF: where to load it (`crate::chat::media::payload`), null for text.
    media: Option<Value>,
    /// Emojis on it, as the reader sees them (`crate::chat::reactions::for_messages`).
    reactions: Vec<crate::chat::reactions::Reaction>,
    /// Hidden by reports: only the room owner gets it (`true`), and may restore it.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    hidden: bool,
}

impl MessageRow {
    fn new(state: &AppState, m: MessageDb) -> Self {
        let mut media = crate::chat::media::payload(state, m.id, &m.kind, m.media.as_deref());
        // A hidden photo loads for its owner only through a review link.
        if let (true, Some(media)) = (m.hidden && m.kind == "image", media.as_mut()) {
            media["url"] = json!(crate::chat::media::image_url(state, m.id, true));
        }
        Self {
            id: m.id,
            member_id: m.member_id,
            user_id: m.user_id,
            text: m.text,
            created_at: m.created_at,
            mentions: serde_json::from_str(&m.mentions).unwrap_or_default(),
            everyone: m.everyone,
            kind: m.kind,
            caption: m.caption,
            media,
            reactions: Vec::new(),
            hidden: m.hidden,
        }
    }
}

/// The most recent messages of a chat the app gets (older ones are left out).
const MESSAGES_SHOWN: i64 = 200;

/// The chat under a finished question, oldest first (its last `MESSAGES_SHOWN`), without people I
/// blocked. Messages hidden by reports only for the room owner (`hidden: true`), who may restore them.
pub(crate) async fn messages(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
) -> AppResult<Json<Vec<MessageRow>>> {
    game::round(&state.db, me.group_id, id).await?;
    let rows: Vec<MessageDb> = sqlx::query_as(concat!(
        "SELECT ",
        message_columns!(),
        " FROM messages x \
         JOIN members m ON m.id = x.member_id WHERE x.round_id = ? AND (x.hidden_at IS NULL OR ?) \
         AND NOT EXISTS (SELECT 1 FROM blocks b WHERE b.user_id = ? AND b.blocked_id = m.user_id) \
         ORDER BY x.id DESC LIMIT ?"
    ))
    .bind(id)
    .bind(me.role == "owner")
    .bind(me.user_id)
    .bind(MESSAGES_SHOWN)
    .fetch_all(&state.db)
    .await?;
    let ids: Vec<i64> = rows.iter().map(|m| m.id).collect();
    let mut reactions = crate::chat::reactions::for_messages(&state.db, &me, &ids).await?;
    Ok(Json(
        rows.into_iter()
            .rev()
            .map(|m| {
                let mut row = MessageRow::new(&state, m);
                row.reactions = reactions.remove(&row.id).unwrap_or_default();
                row
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
pub(crate) struct MessageBody {
    text: String,
    /// Member ids the text mentions; only current members of the room other than me are kept.
    #[serde(default)]
    mentions: Vec<i64>,
    #[serde(default)]
    everyone: bool,
    /// The app's own id for this message (up to 64 chars): a retry with the same key gets the
    /// message already posted instead of a second one.
    #[serde(default)]
    client_key: Option<String>,
}

/// The message this seat already posted with this client key, if any.
pub(crate) async fn posted_message(
    state: &AppState,
    member_id: i64,
    key: &str,
) -> AppResult<Option<MessageRow>> {
    Ok(sqlx::query_as::<_, MessageDb>(concat!(
        "SELECT ",
        message_columns!(),
        " FROM messages x JOIN members m ON m.id = x.member_id WHERE x.member_id = ? AND x.client_key = ?"
    ))
    .bind(member_id)
    .bind(key)
    .fetch_optional(&state.db)
    .await?
    .map(|m| MessageRow::new(state, m)))
}

/// The app's own id for a message (`client_key`): none if empty, `400 bad_message` past 64 chars.
pub(crate) fn client_key(raw: Option<&str>) -> AppResult<Option<String>> {
    let key = raw.filter(|k| !k.is_empty());
    if key.is_some_and(|k| k.len() > 64) {
        return Err(AppError::BadRequest("bad_message"));
    }
    Ok(key.map(str::to_owned))
}

/// A retry of a send whose answer was lost (same client key): the message already posted.
pub(crate) async fn retried(
    state: &AppState,
    me: &MemberRow,
    key: Option<&str>,
) -> AppResult<Option<MessageRow>> {
    match key {
        Some(key) => posted_message(state, me.id, key).await,
        None => Ok(None),
    }
}

pub(crate) async fn post_message(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
    Json(body): Json<MessageBody>,
) -> AppResult<Json<MessageRow>> {
    let round = chat_round(&state, &me, id).await?;
    // Hidden characters dropped, one blank line in a row at most.
    let text = crate::platform::text::lines(&body.text);
    if text.is_empty() || text.chars().count() > MESSAGE_MAX {
        return Err(AppError::BadRequest("bad_message"));
    }
    let key = client_key(body.client_key.as_deref())?;
    // A retry of a send whose answer was lost: the same message, posted once.
    if let Some(posted) = retried(&state, &me, key.as_deref()).await? {
        return Ok(Json(posted));
    }
    chat_quota(&state, &me)?;
    let message = NewMessage {
        kind: "text",
        text,
        caption: None,
        media: None,
        mentions: body.mentions,
        everyone: body.everyone,
        client_key: key,
    };
    Ok(Json(post_chat(&state, &me, &round, message).await?.row))
}

/// The finished question of my room a chat message goes under: `404` if not mine, `400 round_open`
/// while it's open (votes stay secret until the result).
pub(crate) async fn chat_round(
    state: &AppState,
    me: &MemberRow,
    id: i64,
) -> AppResult<crate::rounds::model::RoundRow> {
    let round = game::round(&state.db, me.group_id, id).await?;
    if round.closed_at.is_none() {
        return Err(AppError::BadRequest("round_open"));
    }
    Ok(round)
}

/// The chat limits every message (text, image, GIF) counts against.
pub(crate) fn chat_quota(state: &AppState, me: &MemberRow) -> AppResult<()> {
    if let Some(user) = me.user_id {
        crate::platform::quotas::hit_user(state, &crate::platform::quotas::CHAT_USER, user)?;
        crate::platform::quotas::hit_user(state, &crate::platform::quotas::CHAT_HOUR_USER, user)?;
    }
    Ok(())
}

/// A caption under an image or a GIF: normalised like a text message (`crate::platform::text::lines`),
/// none if empty, `400 bad_caption` if too long.
pub(crate) fn clean_caption(raw: Option<&str>) -> AppResult<Option<String>> {
    let caption = raw
        .map(crate::platform::text::lines)
        .filter(|c| !c.is_empty());
    if caption
        .as_ref()
        .is_some_and(|c| c.chars().count() > MESSAGE_MAX)
    {
        return Err(AppError::BadRequest("bad_caption"));
    }
    Ok(caption)
}

/// A chat message about to be stored (checked by the caller, quotas taken).
pub(crate) struct NewMessage {
    /// `text`, `image` or `gif`.
    pub kind: &'static str,
    /// The text; for media, the fallback (`crate::chat::media::fallback`) older apps, the inbox and pushes show.
    pub text: String,
    pub caption: Option<String>,
    /// Media JSON as stored (`crate::chat::media`).
    pub media: Option<Value>,
    /// Member ids named in the text or caption; only current members of the room other than me are kept.
    pub mentions: Vec<i64>,
    pub everyone: bool,
    /// Checked by [`client_key`]: a second message with the same key is never stored.
    pub client_key: Option<String>,
}

/// What [`post_chat`] answers.
pub(crate) struct Posted {
    pub row: MessageRow,
    /// False when the same client key landed twice at once: `row` is the first one, nothing was
    /// stored or announced (an uploaded file for this one is the caller's to remove).
    pub fresh: bool,
}

/// Stores a chat message under a finished question and tells the room: inbox entries, pushes
/// (mentions and `@tous` ring through), the live `message_posted`. Answers it as I see it.
pub(crate) async fn post_chat(
    state: &AppState,
    me: &MemberRow,
    round: &crate::rounds::model::RoundRow,
    message: NewMessage,
) -> AppResult<Posted> {
    let id = round.id;
    // Everyone else still in the room: who hears about it, and who can be mentioned.
    let others: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM members WHERE group_id = ? AND removed_at IS NULL AND id != ? ORDER BY id",
    )
    .bind(me.group_id)
    .bind(me.id)
    .fetch_all(&state.db)
    .await?;
    let mut mentions: Vec<i64> = message
        .mentions
        .into_iter()
        .filter(|m| others.contains(m))
        .collect();
    mentions.sort_unstable();
    mentions.dedup();
    // Naming more than a few people is the same as @tous: it shares its cooldown.
    let crowd = mentions.len() > MENTIONS_MAX;
    // @tous rings the whole room once in a while; more often, it's a plain message.
    let rings = (message.everyone || crowd)
        && !sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM messages WHERE member_id = ? \
             AND (everyone = 1 OR json_array_length(mentions) > ?) \
             AND created_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?))",
        )
        .bind(me.id)
        .bind(MENTIONS_MAX as i64)
        .bind(EVERYONE_COOLDOWN)
        .fetch_one(&state.db)
        .await?;
    let ring_everyone = message.everyone && rings;
    // A crowd of mentions during the cooldown: a plain message for everyone.
    if crowd && !rings {
        mentions.clear();
    }
    let inserted: Option<MessageDb> = sqlx::query_as(
        "INSERT INTO messages (round_id, member_id, text, mentions, everyone, kind, caption, media, client_key) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT DO NOTHING \
         RETURNING id, member_id, ? AS user_id, text, created_at, mentions, everyone, kind, caption, media",
    )
    .bind(id)
    .bind(me.id)
    .bind(&message.text)
    .bind(serde_json::to_string(&mentions).unwrap_or_else(|_| "[]".into()))
    .bind(message.everyone)
    .bind(message.kind)
    .bind(&message.caption)
    .bind(message.media.as_ref().map(Value::to_string))
    .bind(&message.client_key)
    .bind(me.user_id)
    .fetch_optional(&state.db)
    .await?;
    let row = match inserted {
        Some(m) => MessageRow::new(state, m),
        // The same key landed twice at once: the first one is the message.
        None => {
            let key = message.client_key.as_deref().unwrap_or_default();
            let row = posted_message(state, me.id, key)
                .await?
                .ok_or(AppError::Conflict("bad_message"))?;
            return Ok(Posted { row, fresh: false });
        }
    };
    // Mentioned people get a mention (inbox and push) instead of the plain message.
    let pinged: Vec<i64> = if ring_everyone {
        others.clone()
    } else {
        mentions
    };
    let rest: Vec<i64> = others.into_iter().filter(|m| !pinged.contains(m)).collect();
    let said = Text::same(message.text);
    // The message is stored: from here on, failures are logged (an error would make the app
    // retry, and the retry answers the stored message without telling anyone).
    for (to, kind) in [(&rest, "message"), (&pinged, "mention")] {
        if let Err(e) =
            crate::chat::inbox::to_members(&state.db, to, kind, Some(id), Some(me.id), &said).await
        {
            tracing::warn!(round_id = id, kind, error = ?e, "chat inbox failed");
        }
    }
    state.live.send(
        me.group_id,
        Event::MessagePosted {
            round_id: id,
            message_id: row.id,
            member_id: me.id,
            user_id: me.user_id,
        },
    );
    state.live.send(me.group_id, Event::InboxChanged);
    state.push.notify_members(
        state.db.clone(),
        rest,
        me,
        Notification {
            kinds: vec![Kind::Chat],
            title: Text::by(|l| i18n::chat_title(l, &me.name, &round.text)),
            body: said.clone(),
            tag: format!("chat-{id}"),
            url: format!("/round/{id}"),
            room: me.group_id,
        },
    );
    // Through even with chat pushes off (or with mentions off but chat on); room mute,
    // the master switch, quiet hours and blocks still apply.
    state.push.notify_members(
        state.db.clone(),
        pinged,
        me,
        Notification {
            kinds: vec![Kind::Mention, Kind::Chat],
            title: Text::by(|l| i18n::mention_title(l, &me.name, &round.text)),
            body: said,
            tag: format!("mention-{id}"),
            url: format!("/round/{id}"),
            room: me.group_id,
        },
    );
    Ok(Posted { row, fresh: true })
}
