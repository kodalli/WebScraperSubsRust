# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Anime tracking and auto-download system built in Rust. Combines a web UI for browsing/tracking anime with automated RSS feed monitoring, Transmission BitTorrent integration, and VPN support. Runs as a single Docker container with integrated Transmission and OpenVPN.

## Build & Development Commands

```bash
# Install dependencies
pnpm i                    # Node dependencies for Tailwind
cargo install cargo-make  # Build task runner
cargo install cargo-watch # File watcher for dev

# Development (runs Tailwind watcher + cargo watch in parallel)
cargo make run            # Starts on port 42069

# Build styles only
cargo make styles         # Watches styles/tailwindcss.css

# Production Docker build
docker build -f docker/Dockerfile -t anime-tracker .
docker-compose up -d
```

## Architecture

### Module Structure

- **`src/main.rs`** - Axum web server setup, routes, and AppState definition
- **`src/db/`** - SQLite database layer with global connection pool via `OnceLock<Mutex<Connection>>`
  - `mod.rs` - Connection management, `with_db()` and `with_db_mut()` async wrappers
  - `shows.rs` - Show CRUD operations
  - `config.rs` - RSS configuration
  - `history.rs` - Download history tracking
  - `schema.rs` - DB init and JSON migration
- **`src/pages/`** - Axum handlers and Askama templates
  - `home.rs` - Main UI handlers, user state, API endpoints
  - `html_template.rs` - Template wrapper with custom headers
- **`src/scraper/`** - Content fetching
  - `anilist.rs` - GraphQL API for seasonal anime metadata
  - `nyaasi.rs` - Nyaa.si HTTP scraping for torrent search
  - `rss.rs` - Nyaa.si RSS feed parsing (primary for auto-download)
  - `transmission.rs` - Transmission RPC client
  - `tracker.rs` - Background task for automated episode monitoring
  - `subsplease.rs` - WebDriver scraping (requires geckodriver, optional)

### Key Patterns

**Database Access**: Global `OnceLock<Mutex<Connection>>` with async wrapper functions:
```rust
db::with_db(|conn| { /* read operations */ }).await
db::with_db_mut(|conn| { /* write operations */ }).await
```

**State Management**: `AppState` contains `Arc<Mutex<UserState>>` shared across handlers.

**Background Tracker**: Spawned at startup via `tokio::spawn(run_tracker())`, polls RSS feeds based on configurable `poll_times_per_day`.

### Data Flow

1. **Seasonal browsing**: AniList GraphQL -> anime metadata with cover images
2. **Torrent search**: Nyaa.si HTTP scraping -> results table
3. **Auto-download**: RSS feed parsing -> Transmission RPC -> download history

### Templates

Askama templates in `templates/` with Tailwind CSS. Base layout at `templates/layouts/base.html`, components in `templates/components/`.

## Environment Variables

Key variables (see `.env.example`):
- `PORT` - Web server port (default: 42069 dev, 8080 docker)
- `DATABASE_PATH` - SQLite database location (default: tracker.db)
- `TRANSMISSION_HOST`, `TRANSMISSION_PORT` - Transmission RPC connection
- `OPENVPN_PROVIDER`, `OPENVPN_USERNAME`, `OPENVPN_PASSWORD` - VPN config

## Docker Deployment

Multi-stage build based on `haugene/transmission-openvpn`. The Rust binary hooks into `tunnelUp.sh` to start after VPN connects. Ports: 9091 (Transmission), 8080 (Anime Tracker).
