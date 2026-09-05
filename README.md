# rustisflying

Know what's flying over your head. Watches a circular geofence around your
home on a timer, and prints details about the closest aircraft: callsign,
airline, origin, destination, distance, altitude, speed, aircraft type and
registration.

<img width="756" height="1006" alt="epaper_sample" src="https://github.com/user-attachments/assets/071df5b9-d153-4ca3-8d19-831174302a6a" />

```
[20:33:19] AEE166 · Aegean Airlines
    Athens International Airport (Eleftherios Venizelos Airport) (ATH) → Ioannina National Airport (IOA)
    5.6 km away · 6525 ft · 296 km/h · AT76 · SX-OBN
```

## Quick start

```sh
cargo run
```

On startup it reads `config.toml` from the current directory. The repository
ships `config_sample.toml` instead — copy it to `config.toml`:

```sh
cp config_sample.toml config.toml
```

Then edit the `[home]` section: right-click your house in Google Maps and
copy the `latitude, longitude` pair it shows. `radius_km` (default 30) is
the geofence radius. If no file is found, built-in defaults are used and a
warning is printed.

Stop with `Ctrl+C`.

## Web UI

Enabled by default; see the `[web]` section of `config.toml`:

```toml
[web]
enabled = true
bind = "127.0.0.1:8080" # use "0.0.0.0:8080" to expose the UI on your LAN (e.g. from a phone)
```

Then open <http://localhost:8080/> — a self-contained dark page showing
the closest aircraft (callsign, airline, route, distance, altitude, speed,
type, registration) that refreshes itself every 10 seconds. The same
snapshot is available as JSON at `/api/latest` (the schema is the
`TickResult` type in `src/model.rs`); before the first tick it returns
`null`. The terminal output keeps printing regardless.

## How it works

Each tick (default every 15 s):

1. **Positions** — `api.adsb.lol/v2/point/{lat}/{lon}/{radius}` returns the
   aircraft currently reporting within the radius (free, no API key). The
   server does the geofence filtering.
2. **Filtering** — aircraft reporting `"ground"` altitude (disabled via
   `filter.include_ground`) or stale positions (older than
   `filter.max_seen_pos_seconds`) are skipped.
3. **Selection** — the closest aircraft to home is chosen by haversine
   distance.
4. **Enrichment** — the airline name is decoded from the callsign's ICAO
   designator (`AEE251` → `AEE` → Aegean Airlines) using an embedded lookup
   table in `src/enrich/airlines.rs`; the route comes from the free
   hexdb.io API (`route/icao/{callsign}` → `LGAV-LGIO`), with airport ICAO
   codes resolved to names (`airport/icao/{icao}`). Lookups are cached for
   the process lifetime and best-effort: unknown routes degrade to
   `route unknown`, never to errors.
5. **Output** — plain text to the terminal (see `src/output/`).

## Data sources & etiquette

- [adsb.lol](https://adsb.lol) — community ADS-B aggregator. Free, keyless,
  rate limit ~1 request/second; the default 15 s interval is well within it.
  Coverage near you depends on volunteer receivers; Athens is well covered.
- [hexdb.io](https://hexdb.io) — community aircraft/route metadata. Route
  data is crowdsourced, so some flights legitimately have no route entry.

Both deserve courtesy traffic: don't lower `polling.interval_seconds`
below ~10 s without a good reason.

## Configuration

See `config.toml` — every key has a default (documented in
`src/config.rs`), and missing keys fall back to defaults.

## Project layout

```
src/
├── main.rs            # async timer loop + per-tick orchestration
├── config.rs          # config.toml loading (serde defaults)
├── model.rs           # feed Aircraft, display FlightInfo, TickResult snapshot
├── geo.rs             # haversine, Geofence, closest selection
├── provider/          # position feeds behind the FlightProvider trait
│   └── adsb_lol.rs
├── enrich/            # route/airline enrichment behind Enricher
│   ├── airlines.rs    #   embedded ICAO designator → airline table
│   └── hexdb.rs       #   route + airport name client
├── output/            # renderers behind the FlightOutput trait
│   ├── terminal.rs    #   plain-text sink
│   └── web_state.rs   #   shared state sink feeding the web server
└── web/               # axum web UI (page + /api/latest JSON)
```

The three traits (`FlightProvider`, `FlightOutput`, and the `Enricher`
seam) are the extension points for the planned next steps: an OpenSky or
FlightRadar24 provider, a text-based web UI, and an E-ink display renderer.
Additional feed fields already parsed (e.g. `track`, heading) are there for
a compass display.

## Tests & linting

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```
