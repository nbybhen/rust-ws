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
use std::io::Stderr;
use std::path::Path;

extern crate log;

#[derive(Debug, Clone, Serialize)]
pub struct LangInfo<'li> {
    name: &'li str,
    cmds: &'li [&'li str],
    test_cmds:&'li [&'li str],
    ext: &'li str
}

impl<'li> LangInfo<'li> {
    fn new(name: &'li str, cmds: &'li [&'li str], test_cmds:  &'li [&'li str], ext: &'li str) -> Self {
        Self {name, cmds, test_cmds, ext}
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Code {
    lang: String,
    code: String,
    is_ide: bool,
}

pub struct Session {
    socket: SocketRef,
    data: Code,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Return {
    data: String,
    code: i32
}


static CELL: OnceLock<HashMap<&'static str, LangInfo>> = OnceLock::new();

impl Session {
    pub fn new(socket: SocketRef, data: Code) -> Self {
        Self {socket, data}
    }

    // Runs the shell commands in cmd or zsh (depending on OS)
    pub fn run_shell_cmds(&self, cmds: &[&'static str]) -> std::process::Output {
        let child: Child;

        if cfg!(windows) {
            child = Command::new("cmd")
                .args(&*cmds)
                .stdout(std::process::Stdio::piped())
                .stdin(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn().expect("Could not run the command(s)");
        }
        else {
            child = Command::new("zsh")
                .args(&*cmds)
                .stdout(std::process::Stdio::piped())
                .stdin(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn().expect("Could not run the command(s)");
        }

        child.wait_with_output().expect("Could not wait for child process")
    }

    // Handles writing code to correct files within correct directories based on the language
    pub fn write_to_file(&self, lang: &LangInfo) {
        match lang.name {
            // Creates inner Cargo project to allow tests if it doesn't exist
            "rust" => {
                if Path::new("tmp/rust").exists() {
                    fs::write("tmp/rust/src/main.rs", &self.data.code).expect("Failed to write to Cargo project.");
                }
                else {
                    let kid = Command::new("zsh")
                        .args(&["-c", "cargo new tmp/rust --name rust"])
                        .stderr(std::process::Stdio::piped())
                        .stdout(std::process::Stdio::piped())
                        .stdin(std::process::Stdio::piped())
                        .spawn().expect("Could not run the command(s)");
                    let output = kid.wait_with_output().expect("Could not wait for child process");
                    log::debug!("CREATING CARGO PROJECT {output:?}");
                    fs::write("tmp/rust/src/main.rs", &self.data.code).expect("Failed to write to Cargo project.");
                }
            },
            _ => {
                let dir = format!("tmp/main{}", lang.ext);
                log::debug!("File location: {dir:?}");
                fs::write(dir, &self.data.code).expect("Failed to write to file.");
            }
        };
    }

    pub fn run_ide(&self) {
        self.socket.emit("message-back", "Running code...").ok();
        if let Some(lang) = CELL.get().unwrap().get(self.data.lang.as_str()) {
            log::debug!("Chosen language: {:?}", lang.name);
            fs::create_dir_all("tmp/").expect("Unable to create directory.");

            self.write_to_file(lang);

            let output = self.run_shell_cmds(lang.cmds);
            log::debug!("{output:?}");

            let f_output = output.stdout.iter().map(|&x| x as char).collect::<String>();
            log::debug!("Output: {:?}", f_output);
            self.socket.emit("response", f_output).unwrap();
        }
    }

    pub fn run_tests(&self) {
        self.socket.emit("message-back", "Running code...").ok();
        if let Some(lang) = CELL.get().unwrap().get(self.data.lang.as_str()) {
            log::debug!("Chosen language: {:?}", lang.name);

            self.write_to_file(lang);

            let output = self.run_shell_cmds(lang.test_cmds);
            log::debug!("{output:?}");
            log::debug!("Test status: {:?}", output.status.code());

            let f_error: String = output.stderr.iter().map(|&x| x as char).collect();
            log::debug!("Error Output: {:?}", f_error);
            self.socket.emit("response", Return {
                data: format!("{}{f_error}", output.stdout.iter().map(|&x| x as char).collect::<String>()),
                code: output.status.code().expect("Unable to determine status code")
            }).unwrap();
        }
        else {
            log::error!("Language not found! {}", self.data.lang.as_str());
        }
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
        if cfg!(windows) {
            hash.insert("python", LangInfo::new("python3", &["/C", "python3 tmp/main.py"], &[""], ".py"));
            hash.insert("javascript", LangInfo::new("javascript", &["/C", "node", "tmp/main.js"], &[], ".js"));
            hash.insert("typescript", LangInfo::new("typescript", &["/C", "npx tsx tmp/main.ts"], &[], ".ts"));
            hash.insert("cpp", LangInfo::new("cpp", &["clang++ -std=c++20 tmp/main.cpp -o tmp/main.exe", "&&", "tmp\\main.exe"], &[],".cpp"));
            hash.insert("c", LangInfo::new("c", &["gcc tmp/main.c -o tmp/main.out", "tmp\\main.out"], &[],".c"));
            hash.insert("rust", LangInfo::new("rust", &["/C", "rustc tmp/main.rs -o tmp/main.exe", "&&", "tmp\\main.exe"], &[],".rs"));
            hash.insert("kotlin", LangInfo::new("kotlin", &["/C", "kotlinc -script tmp/main.kts"], &[],".kts"));
            hash.insert("java", LangInfo::new("java", &["/C", "javac tmp/Main.java", "&&", "java -classpath tmp Main"], &[],".java"));
            hash.insert("go", LangInfo::new("go", &["/C", "go run tmp/main.go"], &[],".go"));
            hash.insert("elixir", LangInfo::new("elixir", &["/C", "elixir tmp/main.exs"], &[],".exs"));
        }
        else {
            hash.insert("python", LangInfo::new("python3", &["-c", "python3 tmp/main.py"], &["-c", "python3 tmp/main.py"],".py"));
            hash.insert("javascript", LangInfo::new("javascript", &["-c", "node tmp/main.js"], &[],".js"));
            hash.insert("typescript", LangInfo::new("typescript", &["-c", "npx tsx tmp/main.ts"], &[],".ts"));
            hash.insert("cpp", LangInfo::new("cpp", &["-c","clang++ -std=c++20 tmp/main.cpp -o tmp/main.out && tmp/main.out"], &[],".cpp"));
            hash.insert("c", LangInfo::new("c", &["-c","gcc tmp/main.c -o tmp/main.out && tmp/main.out"], &[],".c"));
            hash.insert("rust", LangInfo::new("rust", &["-c", "cargo run --manifest-path tmp/rust/Cargo.toml"], &["-c", "cargo test --manifest-path tmp/rust/Cargo.toml"],".rs"));
            hash.insert("kotlin", LangInfo::new("kotlin", &["-c", "kotlinc -script tmp/main.kts"], &[],".kts"));
            hash.insert("java", LangInfo::new("java", &["-c", "javac tmp/Main.java && java -classpath tmp Main"], &[],".java"));
            hash.insert("go", LangInfo::new("go", &["-c", "go run tmp/main.go"], &[],".go"));
            hash.insert("elixir", LangInfo::new("elixir", &["-c", "elixir tmp/main.exs"], &[],".exs"));
        }

        hash
    }).expect("Unable to set OnceLock");

    io.ns("/", |s: SocketRef| {
        log::debug!("Connected! {}", s.id);
        s.on("message", |s: SocketRef, TryData::<Code>(data)| {
            if let Some(code) = data.ok() {
                log::debug!("Received message: {:?}", code);
                let response = Session::new(s, code);

                match response.data.is_ide {
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