use axum::routing::get;
use socketioxide::{
    extract::SocketRef,
    SocketIo,
};
use socketioxide::extract::TryData;
use socketioxide::socket::Socket;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use serde::{Deserialize, Serialize};

extern crate log;

pub struct Session {
    socket: SocketRef,
    data: Code,
    is_ide: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Code {
    lang: String,
    code: String
}

impl Session {
    pub fn new(socket: SocketRef, data: Code, is_ide: bool) -> Self {
        Self {socket, data, is_ide}
    }

    pub fn run_ide(&self) {

    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (layer, io) = SocketIo::new_layer();

    // Initializes the logger
    std::env::set_var("RUST_LOG", "debug");
    env_logger::init();

    // Register a handler for the default namespace
    io.ns("/", |s: SocketRef| {
        log::debug!("Connected! {}", s.id);
        s.on("message", |s: SocketRef, TryData::<Code>(data)| {
            println!("Received message: {:?}", data.ok());
            if let Some(code) = data.clone() {
                s.emit("message-back", "Running code...").ok();
                let response = Session::new(s, code, true);
            }

        });
    });

    let app = axum::Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .layer(
            ServiceBuilder::new()
                .layer(CorsLayer::permissive())
                .layer(layer)
        );

    let listener = tokio::net::TcpListener::bind("localhost:4000").await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}