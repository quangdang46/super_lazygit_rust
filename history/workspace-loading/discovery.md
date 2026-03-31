## Architecture Snapshot
> gkg available but local subcommand surface differs from the skill expectations; used scope/fff/linehash fallback.
Generated: 2026-03-29T18:34:00Z
Key modules:
- crates/app/src/main.rs: runtime bootstrap, watcher wiring, periodic refresh loop
- crates/core/src/reducer.rs: event reducer, scan lifecycle, refresh scheduling, watcher degradation handling
- crates/core/src/state.rs: workspace/repo state models, visible/prioritized repo selection
- crates/tui/src/lib.rs: workspace and repo rendering, key routing, status badges

## Existing Patterns
Query: "workspace loading empty partial failure switching"
Matches:
- crates/core/src/reducer.rs: scan completion and watcher health transitions drive workspace shell state
- crates/tui/src/lib.rs: workspace header/status lines already expose root and watcher state
- crates/app/src/main.rs: degraded watcher mode already falls back to polling via periodic refresh

## Dependency Graph
File: crates/core/src/reducer.rs
Imports: action/state/effects/domain event types
Imported by: crates/app runtime loop and tests via core public API
