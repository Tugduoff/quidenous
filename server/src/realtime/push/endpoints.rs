/// The Web Push services browsers use: Chrome and most Chromium browsers (FCM), Firefox
/// (Mozilla autopush), Safari (Apple) and Edge (Windows Push Notification Services).
/// Endpoints anywhere else are refused, so the server never posts to arbitrary hosts.
pub(super) const WEB_PUSH_HOSTS: [&str; 4] = [
    "fcm.googleapis.com",
    "push.services.mozilla.com",
    "push.apple.com",
    "notify.windows.com",
];

/// A Web Push endpoint on one of [`WEB_PUSH_HOSTS`] (or a subdomain), over https on the
/// default port, 1 KB at most.
pub fn web_endpoint_allowed(endpoint: &str) -> bool {
    if endpoint.len() > 1024 {
        return false;
    }
    let Ok(url) = reqwest::Url::parse(endpoint) else {
        return false;
    };
    // `domain()` is none for IP addresses.
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

/// A browser key (`p256dh`, `auth`): base64 characters, `max` at most.
pub fn key_ok(key: &str, max: usize) -> bool {
    !key.is_empty()
        && key.len() <= max
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'+' | b'/' | b'='))
}

/// `Sent.title` of a recorded widget ping (tests); its `body` is the room id.
pub const WIDGET_PING: &str = "widget";

/// A widget refresh: data only (no `notification`, so nothing shows), `widget` + `room` for the app's
/// background task, one pending per room and device (`collapse_key`), normal priority, dropped after
/// an hour undelivered (the widget's own timer catches up by then).
pub fn widget_ping_message(fcm_token: &str, group_id: i64) -> serde_json::Value {
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

/// An FCM registration token: printable ASCII, 4 KB at most.
pub fn fcm_token_ok(token: &str) -> bool {
    !token.is_empty() && token.len() <= 4096 && token.bytes().all(|b| b.is_ascii_graphic())
}

/// Did FCM say this token is dead? `UNREGISTERED`, or `INVALID_ARGUMENT` about the token itself;
/// any other `INVALID_ARGUMENT` is our message's fault, and the device stays.
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
