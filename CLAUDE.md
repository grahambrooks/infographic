# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Skills and tools that make Claude reliably produce compelling, animated SVG
infographics: technical explainers (patterns, processes, data flow) and,
later, GitHub README headers.

The entry point is the skill `.claude/skills/infographic/`, driven by a
one-line request ("create an infographic for the circuit breaker pattern").
Claude designs from its own knowledge and writes a YAML spec. The Rust
`infographic` CLI does all layout, timing, SVG emission and verification.
Claude fixes findings in the spec, never in the emitted SVG.

`docs/design.md` is the design: milestones, done-when criteria, and an
explicit **out of scope** list. Don't build what it cuts without asking.
Milestone 1 (circuit breaker end to end, `readme` profile, static gate) is
implemented. The browser gate (milestone 2) and the `interactive` profile
(milestone 3) are not.

## Commands

```bash
cargo build
cargo test                                   # unit tests; the fixture is examples/circuit-breaker.yaml
cargo test timeline::tests::exactly_one      # a single test, by name filter
cargo clippy && cargo fmt
cargo install --path .                       # the skill expects `infographic` on PATH

infographic build examples/circuit-breaker.yaml -o /tmp/out [--frames]
infographic check any.svg [--profile readme|interactive] [--png out.png]
```

Both commands print a JSON report. Exit status is 0 if the gate passed, 1 on
findings with severity error, and 2 if the command itself failed.

Browser verification (Node + Playwright, kept in `spikes/`, not in the crate):

```bash
infographic build <spec> -o <dir> --frames
cd spikes/embedding-modes && npm install && node verify.mjs <dir> <slug>
```

`verify.mjs` checks playback inside `<img>`. It also seeks Chromium, Firefox
and WebKit to each storyboard frame's time, and asserts that the browser shows
exactly the elements the timeline says, with tokens within 1.5px. It has been
confirmed to fail on tampered output. Playwright is pinned to 1.56.1 to match
the browser builds cached in `~/Library/Caches/ms-playwright`.

## Architecture

The pipeline is `spec` → `layout` → `timeline` → `emit` → `check`. It is
built around two single sources of truth, and most bugs this project exists
to prevent come from breaking one of them:

- **`layout::Scene`** holds every position, including measured text runs. The
  emitter only ever reads it.
  - Tokens move along the drawn edge via `<mpath href="#e-…">`. Path data is
    never copied.
  - Every state and counter value is a pre-measured text alternative, so node
    boxes fit the widest one.
- **`timeline::Timeline`** holds every time, as visibility intervals, token
  segments and shrinks over one cycle. Every SMIL animation gets
  `dur = cycle` and `repeatCount="indefinite"`, and is placed by `keyTimes`.
  `check::lint` fails any SVG where durations differ.
- The timeline looks up element ids from the Scene by value. Don't
  re-`format!` ids in two places.

**One emitter, two modes.** `emit::Mode::Animated` writes the deliverable.
`Mode::Frame(t)` writes a still of the same scene at time `t`. The storyboard,
the geometry checks and the contrast checks all run on frames, so they see
every state alternative. `verify.mjs` compares frames against real browsers,
so a frame and the animation can't silently disagree.

**Checks run on SVG, not on the Scene** (`check::geometry` via usvg with the
pinned fonts). That way `check` also works on hand-written SVGs.

- Emitted elements carry `data-role` (`node`, `edge`, `token`) and
  `data-container` (the element whose box a text must stay inside).
- `finding::describe` maps ids back to spec names, so findings read
  `node breaker`, not `n-breaker`. Keep the id scheme in `layout.rs` and
  `describe` in sync.

**Fonts.** IBM Plex Sans (OFL, `assets/fonts/`) is compiled in. The same
bytes are used for:
- measuring text (rustybuzz);
- the subset embedded in every SVG as base64 `@font-face` (fontcull, built
  without its C++ WOFF2 feature);
- usvg rendering, with `sans-serif` mapped onto it.

The spike verified that embedded fonts apply inside `<img>` in all three
engines.

## Constraints and lessons that apply here

- **Emit real SVG `<text>`, never `<foreignObject>`.** SVG-only consumers drop
  it silently (learned on `mmd`). Use presentation attributes, not a CSS
  cascade.
- **GitHub README embedding = `<img>`** (spike, 2026-09-27, not yet through
  GitHub's camo proxy):
  - CSS and SMIL animation run in all engines;
  - `:hover` runs in none;
  - WebKit ignores `prefers-color-scheme` inside the SVG, so light/dark are
    two files switched by `<picture>` (the report's `embed`).
- **A passing gate is not a good infographic.** Always look at the storyboard
  PNG. Conversely, when adding a check, confirm it fails on bad input.
- **Stack:** pure Rust, one crate with directory modules. The one recorded
  exception is a browser for animation and hover verification. It is still
  Node/Playwright in `spikes/`; the design calls for chromiumoxide in the
  crate.

## Workflow

Trunk-based: commit and push directly to `main`; no branches or PRs unless asked.
