# infographic

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="infographics/infographic-pipeline-dark.svg">
  <img alt="How infographic works: Claude writes a YAML spec; the CLI draws, checks and reports until the gate passes" src="infographics/infographic-pipeline.svg">
</picture>

Skills and tools that make Claude reliably produce animated SVG infographics
explaining technical patterns and processes.

Ask for one in Claude Code with the `infographic` skill:

```
/infographic for the circuit breaker pattern
```

Claude designs the story and writes a YAML spec. The Rust `infographic` CLI
does all layout, timing, SVG output and verification, and reports findings
against the spec. Claude fixes them in the spec, never in the SVG.

## Install

```bash
cargo install --path .
```

## Use

```bash
infographic build examples/circuit-breaker.yaml -o /tmp/out
infographic check any.svg [--profile readme] [--png out.png]
```

Both print a JSON report. `build` writes light and dark SVGs, a storyboard
PNG, and a `<picture>` snippet for embedding in a GitHub README.

See [`docs/design.md`](docs/design.md) for the design and milestones.
