# super_lazygit_rust Master Plan

## 0. Document Status

- Status: active planning document
- Planning mode: flywheel-planner, plan-space only
- Repo: `super_lazygit_rust`
- Primary language target: Rust
- Primary delivery target: terminal-native Git TUI
- Primary user target: terminal-first developer managing many repositories on low-RAM machines
- Current planning objective: expand the original high-level outline into an explicit product and architecture specification
- Long-term planning objective: iterate this document through multiple refinement rounds until it reaches "bead-ready" stability
- Document growth target: this plan should continue expanding over multiple rounds toward the 3,000-6,000+ line scale recommended by the flywheel-planner workflow
- Current round objective: make the plan materially more concrete, more explicit, and more grounded in the local references

## 1. Foundation Bundle

### 1.1 Planning Preconditions

- Tech stack decision: sufficiently clear to plan
- Product intent: sufficiently clear to plan
- User workflows: partially clear, expanded in this document
- Repo guidance: present in `AGENTS.md`
- Reference implementations: present under `references/`

### 1.2 Tech Stack Decision

The plan assumes:

- Rust workspace at repo root
- no `src/` at repo root
- multiple crates under `crates/`
- `ratatui` + `crossterm` for UI shell
- `notify` or equivalent for filesystem watching
- `gix`, `git2`, and Git CLI behind a unified facade
- channel-based concurrency rather than a large async runtime-first architecture

### 1.3 Why the Stack Is Clear Enough

- `gitui` proves that a high-quality Git TUI in Rust is viable
- `gitui` also provides concrete Rust-side patterns for watchers, diff/state management, and asynchronous Git work
- `git-dash` proves a simple Rust multi-repo dashboard can be fast and responsive
- `lazygit` remains the strongest reference for single-repo feature depth and UX philosophy
- `git-scope` gives additional evidence that the multi-repo "command center" problem is real and valuable

### 1.4 Remaining Preconditions Still Missing

- There is no actual Rust workspace scaffold yet
- There is no code-level architecture yet
- There is no explicit feature-to-phase bead structure yet
- There is no performance harness yet
- There is no UX mock or static TUI prototype yet

These are implementation gaps, not planning blockers.

## 2. Reference Map

This plan is intentionally grounded in the local `references/` folder.

### 2.1 Primary References

- `references/lazygit-master/README.md`
- `references/lazygit-master/VISION.md`
- `references/lazygit-master/docs/keybindings/Keybindings_en.md`
- `references/lazygit-master/docs/dev/Codebase_Guide.md`
- `references/gitui-master/README.md`
- `references/gitui-master/KEY_CONFIG.md`
- `references/gitui-master/src/watcher.rs`
- `references/gitui-master/src/tabs/status.rs`
- `references/gitui-master/asyncgit/Cargo.toml`
- `references/gitui-master/asyncgit/src/sync/status.rs`
- `references/gitui-master/asyncgit/src/sync/hunks.rs`
- `references/git-dash-main/README.md`
- `references/git-dash-main/src/app.rs`
- `references/git-dash-main/src/worker.rs`
- `references/git-dash-main/src/status.rs`
- `references/git-scope-main/README.md`
- `references/git-scope-main/internal/cache/cache.go`
- `references/git-scope-main/internal/tui/app.go`

### 2.2 What Each Reference Contributes

`lazygit` contributes:

- repo-mode feature inventory
- repo-mode layout semantics
- keybinding philosophy
- discoverability/safety/speed principles
- examples of power-user Git flows that matter in practice

`gitui` contributes:

- Rust-first architectural patterns
- watcher implementation patterns
- diff and status component patterns
- hybrid Git backend evidence
- customizable keybinding model

`git-dash` contributes:

- multi-repo dashboard baseline
- worker-pool status refresh model
- concise repo table information design
- simple, pragmatic Rust implementation patterns

`git-scope` contributes:

- strong framing for workspace-first Git tooling
- caching strategy ideas
- workspace switching and search/filter ergonomics
- evidence that timeline, recent activity, and macro visibility matter

### 2.3 Reference-Derived Product Statement

The product should become:

- `git-dash` outside the repo
- a 1:1 `lazygit` clone inside the repo for UI/UX, keybindings, panel semantics, and workflow coverage
- Rust-native in implementation
- Lazygit-like in visual language throughout repo mode
- better than both for real-time multi-repo terminal work

## 3. Core Product Thesis

`super_lazygit_rust` should be a Rust-native workspace shell that embeds a deliberate 1:1 Lazygit clone for repo-mode UX.

It should be:

- a workspace command center for many repositories
- with a deep single-repo mode that intentionally clones Lazygit's UI/UX, keybindings, panel layout, and workflow semantics as closely as possible
- with low-resource realtime awareness
- with a consistent terminal-native UI across both scopes

### 3.1 Why a Whole-Product Lazygit Port Is the Wrong Product

Lazygit is excellent at:

- staging precisely
- reviewing diffs
- committing quickly
- managing branches and history
- handling advanced Git flows

Lazygit is not primarily designed for:

- managing 5-50 repos at once
- scanning many repos for attention
- seeing stale sync state across a workspace
- recovering macro context after working in many projects

Your pain points begin before the single-repo workflow starts.
That changes the whole-product architecture, but it does not weaken the requirement that repo mode itself should target a faithful Lazygit clone.

### 3.2 The Sharp Product Rule

The product should obey one simple rule:

- outside a repo, think `git-dash`
- inside a repo, clone `Lazygit` 1:1 unless Git correctness, terminal constraints, or explicit product safety requirements make that impossible
- visually, feel like Lazygit throughout

### 3.3 The Strongest Version of the Product

The strongest version of `super_lazygit_rust` is:

- macro visibility first
- micro control second
- live freshness as the differentiator

Not:

- a generic Git TUI
- a whole-product clone that ignores the workspace command-center problem
- a workspace dashboard with no deep actions
- a deep repo UI with a weak workspace view

## 4. User and Constraint Model

### 4.1 Primary User

The primary user is:

- technical
- terminal-native
- keyboard-driven
- managing many codebases
- often working in Rust or similarly CLI-heavy environments
- frequently context-switching across repositories
- operating with limited RAM and low tolerance for IDE overhead

### 4.2 Typical Real-World Environments

- low-RAM Linux laptop
- remote server over SSH
- tmux-heavy workstation
- Codespaces or devcontainers
- monorepo plus many side repositories
- microservice workspace
- infrastructure plus application repos

### 4.3 Hard Constraints

- must work well in one terminal window
- must not require an always-on background daemon
- must remain usable on low-RAM systems
- must not block the UI during Git operations
- must preserve Git behavior and user trust
- must avoid storing fragile proprietary session state when Git itself can be the source of truth

### 4.4 Soft Constraints

- startup should feel fast
- learning curve should be gentle for Lazygit users
- advanced flows should remain reachable for power users
- configuration should be minimal and sensible by default

## 5. Pain Point Mapping

This section grounds the plan directly in `PAINPOINT.md`.

### 5.1 P1 - No Multi-Repo Visibility

Problem:

- user cannot see repo state across the workspace at once

Required product response:

- workspace dashboard
- repo discovery
- live dirty/ahead/behind indicators
- sort/filter/search
- glanceable attention ordering

### 5.2 P2 - Must Open IDE Just To See a Diff

Problem:

- terminal diff exists, but actionability is poor

Required product response:

- repo-mode diff pane
- integrated stage/unstage
- inline file/hunk/line actions
- commit flow in the same screen

### 5.3 P3 - No Realtime File Watching

Problem:

- state goes stale
- user loses trust in the dashboard

Required product response:

- watcher-based invalidation
- debounce and background refresh
- freshness indicators
- fallback polling if watcher backend degrades

### 5.4 P4 - Cannot Stage Selectively Without an IDE

Problem:

- `git add -p` is too awkward

Required product response:

- hunk staging
- line staging
- range selection where feasible
- diff focus and navigation that make selective staging fast

### 5.5 P5 - Commit + Push Requires Leaving Current Context

Problem:

- commit loop is fragmented

Required product response:

- commit box in repo mode
- push/pull/fetch without leaving the diff/status screen
- explicit operation progress and confirmations

### 5.6 P6 - No Overview of Sync State Across Repos

Problem:

- stale or divergent branches go unnoticed

Required product response:

- ahead/behind/diverged indicators in workspace mode
- optional background fetch policy
- stale-fetch age column

### 5.7 P7 - Repo Switching Destroys Focus

Problem:

- switching repos costs orientation time

Required product response:

- one app for many repos
- one keystroke to descend into repo mode
- preserve workspace selection and filters on return
- keep a notion of recent repo history

### 5.8 P8 - No Commit History Visibility Without Leaving Terminal

Problem:

- `git log --oneline` is insufficient

Required product response:

- commit list in repo mode
- commit diff preview
- branch/ref comparison
- reflog/history navigation

### 5.9 Pain Point Coverage Matrix

Phase 1 should solve:

- P1
- part of P6
- part of P7

Phase 2 should solve:

- P3

Phase 3 should solve:

- P2
- P4
- P5

Phase 4 should solve:

- P8
- more of P7

Phase 5 should close:

- advanced Lazygit parity gaps

## 6. Success Criteria

### 6.1 Product Success Criteria

The product is successful if a target user can:

- open one app and immediately know which repos need attention
- enter a repo, stage exact changes, commit, and push without opening an IDE
- trust that repo status is fresh enough to rely on
- move between repositories without losing orientation
- do their most common Git work without dropping to shell often

### 6.2 Usability Success Criteria

- a Lazygit user should recognize repo mode quickly
- a new user should discover core actions without memorizing the whole keymap
- confirmation prompts should be predictable
- disabled actions should explain why
- `esc` should reliably back out of transient states

### 6.3 Performance Success Criteria

- warm start should feel near-instant for medium workspaces
- UI must stay responsive during scans and Git commands
- one busy repo should not stall the entire workspace
- watch-triggered refresh should feel live without causing churn

### 6.4 Trust Success Criteria

- actions should honor Git config where possible
- the app should not surprise users with hidden state
- write operations should behave like Git users expect
- destructive actions should require clear confirmation

## 7. Non-Goals

### 7.1 Clear Non-Goals for v1

- cloning Lazygit's internal Go architecture or implementation details instead of its user-facing repo-mode behavior
- PR review or forge workflow integration
- issue tracker integration
- background daemon architecture
- collaborative or networked features
- replacing shell scripting for unusual Git workflows

### 7.2 Likely Future Non-Goals

- embedded code editor
- full IDE replacement
- complex visual dashboards unrelated to Git state
- over-configurable theming and layout permutations in early versions

## 8. Product Principles

This section adapts Lazygit's design principles and extends them for multi-repo use.

### 8.1 Discoverability

- the app should surface contextual key hints
- visible commands should reflect current focus
- actions should be named clearly
- users should not need to memorize every key
- search and filter should be first-class
- if an action is unavailable, show the reason

### 8.2 Simplicity

- common flows must be dead simple
- avoid forcing users through nested menus for routine tasks
- prefer obvious defaults
- configuration must not explode prematurely

### 8.3 Safety

- destructive actions require confirmation
- force-push, reset, discard, and nuke flows must be explicit
- `esc` should cancel or step back predictably
- merge/rebase/conflict states must be obvious

### 8.4 Power

- common advanced Git flows should remain reachable
- power should come from tight workflows, not sprawling configuration
- rare edge-cases should still fall back cleanly to shell or custom commands later

### 8.5 Speed

- keypress count matters
- background work must be non-blocking
- startup should not wait on expensive work
- muscle memory should transfer across contexts

### 8.6 Conformity with Git

- work with Git, not against it
- if Git CLI semantics differ from library behavior, prefer the CLI where it matters
- honor credential helpers, hooks, config, and branch/upstream semantics
- keep app-specific hidden magic to a minimum

### 8.7 Think of the Codebase

- keep write operations behind a small facade
- keep view rendering separate from business logic
- avoid imitating Lazygit's Go architecture literally
- design for testability from the start

### 8.8 Multi-Repo Principle

This is the new principle that the original Lazygit vision does not cover.

- the app must help users decide where to spend attention
- the default home screen should optimize triage, not deep interaction
- workspace state should remain lightweight and glanceable
- the app should feel useful even before entering repo mode

## 9. Reference-Derived Design Decisions

### 9.1 Decisions Borrowed from Lazygit

- repo mode should mirror Lazygit's panel relationships closely
- contextual help should be persistent
- safety prompts should be explicit
- screen modes and focus semantics should be predictable
- repo mode should favor dense information display over decorative emptiness

### 9.2 Decisions Borrowed from GitUI

- use Rust-native components rather than emulating gocui
- treat watchers as event sources that trigger updates
- keep status and diff UI as separate but coordinated components
- allow customizable keybinding files later
- accept that hybrid Git backends are often necessary

### 9.3 Decisions Borrowed from git-dash

- multi-repo status should be table-like and scan-friendly
- status refresh should happen in background workers
- progress during scan/refresh should be visible
- columns should prioritize branch/dirty/ahead-behind/remote freshness

### 9.4 Decisions Borrowed from git-scope

- workspace mode is a valid product center, not just a launcher
- caching matters for fast startup
- search/filter/switch-workspace flows deserve first-class UX
- timeline/recent activity ideas are worth later expansion

## 10. Global Information Architecture

### 10.1 Top-Level Modes

The app should have these top-level modes:

- `WorkspaceMode`
- `RepoMode`
- `ModalOverlay`

`ModalOverlay` is not a true peer mode.
It is a transient layer for:

- confirmations
- prompts
- picker menus
- progress dialogs
- help

### 10.2 Workspace-to-Repo Transition

Expected user flow:

- start in workspace mode
- identify repo that needs action
- press `enter`
- land directly in repo-mode status view for that repo
- complete work
- press `esc` or a dedicated back key
- return to workspace mode with selection, filters, and scroll position preserved

### 10.3 Repo-to-Workspace Return Rules

Returning to workspace mode should preserve:

- selected repo
- sort mode
- filters
- search query where reasonable
- workspace path/group selection

### 10.4 Focus Model

Focus should always be explicit.

The app should always know:

- active mode
- active pane
- active item
- active overlay if any
- whether the current action is blocked on a background operation

## 11. Workspace Mode Specification

### 11.1 Purpose

Workspace mode is the product's command center.

Its job is to answer:

- which repos need attention
- what kind of attention
- what changed recently
- where should I jump next

### 11.2 Structural Inspiration

Workspace mode should be structurally inspired by `git-dash`.

That means:

- dense row-oriented repo list
- fast navigation
- immediate visibility of status columns
- low-friction refresh/search/filter

### 11.3 Visual Style

Workspace mode should still feel visually close to Lazygit:

- familiar border density
- familiar highlight language
- similar bottom-line help treatment
- consistent warning/danger color semantics
- consistent selected-row behavior

### 11.4 Default Layout

Recommended baseline layout:

- left/main: repo table
- right/top: selected repo summary
- right/middle: selected repo changed-files summary or last local activity
- right/bottom: preview area for last commits, diff summary, or operation output
- bottom line: mode, key hints, watcher health, fetch policy, status messages

### 11.5 Repo Table Columns

Required columns:

- repository name
- branch
- dirty state
- staged count
- unstaged count
- untracked count
- ahead/behind
- remote shortcut
- last fetch age
- last activity age

Optional columns for later:

- group/team
- disk cost
- worktree count
- conflict state
- error badge

### 11.6 Row Semantics

A row must make it obvious whether the repo is:

- clean
- dirty
- staged
- untracked
- ahead
- behind
- diverged
- conflicted
- fetch-stale
- unavailable

### 11.7 Selected Repo Summary Pane

When a repo is selected, the summary pane should show:

- absolute or abbreviated path
- current HEAD branch or detached HEAD
- upstream branch if present
- ahead/behind state
- last fetch time
- watcher freshness state
- current operation if any
- last refresh age

### 11.8 Selected Repo Preview Modes

The preview area should support multiple sub-modes:

- changed files summary
- recent commits
- lightweight diff summary
- operation log
- timeline snippet

This preview should be switchable without leaving workspace mode.

### 11.9 Primary Workspace Actions

Required:

- search repos
- filter repos
- sort repos
- refresh selected repo
- refresh all visible repos
- fetch selected repo
- fetch visible repos
- open repo mode
- switch workspace root/group

Later:

- bulk fetch selected repos
- bulk pull fast-forward-only for safe repos
- mark favorites
- pin groups
- open external editor from workspace mode

### 11.10 Workspace Search

Search should match:

- repo name
- path
- branch
- remote text
- group label

Search should be:

- incremental
- non-blocking
- fuzzy enough to feel forgiving

### 11.11 Workspace Filters

Initial filter set:

- all
- dirty
- clean
- staged
- ahead
- behind
- diverged
- conflicts
- stale
- recently active

Later:

- grouped views
- custom saved filters

### 11.12 Workspace Sort Modes

Required sorts:

- attention score
- name
- branch
- recent local activity
- ahead/behind severity
- fetch staleness

### 11.13 Workspace Persistence

The app should remember:

- last opened workspace root
- recent workspace roots
- active sort mode
- default filter
- column visibility choices if this becomes configurable later

## 12. Repo Mode Specification

### 12.1 Purpose

Repo mode is the deep work view.

Its job is to answer:

- what changed
- what should be staged
- what commit is needed
- what sync action is needed
- what recent history matters

### 12.2 Structural Inspiration

Repo mode should intentionally mirror Lazygit closely.

That means:

- status-first orientation
- files/staging/diff loop
- side and main panes with strong focus semantics
- contextual bottom help
- transient overlays for options and confirmations

### 12.3 Core Repo Subviews

Required subviews:

- status
- branches
- commits
- stash
- remotes
- reflog
- worktrees

Later:

- tags
- submodules
- patch explorer equivalents
- merge conflict dedicated tools

### 12.4 Status View Layout

Initial target layout:

- left/top: working tree tree/list
- left/bottom: staged tree/list
- right/main: diff/details
- bottom strip: branch, upstream, operation state, key hints

Alternative layouts should be possible later:

- fullscreen diff
- compact half-screen
- panel zoom mode

### 12.5 Status View Panel Contracts

Working tree panel should provide:

- file tree
- directory collapse/expand
- filter by file status
- stage/unstage all
- open file
- edit file
- discard/reset options
- stash shortcuts

Staged panel should provide:

- staged file list/tree
- unstage actions
- amend-ready visibility
- commit readiness cues

Diff panel should provide:

- syntax-aware color where feasible
- hunk boundaries
- line-level selection hooks
- stage/unstage current selection
- diff target awareness

### 12.6 Commit Entry

Commit entry should live in repo mode, not a separate terminal context.

Required behavior:

- write commit message inline or in prompt overlay
- commit staged changes
- amend HEAD commit
- optional bypass hooks action with explicit warning
- validation for empty commit messages where appropriate

### 12.7 Branch View

Required actions:

- checkout
- create branch
- rename branch
- delete branch
- set/unset upstream
- fast-forward from upstream
- merge selected branch
- rebase current branch onto selected branch
- create worktree from branch later

### 12.8 Commits View

Required actions:

- inspect commit list
- open commit files
- compare commits/refs
- cherry-pick selected commit
- revert selected commit
- tag selected commit
- create branch from commit
- amend older commit later
- reword later
- interactive rebase later

### 12.9 Stash View

Required actions:

- create stash
- apply stash
- pop stash
- inspect stash diff
- drop stash

### 12.10 Reflog View

Required actions:

- see recent branch/head movements
- inspect diff target from reflog
- support recovery-oriented flows later

### 12.11 Worktrees View

Required later:

- list worktrees
- create worktree from branch
- switch into worktree path
- remove worktree with confirmations

## 13. Global Interaction Model

### 13.1 Keybinding Philosophy

The keybinding model should inherit from Lazygit and GitUI:

- keyboard-first
- contextual
- visible
- learnable
- customizable later

### 13.2 Baseline Navigation Keys

Recommended default:

- `j/k` or arrows: move selection
- `h/l` or `tab` and `shift-tab`: switch pane/focus
- `enter`: descend/open/confirm
- `esc`: back/cancel/close overlay
- `/`: search current context
- `?`: keybinding/help overlay

### 13.3 Action Keys

Recommended defaults:

- `space`: stage or toggle
- `c`: commit
- `p`: pull
- `P`: push
- `f`: fetch
- `r`: refresh current context
- `R`: full refresh or background refresh semantics
- `s`: stash or sort depending on context, only if conflict is avoidable

### 13.4 Keybinding Safety Rules

- avoid overloaded meanings when the context is not obvious
- preserve muscle memory where possible
- if a key is disabled, explain why
- favor disabling menu entries over hiding them

### 13.5 Help Model

The app should always offer:

- bottom-line key hints
- a full help/keybinding overlay
- contextual descriptions for transient menus
- reasons when actions are unavailable

### 13.6 Search Model

Search should exist in:

- workspace repo list
- branches list
- commits list
- files list
- stash list

Search behavior should be:

- incremental where possible
- cancelable with `esc`
- context-specific rather than one global search bar

### 13.7 Range Selection

Range selection is valuable for:

- commit lists
- file selection
- hunk selection

It should be supported later only if it does not destabilize the state model.

## 14. Workflow Specifications

### 14.1 Workflow A - Morning Workspace Triage

Goal:

- see where attention is needed across all repos

Steps:

- open app
- workspace cache loads immediately
- background refresh begins
- attention-sorted repo list appears
- user filters to dirty or behind repos
- user previews recent activity
- user enters a selected repo

Success condition:

- user knows where to start within seconds

### 14.2 Workflow B - Selective Stage and Commit

Goal:

- create a clean atomic commit without opening an IDE

Steps:

- enter repo mode
- inspect working tree files
- enter diff
- select hunk or line
- stage selection
- review staged tree
- write commit message
- commit

Success condition:

- user never leaves the app

### 14.3 Workflow C - Commit and Push

Goal:

- finish local work and sync it upstream

Steps:

- repo mode shows ahead state
- user commits
- push action is visible
- push progress is shown
- final state refreshes automatically

Success condition:

- user sees push result and final clean/ahead-behind state in one place

### 14.4 Workflow D - Pull and Resolve Staleness

Goal:

- update a behind repo safely

Steps:

- workspace mode highlights repo as behind
- user enters repo mode or acts from workspace mode
- user fetches and previews state
- user performs pull or rebase flow
- repo refreshes

Success condition:

- user clearly understands sync result and next action

### 14.5 Workflow E - Branch Inspection

Goal:

- move between branches without losing context

Steps:

- open branches view
- filter/search branches
- inspect ahead/behind relative to upstream where possible
- checkout branch
- status view updates

Success condition:

- branch change is obvious and post-checkout state is trustworthy

### 14.6 Workflow F - History Archaeology

Goal:

- inspect recent commits and understand what changed

Steps:

- open commits view
- select commit
- preview diff or files
- compare against another ref later if desired

Success condition:

- user does not need browser or IDE for basic history review

### 14.7 Workflow G - Rebase and Fixup

Goal:

- handle common history cleanup flows

Early scope:

- limited support or deferred

Later scope:

- interactive rebase
- fixup/squash
- move commits
- amend older commit

Success condition:

- advanced Git users feel the roadmap is credible even before full parity lands

### 14.8 Workflow H - Merge Conflict Handling

Goal:

- make conflicts obvious and recoverable

Early scope:

- detect conflicts
- highlight conflicted files
- guide user to file and operation options

Later scope:

- conflict hunk selection
- conflict-specific actions
- better visual conflict assistance

## 15. Feature Matrix

### 15.1 Must-Have for MVP

- repo discovery
- multi-repo table
- cache-backed startup
- watcher-backed invalidation
- repo-mode status view
- diff pane
- file/hunk staging
- commit
- fetch/pull/push
- branch checkout
- commit history preview

### 15.2 Should-Have Soon After MVP

- line-level staging
- stash view
- reflog view
- compare refs/commits
- worktree basics
- custom keybinding file

### 15.3 Power Features Worth Planning Now

- interactive rebase
- cherry-pick
- amend older commit
- patch explorer/custom patch workflows
- external editor integration
- custom shell commands

### 15.4 Explicitly Deferred

- forge integrations
- PR creation
- issue tracking
- remote review UI
- complicated dashboards unrelated to Git work

## 16. Technical Architecture

### 16.1 Workspace Layout

Recommended repo structure:

```text
super_lazygit_rust/
  Cargo.toml
  crates/
    app/
    core/
    tui/
    git/
    workspace/
    config/
    test-support/
```

### 16.2 Crate Responsibilities

`crates/app`:

- CLI entrypoint
- config discovery
- logging bootstrap
- app bootstrapping
- panic/report wiring

`crates/core`:

- app state
- actions/events
- reducers
- selection models
- mode transitions
- shared domain types

`crates/tui`:

- rendering
- layout
- input translation
- widgets
- view controllers or presenters

`crates/git`:

- `GitFacade`
- backend routing
- command execution
- Git result normalization
- write-operation safety wrappers

`crates/workspace`:

- repo discovery
- repo registry
- caching
- watcher orchestration
- background refresh scheduling

`crates/config`:

- config file schema
- defaults
- theme config
- keybinding config
- workspace persistence

`crates/test-support`:

- temp repo fixtures
- synthetic Git topologies
- UI harness helpers

### 16.3 Concurrency Model

Use a message-driven, channel-based concurrency model:

- UI thread handles input and render scheduling
- worker threads handle Git reads and writes
- watcher threads emit repo invalidation events
- reducer serializes state updates

Why this over full async-runtime-first design:

- simpler mental model for TUI state
- easier deterministic testing
- fewer cross-cutting async concerns in rendering
- sufficient for this workload class

### 16.4 Core Runtime Loop

The runtime loop should process:

- input events
- worker completion events
- watcher invalidation events
- timer events
- render ticks when needed

The reducer should produce:

- state updates
- effects/jobs to schedule
- notifications/toasts
- render invalidation flags

### 16.5 State Ownership Rule

Business state belongs in `core`, not UI widgets.

Widgets may own:

- local scroll position
- render cache
- text measurement cache

Widgets should not own:

- authoritative repo state
- operation lifecycle state
- cross-pane coordination rules

## 17. State Model

### 17.1 AppState

`AppState` should include:

- active top-level mode
- focused pane id
- modal stack
- status message queue
- notification queue
- background jobs map
- global settings snapshot
- recent repo stack

### 17.2 WorkspaceState

`WorkspaceState` should include:

- current workspace root or group
- discovered repo ids
- repo summary map
- selected repo id
- sort mode
- filter mode
- search query
- preview mode
- scan status
- watcher health
- last full refresh timestamp

### 17.3 RepoModeState

`RepoModeState` should include:

- current repo id
- active subview
- status view state
- branches view state
- commits view state
- stash view state
- reflog view state
- worktree view state
- operation progress state

### 17.4 RepoSummary

`RepoSummary` is the lightweight workspace-level model.

It should include:

- repo id
- display name
- real path
- display path
- branch
- head kind
- dirty bool
- staged count
- unstaged count
- untracked count
- ahead count
- behind count
- conflict bool
- last fetch timestamp
- last local activity timestamp
- last refresh timestamp
- watcher freshness
- remote summary
- last error if any

### 17.5 RepoDetail

`RepoDetail` is the heavier repo-mode model.

It should include:

- file tree status models
- diff model
- branch list
- commit list
- stash list
- reflog items
- worktree items
- commit input buffer
- merge/rebase state
- selected comparison target

### 17.6 View State vs Domain State

Domain state:

- Git data
- operation state
- repo summaries
- selected repo ids
- mode transitions

View state:

- scroll positions
- active tab
- expanded directories
- filter text cursor
- diff viewport

Keep them separate.

## 18. Event and Effect Model

### 18.1 Input Events

Examples:

- `KeyPressed`
- `Resize`
- `Paste`
- `MouseEvent` later if ever needed

### 18.2 Domain Actions

Examples:

- `EnterRepoMode`
- `LeaveRepoMode`
- `SelectNext`
- `RefreshSelectedRepo`
- `RefreshVisibleRepos`
- `StageSelection`
- `CommitStaged`
- `PushCurrentBranch`

### 18.3 Worker Events

Examples:

- `RepoScanCompleted`
- `RepoSummaryUpdated`
- `RepoDetailLoaded`
- `GitOperationStarted`
- `GitOperationCompleted`
- `GitOperationFailed`

### 18.4 Watcher Events

Examples:

- `RepoInvalidated`
- `WatcherDegraded`
- `WatcherRecovered`

### 18.5 Timer Events

Examples:

- `PeriodicRefreshTick`
- `PeriodicFetchTick`
- `ToastExpiryTick`

### 18.6 Effects

Reducers should emit effects like:

- start repo scan
- refresh repo summary
- load repo detail
- run Git write command
- persist cache
- persist config
- schedule render

## 19. Git Engine Strategy

### 19.1 Non-Negotiable Principle

All Git interactions must go through one coherent facade.

Do not let random parts of the UI call:

- `git2` directly
- `gix` directly
- `Command::new("git")` directly

### 19.2 Facade Overview

Recommended core trait families:

- `WorkspaceGitReader`
- `RepoGitReader`
- `RepoGitWriter`
- `RepoGitWatcherHints`

Or one facade with clearly separated read/write modules.

### 19.3 Backend Routing Decision

Use:

- `gix` for high-volume local reads when it is fast and reliable
- `git2` selectively for patch and repository helper operations when coverage is strong
- Git CLI for parity-critical writes and operations affected heavily by hooks/config/credential helpers

### 19.4 Evidence from References

Why not pure `git2`:

- `gitui` already mixes `gix` and `git2`
- `gitui` notes behavioral differences between `libgit2` and Git CLI in stash behavior
- rebase-like operations are especially sensitive to real Git semantics

Why not pure Git CLI:

- status refresh volume in workspace mode benefits from lower-level access
- local object/ref reads can be faster and more structured through libraries
- heavy shelling-out for every small read may add avoidable overhead

### 19.5 Read Operation Routing

Likely library-backed:

- repo status summary
- branch and ref reads
- revision walking
- commit metadata
- file tree status
- diff metadata

### 19.6 Write Operation Routing

Likely CLI-backed:

- fetch
- pull
- push
- interactive rebase
- cherry-pick
- revert
- worktree creation/removal
- any operation needing maximum fidelity with Git hooks and config

### 19.7 Patch Operation Routing

Candidate approaches:

- use `git2` apply/index functions for hunk operations when robust
- fall back to CLI patch generation/application for parity-critical edge cases

This area needs dedicated parity testing before finalizing.

## 20. Git Operation Semantics

### 20.1 Status

Workspace summary status must expose:

- dirty state
- staged/unstaged/untracked counts
- branch
- ahead/behind
- conflict state if detectable
- fetch age

Repo detail status must expose:

- itemized file changes
- staged and unstaged trees
- rename awareness where feasible
- untracked files
- conflict markers

### 20.2 Fetch

Fetch should support:

- selected repo
- visible repos later
- background optional policy
- explicit progress feedback where available

### 20.3 Pull

Default policy should be conservative.

Recommended default:

- fast-forward-only when safe
- explicit alternate actions for merge or rebase-based pulls later

### 20.4 Push

Push should support:

- normal push
- upstream setup flow when missing
- force-with-lease later
- explicit confirmation for dangerous variants

### 20.5 Commit

Commit should support:

- normal commit
- amend HEAD
- bypass hooks variant only with explicit warning
- external editor fallback later

### 20.6 Stage / Unstage

Must support:

- file-level stage/unstage
- hunk-level stage/unstage
- line-level stage/unstage if technically reliable

### 20.7 Branching

Must support:

- checkout branch
- create branch
- rename branch
- delete branch
- set upstream
- checkout previous branch later

### 20.8 History Rewrite

Planned later:

- squash/fixup
- reword
- interactive rebase
- amend older commit

These features must not ship half-safe.

## 21. Workspace Subsystem

### 21.1 Discovery Requirements

Discovery must:

- recursively scan configured roots
- recognize `.git` directories
- recognize gitdir files used by worktrees/submodules
- stop descending once a repo root is found
- support symlink resolution
- store both canonical and display paths

### 21.2 Discovery Ignore Rules

Support ignore globs for:

- `.git` internals not needed for traversal
- large generated directories
- common dependency trees
- user-defined exclusions

### 21.3 Cache Strategy

Borrowing from `git-scope`, cache should store:

- repo summaries
- cache timestamp
- workspace roots fingerprint
- maybe app schema version

### 21.4 Cache Goals

- fast warm startup
- avoid full rescans when unnecessary
- allow immediate render with stale-but-useful data
- refresh in background after paint

### 21.5 Repo Registry

The workspace subsystem should maintain:

- stable repo ids
- path to repo-id mapping
- root/group membership
- last known summary
- active watch subscriptions

### 21.6 Attention Scoring

Workspace list should default to attention score.

Candidate weighting:

- conflicts: highest
- diverged: very high
- behind: high
- ahead and dirty: medium-high
- dirty unstaged only: medium
- staged pending commit: medium
- stale fetch: medium
- recent local activity: bump

This scoring should be transparent enough to reason about and tunable later if necessary.

## 22. Realtime Watch Subsystem

### 22.1 Core Principle

Watchers should invalidate repo summaries and details.
Watchers should not try to compute final Git state from filesystem events alone.

### 22.2 Why This Principle Matters

Filesystem events are:

- noisy
- editor-dependent
- bursty
- platform-dependent

Trying to infer Git state directly from events invites bugs.

### 22.3 Watch Scope

Watch:

- repo working trees recursively
- `.git/HEAD`
- `.git/index`
- `.git/FETCH_HEAD`
- branch ref updates where practical

### 22.4 Event Coalescing

Watcher events should be:

- debounced
- grouped by repo id
- converted into refresh requests
- prioritized by visibility and focus

### 22.5 Refresh Priority Tiers

- active repo in repo mode: immediate
- selected repo in workspace mode: very fast debounce
- visible repos in list: short debounce
- hidden repos: lazy refresh

### 22.6 Watch Failure Policy

If watcher backend fails:

- show degraded badge
- log detail
- switch to timer-based refresh
- keep the app usable

### 22.7 Watch Resource Policy

The watcher subsystem must be low overhead.

Avoid:

- one wasteful thread per file
- recomputing full workspace on every event
- fetching on watcher invalidation

## 23. UI Architecture

### 23.1 Rendering Stack

Use:

- `ratatui` for layout and drawing
- `crossterm` for event/input terminal integration

### 23.2 UI Layering

Recommended layers:

- app shell
- mode layout
- pane widgets
- overlays
- status/toast layer

### 23.3 Component Model

Use components for:

- repo table
- file tree
- staged tree
- diff viewer
- commit list
- branches list
- stash list
- input prompt
- modal menu
- help overlay

### 23.4 Component Responsibility Rule

Components should:

- render
- own ephemeral view state
- emit actions

Components should not:

- run Git commands directly
- coordinate complex cross-pane business logic

### 23.5 Screen Modes

Borrowing from Lazygit, support later:

- normal mode
- larger main pane
- fullscreen-focused pane

This is valuable for:

- diff-heavy work
- log inspection
- narrow terminals

## 24. Configuration Model

### 24.1 Config Goals

- minimal defaults
- clear file location
- sensible zero-config startup
- gradual extensibility

### 24.2 Config Domains

- workspace roots
- ignore globs
- auto-refresh interval
- auto-fetch policy
- theme overrides
- keybinding overrides later
- editor command

### 24.3 Initial Config File Shape

Illustrative example:

```yaml
workspaces:
  - name: main
    roots:
      - ~/code
      - ~/work
ignore:
  - node_modules
  - target
  - .venv
ui:
  theme: lazygit-default
  show_key_hints: true
git:
  auto_refresh: true
  auto_fetch: false
editor:
  command: nvim
```

### 24.4 Keybinding Config

Do not over-build keybinding customization in v1.

But plan for it:

- keep action ids explicit
- map keys through config layer later
- borrow from GitUI's configurable key file model when ready

## 25. Error Handling and Trust Model

### 25.1 User-Facing Errors Must Be Clear

Examples:

- no upstream configured
- fetch failed
- authentication failed
- rebase in progress
- merge conflicts present
- repo inaccessible
- watcher degraded

### 25.2 Action Failures Must Preserve Orientation

After a failed action:

- the user should know what failed
- the affected repo should still be selected
- the app should refresh relevant state
- the error should be visible, not swallowed

### 25.3 Safety Confirmations

Must confirm:

- discard file changes
- reset hard
- nuke working tree
- delete branch
- force-like push operations
- destructive worktree removal

### 25.4 Dangerous Actions Must Explain Impact

The prompt should answer:

- what object will be affected
- whether it is reversible
- what Git command or conceptual operation is about to happen

## 26. Performance Model

### 26.1 Startup Budget

Warm startup target:

- render cached workspace quickly
- do not block on full scan

Cold startup target:

- show shell promptly
- scan in background
- display progress

### 26.2 Render Budget

- no Git I/O on render path
- heavy diff parsing must be cached or background-loaded
- only repaint what changed when practical

### 26.3 Workspace Scale Targets

Reasonable target classes:

- 5 repos: trivial
- 20 repos: normal
- 50 repos: should remain comfortable
- 100 repos: usable with some lazy-loading and limits

### 26.4 Heavy Repo Targets

Need to remain acceptable for:

- large Rust repos
- repos with many changed files
- histories with long commit lists

### 26.5 Background Operation Rules

- long operations should show progress state
- one stuck repo should not freeze the rest
- per-repo refresh should be incremental

## 27. Testing Strategy

### 27.1 Layer 1 - Reducer Tests

Test:

- mode transitions
- selection movement
- overlay open/close
- refresh scheduling decisions
- operation state transitions

### 27.2 Layer 2 - Git Integration Tests

Create temp repos covering:

- clean repo
- dirty repo
- staged/unstaged mix
- rename detection cases
- detached HEAD
- upstream configured and missing
- ahead/behind/diverged
- stash states
- conflict states

### 27.3 Layer 3 - Watcher Tests

Test:

- debounce behavior
- repo invalidation coalescing
- watcher degradation fallback
- active vs hidden repo prioritization

### 27.4 Layer 4 - UI Snapshot Tests

Snapshot:

- workspace table states
- repo status view
- diff view
- modals
- error states

### 27.5 Layer 5 - End-to-End Keyboard Flow Tests

Test flows:

- workspace triage -> enter repo -> stage -> commit
- fetch/pull/push
- branch checkout
- view commit diff
- stash create/apply

### 27.6 Manual Test Matrix

Manual testing should cover:

- low-RAM environment
- SSH session
- narrow terminal
- fast file churn from editors
- large number of repos

## 28. Documentation Strategy

### 28.1 Documentation Goals

- make the app learnable
- make advanced flows discoverable
- explain "workspace mode vs repo mode" clearly
- show why the tool exists

### 28.2 Required Docs

- README with product framing
- keybinding reference
- config reference
- troubleshooting guide
- "how do I do X" workflows

### 28.3 Feature Explanation Docs

Document how to:

- triage many repos
- stage precisely
- commit and push
- compare commits
- recover from conflicts or rebase states later

## 29. Release and Distribution Plan

### 29.1 Initial Distribution Goal

Aim for:

- source build first
- release binaries later
- straightforward install path for Linux and macOS first

### 29.2 Release Quality Bar

Before recommending broadly:

- solid temp-repo integration coverage
- stable watcher fallback behavior
- no destructive-operation surprises
- acceptable performance on medium workspaces

## 30. Milestone Plan

### 30.1 Phase 0 - Foundations

Deliver:

- Rust workspace scaffold
- app shell
- reducer/event architecture
- placeholder panes
- logging and config bootstrap
- test support crate

Exit criteria:

- app boots
- layout resizes correctly
- reducer tests exist

### 30.2 Phase 1 - Workspace Dashboard MVP

Deliver:

- repo discovery
- repo summary model
- repo table
- sort/filter/search
- refresh worker pool
- selected repo preview
- enter repo mode

Exit criteria:

- user can see and navigate many repos in one app

### 30.3 Phase 2 - Realtime Workspace Freshness

Deliver:

- watcher subsystem
- invalidation and refresh scheduling
- freshness indicators
- fallback polling mode

Exit criteria:

- dashboard stays trustworthy without manual refresh spam

### 30.4 Phase 3 - Repo Mode Core

Deliver:

- status view
- diff pane
- file stage/unstage
- hunk stage/unstage
- commit box
- fetch/pull/push actions

Exit criteria:

- daily commit loop works end to end

### 30.5 Phase 4 - Repo History and Branching

Deliver:

- branch view
- commit view
- stash view
- reflog view
- compare refs/commits

Exit criteria:

- user can inspect recent history and branch state without leaving terminal

### 30.6 Phase 5 - Power User Git Flows

Deliver:

- line staging if not already complete
- cherry-pick
- amend older commit
- interactive rebase
- worktree basics
- conflict assistance improvements

Exit criteria:

- Lazygit replacement story becomes credible for advanced users

## 31. Detailed Backlog Seeds by Phase

### 31.1 Phase 0 Backlog Seeds

- create workspace `Cargo.toml`
- create `crates/app`
- create `crates/core`
- create `crates/tui`
- create `crates/git`
- create `crates/workspace`
- create `crates/config`
- create `crates/test-support`
- define `AppState`
- define `Action`
- define `Effect`
- define initial event loop

### 31.2 Phase 1 Backlog Seeds

- implement root scanner
- detect `.git` and gitdir file repos
- create `RepoSummary`
- create attention score
- render repo table
- search/filter/sort reducer
- selected repo preview
- enter repo mode transition

### 31.3 Phase 2 Backlog Seeds

- add watcher abstraction
- per-repo debounce queues
- watcher health indicators
- timer fallback
- visible-repo priority scheduling

### 31.4 Phase 3 Backlog Seeds

- file tree widget
- staged tree widget
- diff widget
- file-level stage
- hunk-level stage
- commit action
- pull action
- push action
- refresh after writes

### 31.5 Phase 4 Backlog Seeds

- branch list widget
- commit list widget
- commit detail preview
- stash list widget
- reflog list widget
- compare mode state

### 31.6 Phase 5 Backlog Seeds

- cherry-pick flow
- rebase mode state machine
- amend older commit flow
- worktree list and create flow
- conflict assistance

## 32. Open Technical Questions

### 32.1 Hunk and Line Staging Implementation

Question:

- can `git2` patch apply remain reliable enough for all important cases

Need:

- spike
- parity tests
- fallback strategy

### 32.2 Rename and Diff Semantics

Question:

- how much rename detection complexity belongs in MVP

Need:

- workspace summary heuristic
- repo-mode detail parity decisions

### 32.3 Auto-Fetch Policy

Question:

- should auto-fetch be off by default in MVP

Current recommendation:

- yes, off by default
- explicit manual fetch first
- optional later config

### 32.4 Timeline and Activity Views

Question:

- do timeline and contribution-graph ideas belong before repo-mode parity work

Current recommendation:

- no
- keep them in later exploration

### 32.5 Custom Commands

Question:

- should the app plan for Lazygit-like custom command support early

Current recommendation:

- plan for it architecturally
- defer UI exposure until core flows are stable

## 33. Opinionated Decisions

### 33.1 Decision: Repo Mode Should Be Very Close to Lazygit

Rationale:

- Lazygit already solved the deep repo UX problem well
- deviating too much wastes learning from a proven design
- users already understand that mental model

### 33.2 Decision: Workspace Mode Should Not Be Lazygit-Shaped

Rationale:

- the user's pain starts at workspace scale
- repo-deep panels are the wrong default for scanning many repos
- `git-dash` is a better structural baseline here

### 33.3 Decision: Visual Language Should Still Be Unified

Rationale:

- two unrelated UIs would make mode switching feel like app switching
- consistent panel/title/help styling lowers cognitive load

### 33.4 Decision: Prefer Safety and Conformity Over Cleverness

Rationale:

- Git users care deeply about trust
- a small amount of friction is acceptable
- surprising write behavior is not acceptable

## 34. Canonical Execution Graph

This section freezes the execution graph that future beads should follow.
It updates the original milestone/backlog sections with the live graph reality from the current repo and bead state.
If later work conflicts with this graph, the graph should win unless the root epic is intentionally re-planned.

### 34.1 Product-Contract Gates

These gates are not optional side work.
They exist to prevent the port from regressing into the wrong product.

- `supergit-xaa.9` is the standing guardrail bead for product-contract enforcement
- workspace mode must stay workspace-first rather than becoming a weak copy of repo mode
- repo mode must stay intentionally Lazygit-like rather than drifting into a generic Git widget shell
- Git correctness, trust, and operator predictability outrank feature-count vanity
- testing, performance, and hardening are parallel delivery tracks, not a final cleanup phase

### 34.2 Current Live Graph Snapshot

As of the latest checkpoint, live bead state, and current agent-mail activity:

- foundation scaffold is already materially present in code
- `supergit-xaa.2.1` is landed as the GitFacade / backend-routing contract seam
- `supergit-xaa.1.7` is completed as the runtime / effect-executor bridge
- `supergit-xaa.2.3` is completed for repo-detail readers
- `supergit-xaa.2.4` is completed for core write operations
- `supergit-xaa.8.2` is closed as the Git topology integration guardrail, with executable coverage now protecting facade/detail behavior
- the graph remains cycle-free under `bv --robot-triage`
- the highest-value open seams are currently `supergit-xaa.5.1`, `supergit-xaa.3.1`, `supergit-xaa.3.2`, and `supergit-xaa.3.3`

This means the execution graph must reflect real codebase maturity, not the earlier "plan-only" assumption or stale pre-close snapshots.

### 34.3 Execution Tracks

The graph should be understood as six coupled tracks:

1. foundation and runtime ownership
2. Git engine parity
3. workspace-mode dataflow and UX
4. realtime freshness
5. repo-mode parity
6. trust, performance, and hardening

Tracks can progress in parallel only when their blocker edges are respected.

### 34.4 Track A — Foundation and Runtime Ownership

Intent:

- keep reducer ownership in `crates/core`
- keep effect interpretation in `crates/app`
- keep rendering/input in `crates/tui`
- prevent Git logic or discovery logic from leaking upward into widgets

Status:

- foundation scaffold exists
- runtime bridge exists after `supergit-xaa.1.7`
- this track should now be treated as a platform track, not the main bottleneck

Remaining obligations:

- preserve crate boundaries while downstream beads land
- avoid reintroducing effect execution into `TuiApp`
- keep tests aligned with reducer-first ownership

### 34.5 Track B — Git Engine Parity

Intent:

- make `GitFacade` the only Git caller entrypoint
- preserve explicit backend-routing semantics
- keep reads and writes separable so CLI-backed correctness remains possible where needed

Execution order:

1. `supergit-xaa.2.1` — facade contract and routing policy
2. `supergit-xaa.2.2` — workspace summary readers
3. `supergit-xaa.2.3` — repo detail readers
4. `supergit-xaa.2.4` — core write operations for commit / checkout / fetch / pull / push
5. `supergit-xaa.2.5` and follow-ons — diff/patch precision, safety semantics, and parity cleanup

Graph rule:

- workspace and repo-mode tracks may not bypass the facade by talking directly to ad hoc git helpers
- all testing beads for Git semantics should target facade behavior first

Current bottlenecks:

- `supergit-xaa.2.5` and `supergit-xaa.2.6` are closed, so the next Git-engine precision work now lives in downstream diff/staging UX beads (`supergit-xaa.5.3`, `5.5`) and any future parity follow-ons opened by tests

### 34.6 Track C — Workspace Mode

Intent:

- deliver the workspace-first command center before deep power-user repo polish
- make macro triage useful even before full repo-mode parity exists

Execution order:

1. `supergit-xaa.3.1` — recursive repo discovery with `.git` dir and gitdir-file support
2. `supergit-xaa.3.2` — stable repo registry and cache
3. `supergit-xaa.3.3` — background summary refresh workers and scheduling
4. `supergit-xaa.3.4` — repo table and selection model
5. later workspace search/filter/sort/preview refinement beads

Graph rule:

- workspace mode is allowed to ship ahead of full repo-mode parity
- but it must always remain connected to the same `RepoSummary` source of truth that repo mode relies on

### 34.7 Track D — Realtime Freshness

Intent:

- keep the dashboard trustworthy without forcing manual refresh spam
- make watcher-driven invalidation feed reducer/effect scheduling rather than mutate state directly

Execution order:

1. `supergit-xaa.4.1` — watcher abstraction and backend health reporting
2. `supergit-xaa.4.2` — per-repo invalidation debounce and refresh scheduling
3. `supergit-xaa.4.3` / `4.4` — fallback polling, freshness surfacing, and recovery semantics

Graph rule:

- freshness work depends on workspace discovery/registry stability
- watchers must invalidate and schedule refreshes; they must not become an alternate Git state engine

### 34.8 Track E — Repo Mode Parity

Intent:

- make repo interior feel recognizably Lazygit-like
- prioritize the daily status → diff → stage → commit → push loop before advanced history flows

Execution order:

1. `supergit-xaa.5.1` — repo mode shell and focus model
2. `supergit-xaa.5.2` / `5.3` — status tree and diff viewer
3. `supergit-xaa.5.4` / `5.5` — file and hunk staging actions
4. commit and sync-loop beads built on `2.4`
5. only then expand further toward advanced parity

Graph rule:

- `5.1` depends on `2.3` because repo shell without real detail data would be fake progress
- staging UX depends on both diff fidelity and write-path safety
- repo mode should mirror Lazygit relationships closely, but not copy Lazygit’s Go internals

### 34.9 Track F — History, Power-User Flows, and Trust

Intent:

- layer advanced Git depth only after the daily loop is credible
- treat hardening/performance/docs as first-class support tracks for every major seam

Execution order:

- history/reference views (`supergit-xaa.6.*`) follow repo-mode core parity
- power-user flows (`supergit-xaa.7.*`) follow staging, diff, and write-path confidence
- testing/performance/hardening/docs (`supergit-xaa.8.*`) should advance in parallel with each track, not after them

Graph rule:

- no advanced rebase/cherry-pick/amend work should land on top of an untrusted write path
- no recommendation to broader users should happen before integration/perf coverage exists for the relevant seam

### 34.10 Canonical Near-Term Priority Order

For the current repo state, the preferred near-term order is:

1. land `supergit-xaa.5.1`
2. land `supergit-xaa.3.1`
3. land `supergit-xaa.3.2`
4. land `supergit-xaa.3.3`
5. continue watcher/freshness and downstream quality beads in parallel where unblocked

Rationale:

- `5.1` is now the highest-leverage open product seam because `2.3` is already done and repo shell parity unblocks a large repo-mode subtree
- `3.1` and `3.2` still unlock the real workspace-first differentiator and are the correct path into trustworthy workspace freshness
- `3.3` converts discovery and registry seams into credible always-fresh workspace behavior
- `8.2` is already closed, so topology coverage now serves as a guardrail rather than a near-term planning target
- `2.5`, `2.6`, and `8.2` are already closed, so the live near-term pressure stays on `5.1` plus the workspace discovery/cache/refresh chain rather than reopening Git-engine foundation work

### 34.11 Architectural Invariants

Every future bead under this epic should preserve these invariants:

- reducer owns state transitions
- runtime owns effect execution
- TUI owns presentation and input routing only
- GitFacade owns Git entrypoints
- watcher events invalidate; they do not author state truth
- workspace mode and repo mode share domain models rather than forking them
- write paths favor trust and parity over clever local shortcuts
- tests and diagnostics grow with each seam instead of trailing by a full phase

### 34.12 Re-Planning Trigger Conditions

Re-plan the graph only if one of these becomes true:

- Git backend routing assumptions prove incorrect under parity testing
- watcher resource costs violate the workspace scale targets
- repo-mode Lazygit similarity creates unacceptable structural drag in Rust
- workspace-first ergonomics are being compromised to chase deep single-repo parity too early
- bead triage begins recommending work that conflicts with the product contract in section 3

### 34.13 Execution-Ready Implementation Map

The next execution beads should attach to the current crate and file seams directly rather than reopening architecture work.

- `supergit-xaa.5.1` — repo-mode shell and focus model
  - primary files: `crates/tui/src/lib.rs`, `crates/core/src/state.rs`, `crates/core/src/reducer.rs`
  - required shift: bind the existing repo shell to real `RepoDetail` data and Lazygit-like focus relationships instead of placeholder views, now that `2.3` is landed
  - verification: reducer/TUI tests for focus traversal, subview switching, and selected-repo entry from workspace mode
- `supergit-xaa.3.1` — recursive repo discovery
  - primary files: `crates/git/src/lib.rs`, `crates/workspace/src/lib.rs`, `crates/test-support`
  - required shift: upgrade workspace scanning from simple `.git`-directory detection to `.git` dir plus gitdir-file/worktree-aware discovery with correct descent-stop behavior
  - verification: fixture cases for normal repos, linked worktrees, nested roots, and non-repo directories
- `supergit-xaa.3.2` — stable repo registry and cache
  - primary files: `crates/workspace/src/lib.rs`, `crates/core/src/state.rs`, `crates/core/src/effect.rs`, `crates/app/src/runtime.rs`
  - required shift: turn the current workspace shell into a stable registry/cache layer that preserves repo identity, scan freshness, and selection continuity across refreshes
  - verification: reducer tests for identity preservation and runtime tests for cache refresh/persist behavior
- `supergit-xaa.3.3` — background summary refresh workers and scheduling
  - primary files: `crates/app/src/runtime.rs`, `crates/core/src/event.rs`, `crates/core/src/reducer.rs`, `crates/workspace/src/lib.rs`
  - required shift: convert manual refresh behavior into scheduled worker-driven summary refresh with reducer-visible progress and stale-state surfacing
  - verification: event-loop tests proving debounce/scheduling behavior and that stale repos never appear falsely fresh
- `supergit-xaa.8.2` — closed Git topology integration guardrail
  - primary files: `crates/git/src/lib.rs`, `crates/test-support/src/lib.rs`, `crates/git/tests/` or workspace integration-test surfaces
  - required shift: turn the planned topology matrix into executable integration coverage for clean, dirty, staged/unstaged, detached HEAD, divergence, conflicts, stash, and worktree cases
  - verification: reproducible topology fixtures that fail on facade/backend regressions rather than only documenting expected behavior
- `supergit-xaa.2.5` — closed selective staging parity spike guardrail
  - primary files: `crates/git/src/lib.rs`, `crates/core/src/state.rs`, `crates/core/src/effect.rs`
  - required shift: prove or reject the patch/hunk-application approach needed for file/hunk staging without compromising trust in Git semantics
  - verification: spike artifacts or targeted tests that make the chosen patch/hunk path explicit enough for `2.6` follow-on parity work

Execution rule:

- prefer landing each bead with its owning seam, tests, and diagnostics in the same change
- do not count placeholder UI or simulated runtime behavior as progress for a bead whose contract is data- or Git-semantics-heavy

### 34.14 Concrete Implementation Approach

The implementation approach should follow one product-preserving shape rather than letting each bead invent its own local design.

1. build shared domain truth first
   - keep `RepoSummary`, `RepoDetail`, `WorkspaceState`, and `RepoModeState` as the cross-crate contract surface
   - every workspace table row, preview pane, repo-mode panel, and Git write flow should consume those shared models rather than creating view-local Git structs
2. drive behavior through reducer and effects
   - user input, timer ticks, watcher invalidations, and Git worker completions should all enter through `Event`
   - `crates/core` should decide state transitions and requested work
   - `crates/app` should remain the only place that interprets `Effect` into Git/workspace/runtime activity
3. keep Git correctness behind the facade
   - `crates/git` should own discovery, summary/detail reads, write operations, backend routing, and patch semantics
   - TUI and workspace crates must not shell out or parse Git output directly
4. finish workspace-first value before deep repo-mode polish
   - the app should become genuinely useful as a multi-repo command center before spending major effort on advanced single-repo power flows
   - this means discovery, stable identity, refresh scheduling, table rendering, preview, search/filter/sort, and freshness visibility must become real before history and power-user parity dominate the roadmap
5. make repo mode feel like Lazygit by matching interaction relationships, not by copying internals
   - left pane should stay navigation-oriented
   - right pane should stay detail/diff/secondary-content oriented
   - subviews should map closely to status, branches, commits, stash, reflog, and worktrees
   - commit/sync actions should stay in the same working context rather than bouncing the user into detached workflows
6. land each seam with proof
   - every bead should close with the tests, fixtures, diagnostics, or performance evidence that prove that seam is real
   - placeholder rendering, synthetic freshness, or stubbed command paths do not satisfy a bead whose contract is data, Git semantics, or operator trust

### 34.15 Execution-Ready Decomposition By Bead Cluster

The next implementation wave should be decomposed into the following execution-ready clusters.

#### Cluster 1 — Make repo mode structurally real

Primary bead:
- `supergit-xaa.5.1`

Objective:
- turn the current repo shell into a true Lazygit-like navigation frame wired to live `RepoDetail` data

Concrete steps:
1. bind repo-mode entry so selected workspace repo always hydrates `RepoModeState::detail`
2. define stable focus movement between repo navigation pane and detail pane
3. render subview-specific list content from existing detail readers instead of placeholder counters
4. keep subview switching stateful across refreshes when the underlying item still exists
5. surface operation-progress and failure state inside repo mode without breaking navigation

Done criteria:
- entering repo mode from workspace mode always shows real data for the selected repo
- switching between status/branches/commits/stash/reflog/worktrees is reducer-driven and test-covered
- the shell is credible enough for downstream status-tree and diff beads to attach without reworking navigation structure

#### Cluster 2 — Make workspace discovery and identity trustworthy

Primary beads:
- `supergit-xaa.3.1`
- `supergit-xaa.3.2`

Objective:
- ensure the workspace surface understands real repo topology and preserves stable identity across rescans

Concrete steps:
1. extend discovery to handle `.git` directories, gitdir files, and linked worktrees correctly
2. stop descent at repo roots while still supporting nested-workspace policy explicitly
3. introduce a stable registry/cache keyed by canonical repo identity instead of transient scan order
4. preserve selection, recent-repo history, and summary continuity across rescans
5. attach scan failures or invalid repos to visible diagnostics rather than silently dropping them

Done criteria:
- rescanning does not reshuffle identity for unchanged repos
- worktree-linked repos appear once and resolve to the correct real path and display path
- reducer/runtime tests prove selection continuity and registry stability

#### Cluster 3 — Turn freshness into a real product differentiator

Primary beads:
- `supergit-xaa.3.3`
- `supergit-xaa.4.1`
- `supergit-xaa.4.2`
- `supergit-xaa.4.3`
- `supergit-xaa.4.4`

Objective:
- move from manual refresh semantics to trustworthy background freshness with clear degraded-mode behavior

Concrete steps:
1. add scheduled summary refresh work on top of the runtime bridge from `supergit-xaa.1.7`
2. layer watcher invalidation on top of registry identities rather than raw paths alone
3. debounce repeated invalidations per repo so active write bursts do not thrash the UI
4. prioritize active and visible repos over hidden repos during refresh scheduling
5. expose freshness age, degraded watch mode, and fallback polling state in workspace summaries and preview

Done criteria:
- a busy repo cannot starve the rest of the workspace
- stale data is always visible as stale rather than misrepresented as fresh
- watcher degradation falls back predictably instead of silently disabling freshness

#### Cluster 4 — Complete the daily repo loop

Primary beads:
- `supergit-xaa.5.2`
- `supergit-xaa.5.3`
- `supergit-xaa.5.4`
- `supergit-xaa.5.5`
- `supergit-xaa.5.6`
- `supergit-xaa.5.7`

Objective:
- make status → diff → stage → commit → pull/push usable without leaving repo mode

Concrete steps:
1. render working-tree and staged-tree panes from detail status data
2. promote `DiffModel` from placeholder path selection into a real diff/hunk model
3. wire file stage/unstage actions through existing write-command infrastructure
4. wire hunk stage/unstage actions through the selective patch path proven in `supergit-xaa.2.5`
5. add commit input, validation, amend-head flow, and post-write refresh sequencing
6. integrate fetch/pull/push with confirmations, progress, and refresh on completion

Done criteria:
- the common daily Git loop works end to end in one screen
- every write path refreshes affected summaries/details so the UI remains trustworthy after mutation
- failure cases remain visible and recoverable without shelling out for routine operations

#### Cluster 5 — Layer history depth, then power-user depth

Primary beads:
- `supergit-xaa.6.*`
- `supergit-xaa.7.*`

Objective:
- add serious Git depth without compromising the now-credible daily loop

Concrete steps:
1. build branches, commits, stash, reflog, worktrees, and compare flows on top of shared repo-mode navigation and diff infrastructure
2. only after that, add line staging, cherry-pick, revert, rebase, advanced amend/fixup, and reset/discard flows
3. treat every dangerous action as a UX problem first and a command problem second

Done criteria:
- advanced history features reuse existing shell, list, diff, and confirmation infrastructure
- power-user flows never bypass safety overlays or parity tests

#### Cluster 6 — Keep proof, performance, and operator trust attached throughout

Primary beads:
- `supergit-xaa.8.3`
- `supergit-xaa.8.4`
- `supergit-xaa.8.5`
- `supergit-xaa.8.6`
- `supergit-xaa.8.7`

Objective:
- prevent the port from becoming feature-rich but operationally untrustworthy

Concrete steps:
1. add watcher/debounce correctness tests as freshness work lands
2. add end-to-end TUI harness coverage as repo-mode interaction becomes real
3. build startup/scan/refresh/diff performance harnesses against realistic workspace sizes
4. encode write-path and sync failure modes explicitly
5. publish operator-facing docs only after the implemented behavior is stable enough to describe accurately

Done criteria:
- quality beads move in parallel with implementation seams rather than trailing an entire phase
- performance budgets and failure semantics are checked by executable artifacts, not memory or intention

## 35. Competitive Positioning

### 34.1 Against Lazygit

`super_lazygit_rust` should eventually be better when:

- many repos matter at once
- freshness matters
- workspace triage matters

Lazygit will remain a stronger reference for:

- mature power-user flows during early phases

### 34.2 Against GitUI

`super_lazygit_rust` should eventually differentiate via:

- workspace-first architecture
- stronger multi-repo visibility
- clearer Lazygit-inspired repo workflow

### 34.3 Against git-dash and git-scope

`super_lazygit_rust` should eventually differentiate via:

- deep repo actions
- precise staging
- integrated commit/push workflows
- not just visibility

## 35. Validation Gate Before Beads

This plan is ready for beads only when:

- all major user workflows are spelled out end to end
- repo mode and workspace mode boundaries are stable
- Git backend routing decisions are stable enough to implement
- watcher strategy is explicit enough to build
- testing expectations are concrete
- the remaining questions are incremental rather than foundational

At the current stage:

- this plan is much stronger than the original outline
- the repo is no longer pre-scaffold; the Rust workspace, reducer/app shell, runtime loop, Git facade seam, CLI-backed write paths, diagnostics, and baseline test-support surfaces now exist in code
- the remaining gaps are execution-sequencing and contract-precision gaps rather than blank-page architecture gaps
- further refinement rounds should tighten per-view UX contracts, parity boundaries, and verification matrices without resetting the current execution graph

## 36. Immediate Next Planning Moves

The next best planning moves are:

1. Expand this plan again with even more explicit per-view UI contracts.
2. Add a full feature parity matrix against Lazygit.
3. Add a command-by-command Git operation appendix.
4. Add a crate-by-crate API sketch.
5. Add a concrete test-fixture matrix with exact repo topologies.
6. Keep reconciling plan text against live bead closures so older pre-scaffold instructions do not drift back into the canonical graph.
7. Run competing-model refinement rounds only where they sharpen open seams rather than re-planning already landed scaffolding.

## 37. Immediate Next Build Moves

Given the current repo state, the correct build order is:

1. land the repo-mode shell and focus seam (`supergit-xaa.5.1`)
2. land recursive repo discovery with worktree-aware correctness (`supergit-xaa.3.1`)
3. land the stable repo registry/cache seam (`supergit-xaa.3.2`)
4. land background summary refresh workers and scheduling (`supergit-xaa.3.3`)
5. continue watcher/freshness delivery from those real workspace seams (`supergit-xaa.4.1` → `4.4`)
6. continue deeper repo-mode diff/staging/commit-loop parity from the real repo shell and landed Git facade/write-path seams (`supergit-xaa.5.2` → `5.7`)
7. keep topology, trust, performance, and hardening guardrails current as ongoing parallel support tracks rather than re-opening already closed foundation guardrail beads

## 38. Final Planning Call

The strongest reading of the references is:

- `lazygit` is the best repo-mode reference
- `gitui` is the best Rust implementation reference
- `git-dash` is the best workspace structure reference
- `git-scope` is the best supporting reference for caching, switching, and macro framing

So the correct product direction is not:

- "port Lazygit and tack on workspace support"

It is:

- "design a workspace-first Git TUI whose repo interior feels like Lazygit"

That is the version of `super_lazygit_rust` most likely to solve the pain in `PAINPOINT.md` and become meaningfully better than a clone.

## 39. Lazygit Feature Parity Target Matrix

This section maps notable Lazygit features from `references/lazygit-master/README.md`
into explicit `super_lazygit_rust` planning decisions.

### 39.1 Stage Individual Lines

Reference value:

- extremely high

Why it matters:

- directly addresses P4
- one of Lazygit's strongest value propositions

Target:

- support in repo-mode diff view

Delivery:

- phase 3 if technically stable
- otherwise phase 5 with file/hunk staging landing first

Blocking questions:

- patch-application reliability
- reverse patch semantics
- untracked file handling

### 39.2 Interactive Rebase

Reference value:

- very high for power users

Why it matters:

- major Lazygit expectation
- enables history cleanup without leaving the TUI

Target:

- support later, not MVP

Delivery:

- phase 5

Notes:

- better to ship late and correct than early and fragile

### 39.3 Cherry-Pick

Reference value:

- high

Why it matters:

- common in multi-branch maintenance work
- useful enough to plan explicitly

Target:

- repo-mode commits view action

Delivery:

- phase 5

### 39.4 Bisect

Reference value:

- medium

Why it matters:

- valuable but not part of the user's stated primary pain

Target:

- defer

Delivery:

- post-phase-5 unless user demand proves strong

### 39.5 Nuke Working Tree

Reference value:

- high risk
- moderate value

Why it matters:

- users need recovery tools
- destructive actions need excellent safety

Target:

- support later through explicit reset/discard menu

Delivery:

- phase 4 or phase 5

Safety requirements:

- clear wording
- affected scope visible
- extra confirmation

### 39.6 Amend an Old Commit

Reference value:

- high for power users

Target:

- planned

Delivery:

- phase 5

### 39.7 Filter

Reference value:

- essential

Target:

- search/filter in all major list contexts

Delivery:

- workspace mode in phase 1
- repo-mode lists in phase 3 and phase 4

### 39.8 Custom Commands

Reference value:

- medium-high for advanced workflows

Target:

- architect for it
- do not prioritize UI in MVP

Delivery:

- later extension once action system stabilizes

### 39.9 Worktrees

Reference value:

- medium-high

Why it matters:

- especially useful for many-repo and multi-branch users

Target:

- visible in repo mode
- action support later

Delivery:

- phase 5

### 39.10 Rebase Magic / Custom Patches

Reference value:

- high sophistication
- low MVP relevance

Target:

- explicitly not an early goal

Delivery:

- far-later exploration

### 39.11 Rebase onto Base Commit

Reference value:

- medium

Target:

- defer

Delivery:

- later phase after baseline interactive rebase is stable

### 39.12 Undo / Redo

Reference value:

- high trust value

Why it matters:

- safety net
- helps experimentation

Target:

- plan now
- implement only where semantics are clear

Delivery:

- phase 5 or later

Constraint:

- must not claim more reversibility than Git actually supports

### 39.13 Commit Graph

Reference value:

- medium-high

Target:

- commit history view should support graph later

Delivery:

- phase 4 for basic history
- graph later if complexity is acceptable

### 39.14 Compare Two Commits

Reference value:

- high

Target:

- repo-mode compare mode

Delivery:

- phase 4

### 39.15 Recent Repo Switching

Reference source:

- Lazygit global keybinding `<c-r>` for recent repo

Target:

- maintain recent repo stack
- allow jump back to recent repos in workspace context later

Delivery:

- phase 1 or phase 4, depending on complexity

## 40. git-dash and git-scope Feature Assimilation Matrix

### 40.1 git-dash Features to Assimilate Directly

- automatic recursive discovery
- rich status table
- parallel status fetching
- safe push/pull confirmation prompts
- progress indicators during scan
- clear column semantics

### 40.2 git-dash Features to Modify

- push/pull from dashboard should remain available, but likely not central in early versions
- confirmation flows should use Lazygit-like overlay style instead of bare dashboard prompts

### 40.3 git-dash Features Not to Copy Literally

- single-screen simplicity as the whole product
- absence of deep repo interaction

### 40.4 git-scope Features to Assimilate Directly

- workspace switching
- fuzzy search
- dirty filter
- fast cache-backed startup
- recent activity framing

### 40.5 git-scope Features to Consider Later

- contribution graph
- disk usage view
- timeline view
- custom team dashboards

### 40.6 git-scope Features Not to Copy Literally

- read-only philosophy

`super_gittui` should remain safe-first, but it is intentionally not read-only.

## 41. Detailed Workspace Mode UI Contract

### 41.1 Workspace Header Area

Header should show:

- current workspace name or root
- number of repos loaded
- number of dirty repos
- number of ahead repos
- number of behind repos
- cache vs live state indicator
- current sort and filter

### 41.2 Workspace Footer / Bottom Line

Bottom line should show:

- active mode
- focused pane
- key hints
- watcher health
- background job status
- last status message

### 41.3 Repo Table Width Strategy

If terminal is wide:

- show full column set

If terminal is medium:

- collapse low-priority columns

If terminal is narrow:

- prioritize
  - repo name
  - branch
  - dirty summary
  - ahead/behind

### 41.4 Repo Name Column

Should display:

- short display name
- optional group prefix later

Should truncate:

- gracefully

Should hint:

- if path conflicts exist between repos with the same name

### 41.5 Branch Column

Should display:

- current branch name
- detached HEAD marker if needed

Should visually differentiate:

- normal branch
- detached HEAD
- error/unavailable

### 41.6 Dirty Summary Columns

Should distinguish:

- staged
- unstaged
- untracked

Candidate display:

- compact counts
- color-coded status badges

### 41.7 Ahead / Behind Column

Must represent:

- ahead only
- behind only
- diverged
- no upstream

No upstream should not look like clean sync.

### 41.8 Last Fetch Column

Should display:

- relative age
- unknown state if never fetched

Should be cheap to compute and cache.

### 41.9 Last Activity Column

May derive from:

- recent commits
- recent file mtimes later

For MVP, commit-based or refresh-based approximation is acceptable.

### 41.10 Preview Pane Modes

Preview mode choices:

- `Summary`
- `RecentCommits`
- `FileChanges`
- `Operations`
- `TimelineLater`

Each mode should:

- be switchable by keys
- remember last choice per session if simple

### 41.11 Empty Workspace State

If no repos are found:

- show clear explanation
- show how to add/switch workspace
- show current root being scanned

### 41.12 Loading State

During scan:

- render shell immediately
- show progress
- allow cancel if practical later
- do not freeze navigation once enough data is available

### 41.13 Error State

If some repos fail to inspect:

- do not fail entire workspace
- show repo row with error badge
- allow selecting the row for details

### 41.14 Workspace Modal Flows

Workspace mode needs modals for:

- switch workspace
- confirm fetch all
- confirm destructive workspace-scoped actions later
- filter chooser if needed

## 42. Detailed Repo Mode UI Contract

### 42.1 Repo Header Area

Should show:

- repo name
- path
- branch
- upstream
- ahead/behind
- mode/subview
- operation state

### 42.2 Status View Working Tree Pane

Each row should support:

- stage toggle
- open diff
- open file externally
- discard/reset menu
- collapse/expand directory

Each row should indicate:

- modified
- added
- deleted
- renamed later if supported
- untracked
- conflicted

### 42.3 Status View Staged Pane

Each row should support:

- unstage
- open diff
- amend awareness

Pane should visually answer:

- what is currently staged
- whether the staged set appears commit-ready

### 42.4 Status View Diff Pane

Diff pane should support:

- scrolling
- focus
- hunk selection
- line selection later
- syntax coloration when feasible
- context-size control later

### 42.5 Status View Commit Box

The commit box should:

- appear without leaving the status context
- be obvious when focused
- show validation hints
- allow confirm and cancel

### 42.6 Branch View Rows

Rows should indicate:

- local branch
- remote branch later if combined
- current branch
- upstream relation if available
- merge/rebase relevance later

### 42.7 Commits View Rows

Rows should indicate:

- short hash
- author or color grouping later
- commit summary
- relative time
- branch/tag decoration where practical

### 42.8 Commit Detail / Files View

Should support:

- file list inside selected commit
- file diff preview
- compare mode later

### 42.9 Stash View Rows

Rows should indicate:

- stash name/index
- summary line
- timestamp if available

### 42.10 Reflog View Rows

Rows should indicate:

- ref movement summary
- time
- target identifier

### 42.11 Worktree View Rows

Rows should indicate:

- path
- branch
- HEAD relation
- dirty summary if cheap enough later

## 43. Draft Keybinding Plan

This is not final.
It exists to make the plan concrete.

### 43.1 Global Keys

- `q`: quit
- `?`: help
- `esc`: cancel / close overlay / step back
- `/`: search current list
- `tab`: next pane
- `shift-tab`: previous pane
- `+`: larger focus mode later
- `_`: smaller focus mode later

### 43.2 Workspace Keys

- `j/k`: move repo selection
- `enter`: open repo mode
- `r`: refresh selected repo
- `R`: refresh visible or all repos
- `f`: cycle filter
- `s`: cycle sort
- `w`: switch workspace
- `g`: later timeline/activity shortcut if implemented

### 43.3 Repo Status Keys

- `j/k`: move selection
- `h/l`: move focus between panes
- `space`: stage/unstage current item
- `enter`: descend into diff or detail
- `c`: commit
- `A`: amend HEAD
- `p`: pull
- `P`: push
- `f`: fetch
- `s`: stash
- `d`: discard/reset menu

### 43.4 Repo Branch Keys

- `space`: checkout
- `n`: new branch
- `R`: rename branch
- `d`: delete branch
- `u`: upstream options
- `r`: rebase current onto selected later

### 43.5 Repo Commit Keys

- `enter`: inspect commit files
- `C`: mark/copy for cherry-pick later
- `V`: paste/cherry-pick later
- `t`: revert later
- `T`: tag later
- `A`: amend older commit later
- `i`: start interactive rebase later

### 43.6 Help Overlay Requirements

The help overlay should:

- group actions by context
- show both key and action
- note disabled state when relevant
- ideally be generated from action metadata rather than hardcoded docs only

## 44. GitFacade API Sketch

### 44.1 Workspace-Facing Reads

Potential methods:

- `scan_workspaces(roots, ignore) -> RepoList`
- `refresh_repo_summary(repo_id) -> RepoSummary`
- `refresh_repo_summaries(repo_ids) -> Vec<RepoSummary>`

### 44.2 Repo-Facing Reads

Potential methods:

- `load_repo_detail(repo_id) -> RepoDetail`
- `load_branch_list(repo_id) -> BranchList`
- `load_commit_list(repo_id, filter) -> CommitList`
- `load_stash_list(repo_id) -> StashList`
- `load_reflog(repo_id) -> ReflogList`
- `load_worktrees(repo_id) -> WorktreeList`

### 44.3 Diff and Status Reads

Potential methods:

- `load_working_tree(repo_id) -> FileTreeStatus`
- `load_staged_tree(repo_id) -> FileTreeStatus`
- `load_diff_for_selection(repo_id, selection, diff_target) -> DiffModel`

### 44.4 Write Operations

Potential methods:

- `stage_file(repo_id, path)`
- `unstage_file(repo_id, path)`
- `stage_hunk(repo_id, path, hunk_id)`
- `unstage_hunk(repo_id, path, hunk_id)`
- `stage_lines(repo_id, path, range)` later
- `commit(repo_id, message, options)`
- `amend_head(repo_id, message_options)`
- `fetch(repo_id, options)`
- `pull(repo_id, options)`
- `push(repo_id, options)`
- `checkout_branch(repo_id, branch_ref)`
- `create_branch(repo_id, spec)`

### 44.5 Advanced Writes

Potential methods:

- `cherry_pick(repo_id, commit_id)` later
- `rebase_interactive(repo_id, spec)` later
- `create_worktree(repo_id, spec)` later
- `drop_stash(repo_id, stash_id)` later

### 44.6 Progress Reporting

Long-running methods should support:

- started event
- progress update events when available
- completed event
- failed event

### 44.7 Result Normalization

The facade should normalize:

- path types
- branch/upstream state
- error classes
- operation progress state

## 45. Failure Mode Appendix

### 45.1 Repository Not Accessible

Possible causes:

- deleted path
- permissions changed
- broken symlink

Required behavior:

- keep row visible
- mark error
- allow retry

### 45.2 Repository Mid-Rebase

Required behavior:

- surface rebase state immediately in repo mode
- restrict actions that would conflict
- expose continue/abort options later

### 45.3 Repository Mid-Merge

Required behavior:

- surface merge state
- prioritize conflict visibility
- route user to resolution options

### 45.4 Missing Upstream

Required behavior:

- ahead/behind should not pretend to be zero/zero
- push action should offer upstream setup

### 45.5 Authentication Failure

Required behavior:

- preserve user orientation
- show operation failure clearly
- do not hide stderr context completely

### 45.6 Slow Git Command

Required behavior:

- spinner/progress state
- cancel later if practical
- UI remains responsive

### 45.7 Watcher Backend Failure

Required behavior:

- clear degraded indicator
- timer fallback
- no crash

### 45.8 Cache Corruption

Required behavior:

- ignore bad cache
- rescan
- log issue

## 46. Test Topology Matrix

### 46.1 Repo Fixture: Clean Repo

Needed to test:

- empty status
- baseline summaries

### 46.2 Repo Fixture: Dirty Modified File

Needed to test:

- basic dirty indicators
- diff loading

### 46.3 Repo Fixture: Staged and Unstaged Mix

Needed to test:

- split tree logic
- staged pane
- commit readiness

### 46.4 Repo Fixture: Untracked Files

Needed to test:

- untracked indicators
- stage new file

### 46.5 Repo Fixture: Rename and Modify

Needed to test:

- rename handling
- diff semantics

### 46.6 Repo Fixture: Detached HEAD

Needed to test:

- branch column edge case
- repo header formatting

### 46.7 Repo Fixture: Ahead of Upstream

Needed to test:

- push visibility
- workspace sync summary

### 46.8 Repo Fixture: Behind Upstream

Needed to test:

- pull visibility
- stale sync triage

### 46.9 Repo Fixture: Diverged Branch

Needed to test:

- danger highlighting
- action restrictions or warnings

### 46.10 Repo Fixture: Merge Conflict

Needed to test:

- conflict indicators
- repo-mode error states

### 46.11 Repo Fixture: Stash Present

Needed to test:

- stash list loading
- stash apply/drop flows later

### 46.12 Repo Fixture: Multiple Worktrees

Needed to test:

- worktree discovery
- worktree list

### 46.13 Workspace Fixture: Many Repos

Needed to test:

- list performance
- search/filter responsiveness
- cache startup

### 46.14 Workspace Fixture: Broken Repo Entries

Needed to test:

- partial failure handling

## 47. Observability and Diagnostics Plan

### 47.1 Logging Goals

- diagnose watcher failures
- diagnose Git command latency
- diagnose cache behavior
- diagnose refresh churn

### 47.2 Metrics Worth Capturing in Debug Mode

- startup time
- scan duration
- per-repo summary refresh duration
- repo detail load duration
- diff load duration
- watcher event counts
- cache hit rate

### 47.3 Debug Views Later

Potentially useful later:

- command log
- diagnostics overlay
- watcher state snapshot

## 48. Planning Gaps Still Remaining

Even at 2,400+ lines, the plan is still missing some things that a final 5,000-line version should add.

### 48.1 Still-Missing Expansions

- exact per-pane render examples
- full theme spec
- exact data structure sketches
- exact state transition tables
- full parity table against GitUI and Lazygit
- concrete CLI command catalog
- full documentation outline

### 48.2 Why This Is Still Acceptable

The document is now strong enough to:

- drive more serious planning conversations
- guide scaffolding
- support future synthesis rounds

But it is not yet the final maximal flywheel plan.

## 49. Next Flywheel Refinement Prompts

Use this document in fresh-model rounds with prompts like:

### 49.1 Architecture Refinement Prompt

```text
Carefully review this entire plan and identify all architecture weaknesses, hidden complexity traps, performance risks, UX contradictions, Git semantic mismatches, and state-model ambiguities. Propose exact revisions in git-diff style.
```

### 49.2 Workflow Completeness Prompt

```text
Review this plan only from the perspective of user workflows and failure states. Tell me every missing workflow, every unclear transition, and every place the product might confuse a terminal-first multi-repo developer.
```

### 49.3 Git Semantics Prompt

```text
Review this plan only from the perspective of Git correctness and safety. Identify every place where library behavior could drift from Git CLI behavior and propose a stricter backend-routing plan.
```

### 49.4 Performance Prompt

```text
Review this plan only from the perspective of performance on low-RAM machines and large workspaces. Identify every potential bottleneck and propose concrete mitigations.
```

### 49.5 UI Prompt

```text
Review this plan only from the perspective of TUI information architecture, keybinding discoverability, and mode-switch clarity. Identify where the product may feel inconsistent between workspace mode and repo mode.
```

## 50. Handoff Gate to Beads

Do not hand this plan to beads until the following are true:

- workspace mode shape feels stable
- repo mode shape feels stable
- GitFacade routing is stable enough to implement
- watcher strategy has no major unresolved architectural debates
- phase boundaries feel defensible
- test topology matrix is adequate

If those gates pass, the next skill to use is `flywheel-beads`.
