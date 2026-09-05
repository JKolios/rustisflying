# Build with a full Rust toolchain, run on slim Debian (rustls => no OpenSSL
# needed at runtime; the web UI's index.html is compiled into the binary).
FROM rust:1-slim-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim
# config.toml is read from the working directory: mount it at /app/config.toml
WORKDIR /app
COPY --from=builder /build/target/release/rustisflying /usr/local/bin/rustisflying
# The web UI must listen on all interfaces to be reachable outside the
# container: set bind = "0.0.0.0:8080" under [web] in the mounted config.
EXPOSE 8080
# Terminal output goes to stdout/stderr as usual: `docker logs -f` shows it.
CMD ["rustisflying"]
