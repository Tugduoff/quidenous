//! The live WebSocket (`/api/live`) and push subscriptions.

mod common;

use axum::http::StatusCode;
use common::{call, join, room_id, setup};
use futures::{SinkExt, StreamExt};
use quidenous_server::live::Event;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

/// The app on a real local port (WebSockets need a socket): its address.
async fn serve(app: axum::Router) -> std::net::SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });
    addr
}

type Socket = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

async fn connect(addr: std::net::SocketAddr, query: &str) -> Result<Socket, StatusCode> {
    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let request = format!("ws://{addr}/api/live?{query}")
        .into_client_request()
        .unwrap();
    match tokio_tungstenite::client_async(request, stream).await {
        Ok((socket, _)) => Ok(socket),
        Err(tokio_tungstenite::tungstenite::Error::Http(res)) => Err(res.status()),
        Err(e) => panic!("{e}"),
    }
}

/// The next text message as JSON, if one comes within `ms`.
async fn next_event(socket: &mut Socket, ms: u64) -> Option<Value> {
    loop {
        let msg = tokio::time::timeout(std::time::Duration::from_millis(ms), socket.next())
            .await
            .ok()??
            .unwrap();
        if let Message::Text(text) = msg {
            return Some(serde_json::from_str(&text).unwrap());
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_live_socket_relays_my_rooms_events() {
    let (app, state) = setup().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, _) = join(&app, "Bob").await;
    let room = room_id(&app, &a).await;
    let addr = serve(app.clone()).await;

    assert_eq!(
        connect(addr, "token=nope").await.err(),
        Some(StatusCode::UNAUTHORIZED)
    );
    let mut socket = connect(addr, &format!("token={a}")).await.unwrap();
    // The socket subscribes once the upgrade is done: ping the room until it hears it.
    let mut heard = None;
    for _ in 0..50 {
        state.live.send(room, Event::RoomUpdated);
        if let Some(e) = next_event(&mut socket, 100).await {
            heard = Some(e);
            break;
        }
    }
    assert_eq!(heard, Some(json!({ "type": "room_updated" })));
    // Another room's events don't reach it.
    state.live.send(room + 1000, Event::RoomDeleted);
    // A vote in my room does.
    let (s, _) = call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&b),
        None,
        Some(json!({ "target_id": a_id })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let mut event = next_event(&mut socket, 2000).await.unwrap();
    while event["type"] == "room_updated" {
        event = next_event(&mut socket, 2000).await.unwrap();
    }
    assert_eq!(event["type"], "vote_cast");
    assert_eq!(
        event["voter_id"].as_i64(),
        Some(room_member(&state, &b).await)
    );
    // The app's presence pings are accepted; closing ends it cleanly.
    socket.send(Message::Text("here".into())).await.unwrap();
    socket.close(None).await.unwrap();
}

async fn room_member(state: &quidenous_server::AppState, token: &str) -> i64 {
    quidenous_server::auth::member_in_room(state, token, None)
        .await
        .unwrap()
        .id
}

#[tokio::test]
async fn devices_can_unsubscribe_from_push() {
    let (app, state) = setup().await;
    let (a, _) = join(&app, "Ana").await;
    let (b, _) = join(&app, "Bob").await;
    let web =
        |endpoint: &str| json!({ "kind": "web", "endpoint": endpoint, "p256dh": "x", "auth": "y" });
    for (t, e) in [
        (&a, "https://push.example/ana"),
        (&b, "https://push.example/bob"),
    ] {
        let (s, _) = call(&app, "POST", "/push/subscribe", Some(t), None, Some(web(e))).await;
        assert_eq!(s, StatusCode::OK);
    }
    let count = |endpoint: &'static str| {
        let db = state.db.clone();
        async move {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM push_subscriptions WHERE endpoint = ?",
            )
            .bind(endpoint)
            .fetch_one(&db)
            .await
            .unwrap()
        }
    };
    // Someone else's device stays.
    let (s, _) = call(
        &app,
        "POST",
        "/push/unsubscribe",
        Some(&a),
        None,
        Some(json!({ "endpoint": "https://push.example/bob" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(count("https://push.example/bob").await, 1);
    // Mine goes; doing it again is harmless.
    for _ in 0..2 {
        let (s, _) = call(
            &app,
            "POST",
            "/push/unsubscribe",
            Some(&a),
            None,
            Some(json!({ "endpoint": "https://push.example/ana" })),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
    }
    assert_eq!(count("https://push.example/ana").await, 0);
    let (s, _) = call(
        &app,
        "POST",
        "/push/unsubscribe",
        None,
        None,
        Some(json!({ "endpoint": "https://push.example/bob" })),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}
