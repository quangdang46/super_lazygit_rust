# super_lazygit_rust — Pain Points

## Who is this for?

A developer working on **5+ Rust projects simultaneously** on a low-RAM machine.
Cannot open IDEs. Lives in the terminal.

---

## Pain Points

### P1 — No multi-repo visibility
You have 6 repos open. You don't know which ones are dirty, which are ahead, which need a pull.
You have to `cd` into each one and run `git status` manually.
**Cost: ~2 min of context-switching per check. Done 10x a day.**

### P2 — Must open IDE just to see a diff
`git diff` in terminal is readable but you can't act on it — no staging, no hunk selection.
Opening VS Code costs 700MB+ RAM. On a machine already running 5 projects, this kills everything.
**Cost: IDE open = other processes swap to disk. Work stops.**

### P3 — No realtime file watching
Status goes stale the moment you switch away.
You forget a file is modified, commit something incomplete, push broken state.
**Cost: broken commits, wasted CI runs, mental overhead tracking "did I save that?"**

### P4 — Cannot stage selectively without an IDE
`git add -p` exists but it's painful — no visual context, no easy navigation between hunks.
You end up staging everything or nothing, losing the ability to make clean atomic commits.
**Cost: messy commit history. Every commit is a blob of unrelated changes.**

### P5 — Commit + push requires leaving your current context
You're looking at a diff in one terminal. To commit you switch to another.
To push you type the full command. To verify you open a third.
**Cost: 3-5 tool switches per commit. Multiplied across 6 repos per day.**

### P6 — No overview of sync state across repos
You don't know which repos are ahead of remote, which are behind, which have diverged.
You find out when a PR fails or a teammate asks why your branch is stale.
**Cost: sync issues discovered late. Force pushes. Lost work.**

### P7 — Context switching between repos destroys focus
Each repo needs its own terminal, its own mental model, its own state.
Switching means: find the right tab, `cd`, remember where you were, re-orient.
**Cost: ~5 min to recover context per switch. With 6 repos this is your entire day.**

### P8 — No commit history visibility without leaving the terminal
`git log --oneline` gives you hashes and messages but no graph, no file context, no diffing.
You open IDE or GitHub just to understand what changed in the last 3 commits.
**Cost: another IDE open, another 700MB gone.**

---

## What the ideal tool does

One terminal window. One tool.

No IDE. No RAM spike. No context switching.
Everything a developer needs to manage 6 repos fits in a single terminal pane.
