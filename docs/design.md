# Infographic: design

Status: draft, 2026-09-27. Nothing is implemented yet.

## Problem

Claude can write SVG infographics by hand, but every one so far has needed
several refinement rounds. The recurring defects were:

- text overflowing or clipping its box, found only by rasterizing to PNG and
  looking;
- flow animations drifting off the edges they are meant to follow;
- hover targets and tooltips misplaced relative to their elements.

All three come from the same cause: **geometry is written more than once.** A
path is drawn in one place and its motion path is copied somewhere else, a
tooltip is positioned separately from its anchor, and a box is sized without
measuring its text. The copies disagree, and nothing notices until a human
looks.

## Goal

Claude produces an infographic that is right on the first render, or is told
exactly what is wrong in a form it can act on. Two deliverables:

1. **`infographic`**, a Rust CLI that turns a declarative spec into SVG and
   verifies any SVG, hand-written or generated.
2. **A Claude Code skill** that drives the CLI through a bounded
   spec → build → check → critique → refine loop.

Target subjects: software architecture, process and data flow, and GitHub
project headers.

## Constraint that shapes everything: embedding mode

The spike in `spikes/embedding-modes/` checked what each browser engine
actually does with an SVG embedded as `<img>` (the way GitHub READMEs show
images) and as `<object>` (a stand-in for inline). Results from 2026-09-27,
using Playwright 1.56.1 (Chromium 1194, Firefox 1495, WebKit 2215) plus
installed Google Chrome:

| Feature | `<img>`: Chromium/Chrome | `<img>`: Firefox | `<img>`: WebKit | `<object>`: all |
|---|---|---|---|---|
| CSS `@keyframes` animation | runs | runs | runs | runs |
| SMIL `<animate>` | runs | runs | runs | runs |
| CSS `:hover` | **no** | **no** | **no** | works |
| `@media (prefers-color-scheme)` inside the SVG | works | works | **no** | works |

This settles an earlier conflict in the research: CSS animation does run inside
`<img>` in current Chromium. It gives two fixed output profiles:

- **`readme`**: animation only, with no hover, links, tooltips, scripts or
  external resources. Dark mode is delivered as **two files** chosen by
  `<picture><source media="(prefers-color-scheme: dark)">`, never by a media
  query inside the SVG, because WebKit/Safari ignores it in `<img>`.
- **`interactive`**: for inlining in an HTML page, and allowed to use hover,
  tooltips, links, step controls and in-SVG theme queries.

The spike did not go through GitHub's image proxy (camo). The proxy only
serves the file, and the browser enforces the `<img>` restrictions, so the
results should carry over. Confirm this once with a real README before
relying on it.

## Architecture

One crate with directory modules (Graham's convention), pure Rust except for
the one browser dependency justified below.

```
spec (YAML/TOML) ──parse──▶ model ──layout──▶ Scene ──emit──▶ SVG (per profile)
                                                │
                                                └──▶ check (static, then dynamic)
```

### Scene is the single source of truth

Layout ends in a `Scene` of positioned primitives with stable ids: node boxes,
edge paths, measured text runs, and groups. Every SVG construct is **derived**
from it and never positioned independently:

- **Flow animation** references the drawn edge by id, either with
  `<animateMotion><mpath href="#e-3"/>` or with `stroke-dashoffset` on the
  `#e-3` element itself. Path data is never copied into CSS `offset-path` or
  into a second path, and the emitter has no code path that does so.
- **Hover** puts shape, transparent hit area and tooltip in one `<g>`, all
  computed from one box, and reveals the tooltip with `g:hover > .tip`.
  Tooltip placement is chosen so that it stays inside the viewBox.
- **Text** is measured at layout time with the same font file that the SVG
  names and the renderer loads. Labels are real `<text>` elements, never
  `<foreignObject>`, which SVG-only consumers drop silently (learned on
  `mmd`).

Layout is tested on `Scene` geometry, not by matching SVG strings. This is the
same pattern as `mmd` and `puml`.

### Candidate crates (to be confirmed while building)

| Role | Crate |
|---|---|
| Layered graph layout | `rust-sugiyama` (positions only; own orthogonal edge router) |
| Panel and header layout | `taffy` |
| Text measurement | `parley` or `cosmic-text` |
| SVG writing | `svg` |
| Static render and element boxes | `resvg` / `usvg` |
| Dynamic render | `chromiumoxide` driving installed Chrome |
| Image diff | `image-compare` (not `dssim`, which is AGPL) |

## Verification

`infographic check <file.svg> --profile readme|interactive` runs on **any**
SVG, including hand-written ones, and reports findings as JSON (for Claude)
plus a contact-sheet PNG (for Claude and humans to look at).

### Static gate: pure Rust, always runs

- The SVG parses with usvg without warnings.
- **Profile lint**:
  - `readme`: fail on `:hover`, `<script>`, `<a>`, `<title>` used as a
    tooltip, `<foreignObject>`, external `href`/`url()`, or
    `prefers-color-scheme` inside the SVG.
  - Both profiles: every internal `href`/`url(#…)`/`begin="id.event"`
    resolves.
- **Geometry**, from usvg element boxes:
  - text stays inside its container;
  - sibling boxes don't overlap;
  - nothing extends past the viewBox.
- **Contrast**: WCAG ratio of each text fill against the pixels rendered
  behind it. axe-core ignores SVG text, so this has to be implemented here.
- **Render**: resvg PNG with pinned fonts, which is deterministic and so can
  be snapshot-tested.

### Dynamic gate: browser, only when the SVG animates or has hover

No pure-Rust renderer runs SMIL, CSS animation or `:hover`, so a browser is
unavoidable here. **Recorded exception to the pure-Rust preference**, kept
narrow:

- It uses `chromiumoxide` against the user's installed Chrome, with no Node.
- It runs only if the SVG contains animation or hover rules.
- If no Chrome is found, the gate reports **unavailable**, never passed.
- For animation:
  - pause, then seek to N timestamps (`pauseAnimations()` +
    `setCurrentTime()` for SMIL, `getAnimations()` for CSS);
  - check each moving element's distance to its path;
  - add the frames to the contact sheet.
- For `interactive`: force `:hover` on each group, then check that the
  tooltip stays in the viewBox, doesn't cover its anchor, and that its hit
  area covers the visible shape.

The engine differences found by the spike are handled as **static lint rules**,
so per-build runs don't need WebKit or Firefox. Playwright stays in
`spikes/` for occasional cross-engine confirmation.

### Visual critique: last

After the gates pass, the skill has Claude read the contact sheet and score it
against a rubric (below). Deterministic checks come first, so the visual pass
judges design, not bugs a machine could have found.

## The skill

It lives in `.claude/skills/infographic/`. The loop is:

1. **Decide the argument.** State one takeaway, and choose the shape that
   mirrors it: pipeline, fan-out, cycle, boundary or layers.
2. **Write the spec**: nodes, edges, groups, flow steps, profile and theme.
3. **Build and check**: `infographic build spec.yaml --check`.
4. **Fix every finding** in the spec, not by editing the SVG.
5. **Critique the contact sheet** against the rubric, then refine. At most 3
   rounds, then stop and report what remains.

Rubric and design tokens, drawn from prior art (`diagram-design`, Ciechanowski,
ByteByteGo):

- one accent colour plus colours with fixed meanings;
- a 4/8px grid and 2–3 type roles;
- no shadows or decorative gradients, and low density;
- motion only encodes flow or causality, with numbered steps and a loop of
  10s or less;
- motion respects `prefers-reduced-motion` (on the `interactive` profile;
  SMIL can't be gated by it);
- `<title>`/`<desc>` present for accessibility.

## Milestones

1. **`check` on existing SVGs.** Static gate and contact sheet. This helps the
   current ad-hoc workflow right away, and it fixes the quality bar before
   any generator exists.
2. **Dynamic gate**: animation sampling and hover forcing via chromiumoxide.
3. **`build` for flow diagrams**: spec → layered layout → `readme` and
   `interactive` output, with animation derived from edges.
4. **Headers**: taffy-based banner layouts that carry a one-glance
   architecture or flow for the project, not decoration.
5. **The skill** wrapping 1–4, evaluated on a fixed set of example subjects.

## Out of scope (deliberately)

- **Raster or AI image generation.** The output is always vector.
- **An editor or GUI.**
- **Scripts inside SVGs.** Interactivity is CSS/SMIL only; step controls in
  an HTML wrapper are allowed.
- **Accepting Mermaid, D2 or PlantUML as input.** Possible later; `mmd` and
  `puml` already exist for those.
- **Hosting a service** like capsule-render. Output is committed static files.
- **Cross-engine browser testing on every build.** Known differences are lint
  rules instead.

## Open questions

- **Spec format**: YAML or TOML. Nested graphs favour YAML, but the choice of
  Rust YAML crate needs checking because `serde_yaml` is unmaintained.
- **Fonts in `readme`**: whether base64-embedded `@font-face` with a subset
  font works inside `<img>` in all three engines. The `spikes/` harness can
  test this. The fallback is outlining text to paths, which loses
  accessibility.
- **Arrow routing**: whether `rust-sugiyama` plus our own orthogonal router is
  good enough, or D2's ELK port is worth using as an external backend.
- **Optimising output**: whether `oxvg` keeps SMIL and `begin` references
  intact. Until that's known, don't optimise, and snapshot-test whatever is
  emitted.
