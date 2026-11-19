use axum::Router;
use futures::StreamExt;
use serde_json::Value;
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

pub(crate) async fn serve(app: Router) -> std::net::SocketAddr {
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

pub(crate) type Socket = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

pub(crate) async fn connect(addr: std::net::SocketAddr, query: &str) -> Socket {
    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let request = format!("ws://{addr}/api/live?{query}")
        .into_client_request()
        .unwrap();
    tokio_tungstenite::client_async(request, stream)
        .await
        .unwrap()
        .0
}

pub(crate) async fn next_event(socket: &mut Socket, ms: u64) -> Option<Value> {
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
