//! Scene + Timeline → SVG. Animated output and still frames share this one
//! code path, so a frame shows exactly what the animation shows at that time.

use std::fmt::Write;

use crate::font::FAMILY;
use crate::layout::{Anchor, Scene, TOKEN_R, Text};
use crate::theme::Theme;
use crate::timeline::{Segment, Timeline};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Animated,
    /// A still at `t` seconds into the cycle.
    Frame(f64),
}

pub struct Options<'a> {
    pub theme: &'a Theme,
    pub mode: Mode,
    /// `@font-face` rules; animated output only.
    pub font_css: Option<&'a str>,
    /// `false` leaves out all text, to render the background under it for contrast checks.
    pub text: bool,
    pub title: &'a str,
    pub desc: &'a str,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Compact number formatting: at most 2 decimals, no trailing zeros.
fn n(v: f64) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

/// Key time as a fraction of the cycle.
fn kt(t: f64, cycle: f64) -> f64 {
    ((t / cycle).clamp(0.0, 1.0) * 100_000.0).round() / 100_000.0
}

fn join(v: &[f64]) -> String {
    v.iter()
        .map(|x| format!("{x}"))
        .collect::<Vec<_>>()
        .join(";")
}

struct Emitter<'a> {
    scene: &'a Scene,
    tl: &'a Timeline,
    o: &'a Options<'a>,
    out: String,
}

impl Emitter<'_> {
    /// `None` when the element is hidden in this frame (so it is not emitted at all).
    /// Otherwise the extra attributes and the animation child for it.
    fn vis(&self, id: &str) -> Option<(String, String)> {
        match self.o.mode {
            Mode::Frame(t) => self
                .tl
                .is_visible(id, t)
                .then(|| (String::new(), String::new())),
            Mode::Animated => {
                let cycle = self.tl.cycle;
                let attr = if self.tl.is_visible(id, 0.0) {
                    String::new()
                } else {
                    r#" visibility="hidden""#.into()
                };
                let Some(iv) = self.tl.visible.get(id) else {
                    return Some((attr, String::new()));
                };
                let mut bounds: Vec<f64> = iv
                    .iter()
                    .flat_map(|&(a, b)| [a, b])
                    .filter(|x| *x > 0.0 && *x < cycle)
                    .collect();
                bounds.sort_by(f64::total_cmp);
                let mut changes = vec![(0.0, self.tl.is_visible(id, 0.0))];
                for b in bounds {
                    let v = self.tl.is_visible(id, b);
                    let k = kt(b, cycle);
                    if v != changes.last().unwrap().1 && k > changes.last().unwrap().0 {
                        changes.push((k, v));
                    }
                }
                if changes.len() == 1 {
                    return Some((attr, String::new()));
                }
                let times: Vec<f64> = changes.iter().map(|c| c.0).collect();
                let values: Vec<&str> = changes
                    .iter()
                    .map(|c| if c.1 { "visible" } else { "hidden" })
                    .collect();
                let anim = format!(
                    r#"<animate attributeName="visibility" dur="{}s" repeatCount="indefinite" calcMode="discrete" keyTimes="{}" values="{}"/>"#,
                    n(cycle),
                    join(&times),
                    values.join(";")
                );
                Some((attr, anim))
            }
        }
    }

    fn text(&mut self, t: &Text) {
        if !self.o.text {
            return;
        }
        let Some((attr, anim)) = self.vis(&t.id) else {
            return;
        };
        let anchor = match t.anchor {
            Anchor::Start => "",
            Anchor::Middle => r#" text-anchor="middle""#,
        };
        let container = t
            .container
            .as_ref()
            .map(|c| format!(r#" data-container="{c}""#))
            .unwrap_or_default();
        writeln!(
            self.out,
            r#"<text id="{}" x="{}" y="{}" font-family="'{FAMILY}', sans-serif" font-size="{}" font-weight="{}" fill="{}"{anchor}{container}{attr}>{anim}{}</text>"#,
            t.id,
            n(t.x),
            n(t.y),
            n(t.size),
            t.weight.css(),
            self.o.theme.ink(t.ink),
            esc(&t.content)
        )
        .unwrap();
    }

    /// Opens a group for a timed element; returns false if hidden in this frame.
    fn open_group(&mut self, id: &str) -> bool {
        let Some((attr, anim)) = self.vis(id) else {
            return false;
        };
        writeln!(self.out, r#"<g id="{id}"{attr}>{anim}"#).unwrap();
        true
    }

    fn token(&mut self, s: &Segment) {
        let th = self.o.theme;
        let fill = s.tone.map_or(th.accent, |t| th.solid(t));
        let Some(edge) = self.scene.edges.iter().find(|e| e.id == s.edge) else {
            return;
        };
        let Some((attr, anim)) = self.vis(&s.id) else {
            return;
        };
        match self.o.mode {
            Mode::Frame(t) => {
                let (x, y) = edge.point_at((t - s.t0) / (s.t1 - s.t0));
                writeln!(
                    self.out,
                    r#"<circle id="{}" data-role="token" cx="{}" cy="{}" r="{TOKEN_R}" fill="{fill}" stroke="{}" stroke-width="1.5"/>"#,
                    s.id,
                    n(x),
                    n(y),
                    th.background
                )
                .unwrap();
            }
            Mode::Animated => {
                let cycle = self.tl.cycle;
                let (a, b) = (kt(s.t0, cycle), kt(s.t1, cycle));
                // Hold at the start until t0, travel to the end by t1, hold after.
                let mut times = vec![0.0];
                let mut points = vec![0.0];
                if a > 0.0 {
                    times.push(a);
                    points.push(0.0);
                }
                if b > *times.last().unwrap() {
                    times.push(b);
                    points.push(1.0);
                }
                if *times.last().unwrap() < 1.0 {
                    times.push(1.0);
                    points.push(1.0);
                }
                writeln!(
                    self.out,
                    r##"<circle id="{}" data-role="token" r="{TOKEN_R}" fill="{fill}" stroke="{}" stroke-width="1.5"{attr}>{anim}<animateMotion dur="{}s" repeatCount="indefinite" calcMode="linear" keyTimes="{}" keyPoints="{}"><mpath href="#{}"/></animateMotion></circle>"##,
                    s.id,
                    th.background,
                    n(cycle),
                    join(&times),
                    join(&points),
                    edge.id
                )
                .unwrap();
            }
        }
    }

    fn emit(mut self) -> String {
        let (s, th, o) = (self.scene, self.o.theme, self.o);
        writeln!(
            self.out,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" role="img" aria-labelledby="svg-title svg-desc" data-generator="infographic" data-theme="{}" data-cycle="{}"{}>"#,
            th.name,
            n(self.tl.cycle),
            match o.mode {
                Mode::Frame(t) => format!(r#" data-t="{t}""#),
                Mode::Animated => String::new(),
            },
            w = n(s.width),
            h = n(s.height),
        )
        .unwrap();
        writeln!(
            self.out,
            r#"<title id="svg-title">{}</title>"#,
            esc(o.title)
        )
        .unwrap();
        writeln!(self.out, r#"<desc id="svg-desc">{}</desc>"#, esc(o.desc)).unwrap();
        if let (Mode::Animated, Some(css)) = (o.mode, o.font_css) {
            writeln!(self.out, "<style>{css}</style>").unwrap();
        }
        writeln!(
            self.out,
            r#"<rect id="canvas" x="0.5" y="0.5" width="{}" height="{}" rx="12" fill="{}" stroke="{}"/>"#,
            n(s.width - 1.0),
            n(s.height - 1.0),
            th.background,
            th.border
        )
        .unwrap();

        self.text(&s.title);
        if let Some(t) = &s.takeaway {
            self.text(t);
        }

        for e in &s.edges {
            writeln!(
                self.out,
                r#"<path id="{}" data-role="edge" data-from="n-{}" data-to="n-{}" d="M{} {} L{} {}" fill="none" stroke="{}" stroke-width="1.5"/>"#,
                e.id,
                crate::layout::id_part(&e.from_node),
                crate::layout::id_part(&e.to_node),
                n(e.start.0),
                n(e.start.1),
                n(e.end.0),
                n(e.end.1),
                th.edge
            )
            .unwrap();
            let [a, b, c] = e.head;
            writeln!(
                self.out,
                r#"<path id="{}--head" d="M{} {} L{} {} L{} {} Z" fill="{}"/>"#,
                e.id,
                n(a.0),
                n(a.1),
                n(b.0),
                n(b.1),
                n(c.0),
                n(c.1),
                th.edge
            )
            .unwrap();
            if let Some(l) = &e.label {
                self.text(l);
            }
        }

        for node in &s.nodes {
            let r = node.rect;
            writeln!(
                self.out,
                r#"<rect id="{}" data-role="node" x="{}" y="{}" width="{}" height="{}" rx="8" fill="{}" stroke="{}" stroke-width="1.25"/>"#,
                node.id,
                n(r.x),
                n(r.y),
                n(r.w),
                n(r.h),
                th.node_fill,
                th.node_stroke
            )
            .unwrap();
            self.text(&node.label);
            if let Some(t) = &node.sublabel {
                self.text(t);
            }
            for b in &node.badges {
                if self.open_group(&b.id) {
                    let br = b.rect;
                    writeln!(
                        self.out,
                        r#"<rect id="{}-bg" x="{}" y="{}" width="{}" height="{}" rx="{}" fill="{}"/>"#,
                        b.id,
                        n(br.x),
                        n(br.y),
                        n(br.w),
                        n(br.h),
                        n(br.h / 2.0),
                        th.badge_fill(b.tone)
                    )
                    .unwrap();
                    self.text(&b.text);
                    self.out.push_str("</g>\n");
                }
            }
            for c in &node.counters {
                self.text(&c.text);
            }
            if let Some(tm) = &node.timer
                && self.open_group(&tm.id)
            {
                self.text(&tm.label);
                let tr = tm.track;
                writeln!(
                    self.out,
                    r#"<rect x="{}" y="{}" width="{}" height="{}" rx="2" fill="{}"/>"#,
                    n(tr.x),
                    n(tr.y),
                    n(tr.w),
                    n(tr.h),
                    th.track
                )
                .unwrap();
                let shrink = self.tl.shrinks.iter().find(|x| x.id == tm.bar_id);
                let (width, anim) = match (self.o.mode, shrink) {
                    (Mode::Frame(t), Some(sh)) if t >= sh.t0 && t < sh.t1 => {
                        (tr.w * (1.0 - (t - sh.t0) / (sh.t1 - sh.t0)), String::new())
                    }
                    (Mode::Animated, Some(sh)) => {
                        let cycle = self.tl.cycle;
                        let (a, b) = (kt(sh.t0, cycle), kt(sh.t1, cycle));
                        let mut times = vec![0.0];
                        let mut vals = vec![tr.w];
                        if a > 0.0 {
                            times.push(a);
                            vals.push(tr.w);
                        }
                        times.push(b);
                        vals.push(0.0);
                        if b < 1.0 {
                            times.push(1.0);
                            vals.push(0.0);
                        }
                        let vals: Vec<String> = vals.iter().map(|v| n(*v)).collect();
                        (
                            tr.w,
                            format!(
                                r#"<animate attributeName="width" dur="{}s" repeatCount="indefinite" calcMode="linear" keyTimes="{}" values="{}"/>"#,
                                n(cycle),
                                join(&times),
                                vals.join(";")
                            ),
                        )
                    }
                    _ => (tr.w, String::new()),
                };
                writeln!(
                        self.out,
                        r#"<rect id="{}" x="{}" y="{}" width="{}" height="{}" rx="2" fill="{}">{anim}</rect>"#,
                        tm.bar_id,
                        n(tr.x),
                        n(tr.y),
                        n(width),
                        n(tr.h),
                        th.solid(crate::spec::Tone::Warn)
                    )
                    .unwrap();
                self.out.push_str("</g>\n");
            }
        }

        for p in &s.pills {
            writeln!(
                self.out,
                r#"<circle id="{}" cx="{}" cy="{}" r="{}" fill="{}" stroke="{}"/>"#,
                p.off_id,
                n(p.center.0),
                n(p.center.1),
                n(p.r),
                th.node_fill,
                th.node_stroke
            )
            .unwrap();
            self.text(&p.num);
            if self.open_group(&p.on_id) {
                writeln!(
                    self.out,
                    r#"<circle id="{}-dot" cx="{}" cy="{}" r="{}" fill="{}"/>"#,
                    p.on_id,
                    n(p.center.0),
                    n(p.center.1),
                    n(p.r),
                    th.accent
                )
                .unwrap();
                self.text(&p.num_on);
                self.out.push_str("</g>\n");
            }
        }
        for c in &s.captions {
            if self.open_group(&c.id) {
                self.text(&c.name);
                if let Some(b) = &c.body {
                    self.text(b);
                }
                self.out.push_str("</g>\n");
            }
        }
        for l in &s.legend {
            let fill = l.tone.map_or(th.accent, |t| th.solid(t));
            writeln!(
                self.out,
                r#"<circle id="{}" cx="{}" cy="{}" r="{TOKEN_R}" fill="{fill}"/>"#,
                l.id,
                n(l.center.0),
                n(l.center.1)
            )
            .unwrap();
            self.text(&l.label);
        }

        for seg in &self.tl.segments {
            self.token(seg);
        }
        self.out.push_str("</svg>\n");
        self.out
    }
}

pub fn emit(scene: &Scene, tl: &Timeline, o: &Options) -> String {
    Emitter {
        scene,
        tl,
        o,
        out: String::new(),
    }
    .emit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_compact() {
        assert_eq!(n(12.0), "12");
        assert_eq!(n(12.345), "12.35");
        assert_eq!(n(-0.001), "0");
    }
}
