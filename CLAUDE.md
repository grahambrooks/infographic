# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Status

Pre-implementation. `docs/design.md` is the design (architecture, verification
gates, milestones, and an explicit **out of scope** list — don't build what it
cuts without asking). Update this file with build/test commands once the crate
exists.

`spikes/embedding-modes/` is a Playwright probe of which SVG features work in
`<img>` vs `<object>` per browser engine. Run with
`cd spikes/embedding-modes && npm install && npm run probe`. Playwright is pinned
to 1.56.1 because that matches the browser builds cached in
`~/Library/Caches/ms-playwright`; a newer version needs `npx playwright install`.

## Purpose

Skills and tools that make Claude reliably produce compelling, interactive SVG
infographics: technical explainers (software architecture, processes, data
flow) and header/banner images for GitHub project READMEs.

The motivating problem: ad-hoc SVG infographics have so far needed several
rounds of refinement before they were acceptable. The project should shrink
that loop. The known pain points, in order of how often they caused rework:

1. **Visual review.** The only dependable check has been rasterizing the SVG
   to PNG and looking at it. Treat render-and-inspect as a required step of
   every generation, not an optional one — an SVG that parses and an exit code
   of 0 say nothing about whether text is clipped, overlapping, or missing.
2. **Animation** of data/process flow (dashes moving along paths, tokens
   travelling between nodes) staying aligned with the geometry it animates.
3. **Hover/interaction** targets and their tooltips/highlights being aligned
   with, and correctly laid out relative to, the elements they belong to.

A PNG snapshot shows a single frame with no hover state, so pain points 2 and 3
need their own verification strategy (e.g. rendering at several animation
timestamps / with hover states forced, or checking geometry numerically) rather
than relying on one static raster.

## Constraints and lessons that apply here

- **Emit real SVG `<text>`, never `<foreignObject>`.** SVG-only consumers
  (rasterizers, PDF pipelines, `usvg`/`resvg`) silently drop foreignObject
  content — boxes survive, all text vanishes, nothing errors. Learned on the
  `mmd` project. Prefer presentation attributes over a CSS cascade where
  consumer CSS support is uncertain.
- **Fonts:** `usvg` drops text unless generic families (`sans-serif`,
  `monospace`, …) are mapped onto available/embedded faces. A rasterizer that
  renders with different font metrics than the browser will hide
  text-overflow bugs — measure text with the same fonts the viewer will use.
- **GitHub README embedding = `<img>`** (verified by the spike, 2026-09-27, not
  yet through GitHub's camo proxy itself): CSS keyframes and SMIL run in all
  engines; `:hover` runs in none; `prefers-color-scheme` inside the SVG is
  ignored by WebKit/Safari. So the `readme` profile is animation-only with
  light/dark as two files via `<picture>`; hover belongs to the `interactive`
  profile.
- **Stack preference:** pure Rust with no external runtime dependency (no
  Node, headless browser, or JVM) where a Rust option exists; a single crate
  with directory modules rather than a Cargo workspace. If a browser is truly
  required (e.g. to verify hover/animation), make that an explicit, recorded
  design decision.

## Workflow

Trunk-based: commit and push directly to `main`; no branches or PRs unless asked.
