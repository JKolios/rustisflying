# CLAUDE.md

Guidance for agents working in this repo.

## What this is

rustisflying — an overhead flight tracker. On a timer it queries the adsb.lol v2 API for aircraft inside a geofence around home, enriches the closest one (airline from the callsign prefix, route from hexdb.io), and fans the result out to a terminal renderer, a web UI, and (optionally) a Waveshare 2.7inch e-Paper HAT (B) tri-color panel. Rust (edition 2024), tokio, axum, reqwest (rustls).

## Commands

- Run: `cargo run` — reads `config.toml` from the working directory
- Test: `cargo test` — unit tests live in `#[cfg(test)]` modules next to the code
- E-paper: hardware output needs the cargo feature (`cargo run --features epaper`, Linux only); layout/rendering compile and test without it
- Docker: `docker build -t rustisflying .` then
  `docker run -p 8080:8080 -v ./config.toml:/app/config.toml:ro rustisflying`
  (add `--device /dev/spidev0.0 --device /dev/gpiochip0` when `[epaper] enabled = true`)

## Architecture

One-way data flow per polling tick (`run_tick` in `src/main.rs`):

`FlightProvider.aircraft_near()` → `Vec<Aircraft>` → geofence/freshness filters → `closest()` → `Enricher::enrich` → `FlightInfo` → `TickResult` → every `FlightOutput` (Terminal always on, WebState behind `[web] enabled`, Epaper behind `[epaper] enabled` plus the `epaper` cargo feature).

- `src/model.rs` — raw feed records (`Aircraft`) and the display model (`FlightInfo`). Serde conventions: every optional feed field is `Option` + `#[serde(default)]`; small enums (`Altitude` untagged, `VerticalDirection`/`CompassPoint` snake_case) carry their own `from_*` constructors and display helpers.
- `src/geo.rs` — `Geofence`, haversine, `closest`.
- `src/provider/` — `FlightProvider` trait + `AdsbLolClient`. The trait exists so tests and future feeds can swap in.
- `src/enrich/` — airline prefix table, hexdb.io route/airport client, per-callsign caches. All enrichment is best-effort: unknown → `None`, the renderer decides how to present "we don't know".
- `src/output/` — `FlightOutput` trait, `Terminal`, `WebState` (mutex'd `Option<TickResult>`, doubles as the web API's state), `epaper/` (layout → render → hardware worker, see below).
- `src/web/` — axum: `GET /` serves `index.html` via `include_str!` (compiled into the binary), `GET /api/latest` returns the last `TickResult` as JSON. `index.html` polls every 10 s.
- `src/output/epaper/` — Waveshare 2.7inch e-Paper HAT (B) tri-color panel. `layout.rs` (pure wording, unit-tested) and `render.rs` (two 1-bit planes via epd-waveshare + embedded-graphics + profont) always compile; `hw.rs` (SPI/GPIO worker thread) is gated behind `--features epaper`, Linux only. Rendering choices forced by the hardware: the crate's `Display2in7b` framebuffer is black/white-only, so red accents go into a second "chromatic" framebuffer (drawn `Color::Black`, they land as red on the panel); a set bit means a *white* pixel, so planes are cleared to `Color::White` before drawing; profont is latin1-only, so arrows are drawn as line/triangle shapes. The worker skips the refresh when the frame is byte-identical — e-paper refreshes are slow and wear the panel.

## Conventions / gotchas

- **`FlightInfo` is the web API JSON schema.** Adding a display field means: field on `FlightInfo`, populate in `Enricher::enrich`, render in `output/terminal.rs` and `web/index.html`, and update the `FlightInfo` literals in the `model.rs` and `terminal.rs` tests.
- Display logic is deliberately duplicated per renderer (Rust in `terminal.rs`, JS in `index.html` — e.g. the heading arrow map). Keep both in sync.
- Missing feed data is `None` and must degrade gracefully: terminal omits it, web shows an em-dash, JSON shows `null`.
- Constants live near their use (`KMH_PER_KNOT`, `LEVEL_FLIGHT_THRESHOLD_FPM`, `MIN_INTERVAL_SECONDS`).
- adsb.lol is a free courtesy feed: keep ~1 request/second max; `MIN_INTERVAL_SECONDS = 5` in `main.rs` enforces a floor.
- `config.toml` is untracked (it contains home coordinates); `config_sample.toml` is the tracked template — update it when adding config keys.
- Web bind defaults to `127.0.0.1:8080`; in Docker the mounted config needs `bind = "0.0.0.0:8080"` or the port is unreachable outside the container.
- E-paper HAT wiring is the standard Waveshare SPI HAT: SPI0/CE0 (`/dev/spidev0.0`), BCM 24/25/17 for BUSY/DC/RST, constants in `output/epaper/hw.rs`. The `epaper` cargo feature is Linux-only (linux-embedded-hal); without it, `[epaper] enabled = true` prints a warning and is skipped. The Dockerfile bakes the feature in — one image, config-gated at runtime; the container still needs `--device /dev/spidev0.0 --device /dev/gpiochip0`.

## CI / Docker

- `.github/workflows/docker.yml` — buildx multi-arch (`linux/amd64` + `linux/arm/v7` for a Raspberry Pi 2) under one manifest, pushed to `ghcr.io/jkolios/rustisfying` (`latest` + `sha` tags) on pushes to `main`; PRs build without pushing. Note: GHCR requires the lowercase image name — the workflow lowercases `github.repository` explicitly.
- `Dockerfile` — two-stage: `rust:1-slim-bookworm` → `debian:bookworm-slim` (rustls, so no OpenSSL at runtime). `WORKDIR /app` matters: `Config::load` reads `config.toml` from the CWD, so the volume mount target is `/app/config.toml`. Builds with `--features epaper` so the same image runs with or without the panel (config-gated; device access needed at runtime).
