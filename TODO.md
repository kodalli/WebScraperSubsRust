# TODO

## Completed

### Raspberry Pi 4 Optimization (Jan 2026)

- [x] Remove `thirtyfour` (WebDriver/Selenium) dependency
- [x] Delete `src/scraper/subsplease.rs` (WebDriver-based, replaced by RSS)
- [x] Delete `src/scraper/raii_process_driver.rs`
- [x] Reduce tokio features from "full" to specific needed features
- [x] Add optimized release profile (LTO, strip, codegen-units=1, panic=abort)
- [x] Add `release-pi` profile with size optimization (opt-level=z)
- [x] Create `.cargo/config.toml` for aarch64 cross-compilation
- [x] Create `Dockerfile.pi` for minimal container deployment

### SubsPlease RSS + Taiga-Style Filter System (Jan 2026)

- [x] Add filter tables to database schema (`filter_rules`, `show_filter_overrides`)
- [x] Create `src/db/filters.rs` with filter models and CRUD operations
- [x] Create `src/scraper/filter_engine.rs` with filter application logic
- [x] Add SubsPlease RSS parser to `src/scraper/rss.rs` with `RssSource` enum
- [x] Update `src/scraper/tracker.rs` to use source selection and filter engine
- [x] Add filter management API endpoints to `src/pages/home.rs`
- [x] Seed default filters in database initialization

## Future Work

### Web UI for Filters

- [ ] Add filter management page (`/filters`)
- [ ] Add filter list table with enable/disable toggles
- [ ] Add filter create/edit form
- [ ] Add per-show filter overrides in show configuration modal
