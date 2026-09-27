# Showcase

Infographics produced by the `infographic` skill and CLI. Each was built from
the YAML spec linked beside it, and they switch between light and dark with
your GitHub theme.

The benchmark scenarios were each run the same way. A fresh agent got only the
one-line request, then followed the skill from the story, through the spec,
build and storyboard critique, to browser verification. It had no access to
other examples. Every result was then rebuilt and re-verified independently.

"Browser check" means `spikes/embedding-modes/verify.mjs`. It plays the SVG as
`<img>`, then seeks Chromium, Firefox and WebKit to each storyboard frame and
checks, for light and dark, that the browser shows exactly what the timeline
intends.

| Infographic | Phases | Cycle | Build rounds | Final findings | Browser check |
|---|---|---|---|---|---|
| [Circuit breaker](#circuit-breaker) | 5 | 25.7s | 2 | none | 6/6 match |
| [Retry with exponential backoff](#retry-with-exponential-backoff) | 6 | 20.9s | 5 builds, 3 revisions | none | 6/6 match |
| [Transactional outbox](#transactional-outbox) | 4 | 21.0s | 3 | none | 6/6 match |
| [TCP three-way handshake](#tcp-three-way-handshake) | 5 | 17.5s | 5 builds, 3 revisions | 1 warning, kept on purpose | 6/6 match |
| [How infographic works](#how-infographic-works) | 5 | 22.9s | — | none | 6/6 match |

---

## Circuit breaker

Request: *"create an infographic for the circuit breaker pattern"*.
Files: [spec](../infographics/circuit-breaker.yaml),
[storyboard](../infographics/circuit-breaker.storyboard.png).

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="../infographics/circuit-breaker-dark.svg">
  <img alt="Circuit breaker: A circuit breaker stops calling a failing service and fails fast until a trial call shows it has recovered" src="../infographics/circuit-breaker.svg">
</picture>

- **Phases:** closed → failures counted to the threshold → open (fail fast,
  cooldown) → half-open trial → closed again.
- **How it went:** the first round passed the gate. The second round was
  polish only.
- **Left out:** a failed half-open trial (which would reopen the breaker) is
  described in the spec's notes, not animated.

## Retry with exponential backoff

Request: *"create an infographic for retry with exponential backoff"*.
Files: [spec](../infographics/retry-backoff.yaml),
[storyboard](../infographics/retry-backoff.storyboard.png).

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="../infographics/retry-backoff-dark.svg">
  <img alt="Retry with exponential backoff: A failed call is retried after waits that double each time, giving the service room to recover" src="../infographics/retry-backoff.svg">
</picture>

- **Phases:** call fails → wait 1s → retry fails → wait 2s → retry succeeds →
  the retry budget resets.
- **Workaround:** each wait is its own empty phase, with its duration in
  proportion to the wait (2.5s for 1s, 5s for 2s) and a `WAIT 1s` / `WAIT 2s`
  badge. The spec can't express a delay inside a phase.
- **Tool bug found:** a node supports only one timer. The first attempt used
  two cooldown bars on the client. The second bar showed the first bar's label
  and never animated, and **the gate was silent**. Only the storyboard
  critique caught it.

## Transactional outbox

Request: *"create an infographic for the transactional outbox pattern"*.
Files: [spec](../infographics/transactional-outbox.yaml),
[storyboard](../infographics/transactional-outbox.storyboard.png).

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="../infographics/transactional-outbox-dark.svg">
  <img alt="Transactional outbox: Save the event in the same transaction as the data; a relay publishes it later" src="../infographics/transactional-outbox.svg">
</picture>

- **Phases:** atomic write → relay publishes and marks sent → broker down,
  events pile up in the outbox → backlog drains.
- **Workaround:** the backlog *drain* is animated as a ramp from 3 to 0. Its
  *growth* is a jump, because counters only step when a token returns to the
  counter's node, and writes return to the service, not the database. The
  atomic two-row write is one token on an edge labelled "one transaction".
- **Storyboard gap:** the frames for phases 2 and 4 show no tokens. They are
  captured just after the ramp's last step, when nothing is in flight. The
  animation plays correctly.

## TCP three-way handshake

Request: *"create an infographic for the TCP three-way handshake"*.
Files: [spec](../infographics/tcp-handshake.yaml),
[storyboard](../infographics/tcp-handshake.storyboard.png).

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="../infographics/tcp-handshake-dark.svg">
  <img alt="TCP three-way handshake: SYN, SYN-ACK, ACK sync both sides' sequence numbers before any data flows" src="../infographics/tcp-handshake.svg">
</picture>

- **Phases:** SYN → SYN-ACK → ACK → established, with data both ways →
  closed, back to listening. Each side shows its own TCP state.
- **Workarounds:**
  - Two edges in the same direction between the same nodes are drawn on top
    of each other, so SYN and ACK share one edge.
  - Segment fields (`seq=x`, `ack=y+1`) are in the phase names, because labels
    can't change by phase.
  - SYN-ACK is blue ("request"), not a reply colour, because a single-edge
    path always starts outbound.
- **Storyboard gap:** each handshake packet crosses in about 0.5s, and the
  mid-phase frame misses it. Rows 1–3 show states and captions but no packet.
- **Kept warning:** the client jumps from CLOSED to SYN-SENT when the loop
  restarts. That's the real transition, so the warning was left in on
  purpose.

## How infographic works

The project's own header ([spec](../infographics/infographic-pipeline.yaml),
[storyboard](../infographics/infographic-pipeline.storyboard.png)): request →
findings fixed in the spec → gate passes → browser check → delivered.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="../infographics/infographic-pipeline-dark.svg">
  <img alt="How infographic works: Claude writes a YAML spec; the CLI draws, checks and reports until the gate passes" src="../infographics/infographic-pipeline.svg">
</picture>

---

## What the benchmark found

Every run ended with a passing gate and animation verified in the browsers. The
gaps are in what the spec can express, and in what the storyboard shows. In
rough priority order:

| # | Gap | Seen in | Kind |
|---|---|---|---|
| 1 | The storyboard frame misses short or single-token phases. It is captured mid-phase, or after a ramp's last step, even when nothing is in flight then. | TCP rows 1–3, outbox rows 2 and 4, retry (worked around with a shorter `duration`) | storyboard |
| 2 | A second timer on a node keeps the first timer's label and never animates, and nothing reports it. | retry | **bug** |
| 3 | Two edges in the same direction between the same nodes overlap exactly, and only their labels are reported. | TCP | layout |
| 4 | There's no delay or wait inside a phase, so growing backoff needs empty phases. | retry | spec |
| 5 | Colour follows "outbound until it turns back", so a reply that is its own path (SYN-ACK) can't look like a reply. | TCP | spec |
| 6 | Counters only step when tokens return, so a counter can't count writes into a store (the outbox backlog). | outbox | spec |
| 7 | There's no way to mark a jump at the loop restart as intended, so a correct cycle keeps a warning. | TCP | spec |
| 8 | `duration: 2s` is rejected; only a plain number works. A `duration` shorter than the time needed to read the caption isn't warned about. | TCP, retry | spec |
