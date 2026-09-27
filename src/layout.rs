//! Layout: spec → `Scene`, the single source of truth for geometry. Every
//! emitted element is derived from a Scene primitive and never positioned
//! independently.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use indexmap::IndexMap;

use crate::finding::Finding;
use crate::font::{Fonts, Weight};
use crate::spec::{Outcome, SetValue, Spec, Tone, parse_ramp};
use crate::theme::Ink;

pub const PAD: f64 = 32.0;
pub const MAX_WIDTH: f64 = 1200.0;
const GAP_X: f64 = 176.0;
const GAP_Y: f64 = 40.0;
const NODE_PAD_X: f64 = 20.0;
const NODE_PAD_Y: f64 = 14.0;
const NODE_MIN_W: f64 = 112.0;
const LANE: f64 = 10.0;
const EDGE_GAP: f64 = 4.0;
const ARROW_L: f64 = 8.0;
const ARROW_W: f64 = 4.5;
const BADGE_H: f64 = 22.0;
const BADGE_PAD_X: f64 = 10.0;
const PILL_R: f64 = 11.0;
const PILL_STEP: f64 = 30.0;
pub const TOKEN_R: f64 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn right(&self) -> f64 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    Start,
    Middle,
}

#[derive(Debug, Clone)]
pub struct Text {
    pub id: String,
    pub x: f64,
    /// Baseline.
    pub y: f64,
    pub anchor: Anchor,
    pub size: f64,
    pub weight: Weight,
    pub ink: Ink,
    pub content: String,
    pub width: f64,
    /// Id of the element this text must stay inside; the canvas when `None`.
    pub container: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Badge {
    pub id: String,
    pub value: String,
    pub rect: Rect,
    pub tone: Tone,
    pub text: Text,
}

#[derive(Debug, Clone)]
pub struct CounterText {
    pub id: String,
    pub value: i64,
    pub text: Text,
}

#[derive(Debug, Clone)]
pub struct Timer {
    /// The phase (1-based) this timer runs in. A node gets one timer per phase
    /// that uses one, all sharing the same row.
    pub phase: usize,
    pub id: String,
    pub bar_id: String,
    pub track: Rect,
    pub label: Text,
}

#[derive(Debug, Clone)]
pub struct NodeBox {
    pub spec_id: String,
    pub id: String,
    pub rect: Rect,
    pub label: Text,
    pub sublabel: Option<Text>,
    pub badges: Vec<Badge>,
    pub counters: Vec<CounterText>,
    pub timers: Vec<Timer>,
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub spec_id: String,
    pub id: String,
    pub from_node: String,
    pub to_node: String,
    /// The drawn line, and the path tokens travel along.
    pub start: (f64, f64),
    pub end: (f64, f64),
    pub head: [(f64, f64); 3],
    pub label: Option<Text>,
}

impl Edge {
    pub fn length(&self) -> f64 {
        (self.end.0 - self.start.0).hypot(self.end.1 - self.start.1)
    }

    pub fn point_at(&self, frac: f64) -> (f64, f64) {
        let f = frac.clamp(0.0, 1.0);
        (
            self.start.0 + (self.end.0 - self.start.0) * f,
            self.start.1 + (self.end.1 - self.start.1) * f,
        )
    }
}

#[derive(Debug, Clone)]
pub struct Pill {
    pub off_id: String,
    pub on_id: String,
    pub center: (f64, f64),
    pub r: f64,
    pub num: Text,
    pub num_on: Text,
}

#[derive(Debug, Clone)]
pub struct Caption {
    pub id: String,
    pub name: Text,
    pub body: Option<Text>,
}

#[derive(Debug, Clone)]
pub struct LegendItem {
    pub id: String,
    /// `None` is the outbound request colour.
    pub tone: Option<Tone>,
    pub center: (f64, f64),
    pub label: Text,
}

#[derive(Debug, Clone)]
pub struct Scene {
    pub width: f64,
    pub height: f64,
    pub title: Text,
    pub takeaway: Option<Text>,
    pub nodes: Vec<NodeBox>,
    pub edges: Vec<Edge>,
    pub pills: Vec<Pill>,
    pub captions: Vec<Caption>,
    pub legend: Vec<LegendItem>,
}

impl Scene {
    pub fn edge(&self, spec_id: &str) -> Option<&Edge> {
        self.edges.iter().find(|e| e.spec_id == spec_id)
    }

    pub fn node(&self, spec_id: &str) -> Option<&NodeBox> {
        self.nodes.iter().find(|n| n.spec_id == spec_id)
    }

    pub fn texts(&self) -> Vec<&Text> {
        let mut v = vec![&self.title];
        v.extend(&self.takeaway);
        for n in &self.nodes {
            v.push(&n.label);
            v.extend(&n.sublabel);
            v.extend(n.badges.iter().map(|b| &b.text));
            v.extend(n.counters.iter().map(|c| &c.text));
            v.extend(n.timers.iter().map(|t| &t.label));
        }
        v.extend(self.edges.iter().filter_map(|e| e.label.as_ref()));
        for p in &self.pills {
            v.push(&p.num);
            v.push(&p.num_on);
        }
        for c in &self.captions {
            v.push(&c.name);
            v.extend(&c.body);
        }
        v.extend(self.legend.iter().map(|l| &l.label));
        v
    }

    /// Characters used per weight, for font subsetting.
    pub fn used_chars(&self) -> BTreeMap<Weight, HashSet<char>> {
        let mut m: BTreeMap<Weight, HashSet<char>> = BTreeMap::new();
        for t in self.texts() {
            m.entry(t.weight).or_default().extend(t.content.chars());
        }
        m
    }
}

/// Makes a string safe to use inside an element id.
pub fn id_part(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn snap(v: f64) -> f64 {
    (v / 8.0).ceil() * 8.0
}

struct TextMaker<'a> {
    fonts: &'a Fonts,
}

impl TextMaker<'_> {
    #[allow(clippy::too_many_arguments)]
    fn make(
        &self,
        id: String,
        content: &str,
        size: f64,
        weight: Weight,
        ink: Ink,
        container: Option<String>,
    ) -> Text {
        Text {
            id,
            x: 0.0,
            y: 0.0,
            anchor: Anchor::Start,
            size,
            weight,
            ink,
            content: content.to_string(),
            width: self.fonts.width(content, size, weight),
            container,
        }
    }

    /// Baseline that vertically centres capitals within a line box.
    fn baseline(&self, top: f64, line_h: f64, size: f64, w: Weight) -> f64 {
        top + (line_h + self.fonts.cap_height(size, w)) / 2.0
    }
}

pub fn counter_text(label: &str, value: i64, max: Option<i64>) -> String {
    match max {
        Some(m) => format!("{label} {value}/{m}"),
        None => format!("{label} {value}"),
    }
}

/// Every value a counter can show: its start, anything `set`, and every ramp step.
pub fn counter_values(spec: &Spec, counter: &str) -> BTreeSet<i64> {
    let mut v = BTreeSet::from([0]);
    for p in &spec.phases {
        if let Some(SetValue::Int(n)) = p.set.get(counter) {
            v.insert(*n);
        }
        if let Some((a, b)) = p.ramp.get(counter).and_then(|r| parse_ramp(r)) {
            v.extend(a.min(b)..=a.max(b));
        }
    }
    v
}

/// Layered ranks: back edges (found by DFS in declaration order) are ignored,
/// then each node sits one rank after its furthest predecessor.
pub fn ranks(spec: &Spec) -> IndexMap<String, usize> {
    let ids: Vec<&str> = spec.nodes.keys().map(String::as_str).collect();
    let idx = |id: &str| ids.iter().position(|n| *n == id);
    let n = ids.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for e in spec.edges.values() {
        if let (Some(a), Some(b)) = (idx(&e.from), idx(&e.to)) {
            adj[a].push(b);
        }
    }
    // 0 = unvisited, 1 = on stack, 2 = done
    let mut mark = vec![0u8; n];
    let mut forward: Vec<(usize, usize)> = Vec::new();
    fn dfs(u: usize, adj: &[Vec<usize>], mark: &mut [u8], forward: &mut Vec<(usize, usize)>) {
        mark[u] = 1;
        for &v in &adj[u] {
            match mark[v] {
                0 => {
                    forward.push((u, v));
                    dfs(v, adj, mark, forward);
                }
                2 => forward.push((u, v)),
                _ => {} // back edge
            }
        }
        mark[u] = 2;
    }
    for u in 0..n {
        if mark[u] == 0 {
            dfs(u, &adj, &mut mark, &mut forward);
        }
    }
    let mut rank = vec![0usize; n];
    for _ in 0..n {
        for &(u, v) in &forward {
            rank[v] = rank[v].max(rank[u] + 1);
        }
    }
    ids.iter()
        .zip(rank)
        .map(|(id, r)| (id.to_string(), r))
        .collect()
}

/// Parameter range `[t0, t1]` of the segment p + t·d, t ∈ [0,1], that lies inside `r`.
fn clip(p: (f64, f64), d: (f64, f64), r: &Rect) -> Option<(f64, f64)> {
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (pc, dc, lo, hi) in [(p.0, d.0, r.x, r.right()), (p.1, d.1, r.y, r.bottom())] {
        if dc.abs() < 1e-9 {
            if pc < lo || pc > hi {
                return None;
            }
        } else {
            let (a, b) = ((lo - pc) / dc, (hi - pc) / dc);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
    }
    (t0 <= t1).then_some((t0, t1))
}

pub fn layout(spec: &Spec, fonts: &Fonts) -> (Scene, Vec<Finding>) {
    let tm = TextMaker { fonts };
    let mut findings = Vec::new();
    // (phase, node, label) for every phase that runs a timer.
    let phase_timers: Vec<(usize, &str, &str)> = spec
        .phases
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let t = p.timer.as_ref()?;
            Some((i + 1, spec.timer_node(t)?, t.label.as_str()))
        })
        .collect();

    // --- nodes, sized from their measured content, positioned at the origin ---
    let mut nodes: Vec<NodeBox> = Vec::new();
    for (sid, n) in &spec.nodes {
        let nid = format!("n-{}", id_part(sid));
        let label = tm.make(
            format!("{nid}--label"),
            &n.label,
            15.0,
            Weight::SemiBold,
            Ink::Text,
            Some(nid.clone()),
        );
        let sublabel = n.sublabel.as_deref().map(|s| {
            tm.make(
                format!("{nid}--sublabel"),
                s,
                12.0,
                Weight::Regular,
                Ink::Muted,
                Some(nid.clone()),
            )
        });
        let mut badges = Vec::new();
        if let Some(st) = n.state.as_ref().and_then(|s| spec.states.get(s)) {
            for v in st.values() {
                let bid = format!("{nid}--state-{}", id_part(v));
                let tone = st.tone(v);
                let text = tm.make(
                    format!("{bid}-text"),
                    v,
                    11.0,
                    Weight::SemiBold,
                    Ink::ToneText(tone),
                    Some(format!("{bid}-bg")),
                );
                badges.push(Badge {
                    id: bid,
                    value: v.clone(),
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 0.0,
                        h: BADGE_H,
                    },
                    tone,
                    text,
                });
            }
        }
        let mut counters = Vec::new();
        if let Some((cid, c)) = n
            .counter
            .as_ref()
            .and_then(|c| spec.counters.get_key_value(c))
        {
            for v in counter_values(spec, cid) {
                let id = format!("{nid}--count-{}", id_part(&v.to_string()));
                let text = tm.make(
                    id.clone(),
                    &counter_text(&c.label, v, c.max),
                    12.0,
                    Weight::Regular,
                    Ink::Muted,
                    Some(nid.clone()),
                );
                counters.push(CounterText { id, value: v, text });
            }
        }
        let timers: Vec<Timer> = phase_timers
            .iter()
            .filter(|(_, node, _)| *node == sid.as_str())
            .map(|&(phase, _, text)| {
                let id = format!("{nid}--timer-p{phase}");
                let label = tm.make(
                    format!("{id}-label"),
                    text,
                    11.0,
                    Weight::Regular,
                    Ink::Muted,
                    Some(nid.clone()),
                );
                Timer {
                    phase,
                    bar_id: format!("{id}-bar"),
                    id,
                    track: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 0.0,
                        h: 4.0,
                    },
                    label,
                }
            })
            .collect();

        let badge_w = badges
            .iter()
            .map(|b| b.text.width + 2.0 * BADGE_PAD_X)
            .fold(0.0, f64::max);
        let content_w = [
            label.width,
            sublabel.as_ref().map_or(0.0, |s| s.width),
            badge_w,
        ]
        .into_iter()
        .chain(counters.iter().map(|c| c.text.width))
        .chain(timers.iter().map(|t| t.label.width))
        .fold(0.0, f64::max);
        let w = snap((content_w + 2.0 * NODE_PAD_X).max(NODE_MIN_W));

        // vertical stack, relative to the node's top-left
        let mut y = NODE_PAD_Y;
        let mut node = NodeBox {
            spec_id: sid.clone(),
            id: nid,
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w,
                h: 0.0,
            },
            label,
            sublabel,
            badges,
            counters,
            timers,
        };
        node.label.anchor = Anchor::Middle;
        node.label.x = w / 2.0;
        node.label.y = tm.baseline(y, 20.0, 15.0, Weight::SemiBold);
        y += 20.0;
        if let Some(s) = &mut node.sublabel {
            s.anchor = Anchor::Middle;
            s.x = w / 2.0;
            s.y = tm.baseline(y, 16.0, 12.0, Weight::Regular);
            y += 16.0;
        }
        if !node.badges.is_empty() {
            y += 8.0;
            let bw = snap(badge_w);
            for b in &mut node.badges {
                b.rect = Rect {
                    x: (w - bw) / 2.0,
                    y,
                    w: bw,
                    h: BADGE_H,
                };
                b.text.anchor = Anchor::Middle;
                b.text.x = w / 2.0;
                b.text.y = tm.baseline(y, BADGE_H, 11.0, Weight::SemiBold);
            }
            y += BADGE_H;
        }
        if !node.counters.is_empty() {
            y += 6.0;
            for c in &mut node.counters {
                c.text.anchor = Anchor::Middle;
                c.text.x = w / 2.0;
                c.text.y = tm.baseline(y, 16.0, 12.0, Weight::Regular);
            }
            y += 16.0;
        }
        if !node.timers.is_empty() {
            y += 8.0;
            let label_y = tm.baseline(y, 14.0, 11.0, Weight::Regular);
            y += 17.0;
            for t in &mut node.timers {
                t.label.anchor = Anchor::Middle;
                t.label.x = w / 2.0;
                t.label.y = label_y;
                t.track = Rect {
                    x: NODE_PAD_X,
                    y,
                    w: w - 2.0 * NODE_PAD_X,
                    h: 4.0,
                };
            }
            y += 4.0;
        }
        // Even whole-pixel heights keep every node in a row on one integer centre line.
        node.rect.h = ((y + NODE_PAD_Y) / 2.0).ceil() * 2.0;
        nodes.push(node);
    }

    // --- ranks and columns ---
    let ranks = ranks(spec);
    let n_ranks = ranks.values().copied().max().map_or(0, |m| m + 1);
    let mut cols: Vec<Vec<usize>> = vec![Vec::new(); n_ranks];
    for (i, n) in nodes.iter().enumerate() {
        cols[ranks[&n.spec_id]].push(i);
    }
    let col_w: Vec<f64> = cols
        .iter()
        .map(|c| c.iter().map(|&i| nodes[i].rect.w).fold(0.0, f64::max))
        .collect();
    let col_h: Vec<f64> = cols
        .iter()
        .map(|c| {
            c.iter().map(|&i| nodes[i].rect.h).sum::<f64>()
                + GAP_Y * (c.len().saturating_sub(1)) as f64
        })
        .collect();
    let graph_w = col_w.iter().sum::<f64>() + GAP_X * (n_ranks.saturating_sub(1)) as f64;
    let graph_h = col_h.iter().copied().fold(0.0, f64::max);

    // --- header ---
    let mut title = tm.make(
        "title".into(),
        &spec.title,
        20.0,
        Weight::SemiBold,
        Ink::Text,
        None,
    );
    title.x = PAD;
    title.y = PAD + fonts.cap_height(20.0, Weight::SemiBold);
    let mut header_bottom = title.y + fonts.descent(20.0, Weight::SemiBold);
    let takeaway = spec.story.takeaway.as_deref().map(|t| {
        let mut t = tm.make(
            "takeaway".into(),
            t,
            13.0,
            Weight::Regular,
            Ink::Muted,
            None,
        );
        t.x = PAD;
        t.y = title.y + 24.0;
        header_bottom = t.y + fonts.descent(13.0, Weight::Regular);
        t
    });
    let graph_y0 = snap(header_bottom + 28.0);

    // --- captions and legend (measured now, placed after width is known) ---
    let mut pills = Vec::new();
    let mut captions = Vec::new();
    for (i, p) in spec.phases.iter().enumerate() {
        let n = i + 1;
        let num = tm.make(
            format!("pill-{n}-num"),
            &n.to_string(),
            11.0,
            Weight::SemiBold,
            Ink::Muted,
            Some(format!("pill-{n}")),
        );
        let mut num_on = num.clone();
        num_on.id = format!("pill-{n}-num-on");
        num_on.ink = Ink::OnAccent;
        num_on.container = Some(format!("pill-{n}-on"));
        pills.push(Pill {
            off_id: format!("pill-{n}"),
            on_id: format!("pill-{n}-on"),
            center: (0.0, 0.0),
            r: PILL_R,
            num,
            num_on,
        });
        let name = tm.make(
            format!("cap-{n}-name"),
            &p.name,
            15.0,
            Weight::SemiBold,
            Ink::Text,
            None,
        );
        let body = p.caption.as_deref().map(|c| {
            tm.make(
                format!("cap-{n}-body"),
                c,
                14.0,
                Weight::Regular,
                Ink::Muted,
                None,
            )
        });
        captions.push(Caption {
            id: format!("cap-{n}"),
            name,
            body,
        });
    }
    let pills_w = if pills.is_empty() {
        0.0
    } else {
        2.0 * PILL_R + PILL_STEP * (pills.len() - 1) as f64
    };
    let caption_w = captions
        .iter()
        .map(|c| c.name.width + c.body.as_ref().map_or(0.0, |b| 10.0 + b.width))
        .fold(0.0, f64::max);

    let mut outcomes: Vec<Outcome> = Vec::new();
    for f in spec.phases.iter().flat_map(|p| &p.flows) {
        if !outcomes.contains(&f.outcome) {
            outcomes.push(f.outcome);
        }
    }
    outcomes.sort_by_key(|o| *o as u8);
    let mut legend: Vec<LegendItem> = std::iter::once((None, "request"))
        .chain(outcomes.iter().map(|o| (Some(o.tone()), o.label())))
        .enumerate()
        .map(|(k, (tone, label))| LegendItem {
            id: format!("legend-{k}"),
            tone,
            center: (0.0, 0.0),
            label: tm.make(
                format!("legend-{k}-text"),
                label,
                12.0,
                Weight::Regular,
                Ink::Muted,
                None,
            ),
        })
        .collect();
    let legend_w: f64 = legend
        .iter()
        .map(|l| 10.0 + 2.0 * TOKEN_R + l.label.width + 20.0)
        .sum();

    // --- canvas width ---
    let content_w = [
        title.width,
        takeaway.as_ref().map_or(0.0, |t| t.width),
        graph_w,
        pills_w + 16.0 + caption_w,
        legend_w,
    ]
    .into_iter()
    .fold(0.0, f64::max);
    let width = snap(content_w + 2.0 * PAD);
    if width > MAX_WIDTH {
        // Name what actually sets the width, so the fix goes to the right place.
        let widest_caption = captions
            .iter()
            .max_by(|a, b| {
                let w = |c: &Caption| c.name.width + c.body.as_ref().map_or(0.0, |b| b.width);
                w(a).total_cmp(&w(b))
            })
            .map(|c| c.id.trim_start_matches("cap-").to_string())
            .unwrap_or_default();
        let parts = [
            (title.width, "the title; shorten it".to_string()),
            (
                takeaway.as_ref().map_or(0.0, |t| t.width),
                "the story takeaway; shorten it".to_string(),
            ),
            (
                graph_w,
                format!(
                    "the diagram: {n_ranks} columns of nodes; use fewer components in a row or shorter node labels"
                ),
            ),
            (
                pills_w + 16.0 + caption_w,
                format!("the caption of phase {widest_caption}; shorten it"),
            ),
            (legend_w, "the legend".to_string()),
        ];
        let (w, cause) = parts.iter().max_by(|a, b| a.0.total_cmp(&b.0)).unwrap();
        findings.push(Finding::error(
            "layout",
            "canvas".into(),
            format!("canvas is {width:.0}px wide (max {MAX_WIDTH:.0}); the widest part is {cause} ({w:.0}px)"),
        ));
    }

    // --- place nodes ---
    let graph_x0 = ((width - graph_w) / 2.0).round();
    let mut x = graph_x0;
    for (r, col) in cols.iter().enumerate() {
        let mut y = graph_y0 + (graph_h - col_h[r]) / 2.0;
        for &i in col {
            let n = &mut nodes[i];
            let dx = x + (col_w[r] - n.rect.w) / 2.0;
            let dy = y.round();
            n.rect.x = dx;
            n.rect.y = dy;
            let shift = |t: &mut Text| {
                t.x += dx;
                t.y += dy;
            };
            shift(&mut n.label);
            if let Some(s) = &mut n.sublabel {
                shift(s);
            }
            for b in &mut n.badges {
                b.rect.x += dx;
                b.rect.y += dy;
                shift(&mut b.text);
            }
            for c in &mut n.counters {
                shift(&mut c.text);
            }
            for t in &mut n.timers {
                t.track.x += dx;
                t.track.y += dy;
                shift(&mut t.label);
            }
            y += n.rect.h + GAP_Y;
        }
        x += col_w[r] + GAP_X;
    }

    // --- edges: straight, clipped to node borders, paired edges on parallel lanes ---
    let node_idx = |sid: &str| nodes.iter().position(|n| n.spec_id == sid);
    let order: Vec<&String> = spec.nodes.keys().collect();
    let mut edges = Vec::new();
    for (sid, e) in &spec.edges {
        let (Some(a), Some(b)) = (node_idx(&e.from), node_idx(&e.to)) else {
            continue;
        };
        let (ra, rb) = (ranks[&e.from], ranks[&e.to]);
        let a_first = (ra, order.iter().position(|n| **n == e.from))
            < (rb, order.iter().position(|n| **n == e.to));
        let (lo, hi) = if a_first { (a, b) } else { (b, a) };
        let (cl, ch) = (nodes[lo].rect.center(), nodes[hi].rect.center());
        let len = (ch.0 - cl.0).hypot(ch.1 - cl.1);
        let dir = ((ch.0 - cl.0) / len, (ch.1 - cl.1) / len);
        let normal = (dir.1, -dir.0);
        let paired = spec
            .edges
            .values()
            .any(|o| o.from == e.to && o.to == e.from);
        let off = if !paired {
            0.0
        } else if a_first {
            LANE
        } else {
            -LANE
        };
        let (ca, cb) = (nodes[a].rect.center(), nodes[b].rect.center());
        let p = (ca.0 + normal.0 * off, ca.1 + normal.1 * off);
        let d = (cb.0 - ca.0, cb.1 - ca.1);
        let (Some((_, exit)), Some((enter, _))) = (
            clip(p, d, &nodes[a].rect),
            clip((p.0, p.1), d, &nodes[b].rect),
        ) else {
            findings.push(Finding::error(
                "layout",
                format!("edge {sid}"),
                "could not route edge".into(),
            ));
            continue;
        };
        if exit >= enter {
            findings.push(Finding::error(
                "layout",
                format!("edge {sid}"),
                "endpoints overlap; nodes are too close".into(),
            ));
            continue;
        }
        let dl = d.0.hypot(d.1);
        let u = (d.0 / dl, d.1 / dl);
        let at = |t: f64| (p.0 + d.0 * t, p.1 + d.1 * t);
        let (s0, t0) = (at(exit), at(enter));
        let start = (s0.0 + u.0 * EDGE_GAP, s0.1 + u.1 * EDGE_GAP);
        let tip = (t0.0 - u.0 * EDGE_GAP, t0.1 - u.1 * EDGE_GAP);
        let end = (tip.0 - u.0 * ARROW_L, tip.1 - u.1 * ARROW_L);
        let n2 = (u.1, -u.0);
        let head = [
            tip,
            (end.0 + n2.0 * ARROW_W, end.1 + n2.1 * ARROW_W),
            (end.0 - n2.0 * ARROW_W, end.1 - n2.1 * ARROW_W),
        ];
        let eid = format!("e-{}", id_part(sid));
        let label = e.label.as_deref().map(|l| {
            let mut t = tm.make(
                format!("{eid}--label"),
                l,
                11.0,
                Weight::Regular,
                Ink::Muted,
                None,
            );
            let mid = ((start.0 + end.0) / 2.0, (start.1 + end.1) / 2.0);
            // Outward from the pair: the lane's side of the shared centre line.
            let side = if off == 0.0 { 1.0 } else { off.signum() };
            let out = (normal.0 * side, normal.1 * side);
            t.anchor = Anchor::Middle;
            t.x = mid.0 + out.0 * 8.0;
            t.y = if out.1 < 0.0 {
                mid.1 - 7.0
            } else {
                mid.1 + 7.0 + fonts.cap_height(11.0, Weight::Regular)
            };
            t
        });
        edges.push(Edge {
            spec_id: sid.clone(),
            id: eid,
            from_node: e.from.clone(),
            to_node: e.to.clone(),
            start,
            end,
            head,
            label,
        });
    }

    // --- caption row and legend ---
    let row_y = snap(graph_y0 + graph_h + 32.0);
    let cy = row_y + PILL_R;
    for (i, p) in pills.iter_mut().enumerate() {
        p.center = (PAD + PILL_R + PILL_STEP * i as f64, cy);
        for t in [&mut p.num, &mut p.num_on] {
            t.anchor = Anchor::Middle;
            t.x = p.center.0;
            t.y = cy + fonts.cap_height(11.0, Weight::SemiBold) / 2.0;
        }
    }
    let cap_x = PAD + pills_w + 16.0;
    for c in &mut captions {
        c.name.x = cap_x;
        c.name.y = cy + fonts.cap_height(15.0, Weight::SemiBold) / 2.0;
        if let Some(b) = &mut c.body {
            b.x = cap_x + c.name.width + 10.0;
            b.y = c.name.y;
        }
    }
    let legend_cy = row_y + 2.0 * PILL_R + 22.0;
    let mut lx = PAD;
    for l in &mut legend {
        l.center = (lx + TOKEN_R, legend_cy);
        l.label.x = lx + 2.0 * TOKEN_R + 8.0;
        l.label.y = legend_cy + fonts.cap_height(12.0, Weight::Regular) / 2.0;
        lx = l.label.x + l.label.width + 20.0;
    }
    let height = snap(legend_cy + TOKEN_R + PAD - 8.0);

    (
        Scene {
            width,
            height,
            title,
            takeaway,
            nodes,
            edges,
            pills,
            captions,
            legend,
        },
        findings,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec;

    fn breaker() -> Spec {
        spec::parse(include_str!("../examples/circuit-breaker.yaml")).unwrap()
    }

    #[test]
    fn ranks_ignore_back_edges() {
        let r = ranks(&breaker());
        assert_eq!((r["client"], r["breaker"], r["service"]), (0, 1, 2));
    }

    #[test]
    fn nodes_fit_their_widest_content() {
        let fonts = Fonts::new();
        let (scene, findings) = layout(&breaker(), &fonts);
        assert!(findings.is_empty(), "{findings:?}");
        for n in &scene.nodes {
            for t in n
                .badges
                .iter()
                .map(|b| &b.text)
                .chain(n.counters.iter().map(|c| &c.text))
                .chain([&n.label])
            {
                assert!(
                    t.width + 2.0 * NODE_PAD_X <= n.rect.w + 0.01,
                    "{} overflows {}",
                    t.id,
                    n.id
                );
            }
        }
    }

    #[test]
    fn paired_edges_are_parallel_and_do_not_touch_nodes() {
        let fonts = Fonts::new();
        let (scene, _) = layout(&breaker(), &fonts);
        let call = scene.edge("call").unwrap();
        let resp = scene.edge("response").unwrap();
        assert!((call.start.1 - resp.start.1).abs() >= 2.0 * LANE - 0.01);
        for e in &scene.edges {
            for n in &scene.nodes {
                let inside = |p: (f64, f64)| {
                    p.0 > n.rect.x
                        && p.0 < n.rect.right()
                        && p.1 > n.rect.y
                        && p.1 < n.rect.bottom()
                };
                assert!(
                    !inside(e.start) && !inside(e.head[0]),
                    "{} enters {}",
                    e.id,
                    n.id
                );
            }
        }
    }
}
