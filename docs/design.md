# Infographic: design

Status: draft, 2026-09-27. Nothing is implemented yet.

## Problem

Claude can write SVG infographics by hand, but every one so far has needed
several refinement rounds. The recurring defects were:

- text overflowing or clipping its box, found only by rasterizing to PNG and
  looking;
- flow animations drifting off the edges they are meant to follow;
- hover targets and tooltips misplaced relative to their elements.

All three come from the same cause: **geometry and timing are written more than
once.** A path is drawn in one place and its motion path is copied somewhere
else, a tooltip is positioned separately from its anchor, and two animations
that are meant to be in step each have their own clock. The copies disagree,
and nothing notices until a human looks.

## Goal

A one-line request such as

> create an infographic for the circuit breaker pattern

produces a finished, animated infographic that shows the pattern's traffic
flow, built from Claude's own knowledge of the subject, with no further input
needed. Refinement is an optional follow-up ("make the failures start sooner"),
not a required round.

Two deliverables, split by who does what:

| | Owns | Never does |
|---|---|---|
| **Skill** (`.claude/skills/infographic/`) | Understanding the subject, deciding what to show, writing the YAML spec, critiquing the result | Writing or editing SVG |
| **`infographic` CLI** (Rust) | Layout, timing, SVG output, all verification | Deciding content |

Claude designs and the tool draws. The CLI also accepts a hand-written YAML
spec, and `check` accepts any SVG, but the main path starts from the skill.

Target subjects: resilience and architecture patterns, process and data flow,
and GitHub project headers.

## Workflow

```
request ─▶ [skill] story ─▶ spec.yaml ─▶ [CLI] build + check ─▶ findings / storyboard
                 ▲                                                     │
                 └──────── fix the spec (≤3 rounds) ◀── critique ◀──────┘
```

1. **Story.** From its own knowledge, Claude writes down:
   - the one takeaway;
   - the actors;
   - the **phases** the viewer should see, each with what changes and why.

   For the circuit breaker:

   | # | Phase | What the viewer sees |
   |---|---|---|
   | 1 | Closed, healthy | Requests flow client → breaker → service and back, all green |
   | 2 | Service degrades | Responses turn red; the breaker's failure counter climbs to its threshold |
   | 3 | Trips open | Requests stop at the breaker and return straight away as a fallback; nothing reaches the service; a cooldown timer runs down |
   | 4 | Half-open | A single probe request is let through |
   | 5 | Recovers | The probe succeeds, the breaker closes, and normal flow resumes; then the cycle loops |

   The story goes into the spec's `story:` block, so it is reviewable and
   reusable. The skill doesn't stop to ask for approval; it reports the story
   at the end, so the user can redirect it.
2. **Spec.** Claude turns the story into YAML: nodes, edges, state
   indicators, flows and a phase timeline. It never writes coordinates.
3. **Build and check.** `infographic build spec.yaml` emits both profiles and
   runs the gates. It returns JSON findings and a **storyboard**: one frame per
   phase, plus a contact sheet.
4. **Fix** every finding by editing the spec, never the SVG.
5. **Critique** the storyboard against the rubric. Does each frame show what
   its phase claims? Would someone new to the pattern follow it? Refine, at
   most 3 rounds in total, then deliver the files and report the story, the
   files written, and any remaining issues.

## Spec model

The spec describes **what happens**, never where things go or exact
animation timing.

```yaml
title: Circuit breaker
story:
  takeaway: A breaker stops a failing dependency from taking its callers down with it
profile: [readme, interactive]
nodes:
  client:  { label: Client }
  breaker: { label: Circuit breaker, state: breaker_state, counter: failures }
  service: { label: Payment service }
edges:
  call:     { from: client,  to: breaker }
  forward:  { from: breaker, to: service }
  reply:    { from: service, to: breaker }
  response: { from: breaker, to: client }
states:
  breaker_state: [CLOSED, OPEN, HALF-OPEN]
phases:
  - name: Closed, healthy
    set: { breaker_state: CLOSED, failures: 0 }
    flows:
      - { path: [call, forward, reply, response], outcome: ok, rate: 2/s }
  - name: Service degrades
    flows:
      - { path: [call, forward, reply, response], outcome: fail, rate: 2/s }
    ramp: { failures: 0..5 }
  - name: Trips open
    set: { breaker_state: OPEN }
    flows:
      - { path: [call, response], outcome: fallback, rate: 2/s }  # short-circuited at the breaker
    timer: { label: cooldown }
  - name: Half-open
    set: { breaker_state: HALF-OPEN }
    flows:
      - { path: [call, forward, reply, response], outcome: ok, count: 1 }
  - name: Recovers
    set: { breaker_state: CLOSED, failures: 0 }
    flows:
      - { path: [call, forward, reply, response], outcome: ok, rate: 2/s }
```

Illustrative only; the exact schema will be settled while building milestone 1.
`outcome` maps to semantic colours in the theme, and phase durations come from
the flows they contain, unless the spec overrides them.

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

This gives two output profiles, both generated from one spec by default:

- **`readme`**: animation only, with no hover, links, tooltips, scripts or
  external resources. Dark mode is delivered as **two files** chosen by
  `<picture><source media="(prefers-color-scheme: dark)">`, because
  WebKit/Safari ignores in-SVG theme queries in `<img>`. The phase is shown by
  an on-image caption, not a control.
- **`interactive`**: an HTML page that inlines the SVG, adding hover
  explanations on each node and step controls (pause, previous/next phase).

The spike did not go through GitHub's image proxy (camo). Confirm the results
once with a real README before relying on them.

## Architecture

One crate with directory modules (Graham's convention), pure Rust except for
the one browser dependency justified under Verification.

```
spec.yaml ─parse─▶ model ─layout─▶ Scene ─schedule─▶ Timeline ─emit─▶ SVG / HTML per profile
                                     │                   │
                                     └──── check (static, then dynamic) ◀┘
```

### Scene is the single source of truth for geometry

Layout ends in a `Scene` of positioned primitives with stable ids: node boxes,
edge paths, measured text runs, and groups. Every SVG construct is **derived**
from it and never positioned independently:

- **Moving tokens** follow the drawn edge by id with
  `<animateMotion><mpath href="#e-forward"/>`. Path data is never copied into
  CSS `offset-path` or into a second path, and the emitter has no code path
  that does so. A multi-edge flow (`path: [call, forward, …]`) is a sequence of
  motions on the drawn edges, not one merged path.
- **Hover** (`interactive` only) puts shape, transparent hit area and tooltip
  in one `<g>`, all computed from one box. Tooltip placement stays inside the
  viewBox.
- **Text** is measured at layout time with the same font file that the SVG
  names and the renderer loads. Every state label (`CLOSED`/`OPEN`/…) and
  counter value is measured, so the box fits the widest. Labels are real
  `<text>` elements, never `<foreignObject>`, which SVG-only consumers drop
  silently (learned on `mmd`).

### Timeline is the single source of truth for timing

Scheduling turns phases into a `Timeline`, with absolute start and end times
within one cycle of length *T*. **Every animated element has `dur` = *T* and
`repeatCount="indefinite"`, and places its motion through `keyTimes` inside
that cycle.** Nothing has its own clock, so states, counters, tokens and
captions can't drift apart, however many times the loop repeats. This applies
the Scene principle to time.

- State changes and counter steps are `<set>`/discrete `<animate>` on the
  visibility of pre-measured text alternatives. This works in `<img>`.
- The `interactive` step controls drive the same timeline through
  `pauseAnimations()`/`setCurrentTime()`, so they need no second animation
  system.

Layout and scheduling are tested on `Scene`/`Timeline` data, not by matching
SVG strings. This is the same pattern as `mmd` and `puml`.

### Candidate crates (to be confirmed while building)

| Role | Crate |
|---|---|
| Spec parsing | a maintained serde YAML crate (`serde_yaml` is unmaintained; pick one in milestone 1) |
| Layered graph layout | `rust-sugiyama` (positions only; own orthogonal edge router) |
| Panel and header layout | `taffy` |
| Text measurement | `parley` or `cosmic-text` |
| SVG writing | `svg` |
| Static render and element boxes | `resvg` / `usvg` |
| Dynamic render | `chromiumoxide` driving installed Chrome |
| Image diff | `image-compare` (not `dssim`, which is AGPL) |

## Verification

`build` runs these checks automatically. `infographic check <file.svg>
--profile readme|interactive` runs them on any SVG, including hand-written
ones. Findings come out as JSON that refers to **spec names** (`edge
forward`, `phase 3`) wherever the SVG came from a spec, so Claude can fix the
spec directly.

### Static gate: pure Rust, always runs

- **Spec validation**:
  - references resolve;
  - every flow `path` is a connected chain of edges;
  - state values are declared;
  - no phase is empty.
- The SVG parses with usvg without warnings.
- **Profile lint**:
  - `readme`: fail on `:hover`, `<script>`, `<a>`, `<foreignObject>`, external
    `href`/`url()`, or `prefers-color-scheme` inside the SVG.
  - Both profiles: every internal `href`/`url(#…)`/`begin` reference resolves;
    every animation's `dur` equals the timeline cycle.
- **Geometry**, from usvg element boxes:
  - text inside its container, including every state alternative;
  - no overlapping siblings;
  - nothing outside the viewBox.
- **Contrast**: WCAG ratio of each text fill against the pixels rendered
  behind it. axe-core ignores SVG text, so this has to be implemented here.
- **Render**: resvg PNG with pinned fonts, which is deterministic and so can be
  snapshot-tested.

### Dynamic gate: browser, only when the SVG animates or has hover

No pure-Rust renderer runs SMIL, CSS animation or `:hover`, so a browser is
unavoidable here. **Recorded exception to the pure-Rust preference**, kept
narrow:

- It uses `chromiumoxide` against the user's installed Chrome, with no Node.
- If no Chrome is found, the gate reports **unavailable**, never passed.
- **Storyboard**: pause, then seek to the midpoint of each phase. Screenshot
  it, and check that the visible state label and counter match the values the
  phase declares.
- **Token alignment**: sample moving tokens at N timestamps and check each one
  is on its edge.
- **Hover** (`interactive`): force `:hover` on each group, then check that the
  tooltip stays in the viewBox and that its hit area covers the visible shape.

The engine differences found by the spike are handled as static lint rules,
so per-build runs don't need WebKit or Firefox. Playwright stays in `spikes/`
for occasional cross-engine confirmation.

### Visual critique: last

Only after the gates pass does the skill read the storyboard, so the visual
pass judges whether the design explains the subject, not bugs a machine could
have found.

## Skill rubric and design tokens

Drawn from prior art (`diagram-design`, Ciechanowski, ByteByteGo):

- **Accuracy first.** The phases must be true to how the pattern really
  behaves. For named patterns, stay with the canonical description, and put
  any simplification in `story:`.
- **The argument drives the shape.** Choose pipeline, fan-out, cycle, boundary
  or layers to match the idea.
- **Colour:**
  - one accent colour;
  - semantic colours for outcomes (`ok`, `fail`, `fallback`), kept the same
    across every infographic.
- **Layout:** a 4/8px grid, 2–3 type roles, no shadows or decorative
  gradients, low density.
- **Motion and captions:**
  - motion only shows flow or causality;
  - each phase has a numbered caption;
  - the full cycle stays under ~20s, with each phase long enough to read its
    caption.
- **Accessibility:** `<title>`/`<desc>` summarise the whole story;
  `interactive` respects `prefers-reduced-motion` by starting paused.

## Milestones

1. **Circuit breaker, end to end.** The skill, the spec schema, and `build`
   for a small node/edge graph with flows, states, counters and a phase
   timeline. Output is the `readme` profile only, with the static gate and a
   storyboard from resvg at each phase's time. The deliberately narrow first
   slice proves the whole loop on the motivating example.
2. **Dynamic gate**: chromiumoxide storyboard, token-on-edge sampling.
3. **`interactive` profile**: HTML wrapper, hover explanations, step controls,
   hover checks.
4. **Breadth**: a fixed evaluation set of subjects run through the skill:
   - retry with backoff;
   - bulkhead;
   - transactional outbox;
   - a CI pipeline;
   - request lifecycle through a load balancer.
5. **Headers**: taffy-based banner layouts that carry a one-glance
   architecture or flow for the project.

### Done when (milestone 1)

- In a fresh session, "create an infographic for the circuit breaker pattern"
  produces, **with no further input**: `circuit-breaker.yaml`,
  `circuit-breaker.svg` and `circuit-breaker-dark.svg`.
- The SVG shows all five phases in a loop, with tokens visibly following the
  edges and the state label changing in step with the flow.
- The static gate reports zero findings on the delivered files.
- The storyboard shows the correct state, counter and flow in every phase.
- The SVG plays when opened as `<img>` in Chrome, Firefox and WebKit (checked
  with the `spikes/` harness).

## Out of scope (deliberately)

- **Raster or AI image generation.** The output is always vector.
- **An editor or GUI.**
- **Claude writing or patching SVG directly** in the skill's workflow. Fixes
  go through the spec. `check` on hand-written SVGs remains available.
- **Scripts inside the SVG.** The `interactive` HTML wrapper may script the
  SVG's timeline.
- **Accepting Mermaid, D2 or PlantUML as input.** Possible later; `mmd` and
  `puml` already exist for those.
- **Hosting a service** like capsule-render. Output is committed static files.
- **Cross-engine browser testing on every build.** Known differences are lint
  rules instead.

## Open questions

- **Fonts in `readme`**: whether base64-embedded `@font-face` with a subset
  font works inside `<img>` in all three engines. The `spikes/` harness can
  test this. The fallback is outlining text to paths, which loses
  accessibility.
- **Arrow routing**: whether `rust-sugiyama` plus our own orthogonal router is
  good enough, or D2's ELK port is worth using as an external backend.
- **Optimising output**: whether `oxvg` keeps SMIL and `begin` references
  intact. Until that's known, don't optimise, and snapshot-test whatever is
  emitted.
- **Where outputs go**: probably the current directory, with the file slug
  taken from the title. Confirm this when the skill is written.
