# Showcase

Infographics produced by the `infographic` skill and CLI. Each was built from
the YAML spec linked beside it, and they switch between light and dark with
your GitHub theme.

The benchmark scenarios were each run the same way. A fresh agent got only the
one-line request, then followed the skill from the story, through the spec,
build and storyboard critique, to browser verification. It had no access to
examples, earlier outputs or this page. Every result was then rebuilt with the
current tool and re-verified independently.

"Browser check" means `spikes/embedding-modes/verify.mjs`. It plays the SVG as
`<img>`, then seeks Chromium, Firefox and WebKit to each storyboard frame and
checks, for light and dark, that the browser shows exactly what the timeline
intends.

| Infographic | Phases | Cycle | Build rounds | Final findings | Browser check |
|---|---|---|---|---|---|
| [Circuit breaker](#circuit-breaker) | 5 | 25.7s | 2 | none | 6/6 match |
| [Retry with exponential backoff](#retry-with-exponential-backoff) | 5 | 20.0s | 3 | none | 6/6 match |
| [Transactional outbox](#transactional-outbox) | 5 | 24.3s | 3 | 1 warning, kept on purpose | 6/6 match |
| [TCP three-way handshake](#tcp-three-way-handshake) | 6 | 23.6s | 3 | none | 6/6 match |
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
- **Left out:** a failed half-open trial (which would reopen the breaker) is
  described in the spec's notes, not animated.

## Retry with exponential backoff

Request: *"create an infographic for retry with exponential backoff"*.
Files: [spec](../infographics/retry-backoff.yaml),
[storyboard](../infographics/retry-backoff.storyboard.png).

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="../infographics/retry-backoff-dark.svg">
  <img alt="Retry with exponential backoff: Retry transient failures, doubling the wait each time so a struggling service can recover" src="../infographics/retry-backoff.svg">
</picture>

- **Phases:** first try fails (`GET` → `503`) → wait 1s, retry fails → wait
  2s, retry fails → wait 4s, retry succeeds (`200`) → the failure count resets.
- **At true scale:** each wait is a `timer` with `seconds: 1s / 2s / 4s`,
  paired with the same `delay` on the retry, so the retry leaves only after
  its backoff runs out.
- **Left out:** jitter, the delay cap and giving up after N attempts are
  described in the spec's notes.

## Transactional outbox

Request: *"create an infographic for the transactional outbox pattern"*.
Files: [spec](../infographics/transactional-outbox.yaml),
[storyboard](../infographics/transactional-outbox.storyboard.png).

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="../infographics/transactional-outbox-dark.svg">
  <img alt="Transactional outbox: Commit the event with the data in one transaction; a relay publishes it later" src="../infographics/transactional-outbox.svg">
</picture>

- **Phases:**
  1. The order and its event commit in one transaction (`TX` / `commit`).
  2. The relay polls the pending row (`SELECT` / `event`).
  3. The broker is down, so publishing fails (`error`).
  4. After a 2s backoff the retry is acked.
  5. Only then is the row marked sent (`UPDATE` / `ok`), and the outbox badge
     goes from PENDING to SENT.
- **Kept warning:** the badge ends the cycle as SENT and restarts at PENDING.
  That jump is the next order's commit, so the agent left the warning in on
  purpose.
- **Left out:** consumers, and the duplicate delivery after a relay crash
  (at-least-once delivery), are described in the spec's notes.

## TCP three-way handshake

Request: *"create an infographic for the TCP three-way handshake"*.
Files: [spec](../infographics/tcp-handshake.yaml),
[storyboard](../infographics/tcp-handshake.storyboard.png).

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="../infographics/tcp-handshake-dark.svg">
  <img alt="TCP three-way handshake: SYN, SYN-ACK, ACK sync both sides' sequence numbers before any data flows" src="../infographics/tcp-handshake.svg">
</picture>

- **Phases:** listening → `SYN` → `SYN-ACK` → `ACK` → data (`DATA` / `ACK`)
  → torn down, back to listening. Each side shows its TCP state, and the
  transitions match RFC 9293.
- **Messages:** each packet is labelled on the wire, so SYN and ACK are
  distinct even though they share the client-to-server line.
- **Approximations:**
  - SYN-ACK is drawn in the request colour.
  - The FIN teardown is named in the caption, not animated.

## How infographic works

The project's own header ([spec](../infographics/infographic-pipeline.yaml),
[storyboard](../infographics/infographic-pipeline.storyboard.png)): request →
findings fixed in the spec → gate passes → browser check → delivered.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="../infographics/infographic-pipeline-dark.svg">
  <img alt="How infographic works: Claude writes a YAML spec; the CLI draws, checks and reports until the gate passes" src="../infographics/infographic-pipeline.svg">
</picture>

---

## Benchmark history

### Round 1

The three scenarios were first run with the tool as it was at milestone 1:

| Scenario | Builds | Result | Main problem |
|---|---|---|---|
| Retry | 5 builds, 3 revisions | passed | A second timer showed the wrong label and never animated, with no finding. |
| Outbox | 3 | passed | Storyboard rows 2 and 4 were empty. |
| TCP | 5 builds, 3 revisions | passed, 1 warning | Storyboard rows 1–3 showed no packets. |

### Round 2: after fixing timers and frame capture

With fresh agents and the same requests:

| Scenario | Builds | Result | Gate findings on the way | Caught only by the storyboard review |
|---|---|---|---|---|
| Retry | 2 (was 5) | passed, no findings | none | The three backoff phases lasted the same time, so nothing visibly doubled. The timer overlapped the retry it delays. "3/3" read as "gave up" before the success. |
| Outbox | 3 (was 3) | passed, no findings | Canvas too wide (five columns), fixed by dropping the consumer. A loop-jump warning, fixed by reverting a counter change. | The backlog showed 3 before any write in that phase had landed. |
| TCP | 3 (was 5) | passed, no findings (was 1 warning) | A loop-jump warning, fixed by adding a "Listening" opener. | SYN-ACK drawn in the request colour. The close badges changing while the FIN is in flight. |

- **Build rounds** went from 5, 3 and 5 to 2, 3 and 3.
- **Storyboard frames:** every phase with traffic now shows it. A test checks
  this for every spec in the repository.
- **Retry:** uses three timers on one node, which now works.
- **Workarounds dropped:** the round-1 agents had shortened `duration` just to
  get a token into a frame. No round-2 agent had to.

The round-2 runs also found four tool bugs, all now fixed:
- `build --frames` left stale frames from earlier builds, which produced false
  mismatches.
- `verify.mjs` reported "doesn't play" for a spec that opens with a still
  phase.
- `verify.mjs` used a fixed temp file name, so concurrent runs collided.
- A canvas that was too wide because of five columns blamed the widest
  caption.

### Round 3: after adding waits and message labels

The first two open gaps were fixed. Flows gained `delay`, timers gained
`seconds`, and flows gained `label` / `reply_label`, a label that travels with
its token. Fresh agents, same requests:

| Scenario | Builds | Result | New features used | Caught only by the storyboard review |
|---|---|---|---|---|
| Retry | 3 | passed, no findings | `delay` and timer `seconds` at 1s/2s/4s. `GET`, `503` and `200` labels. | An "attempts 0/4" counter lagged behind the attempt in flight, so the agent switched to counting failures. |
| Outbox | 3 | passed, 1 warning kept | `TX`/`commit`, `SELECT`/`event`, `UPDATE`/`ok`, and `error`/`ack` labels. A 2s backoff with `delay`. | Two flows in one phase let the less important one win the frame, so the agent split the phase. A PENDING badge showed before the commit. |
| TCP | 3 | passed, no findings | `SYN`, `SYN-ACK`, `ACK` and `DATA`/`ACK` labels on two shared lines. | Captions claimed "data both ways" and a teardown that the motion didn't show. |

- **Retry:** "wait, then retry" is now drawn at true scale. Round 2 had to
  carry it in phase lengths.
- **TCP:** SYN and ACK are now told apart on the wire. Round 2 had them
  unlabelled on a shared line, with the segment names only in captions.
- **Build rounds:** 3, 3 and 3. The first build passed the gate in every run;
  all later rounds came from the agents' own storyboard review.
- Every feature did what the skill documents.

### Open gaps

These are what the spec can't yet express, in rough priority order:

| Gap | Seen in |
|---|---|
| States change only at phase start, not when a token arrives, so badges switch while the message that causes the change is still in flight. | outbox, TCP (every run) |
| The loop-restart check wants an exact match, so a real restart (CLOSED → SYN-SENT, SENT → PENDING) needs an idle phase or a kept warning. | TCP, outbox |
| The storyboard frame prefers return legs, so outbound labels (`SELECT`, `UPDATE`) and a timer mid-countdown never appear in a still. | outbox, retry |
| Colour follows "outbound until it turns back", so a reply that is its own path (SYN-ACK) can't look like a reply. | TCP |
| Counters only step on tokens returning to their node, so a store can't count incoming writes. | outbox, retry |
| Phases with no traffic work but are undocumented in the skill. | retry, TCP |
| About four columns fit, and edges can't skip a column. | outbox (dropped the consumer) |
