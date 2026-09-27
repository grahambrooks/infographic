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

- **Takeaway**: one sentence, the thing the viewer should leave knowing.
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
title: Circuit breaker                  # short; shown as the heading
story:
  takeaway: A breaker stops a failing dependency from dragging its callers down with it
  notes: Simplified to a consecutive-failure threshold.   # optional
nodes:                                  # left-to-right order comes from the edges
  client:  { label: Client }
  breaker: { label: Circuit breaker, state: breaker, counter: failures }
  service: { label: Payment service, sublabel: optional second line }
edges:                                  # one edge per direction; pairs get parallel lanes
  call:     { from: client,  to: breaker }
  forward:  { from: breaker, to: service }
  reply:    { from: service, to: breaker }
  response: { from: breaker, to: client, label: optional }
states:                                 # a badge on the node; one value visible at a time
  breaker:
    values: [CLOSED, OPEN, HALF-OPEN]
    tones: { CLOSED: ok, OPEN: fail, HALF-OPEN: warn }    # ok | fail | warn | neutral
counters:
  failures: { label: failures, max: 5 } # shown as "failures 3/5"
phases:
  - name: Closed                        # 1–3 words
    caption: requests pass through to the service   # one short clause
    set: { breaker: CLOSED, failures: 0 }
    flows:
      - { path: [call, forward, reply, response], outcome: ok, rate: 1.5/s }
  - name: Service degrades
    caption: each failed reply counts toward the threshold
    ramp: { failures: 0..5 }            # one step per token arriving back at the counter's node
    flows:
      - { path: [call, forward, reply, response], outcome: fail, rate: 1.5/s }
  - name: Open
    caption: calls fail fast with a fallback
    set: { breaker: OPEN }
    timer: { label: cooldown }          # shrinking bar on the node that shows a state (or `node:`)
    flows:
      - { path: [call, response], outcome: fallback, rate: 1.5/s }   # turns back at the breaker
  - name: Half-open
    caption: one trial request is let through
    set: { breaker: HALF-OPEN }
    flows:
      - { path: [call, forward, reply, response], outcome: ok, count: 1 }
  # ... a final phase that resets every state and counter to its phase-1 value
```

`examples/circuit-breaker.yaml` in this repository is a complete, passing spec.

**How flows work:**

- A `path` is a connected chain of edges.
- Tokens are blue on the way out. From the first edge that heads back toward a
  node already visited, they take the `outcome` colour: `ok` is green, `fail`
  red, `fallback` amber.
- A short-circuit is a path that turns back early (`[call, response]`).
- Give `rate` (a steady stream) or `count` (exactly n tokens), never both.

**Timing** is derived from the flows and caption length. Leave out
`duration`, unless a phase has to be held longer.

**Current limits.** The findings will tell you if you break these.

- Edges are straight, and nodes are ranked left to right by the edges.
- An edge that skips a rank goes through the node in between, which is
  reported as an error. Connect adjacent components only.
- The canvas is at most 1200px wide, so keep captions to one short clause.
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
- `gates.dynamic` says whether a browser checked the animation. In this
  version it is not run, so never claim it passed.

## 4. Critique

Read `<slug>.storyboard.png` with the Read tool. It has one row per phase,
light theme on the left and dark on the right, each captured mid-phase. Judge
each row against its phase:

- Does the frame show what the caption claims? Check the right state badge and
  counter, tokens on the right edges in the right colours, and nothing
  reaching components that should be cut off.
- Would someone new to the pattern follow the sequence from the frames alone?
- Is it accurate? Would an expert object to anything?
- Is the text short, is there one idea per phase, and is there nothing
  decorative?

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
that they show exactly what the timeline intends.
