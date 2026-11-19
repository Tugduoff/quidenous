use crate::platform::ids::RoomId;

pub(super) const WEB_PUSH_HOSTS: [&str; 4] = [
    "fcm.googleapis.com",
    "push.services.mozilla.com",
    "push.apple.com",
    "notify.windows.com",
];

pub(crate) fn web_endpoint_allowed(endpoint: &str) -> bool {
    if endpoint.len() > 1024 {
        return false;
    }
    let Ok(url) = reqwest::Url::parse(endpoint) else {
        return false;
    };
    let Some(host) = url.domain() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && WEB_PUSH_HOSTS
            .iter()
            .any(|h| host == *h || host.ends_with(&format!(".{h}")))
}

pub(crate) fn key_ok(key: &str, max: usize) -> bool {
    !key.is_empty()
        && key.len() <= max
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'+' | b'/' | b'='))
}

pub const WIDGET_PING: &str = "widget";

pub fn widget_ping_message(fcm_token: &str, group_id: RoomId) -> serde_json::Value {
    serde_json::json!({
        "message": {
            "token": fcm_token,
            "data": { "widget": "1", "room": group_id.to_string() },
            "android": {
                "priority": "normal",
                "collapse_key": format!("widget-{group_id}"),
                "ttl": "3600s"
            }
        }
    })
}

pub(crate) fn fcm_token_ok(token: &str) -> bool {
    !token.is_empty() && token.len() <= 4096 && token.bytes().all(|b| b.is_ascii_graphic())
}

pub(super) fn fcm_token_gone(status: u16, body: &str) -> bool {
    if status == 404 {
        return true;
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(body) else {
        return body.contains("UNREGISTERED");
    };
    let error = &v["error"];
    let details = error["details"].as_array().cloned().unwrap_or_default();
    let code = |c: &str| error["status"] == c || details.iter().any(|d| d["errorCode"] == c);
    if code("UNREGISTERED") {
        return true;
    }
    let about_token = details.iter().any(|d| {
        d["fieldViolations"]
            .as_array()
            .is_some_and(|f| f.iter().any(|f| f["field"] == "message.token"))
    }) || error["message"]
        .as_str()
        .is_some_and(|m| m.contains("registration token"));
    code("INVALID_ARGUMENT") && about_token
}
