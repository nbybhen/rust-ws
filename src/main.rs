use std::collections::HashMap;
use axum::routing::get;
use socketioxide::{
    extract::SocketRef,
    SocketIo,
};
use socketioxide::extract::TryData;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::sync::OnceLock;
use std::process::{Child, Command};
use std::fs;

extern crate log;

#[derive(Debug, Clone, Serialize)]
pub struct LangInfo<'li> {
    name: &'static str,
    cmds: &'li [&'static str],
    ext: &'static str
}

impl<'li> LangInfo<'li> {
    fn new(name: &'static str, cmds: &'li [&'static str], ext: &'static str) -> Self {
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
}

static CELL: OnceLock<HashMap<&'static str, LangInfo>> = OnceLock::new();

impl Session {
    pub fn new(socket: SocketRef, data: Code, is_ide: bool) -> Self {
        Self {socket, data, is_ide}
    }

    pub fn run_ide(&self) {
        self.socket.emit("message-back", "Running code...").ok();
        if let Some(lang) = CELL.get().unwrap().get(self.data.lang.as_str()) {
            log::debug!("Chosen language: {:?}", lang.name);
            let dir = format!("src/tmp/main{}", lang.ext);
            log::debug!("File location: {dir:?}");
            fs::write(dir, &self.data.code).expect("Failed to write to file.");

            let child: Child;

            if cfg!(windows) {
                child = Command::new("cmd")
                    .args(&*lang.cmds)
                    .stderr(std::process::Stdio::null())
                    .stdout(std::process::Stdio::piped())
                    .stdin(std::process::Stdio::piped())
                    .spawn().expect("Could not run the command(s)");
            }
            else {
                child = Command::new("zsh")
                    .args(&*lang.cmds)
                    .stderr(std::process::Stdio::null())
                    .stdout(std::process::Stdio::piped())
                    .stdin(std::process::Stdio::piped())
                    .spawn().expect("Could not run the command(s)");
            }

            let output = child.wait_with_output().expect("Could not wait for child process");
            log::debug!("{output:?}");
            let f_output = output.stdout.iter().map(|&x| x as char).collect::<String>();
            log::debug!("Output: {:?}", f_output);
            self.socket.emit("response", f_output).unwrap();
        }
    }

    pub fn run_tests(&self) {
        todo!("Work on processing question solutions.");
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initializes the logger
    std::env::set_var("RUST_LOG", "debug");
    env_logger::init();

    let (layer, io) = SocketIo::new_layer();

    CELL.set({
        let mut hash: HashMap<&'static str, LangInfo> = HashMap::new();
        if cfg!(target_os = "windows") {
            hash.insert("python", LangInfo::new("python3", &["/C", "python3 src/tmp/main.py"], ".py"));
            hash.insert("javascript", LangInfo::new("javascript", &["/C", "node", "src/tmp/main.js"], ".js"));
            hash.insert("typescript", LangInfo::new("typescript", &["/C", "npx tsx src/tmp/main.ts"], ".ts"));
            hash.insert("cpp", LangInfo::new("cpp", &["clang++ -std=c++20 src/tmp/main.cpp -o src/tmp/main.exe", "&&", "src\\tmp\\main.exe"], ".cpp"));
            hash.insert("c", LangInfo::new("c", &["gcc tmp/main.c -o src/tmp/main.out", "src\\tmp\\main.out"], ".c"));
            hash.insert("rust", LangInfo::new("rust", &["/C", "rustc src/tmp/main.rs -o src/tmp/main.exe", "&&", "src\\tmp\\main.exe"], ".rs"));
            hash.insert("kotlin", LangInfo::new("kotlin", &["/C", "kotlinc -script src/tmp/main.kts"], ".kts"));
            hash.insert("java", LangInfo::new("java", &["/C", "javac src/tmp/Main.java", "&&", "java -classpath src/tmp Main"], ".java"));
            hash.insert("go", LangInfo::new("go", &["/C", "go run src/tmp/main.go"], ".go"));
            hash.insert("elixir", LangInfo::new("elixir", &["/C", "elixir src/tmp/main.exs"], ".exs"));
        }
        else {
            hash.insert("python", LangInfo::new("python3", &["-c", "python3 src/tmp/main.py"], ".py"));
            hash.insert("javascript", LangInfo::new("javascript", &["-c", "node src/tmp/main.js"], ".js"));
            hash.insert("typescript", LangInfo::new("typescript", &["-c", "npx tsx src/tmp/main.ts"], ".ts"));
            hash.insert("cpp", LangInfo::new("cpp", &["-c","clang++ -std=c++20 src/tmp/main.cpp -o src/tmp/main.out && src/tmp/main.out"], ".cpp"));
            hash.insert("c", LangInfo::new("c", &["-c","gcc src/tmp/main.c -o src/tmp/main.out && src/tmp/main.out"], ".c"));
            hash.insert("rust", LangInfo::new("rust", &["-c", "rustc src/tmp/main.rs -o src/tmp/main.out && src/tmp/main.out"], ".rs"));
            hash.insert("kotlin", LangInfo::new("kotlin", &["-c", "kotlinc -script src/tmp/main.kts"], ".kts"));
            hash.insert("java", LangInfo::new("java", &["-c", "javac src/tmp/Main.java && java -classpath src/tmp Main"], ".java"));
            hash.insert("go", LangInfo::new("go", &["-c", "go run src/tmp/main.go"], ".go"));
            hash.insert("elixir", LangInfo::new("elixir", &["-c", "elixir src/tmp/main.exs"], ".exs"));
        }
        hash
    }).expect("Unable to set OnceLock");

    io.ns("/", |s: SocketRef| {
        log::debug!("Connected! {}", s.id);
        s.on("message", |s: SocketRef, TryData::<Code>(data)| {
            if let Some(code) = data.ok() {
                log::debug!("Received message: {:?}", code);
                let response = Session::new(s, code, true);

                match response.is_ide {
                    true => response.run_ide(),
                    false => response.run_tests()
                }
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