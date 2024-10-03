# rust-ws

Rust implementation of backend server for CodeArena.
In order to run code within the editor, you can either run the server locally with `cargo run`, or you can run the `Dockerfile` with the below instructions:
```bash
# Builds the Dockerfile
docker build -t <name> .

# Runs the Docker container on the specified port
docker run -p <client-port>:<server-port> <name>
```

The server and Dockerfile are currently setup to run on `4000:4000`, but that can be changed within the `src/main.rs`, `Dockerfile`, and `CodeArena/src/App.tsx` files manually.

