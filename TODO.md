# TODO

## Completed

### Raspberry Pi 4 Optimization (Jan 2026)

- [x] Remove `thirtyfour` (WebDriver/Selenium) dependency
- [x] Delete `src/scraper/subsplease.rs` (WebDriver-based, replaced by RSS)
- [x] Reduce tokio features from "full" to specific needed features
- [x] Add optimized release profile (LTO, strip, codegen-units=1, panic=abort)

### SubsPlease RSS + Taiga-Style Filter System (Jan 2026)

- [x] Add filter tables to database schema (`filter_rules`, `show_filter_overrides`)
- [x] Create `src/db/filters.rs` with filter models and CRUD operations
- [x] Create `src/scraper/filter_engine.rs` with filter application logic
- [x] Add SubsPlease RSS parser to `src/scraper/rss.rs` with `RssSource` enum
- [x] Update `src/scraper/tracker.rs` to use source selection and filter engine
- [x] Add filter management API endpoints to `src/pages/home.rs`
- [x] Seed default filters in database initialization

### Docker Memory Optimization (Jan 2026)

- [x] Add shared HTTP client with connection pooling (`src/scraper/mod.rs`)
- [x] Update `anilist.rs`, `transmission.rs`, `rss.rs` to use shared client
- [x] Limit Tokio worker threads to 2 in `main.rs`
- [x] Verified: Container uses ~2 MiB RAM at idle, runs fine with 64MB limit

### Git Branch Merge (Jan 2026)

- [x] Rebase local filter system onto remote match selection workflow
- [x] Resolve conflicts in main.rs, home.rs, mod.rs (kept both features)
- [x] Add season_parser and filter_engine to scraper module
- [x] Remove Pi-specific files (.cargo/config.toml, Dockerfile.pi, release-pi profile)
- [x] Linear history achieved without merge commits

### Auto-Detect Fansub Source (Jan 2026)

- [x] Add `detect_fansub_source()` to `src/scraper/rss.rs` - parses [Group] from titles
- [x] Add `source` field to `MatchCandidate` struct
- [x] Add `source` field to `Link` struct in nyaasi.rs
- [x] Update `search_rss_matches()` to detect source from RSS feed items
- [x] Update `search_nyaasi_matches()` to detect source from Nyaa.si results
- [x] Update `has_exact_match()` to return (title, source) tuple
- [x] Update `confirm_match()` handler to use detected source from payload
- [x] Update `set_tracker()` to use detected source for auto-matched shows
- [x] Update match_selection.html to display Source column and pass source through
- [x] Supports: SubsPlease, Erai-raws, HorribleSubs, Judas, Yameii, Ember, ASM + any custom group

## Future Work

### Web UI for Filters

- [ ] Add filter management page (`/filters`)
- [ ] Add filter list table with enable/disable toggles
- [ ] Add filter create/edit form
- [ ] Add per-show filter overrides in show configuration modal
