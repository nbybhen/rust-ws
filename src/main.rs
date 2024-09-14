use std::collections::HashMap;
use axum::routing::get;
use socketioxide::{
    extract::SocketRef,
    SocketIo,
};
use socketioxide::extract::TryData;
use socketioxide::socket::Socket;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::sync::OnceLock;
use serde::ser::SerializeMap;
use serde_with::serde_as;
use std::process::Command;
use futures_util::stream::iter;
use shutil::pipe;
use std::fs;

extern crate log;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LangInfo {
    name: &'static str,
    cmds: Box<[&'static str]>,
    ext: &'static str
}

impl LangInfo {
    fn new(name: &'static str, cmds: Box<[&'static str]>, ext: &'static str) -> Self {
        Self {name, cmds, ext}
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Code {
    lang: String,
    code: String,
}

pub struct Session {
    socket: SocketRef,
    data: Code,
    is_ide: bool,
    languages: OnceLock<HashMap<&'static str, LangInfo>>,
}

impl Session {
    pub fn new(socket: SocketRef, data: Code, is_ide: bool) -> Self {
        let cell = OnceLock::new();
        cell.set({
            let mut hash: HashMap<&'static str, LangInfo> = HashMap::new();
            hash.insert("python", LangInfo::new("python3", Box::new(["/C", "python3 src/tmp/main.py"]), ".py"));
            hash
        }).expect("Unable to set OnceLock");

        Self {socket, data, is_ide, languages: cell}
    }

    pub fn run_ide(&self) {
        self.socket.emit("message-back", "Running code...").ok();
        if let Some(lang) = self.languages.get().unwrap().get(self.data.lang.as_str()) {
            log::debug!("Chosen: {:?}", lang.name);
            let dir = format!("src/tmp/main{}", lang.ext);
            log::debug!("File location: {dir:?}");
            fs::write(dir, &self.data.code).expect("Failed to write to file.");

            let child = Command::new("cmd")
                .args(&*lang.cmds)
                .stderr(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stdin(std::process::Stdio::piped())
                .spawn().expect("Could not run the command(s)");

            let output = child.wait_with_output().expect("Could not wait for child process");
            let f_output = output.stdout.iter().map(|&x| x as char).collect::<String>();
            log::debug!("{:?}", f_output);
            self.socket.emit("response", f_output).unwrap();
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (layer, io) = SocketIo::new_layer();

    // Initializes the logger
    std::env::set_var("RUST_LOG", "debug");
    env_logger::init();

    io.ns("/", |s: SocketRef| {
        log::debug!("Connected! {}", s.id);
        s.on("message", |s: SocketRef, TryData::<Code>(data)| {
            if let Some(code) = data.ok() {
                println!("Received message: {:?}", code);
                let response = Session::new(s, code, true);
                response.run_ide();
            }
            else {
                log::error!("Failed to process message!");
                s.emit("message-back", "Failed to process message!").ok();
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