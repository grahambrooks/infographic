---
name: infographic
description: Create an animated SVG infographic that explains a technical pattern or process as traffic flowing between components, from a one-line request such as "create an infographic for the circuit breaker pattern". Use when the user asks for an infographic, animated diagram, flow animation, or README-ready SVG explaining how something works (resilience patterns, request lifecycles, pipelines, protocols).
---

# Infographic

You design, the `infographic` CLI draws. You decide what the viewer should
understand and write it as a YAML spec. The CLI does all layout, timing, SVG
output and verification. **Never write or edit SVG yourself.** Every fix goes
into the spec, because the SVG is regenerated from it.

Work through to delivery without asking questions. The user can redirect
afterwards.

## 0. Tool

Run `infographic --version`. If it isn't found, install it from this
repository (the directory three levels above this file) with
`cargo install --path <repo>`.

## 1. Story

From your own knowledge of the subject, decide these things before writing any
YAML:

- **Takeaway**: one sentence, the thing the viewer should leave knowing. It
  is also the subtitle and the image's alt text, so keep it under about 90
  characters.
- **Actors**: 2–5 components. Traffic flows between them.
- **Phases**: 3–6 moments that each show one change in behaviour, in causal
  order, ending where the loop can restart cleanly.

**Be accurate.** Phases must match how the pattern really behaves. For a named
pattern, follow the canonical description. If you simplify something (for
example, a consecutive-failure count instead of a sliding-window rate), say so
in `story.notes`.

## 2. Spec

Write the spec to `infographics/<slug>.yaml` in the current directory, unless
the user named a location. The slug becomes the output file names.

```yaml
title: Health-checked load balancing   # short; shown as the heading
story:
  takeaway: A load balancer stops routing to a backend that fails its health checks
  notes: Health checks are implied, not drawn.   # optional: simplifications you made
nodes:                                  # left-to-right order comes from the edges;
  client: { label: Client }             # nodes in the same rank stack vertically
  lb:     { label: Load balancer, counter: errors }
  a:      { label: Server A }
  b:      { label: Server B, state: health, sublabel: optional second line }
edges:                                  # one edge per direction; pairs get parallel lanes
  req:    { from: client, to: lb }
  to_a:   { from: lb, to: a }
  from_a: { from: a, to: lb }
  to_b:   { from: lb, to: b }
  from_b: { from: b, to: lb, label: optional }
  resp:   { from: lb, to: client }
states:                                 # a badge on the node; one value visible at a time
  health:
    values: [HEALTHY, UNHEALTHY]
    tones: { HEALTHY: ok, UNHEALTHY: fail }   # ok | fail | warn | neutral
counters:
  errors: { label: errors, max: 3 }     # shown as "errors 2/3"; `max` optional
phases:
  - name: Round robin                   # 1–3 words
    caption: requests alternate between both servers   # one short clause
    set: { health: HEALTHY, errors: 0 }
    flows:                              # concurrent flows interleave automatically
      - { path: [req, to_a, from_a, resp], outcome: ok, rate: 0.8/s }
      - { path: [req, to_b, from_b, resp], outcome: ok, rate: 0.8/s }
  - name: Server B fails
    caption: its replies come back as errors
    ramp: { errors: 0..3 }              # one step per token arriving back at the counter's node
    flows:
      - { path: [req, to_b, from_b, resp], outcome: fail, rate: 1/s }
  - name: Marked unhealthy
    caption: traffic is routed to A only
    set: { health: UNHEALTHY }
    timer: { node: b, label: next check }   # shrinking bar for the phase; a node can have one per phase, each with its own label
    flows:
      - { path: [req, to_a, from_a, resp], outcome: ok, rate: 1.6/s }
  - name: Recovered                     # last phase restores phase 1's values,
    caption: B rejoins the pool         # so the loop restarts without a jump
    set: { health: HEALTHY, errors: 0 }
    flows:
      - { path: [req, to_a, from_a, resp], outcome: ok, rate: 0.8/s }
      - { path: [req, to_b, from_b, resp], outcome: ok, rate: 0.8/s }
```

This example is complete and passes the gate (a test builds it). `examples/` holds more. Look at them for syntax only; design each infographic from its subject.

**How flows work:**

- A `path` is a connected chain of edges.
- Tokens are blue on the way out. From the first edge that heads back toward a
  node already visited, they take the `outcome` colour: `ok` is green, `fail`
  red, `fallback` amber.
- A short-circuit is a path that turns back early, for example
  `[call, response]` where `response` leaves the node `call` reached.
- `fallback` (amber) is for a degraded answer that isn't the real result.
- Give `rate` (a steady stream) or `count` (exactly n tokens), never both.

**Timing** is derived from the flows and caption length. Leave out
`duration`, unless a phase has to be held longer.

- A `rate` phase lasts at least 5s. It spawns tokens for about 3s, then lets
  them finish, so `rate` sets how dense the stream looks.
- A `count` or `ramp` phase lasts as long as its tokens take.
- The storyboard captures each phase at its most telling moment: a token in
  flight (preferring the outcome colour on the way back), the ramp as far
  along as possible, and tokens mid-edge. A phase with traffic always shows
  some. There is no need to tune `duration` to get tokens into the frame.

**Ramps** count every token that arrives back at the counter's node on its
return leg, whatever its outcome. Put only the flow being counted in a ramp
phase.

**Current limits.** The findings will tell you if you break these.

- Edges are straight, and nodes are ranked left to right by the edges.
- An edge that skips a rank goes through the node in between, which is
  reported as an error. Connect adjacent components only.
- The canvas is at most 1200px wide, so keep captions to one short clause.
  A row fits about four components; a fifth column rarely fits.
- One loop tells one path through the story. Alternative branches (for
  example, a failed trial that reopens a breaker) go in `story.notes`, not in
  extra phases, unless they are the point.
- Output is the `readme` profile only: animation, no hover.

## 3. Build and check

```bash
infographic build infographics/<slug>.yaml
```

This writes `<slug>.svg`, `<slug>-dark.svg` and `<slug>.storyboard.png`, and
prints a JSON report.

- Exit status 0 means the static gate passed, and 1 means findings with
  severity `error`.
- Each finding names its **spec target**, such as `node breaker`, `phase 3
  caption` or `edge call`. Fix every error, and every warning you can, **in
  the spec**. Then rebuild.
- `gates.dynamic` is always "not run" for `build`. Browser playback counts as
  verified only when `verify.mjs` passes (step 5). Otherwise, say it wasn't
  verified.

## 4. Critique

Read `<slug>.storyboard.png` with the Read tool. It has one row per phase,
light theme on the left and dark on the right. Each row is captured at the
phase's most telling moment (see Timing above). Judge each row against its
phase:

- Does the frame show what the caption claims? Check the right state badge and
  counter, tokens on the right edges in the right colours, and nothing
  reaching components that should be cut off.
- Would someone new to the pattern follow the sequence from the frames alone?
- Is it accurate? Would an expert object to anything?
- Is the text short, is there one idea per phase, and is there nothing
  decorative?
- Does the motion say what the caption says? For example, "alternate" needs
  interleaved tokens, and "only A" needs nothing on B's edges.

Revise the spec and rebuild. Take **at most 3 rounds** in total, then deliver
what you have and say what's left.

## 5. Deliver

Tell the user:

- **The story**: the takeaway and the phases, each in one line.
- **The files** written.
- **The `embed` snippet** from the report (a `<picture>` element that switches
  light/dark on GitHub).
- **Anything unresolved**, including that browser playback was not verified
  unless it was.

Browser verification, when working inside this repository:

```bash
infographic build infographics/<slug>.yaml --frames
cd spikes/embedding-modes && npm install && node verify.mjs ../../infographics <slug>
```

This seeks Chromium, Firefox and WebKit to each storyboard frame, and checks
that they show exactly what the timeline intends. `<slug>.frames/` holds the
frames it compares against. It is diagnostic output, ignored by git; don't
deliver it.
