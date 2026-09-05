# Build with a full Rust toolchain, run on slim Debian (rustls => no OpenSSL
# needed at runtime; the web UI's index.html is compiled into the binary).
# The `epaper` feature is compiled in on both architectures (pure Rust); the
# e-paper output only activates when `[epaper] enabled = true` in the mounted
# config, and the container is given SPI/GPIO device access (see below).
FROM rust:1-slim-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked --features epaper

FROM debian:bookworm-slim
# config.toml is read from the working directory: mount it at /app/config.toml
WORKDIR /app
COPY --from=builder /build/target/release/rustisflying /usr/local/bin/rustisflying
# The web UI must listen on all interfaces to be reachable outside the
# container: set bind = "0.0.0.0:8080" under [web] in the mounted config.
EXPOSE 8080
# Terminal output goes to stdout/stderr as usual: `docker logs -f` shows it.
# For the e-paper HAT, run with device access, e.g.
#   docker run --device /dev/spidev0.0 --device /dev/gpiochip0 ...
CMD ["rustisflying"]
