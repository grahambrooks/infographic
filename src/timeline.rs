//! Scheduling: phases → `Timeline`, the single source of truth for timing.
//! Every animated element is emitted with `dur` equal to the one cycle and
//! positioned inside it by `keyTimes`, so nothing can drift out of step.

use indexmap::IndexMap;

use crate::finding::Finding;
use crate::layout::Scene;
use crate::spec::{SetValue, Spec, Tone, parse_ramp};

/// Token speed along edges, px/s.
pub const SPEED: f64 = 300.0;
const LEAD: f64 = 0.4;
const TAIL: f64 = 0.5;
const DEFAULT_RATE: f64 = 1.5;
const RATE_PHASE: f64 = 5.0;
/// Seconds during which a steady (`rate`) flow keeps spawning tokens, so the rate is visible.
const RATE_WINDOW: f64 = 3.0;
const MIN_PHASE: f64 = 3.0;

#[derive(Debug, Clone)]
pub struct Span {
    pub name: String,
    pub start: f64,
    pub end: f64,
    /// When the storyboard captures this phase: mid-phase, or just after a
    /// ramp's last step so the frame shows where the counter got to.
    pub frame: f64,
}

/// One token crossing one edge. Each segment is its own element, visible only
/// while it travels, moving along the drawn edge it references.
#[derive(Debug, Clone)]
pub struct Segment {
    pub id: String,
    pub edge: String,
    pub t0: f64,
    pub t1: f64,
    /// `None` on the outbound (request) leg; the outcome's tone on the way back.
    pub tone: Option<Tone>,
}

#[derive(Debug, Clone)]
pub struct Shrink {
    pub id: String,
    pub t0: f64,
    pub t1: f64,
}

#[derive(Debug, Clone)]
pub struct Timeline {
    pub cycle: f64,
    pub phases: Vec<Span>,
    /// Half-open `[start, end)` intervals. Elements not listed are always visible.
    pub visible: IndexMap<String, Vec<(f64, f64)>>,
    pub segments: Vec<Segment>,
    pub shrinks: Vec<Shrink>,
}

impl Timeline {
    pub fn is_visible(&self, id: &str, t: f64) -> bool {
        match self.visible.get(id) {
            None => true,
            Some(iv) => iv.iter().any(|&(a, b)| t >= a && t < b),
        }
    }

    fn show(&mut self, id: String, a: f64, b: f64) {
        if b > a {
            self.visible.entry(id).or_default().push((a, b));
        }
    }
}

/// A value that changes at points in time, e.g. a breaker's state.
struct Track {
    events: Vec<(f64, String)>,
}

impl Track {
    fn at(&mut self, t: f64, v: String) {
        self.events.push((t, v));
    }

    /// `(value, start, end)` runs across the cycle.
    fn runs(&self, cycle: f64) -> Vec<(String, f64, f64)> {
        let mut ev = self.events.clone();
        ev.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut out: Vec<(String, f64, f64)> = Vec::new();
        for (i, (t, v)) in ev.iter().enumerate() {
            let end = ev.get(i + 1).map_or(cycle, |n| n.0);
            if end > *t {
                out.push((v.clone(), *t, end));
            }
        }
        out
    }
}

pub fn schedule(spec: &Spec, scene: &Scene) -> (Timeline, Vec<Finding>) {
    let mut findings = Vec::new();
    let mut tl = Timeline {
        cycle: 0.0,
        phases: Vec::new(),
        visible: IndexMap::new(),
        segments: Vec::new(),
        shrinks: Vec::new(),
    };

    // State and counter tracks, keyed by spec name.
    let mut tracks: IndexMap<String, Track> = IndexMap::new();
    for (id, s) in &spec.states {
        let initial = match spec.phases.first().and_then(|p| p.set.get(id)) {
            Some(SetValue::Str(v)) => v.clone(),
            _ => s.values().first().cloned().unwrap_or_default(),
        };
        tracks.insert(
            id.clone(),
            Track {
                events: vec![(0.0, initial)],
            },
        );
    }
    for id in spec.counters.keys() {
        let first = spec.phases.first();
        let initial = match (
            first.and_then(|p| p.set.get(id)),
            first
                .and_then(|p| p.ramp.get(id))
                .and_then(|r| parse_ramp(r)),
        ) {
            (_, Some((a, _))) => a,
            (Some(SetValue::Int(n)), None) => *n,
            _ => 0,
        };
        tracks.insert(
            id.clone(),
            Track {
                events: vec![(0.0, initial.to_string())],
            },
        );
    }

    let mut t = 0.0;
    for (pi, p) in spec.phases.iter().enumerate() {
        let n = pi + 1;
        let label = format!("phase {n} ({})", p.name);

        // Resolve each flow's edges, travel time, and which legs are the return.
        struct Plan<'a> {
            edges: Vec<&'a crate::layout::Edge>,
            ret: Vec<bool>,
            travel: f64,
            count: Option<u32>,
            spacing: f64,
            tone: Tone,
        }
        let ramps: Vec<(String, &str, i64, i64)> = p
            .ramp
            .iter()
            .filter_map(|(c, r)| Some((c.clone(), spec.counter_node(c)?, parse_ramp(r)?)))
            .map(|(c, node, (a, b))| (c, node, a, b))
            .collect();
        let mut plans = Vec::new();
        for f in &p.flows {
            let edges: Vec<_> = f.path.iter().filter_map(|e| scene.edge(e)).collect();
            if edges.len() != f.path.len() || edges.is_empty() {
                continue; // reported by validation or layout
            }
            let mut visited = vec![edges[0].from_node.as_str()];
            let mut returning = false;
            let ret: Vec<bool> = edges
                .iter()
                .map(|e| {
                    returning |= visited.contains(&e.to_node.as_str());
                    visited.push(&e.to_node);
                    returning
                })
                .collect();
            let travel = edges.iter().map(|e| e.length() / SPEED).sum();
            let rate = f.rate.as_ref().and_then(|r| r.per_second());
            // A ramp implies one token per step through the counter's node on the way back.
            let ramp_count = ramps.iter().find_map(|(_, node, a, b)| {
                let passes = edges
                    .iter()
                    .zip(&ret)
                    .any(|(e, r)| *r && e.to_node == *node);
                (passes && f.count.is_none()).then_some((a - b).unsigned_abs() as u32)
            });
            plans.push(Plan {
                edges,
                ret,
                travel,
                count: f.count.or(ramp_count),
                spacing: 1.0 / rate.unwrap_or(DEFAULT_RATE),
                tone: f.outcome.tone(),
            });
        }

        let words = p.name.split_whitespace().count()
            + p.caption
                .as_deref()
                .map_or(0, |c| c.split_whitespace().count());
        let read = 1.5 + 0.3 * words as f64;
        // Concurrent flows interleave instead of moving in lockstep.
        let n_flows = plans.len().max(1) as f64;
        let lead = |fi: usize, pl: &Plan| LEAD + fi as f64 * pl.spacing / n_flows;
        let needed = plans
            .iter()
            .enumerate()
            .map(|(fi, pl)| match pl.count {
                Some(c) => {
                    lead(fi, pl) + (c.saturating_sub(1)) as f64 * pl.spacing + pl.travel + TAIL
                }
                None => (LEAD + RATE_WINDOW + pl.travel + TAIL).max(RATE_PHASE),
            })
            .fold(0.0, f64::max);
        let duration = match p.duration {
            Some(d) => {
                if d + 1e-9 < needed {
                    findings.push(Finding::warning(
                        "timeline",
                        label.clone(),
                        format!("duration {d}s is shorter than its flows need ({needed:.1}s); tokens are cut off"),
                    ));
                }
                d
            }
            None => needed.max(read).max(MIN_PHASE),
        };
        let (start, end) = (t, t + duration);
        let mut frame = (start + end) / 2.0;

        // Tokens.
        let mut arrivals: Vec<(String, f64)> = Vec::new(); // (node, time) on return legs
        for (fi, pl) in plans.iter().enumerate() {
            let lead = lead(fi, pl);
            let spawns: Vec<f64> = match pl.count {
                Some(c) => (0..c)
                    .map(|k| start + lead + k as f64 * pl.spacing)
                    .collect(),
                None => (0..)
                    .map(|k| start + lead + k as f64 * pl.spacing)
                    .take_while(|s| s + pl.travel <= end - TAIL + 1e-9)
                    .collect(),
            };
            if spawns.is_empty() {
                findings.push(Finding::warning(
                    "timeline",
                    format!("{label} flow {}", fi + 1),
                    "no token fits in the phase".into(),
                ));
            }
            for (k, s0) in spawns.into_iter().enumerate() {
                let mut ts = s0;
                for (si, (e, ret)) in pl.edges.iter().zip(&pl.ret).enumerate() {
                    let t1 = ts + e.length() / SPEED;
                    let id = format!("tok-{n}-{}-{}-{si}", fi + 1, k + 1);
                    tl.show(id.clone(), ts, t1);
                    tl.segments.push(Segment {
                        id,
                        edge: e.id.clone(),
                        t0: ts,
                        t1,
                        tone: ret.then_some(pl.tone),
                    });
                    if *ret {
                        arrivals.push((e.to_node.clone(), t1));
                    }
                    ts = t1;
                }
            }
        }

        // State and counter changes.
        for (k, v) in &p.set {
            if let Some(tr) = tracks.get_mut(k) {
                let v = match v {
                    SetValue::Int(i) => i.to_string(),
                    SetValue::Str(s) => s.clone(),
                };
                tr.at(start, v);
            }
        }
        for (c, node, a, b) in &ramps {
            let steps = (a - b).unsigned_abs() as usize;
            let mut at: Vec<f64> = arrivals
                .iter()
                .filter(|(n, _)| n == node)
                .map(|(_, t)| *t)
                .collect();
            at.sort_by(f64::total_cmp);
            if at.len() < steps {
                findings.push(Finding::error(
                    "timeline",
                    label.clone(),
                    format!(
                        "ramp {c}: {a}..{b} needs {steps} tokens arriving back at node {node}, but only {} do; add a flow that returns through {node}",
                        at.len()
                    ),
                ));
            }
            let tr = tracks.get_mut(c).expect("validated counter");
            tr.at(start, a.to_string());
            let dir = (b - a).signum();
            for (i, ti) in at.iter().take(steps).enumerate() {
                tr.at(*ti, (a + dir * (i as i64 + 1)).to_string());
            }
            if let Some(last) = at.get(steps.min(at.len()).wrapping_sub(1)) {
                frame = (last + 0.3).min(end - 0.05);
            }
        }

        // Timer, caption, phase indicator.
        if let Some(node) = p
            .timer
            .as_ref()
            .and_then(|tm| spec.timer_node(tm))
            .and_then(|n| scene.node(n))
            && let Some(tm) = &node.timer
        {
            tl.show(tm.id.clone(), start, end);
            tl.shrinks.push(Shrink {
                id: tm.bar_id.clone(),
                t0: start,
                t1: end,
            });
        }
        tl.phases.push(Span {
            name: p.name.clone(),
            start,
            end,
            frame,
        });
        tl.show(format!("cap-{n}"), start, end);
        tl.show(format!("pill-{n}-on"), start, end);
        t = end;
    }
    tl.cycle = t;

    // A phase's muted indicator number is hidden while its highlighted one shows.
    for (i, span) in tl.phases.clone().iter().enumerate() {
        let id = format!("pill-{}-num", i + 1);
        tl.visible.entry(id.clone()).or_default();
        tl.show(id.clone(), 0.0, span.start);
        tl.show(id, span.end, t);
    }

    // Timer groups outside their phases must be hidden, even if never shown.
    for node in &scene.nodes {
        if let Some(tm) = &node.timer {
            tl.visible.entry(tm.id.clone()).or_default();
        }
    }

    // Tracks → visibility of pre-measured alternatives.
    for (name, tr) in &tracks {
        let runs = tr.runs(tl.cycle);
        if let (Some(first), Some(last)) = (runs.first(), runs.last())
            && first.0 != last.0
        {
            findings.push(Finding::warning(
                    "timeline",
                    format!("{} {name}", if spec.states.contains_key(name) { "state" } else { "counter" }),
                    format!(
                        "ends the cycle as `{}` but the loop restarts at `{}`, so it jumps on repeat; reset it in the last phase",
                        last.0, first.0
                    ),
                ));
        }
        let is_state = spec.states.contains_key(name);
        for node in &scene.nodes {
            let ns = &spec.nodes[&node.spec_id];
            let shows = if is_state {
                ns.state.as_deref() == Some(name)
            } else {
                ns.counter.as_deref() == Some(name)
            };
            if !shows {
                continue;
            }
            // Every alternative starts hidden, then gets its runs.
            let ids: Vec<String> = if is_state {
                node.badges.iter().map(|b| b.id.clone()).collect()
            } else {
                node.counters.iter().map(|c| c.id.clone()).collect()
            };
            for id in &ids {
                tl.visible.entry(id.clone()).or_default();
            }
            for (v, a, b) in &runs {
                let id = if is_state {
                    node.badges
                        .iter()
                        .find(|x| x.value == *v)
                        .map(|x| x.id.clone())
                } else {
                    node.counters
                        .iter()
                        .find(|x| x.value.to_string() == *v)
                        .map(|x| x.id.clone())
                };
                tl.show(id.expect("layout measures every reachable value"), *a, *b);
            }
        }
    }
    (tl, findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{font::Fonts, layout, spec};

    fn breaker() -> (Scene, Timeline, Vec<Finding>) {
        let s = spec::parse(include_str!("../examples/circuit-breaker.yaml")).unwrap();
        let fonts = Fonts::new();
        let (scene, _) = layout::layout(&s, &fonts);
        let (tl, f) = schedule(&s, &scene);
        (scene, tl, f)
    }

    #[test]
    fn example_schedules_cleanly() {
        let (_, tl, f) = breaker();
        assert!(f.is_empty(), "{f:?}");
        assert_eq!(tl.phases.len(), 5);
        assert!(tl.cycle > 15.0 && tl.cycle < 40.0, "cycle {}", tl.cycle);
    }

    #[test]
    fn exactly_one_state_is_visible_at_any_time() {
        let (scene, tl, _) = breaker();
        let b = scene.node("breaker").unwrap();
        let mut t = 0.0;
        while t < tl.cycle {
            let n = b.badges.iter().filter(|x| tl.is_visible(&x.id, t)).count();
            assert_eq!(n, 1, "{n} states visible at t={t}");
            let n = b
                .counters
                .iter()
                .filter(|x| tl.is_visible(&x.id, t))
                .count();
            assert_eq!(n, 1, "{n} counter values visible at t={t}");
            t += 0.05;
        }
    }

    #[test]
    fn failure_counter_steps_when_failed_replies_reach_the_breaker() {
        let (scene, tl, _) = breaker();
        let b = scene.node("breaker").unwrap();
        let degrade = &tl.phases[1];
        let replies: Vec<f64> = tl
            .segments
            .iter()
            .filter(|s| {
                s.edge == "e-reply"
                    && s.tone == Some(Tone::Fail)
                    && s.t0 >= degrade.start
                    && s.t1 <= degrade.end
            })
            .map(|s| s.t1)
            .collect();
        assert_eq!(replies.len(), 5);
        let shown = |t: f64| {
            b.counters
                .iter()
                .find(|c| tl.is_visible(&c.id, t))
                .unwrap()
                .value
        };
        for (i, t) in replies.iter().enumerate() {
            assert_eq!(shown(t - 0.01), i as i64);
            assert_eq!(shown(t + 0.01), i as i64 + 1);
        }
    }

    #[test]
    fn ramp_phase_is_captured_after_its_last_step() {
        let (scene, tl, _) = breaker();
        let b = scene.node("breaker").unwrap();
        let shown = b
            .counters
            .iter()
            .find(|c| tl.is_visible(&c.id, tl.phases[1].frame))
            .unwrap();
        assert_eq!(shown.value, 5);
    }

    #[test]
    fn concurrent_flows_interleave() {
        let s = spec::parse(include_str!("../examples/load-balancer.yaml")).unwrap();
        let fonts = Fonts::new();
        let (scene, _) = layout::layout(&s, &fonts);
        let (tl, f) = schedule(&s, &scene);
        assert!(f.is_empty(), "{f:?}");
        let first = |edge: &str| {
            tl.segments
                .iter()
                .filter(|x| x.edge == edge)
                .map(|x| x.t0)
                .fold(f64::MAX, f64::min)
        };
        let (a, b) = (first("e-to_a"), first("e-to_b"));
        assert!((a - b).abs() > 0.2, "flows start together: {a} vs {b}");
    }

    #[test]
    fn open_phase_never_reaches_the_service() {
        let (_, tl, _) = breaker();
        let open = &tl.phases[2];
        assert!(
            !tl.segments
                .iter()
                .any(|s| s.edge == "e-forward" && s.t0 >= open.start && s.t0 < open.end)
        );
    }
}
