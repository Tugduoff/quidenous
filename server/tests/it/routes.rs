use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use tower::ServiceExt;

use crate::common::setup;

const ROUTES: &[(&str, &str)] = &[
    ("GET", "/"),
    ("POST", "/api/admin/packs/{id}/approve"),
    ("POST", "/api/admin/packs/{id}/feature"),
    ("POST", "/api/admin/packs/{id}/remove"),
    ("POST", "/api/admin/reports"),
    ("GET", "/api/admin/review"),
    ("POST", "/api/admin/suggestions/{id}/approve"),
    ("POST", "/api/admin/suggestions/{id}/reject"),
    ("POST", "/api/auth/2fa/disable"),
    ("POST", "/api/auth/2fa/enable"),
    ("POST", "/api/auth/2fa/recovery"),
    ("POST", "/api/auth/2fa/reset/cancel"),
    ("POST", "/api/auth/2fa/reset/confirm"),
    ("POST", "/api/auth/2fa/reset/send"),
    ("POST", "/api/auth/2fa/setup"),
    ("DELETE GET", "/api/auth/account"),
    ("POST", "/api/auth/credentials"),
    ("POST", "/api/auth/delete/confirm"),
    ("POST", "/api/auth/delete/send"),
    ("POST", "/api/auth/email"),
    ("POST", "/api/auth/email/change"),
    ("POST", "/api/auth/email/confirm"),
    ("POST", "/api/auth/forgot"),
    ("POST", "/api/auth/login"),
    ("POST", "/api/auth/login/2fa"),
    ("POST", "/api/auth/logout"),
    ("POST", "/api/auth/oauth/redeem"),
    ("DELETE", "/api/auth/oauth/{provider}"),
    ("GET", "/api/auth/oauth/{provider}/callback"),
    ("POST", "/api/auth/oauth/{provider}/start"),
    ("POST", "/api/auth/password"),
    ("GET", "/api/auth/providers"),
    ("POST", "/api/auth/reauth"),
    ("POST", "/api/auth/reset"),
    ("GET", "/api/auth/sessions"),
    ("POST", "/api/auth/sessions/revoke-others"),
    ("DELETE", "/api/auth/sessions/{id}"),
    ("POST", "/api/auth/signup"),
    ("POST", "/api/auth/terms"),
    ("POST", "/api/auth/ticket"),
    ("POST", "/api/auth/verify"),
    ("POST", "/api/auth/verify/send"),
    ("GET", "/api/badges"),
    ("GET", "/api/blocks"),
    ("DELETE PUT", "/api/blocks/{user_id}"),
    ("POST", "/api/client-errors"),
    ("GET PUT", "/api/community/me"),
    ("GET", "/api/creators/{user_id}"),
    ("GET", "/api/donate"),
    ("GET", "/api/features"),
    ("GET POST", "/api/friends"),
    ("DELETE POST", "/api/friends/{user_id}"),
    ("GET", "/api/gifs"),
    ("GET", "/api/gifs/media/{token}"),
    ("GET", "/api/global"),
    ("POST", "/api/global/vote"),
    ("GET", "/api/group"),
    ("GET", "/api/health"),
    ("GET", "/api/history"),
    ("GET", "/api/history/export"),
    ("GET", "/api/history/pinned"),
    ("GET", "/api/inbox"),
    ("POST", "/api/inbox/read"),
    ("GET POST", "/api/intro"),
    ("GET", "/api/invites"),
    ("DELETE", "/api/invites/{id}"),
    ("POST", "/api/invites/{id}/accept"),
    ("POST", "/api/join"),
    ("GET", "/api/live"),
    ("GET PUT", "/api/me"),
    ("PUT", "/api/me/avatar"),
    ("GET PUT", "/api/me/locale"),
    ("PUT", "/api/me/photo"),
    ("PUT", "/api/me/title"),
    ("GET", "/api/media/{token}"),
    ("GET", "/api/members/{member_id}/badges"),
    ("DELETE PUT", "/api/members/{member_id}/kick"),
    ("DELETE POST", "/api/messages/{id}/reactions"),
    ("GET PUT", "/api/notifications"),
    ("GET POST", "/api/packs"),
    ("POST", "/api/packs/delete"),
    ("GET", "/api/packs/mine"),
    ("DELETE GET PUT", "/api/packs/{id}"),
    ("POST", "/api/packs/{id}/install"),
    ("DELETE POST", "/api/packs/{id}/like"),
    ("POST", "/api/packs/{id}/report"),
    ("POST", "/api/parties"),
    ("POST", "/api/parties/join"),
    ("GET", "/api/parties/screen/{code}"),
    ("GET PATCH", "/api/parties/{id}"),
    ("POST", "/api/parties/{id}/answer"),
    ("POST", "/api/parties/{id}/end"),
    ("POST", "/api/parties/{id}/keep"),
    ("DELETE POST", "/api/parties/{id}/next"),
    ("POST", "/api/parties/{id}/react"),
    ("POST", "/api/parties/{id}/start"),
    ("POST", "/api/parties/{id}/vote"),
    ("GET POST", "/api/play/{token}"),
    ("GET", "/api/push/config"),
    ("POST", "/api/push/subscribe"),
    ("POST", "/api/push/unsubscribe"),
    ("POST", "/api/questions"),
    ("POST", "/api/questions/ask"),
    ("POST", "/api/questions/delete"),
    ("POST", "/api/questions/import"),
    ("POST", "/api/questions/order"),
    ("DELETE GET", "/api/questions/queue"),
    ("POST", "/api/questions/shuffle"),
    ("DELETE", "/api/questions/{id}"),
    ("POST", "/api/reports"),
    ("GET POST", "/api/rooms"),
    ("POST", "/api/rooms/join"),
    ("GET", "/api/rooms/preview"),
    ("DELETE PATCH", "/api/rooms/{id}"),
    ("POST", "/api/rooms/{id}/add"),
    ("POST", "/api/rooms/{id}/code"),
    ("GET", "/api/rooms/{id}/invites"),
    ("DELETE", "/api/rooms/{id}/invites/{user_id}"),
    ("POST", "/api/rooms/{id}/leave"),
    ("DELETE", "/api/rooms/{id}/members/{member_id}"),
    ("DELETE PUT", "/api/rooms/{id}/mute"),
    ("POST", "/api/rooms/{id}/next"),
    ("POST", "/api/rooms/{id}/owner"),
    ("GET", "/api/round/current"),
    ("POST", "/api/round/current/answer"),
    ("POST", "/api/round/current/nudge"),
    ("DELETE PUT", "/api/round/current/skip/{member_id}"),
    ("POST", "/api/round/current/vote"),
    ("GET", "/api/round/unseen"),
    ("GET", "/api/rounds/{id}"),
    ("POST", "/api/rounds/{id}/bet"),
    ("POST", "/api/rounds/{id}/challenge"),
    ("GET", "/api/rounds/{id}/challenges"),
    ("GET POST", "/api/rounds/{id}/messages"),
    ("POST", "/api/rounds/{id}/messages/gif"),
    ("POST", "/api/rounds/{id}/messages/image"),
    ("DELETE", "/api/rounds/{id}/messages/{message_id}"),
    ("POST", "/api/rounds/{id}/messages/{message_id}/restore"),
    ("DELETE PUT", "/api/rounds/{id}/pin"),
    ("DELETE", "/api/rounds/{id}/question"),
    ("GET POST", "/api/rounds/{id}/reactions"),
    ("POST", "/api/rounds/{id}/seen"),
    ("POST", "/api/rounds/{id}/share"),
    ("GET", "/api/season"),
    ("GET", "/api/shares"),
    ("DELETE", "/api/shares/{token}"),
    ("POST", "/api/start"),
    ("GET", "/api/stats"),
    ("POST", "/api/stats/share"),
    ("GET", "/api/streaks"),
    ("POST", "/api/suggestions"),
    ("GET", "/api/suggestions/mine"),
    ("GET", "/api/suggestions/next"),
    ("POST", "/api/suggestions/{id}/vote"),
    ("GET", "/api/titles"),
    ("GET", "/api/today"),
    ("GET", "/api/tripot"),
    ("GET", "/api/updates"),
    ("GET", "/api/updates/assets/{runtime}/{id}/{name}"),
    ("GET", "/api/updates/manifest"),
    ("POST", "/api/updates/publish"),
    ("DELETE", "/api/updates/{runtime}/{id}"),
    ("GET", "/api/version"),
    ("GET", "/api/vibes"),
    ("GET", "/api/widget"),
    ("DELETE POST", "/api/widget/token"),
    ("GET", "/api/wrapped"),
    ("POST", "/api/wrapped/share"),
    ("GET", "/beta"),
    ("GET", "/delete-account"),
    ("GET", "/en"),
    ("GET", "/en/beta"),
    ("GET", "/en/delete-account"),
    ("GET", "/en/pack-template.md"),
    ("GET", "/en/privacy"),
    ("GET", "/en/terms"),
    ("GET", "/p/{id}"),
    ("GET", "/p/{id}/card.png"),
    ("GET", "/pack-template.md"),
    ("GET", "/privacy"),
    ("GET", "/robots.txt"),
    ("GET", "/s/{token}"),
    ("GET", "/s/{token}/card.png"),
    ("GET", "/terms"),
    ("GET", "/vitrine/{*file}"),
];

const OPEN_TO_ANYONE: &[(&str, &str)] = &[
    ("GET", "/"),
    ("GET", "/en"),
    ("GET", "/vitrine/{*file}"),
    ("GET", "/robots.txt"),
    ("GET", "/privacy"),
    ("GET", "/delete-account"),
    ("GET", "/terms"),
    ("GET", "/beta"),
    ("GET", "/en/privacy"),
    ("GET", "/en/delete-account"),
    ("GET", "/en/terms"),
    ("GET", "/en/beta"),
    ("GET", "/pack-template.md"),
    ("GET", "/en/pack-template.md"),
    ("GET", "/s/{token}"),
    ("GET", "/s/{token}/card.png"),
    ("GET", "/p/{id}"),
    ("GET", "/p/{id}/card.png"),
    ("GET", "/api/features"),
    ("GET", "/api/donate"),
    ("GET", "/api/season"),
    ("GET", "/api/vibes"),
    ("GET", "/api/health"),
    ("GET", "/api/version"),
    ("GET", "/api/push/config"),
    ("GET", "/api/updates/manifest"),
    ("GET", "/api/updates/assets/{runtime}/{id}/{name}"),
    ("GET", "/api/play/{token}"),
    ("GET", "/api/media/{token}"),
    ("GET", "/api/gifs/media/{token}"),
    ("GET", "/api/rooms/preview"),
    ("GET", "/api/parties/screen/{code}"),
    ("GET", "/api/auth/providers"),
    ("GET", "/api/auth/oauth/{provider}/callback"),
    ("POST", "/api/client-errors"),
    ("POST", "/api/start"),
    ("POST", "/api/join"),
    ("POST", "/api/parties/join"),
    ("POST", "/api/auth/email"),
    ("POST", "/api/auth/signup"),
    ("POST", "/api/auth/login"),
    ("POST", "/api/auth/login/2fa"),
    ("POST", "/api/auth/forgot"),
    ("POST", "/api/auth/reset"),
    ("POST", "/api/auth/2fa/reset/send"),
    ("POST", "/api/auth/2fa/reset/confirm"),
    ("POST", "/api/auth/delete/send"),
    ("POST", "/api/auth/delete/confirm"),
    ("POST", "/api/auth/oauth/redeem"),
    ("POST", "/api/auth/oauth/{provider}/start"),
];

const REFUSED_ANOTHER_WAY: &[(&str, &str, StatusCode)] = &[
    ("GET", "/api/live", StatusCode::BAD_REQUEST),
    ("GET", "/api/updates", StatusCode::NOT_FOUND),
    ("POST", "/api/updates/publish", StatusCode::NOT_FOUND),
    (
        "DELETE",
        "/api/updates/{runtime}/{id}",
        StatusCode::NOT_FOUND,
    ),
];

fn concrete(path: &str) -> String {
    path.split('/')
        .map(|part| match part {
            "{*file}" => "site.css",
            p if p.starts_with('{') => "1",
            p => p,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn allowed(declared: &str) -> Vec<String> {
    let mut methods: Vec<String> = declared.split(' ').map(str::to_owned).collect();
    if methods.iter().any(|m| m == "GET") {
        methods.push("HEAD".into());
    }
    methods.sort();
    methods
}

#[tokio::test]
async fn every_known_route_answers_its_methods() {
    let (app, _) = setup().await;
    let mut wrong = Vec::new();
    for (methods, path) in ROUTES {
        let req = Request::builder()
            .method("TRACE")
            .uri(concrete(path))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        let mut allow: Vec<String> = res
            .headers()
            .get(header::ALLOW)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .split(',')
            .map(|m| m.trim().to_owned())
            .filter(|m| !m.is_empty())
            .collect();
        allow.sort();
        if res.status() != StatusCode::METHOD_NOT_ALLOWED || allow != allowed(methods) {
            wrong.push(format!(
                "{methods} {path}: {} allow {allow:?}",
                res.status()
            ));
        }
    }
    assert!(wrong.is_empty(), "routes changed:\n{}", wrong.join("\n"));
}

#[tokio::test]
async fn only_the_listed_routes_answer_without_a_credential() {
    let (app, _) = setup().await;
    let mut wrong = Vec::new();
    for (methods, path) in ROUTES {
        for method in methods.split(' ') {
            let req = Request::builder()
                .method(method)
                .uri(concrete(path))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap();
            let status = app.clone().oneshot(req).await.unwrap().status();
            let listed = OPEN_TO_ANYONE.contains(&(method, *path));
            let answered = status != StatusCode::UNAUTHORIZED
                && !REFUSED_ANOTHER_WAY.contains(&(method, *path, status));
            if answered != listed {
                wrong.push(format!(
                    "{method} {path}: {status}, {}",
                    if listed {
                        "listed in OPEN_TO_ANYONE"
                    } else {
                        "not listed in OPEN_TO_ANYONE"
                    }
                ));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "a route answers a caller without a credential only when OPEN_TO_ANYONE says so on purpose (take who is calling from an extractor, before the body):\n{}",
        wrong.join("\n")
    );
}
