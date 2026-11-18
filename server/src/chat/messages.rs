use crate::platform::ids::{MemberId, MessageId, RoundId, UserId};
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

macro_rules! message_columns {
    () => {
        "x.id, x.member_id, m.user_id, x.text, x.created_at, x.mentions, x.everyone, x.kind, x.caption, x.media, \
         x.hidden_at IS NOT NULL AS hidden"
    };
}

pub(crate) const MESSAGE_MAX: usize = 500;

const MENTIONS_MAX: usize = 5;

const EVERYONE_COOLDOWN: &str = "-10 minutes";

#[derive(sqlx::FromRow)]
pub(crate) struct MessageDb {
    id: MessageId,
    member_id: MemberId,
    user_id: Option<UserId>,
    text: String,
    created_at: String,
    mentions: String,
    everyone: bool,
    kind: String,
    caption: Option<String>,
    media: Option<String>,
    #[sqlx(default)]
    hidden: bool,
}

#[derive(Serialize)]
pub(crate) struct MessageRow {
    pub(crate) id: MessageId,
    member_id: MemberId,
    user_id: Option<UserId>,
    text: String,
    created_at: String,
    mentions: Vec<MemberId>,
    everyone: bool,
    kind: String,
    caption: Option<String>,
    media: Option<Value>,
    reactions: Vec<crate::chat::reactions::Reaction>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    hidden: bool,
}

impl MessageRow {
    fn new(state: &AppState, m: MessageDb) -> Self {
        let mut media = crate::chat::media::payload(state, m.id, &m.kind, m.media.as_deref());
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

const MESSAGES_SHOWN: i64 = 200;

pub(crate) async fn messages(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<RoundId>,
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
    .bind(me.is_owner())
    .bind(me.user_id)
    .bind(MESSAGES_SHOWN)
    .fetch_all(&state.db)
    .await?;
    let ids: Vec<MessageId> = rows.iter().map(|m| m.id).collect();
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
    #[serde(default)]
    mentions: Vec<MemberId>,
    #[serde(default)]
    everyone: bool,
    #[serde(default)]
    client_key: Option<String>,
}

pub(crate) async fn posted_message(
    state: &AppState,
    member_id: MemberId,
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

pub(crate) fn client_key(raw: Option<&str>) -> AppResult<Option<String>> {
    let key = raw.filter(|k| !k.is_empty());
    if key.is_some_and(|k| k.len() > 64) {
        return Err(AppError::BadRequest("bad_message"));
    }
    Ok(key.map(str::to_owned))
}

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
    Path(id): Path<RoundId>,
    Json(body): Json<MessageBody>,
) -> AppResult<Json<MessageRow>> {
    let round = chat_round(&state, &me, id).await?;
    let text = crate::platform::text::lines(&body.text);
    if text.is_empty() || text.chars().count() > MESSAGE_MAX {
        return Err(AppError::BadRequest("bad_message"));
    }
    let key = client_key(body.client_key.as_deref())?;
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

pub(crate) async fn chat_round(
    state: &AppState,
    me: &MemberRow,
    id: RoundId,
) -> AppResult<crate::rounds::model::RoundRow> {
    let round = game::round(&state.db, me.group_id, id).await?;
    if round.closed_at.is_none() {
        return Err(AppError::BadRequest("round_open"));
    }
    Ok(round)
}

pub(crate) fn chat_quota(state: &AppState, me: &MemberRow) -> AppResult<()> {
    if let Some(user) = me.user_id {
        crate::platform::quotas::hit_user(state, &crate::platform::quotas::CHAT_USER, user)?;
        crate::platform::quotas::hit_user(state, &crate::platform::quotas::CHAT_HOUR_USER, user)?;
    }
    Ok(())
}

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

pub(crate) struct NewMessage {
    pub kind: &'static str,
    pub text: String,
    pub caption: Option<String>,
    pub media: Option<Value>,
    pub mentions: Vec<MemberId>,
    pub everyone: bool,
    pub client_key: Option<String>,
}

pub(crate) struct Posted {
    pub row: MessageRow,
    pub fresh: bool,
}

pub(crate) async fn post_chat(
    state: &AppState,
    me: &MemberRow,
    round: &crate::rounds::model::RoundRow,
    message: NewMessage,
) -> AppResult<Posted> {
    let id = round.id;
    let others: Vec<MemberId> = sqlx::query_scalar(
        "SELECT id FROM members WHERE group_id = ? AND removed_at IS NULL AND id != ? ORDER BY id",
    )
    .bind(me.group_id)
    .bind(me.id)
    .fetch_all(&state.db)
    .await?;
    let mut mentions: Vec<MemberId> = message
        .mentions
        .into_iter()
        .filter(|m| others.contains(m))
        .collect();
    mentions.sort_unstable();
    mentions.dedup();
    let crowd = mentions.len() > MENTIONS_MAX;
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
    let row = if let Some(m) = inserted {
        MessageRow::new(state, m)
    } else {
        let key = message.client_key.as_deref().unwrap_or_default();
        let row = posted_message(state, me.id, key)
            .await?
            .ok_or(AppError::Conflict("bad_message"))?;
        return Ok(Posted { row, fresh: false });
    };
    let pinged: Vec<MemberId> = if ring_everyone {
        others.clone()
    } else {
        mentions
    };
    let rest: Vec<MemberId> = others.into_iter().filter(|m| !pinged.contains(m)).collect();
    let said = Text::same(message.text);
    for (to, kind) in [(&rest, "message"), (&pinged, "mention")] {
        if let Err(e) =
            crate::chat::inbox::to_members(&state.db, to, kind, Some(id), Some(me.id), &said).await
        {
            tracing::warn!(round_id = %id, %kind, error = ?e, "chat inbox failed");
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
