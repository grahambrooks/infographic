//! Rendered-geometry checks on any SVG, using usvg's layout with the pinned
//! fonts: text inside its container, nothing overlapping, nothing off-canvas,
//! and WCAG contrast against the pixels actually behind each text.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use resvg::tiny_skia;

use crate::finding::{Finding, describe};

const TOL: f32 = 0.5;

#[derive(Debug, Clone, Copy)]
struct B {
    l: f32,
    t: f32,
    r: f32,
    b: f32,
}

impl B {
    fn of(r: usvg::Rect) -> Self {
        Self {
            l: r.left(),
            t: r.top(),
            r: r.right(),
            b: r.bottom(),
        }
    }
    fn contains(&self, o: &B) -> bool {
        o.l >= self.l - TOL && o.t >= self.t - TOL && o.r <= self.r + TOL && o.b <= self.b + TOL
    }
    fn overlap_area(&self, o: &B) -> f32 {
        let w = self.r.min(o.r) - self.l.max(o.l);
        let h = self.b.min(o.b) - self.t.max(o.t);
        if w > 0.0 && h > 0.0 { w * h } else { 0.0 }
    }
}

struct TextInfo {
    id: String,
    bbox: B,
    color: (u8, u8, u8),
    size: f32,
    weight: u16,
}

pub fn options() -> usvg::Options<'static> {
    usvg::Options { fontdb: Arc::new(crate::font::fontdb()), ..Default::default() }
}

pub fn parse(svg: &str, opt: &usvg::Options) -> Result<usvg::Tree> {
    usvg::Tree::from_str(svg, opt).map_err(|e| anyhow!("usvg could not parse the SVG: {e}"))
}

/// Elements hidden in the source (on themselves or an ancestor), which usvg may still lay out.
fn hidden_ids(doc: &roxmltree::Document) -> std::collections::HashSet<String> {
    doc.descendants()
        .filter(|n| {
            n.ancestors().any(|a| {
                a.attribute("visibility") == Some("hidden")
                    || a.attribute("display") == Some("none")
            })
        })
        .filter_map(|n| n.attribute("id").map(String::from))
        .collect()
}

fn walk<'a>(g: &'a usvg::Group, out: &mut Vec<&'a usvg::Node>) {
    for n in g.children() {
        out.push(n);
        if let usvg::Node::Group(c) = n {
            walk(c, out);
        }
    }
}

fn texts(tree: &usvg::Tree, hidden: &std::collections::HashSet<String>) -> Vec<TextInfo> {
    let mut nodes = Vec::new();
    walk(tree.root(), &mut nodes);
    nodes
        .into_iter()
        .filter_map(|n| match n {
            usvg::Node::Text(t) if !hidden.contains(t.id()) => {
                let span = t.chunks().first()?.spans().first()?;
                let color = match span.fill().map(|f| f.paint()) {
                    Some(usvg::Paint::Color(c)) => (c.red, c.green, c.blue),
                    _ => (0, 0, 0),
                };
                Some(TextInfo {
                    id: if t.id().is_empty() {
                        "<text>".into()
                    } else {
                        t.id().to_string()
                    },
                    bbox: B::of(t.abs_bounding_box()),
                    color,
                    size: span.font_size().get(),
                    weight: span.font().weight(),
                })
            }
            _ => None,
        })
        .collect()
}

/// Layout checks. `frame` names the frame in findings (e.g. `light, phase 2`).
pub fn geometry(svg: &str, opt: &usvg::Options, frame: Option<&str>) -> Result<Vec<Finding>> {
    let tree = parse(svg, opt)?;
    let doc = roxmltree::Document::parse(svg)?;
    let hidden = hidden_ids(&doc);
    let container: HashMap<&str, &str> = doc
        .descendants()
        .filter_map(|n| Some((n.attribute("id")?, n.attribute("data-container")?)))
        .collect();
    let role = |id: &str, r: &str| {
        doc.descendants()
            .any(|n| n.attribute("id") == Some(id) && n.attribute("data-role") == Some(r))
    };
    let canvas = B {
        l: 0.0,
        t: 0.0,
        r: tree.size().width(),
        b: tree.size().height(),
    };
    let mut f = Vec::new();
    let mark = |x: Finding| match frame {
        Some(fr) => x.in_frame(fr),
        None => x,
    };

    let ts = texts(&tree, &hidden);
    if ts.is_empty() && doc.descendants().any(|n| n.has_tag_name("text")) {
        f.push(mark(Finding::error(
            "geometry",
            "text".into(),
            "the SVG has <text> but none was laid out; its font is not available".into(),
        )));
    }
    for t in &ts {
        let what = describe(&t.id);
        if !canvas.contains(&t.bbox) {
            f.push(mark(Finding::error(
                "geometry.bounds",
                what.clone(),
                "text extends outside the canvas".into(),
            )));
        }
        if let Some(c) = container.get(t.id.as_str()) {
            match tree.node_by_id(c) {
                Some(cn) => {
                    let cb = B::of(cn.abs_bounding_box());
                    if !cb.contains(&t.bbox) {
                        f.push(mark(Finding::error(
                            "geometry.overflow",
                            what.clone(),
                            format!(
                                "text overflows {} by {:.1}px",
                                describe(c),
                                [
                                    cb.l - t.bbox.l,
                                    t.bbox.r - cb.r,
                                    cb.t - t.bbox.t,
                                    t.bbox.b - cb.b
                                ]
                                .into_iter()
                                .fold(0.0f32, f32::max)
                            ),
                        )));
                    }
                }
                None => f.push(mark(Finding::error(
                    "geometry.overflow",
                    what.clone(),
                    format!("container #{c} not found"),
                ))),
            }
        }
    }
    for (i, a) in ts.iter().enumerate() {
        for b in &ts[i + 1..] {
            if a.bbox.overlap_area(&b.bbox) > 1.0 {
                f.push(mark(Finding::error(
                    "geometry.overlap",
                    describe(&a.id),
                    format!("text overlaps {}", describe(&b.id)),
                )));
            }
        }
    }

    let mut all = Vec::new();
    walk(tree.root(), &mut all);
    let boxes = |r: &str| -> Vec<(String, B)> {
        all.iter()
            .filter(|n| !n.id().is_empty() && !hidden.contains(n.id()) && role(n.id(), r))
            .map(|n| (n.id().to_string(), B::of(n.abs_bounding_box())))
            .collect()
    };
    let nodes = boxes("node");
    for (i, (ia, a)) in nodes.iter().enumerate() {
        if !canvas.contains(a) {
            f.push(mark(Finding::error(
                "geometry.bounds",
                describe(ia),
                "node extends outside the canvas".into(),
            )));
        }
        for (ib, b) in &nodes[i + 1..] {
            if a.overlap_area(b) > 0.0 {
                f.push(mark(Finding::error(
                    "geometry.overlap",
                    describe(ia),
                    format!("node overlaps {}", describe(ib)),
                )));
            }
        }
    }
    // Straight edges (`M x y L x y`) must not pass through nodes they don't connect.
    for e in doc
        .descendants()
        .filter(|n| n.attribute("data-role") == Some("edge"))
    {
        let nums: Vec<f32> = e
            .attribute("d")
            .unwrap_or("")
            .split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
            .filter_map(|x| x.parse().ok())
            .collect();
        let [x0, y0, x1, y1] = nums[..] else { continue };
        let ends = [e.attribute("data-from"), e.attribute("data-to")];
        for (id, nb) in &nodes {
            if ends.contains(&Some(id.as_str())) {
                continue;
            }
            // Sample the line; nodes are large relative to the step.
            let hit = (0..=64).any(|k| {
                let f = k as f32 / 64.0;
                let (x, y) = (x0 + (x1 - x0) * f, y0 + (y1 - y0) * f);
                x > nb.l && x < nb.r && y > nb.t && y < nb.b
            });
            if hit {
                let eid = e.attribute("id").unwrap_or("edge");
                f.push(mark(Finding::error(
                    "geometry.overlap",
                    describe(eid),
                    format!("passes through {}; edges are straight in this version, so connect only adjacent ranks or reorder nodes", describe(id)),
                )));
            }
        }
    }

    let edges = boxes("edge");
    for t in &ts {
        if container.contains_key(t.id.as_str()) {
            continue;
        }
        for (ie, e) in &edges {
            // An edge's own label sits beside it; anything else crossing a line is clutter.
            let e = B {
                l: e.l,
                t: e.t - 0.75,
                r: e.r,
                b: e.b + 0.75,
            };
            if t.bbox.overlap_area(&e) > 0.0 {
                f.push(mark(Finding::warning(
                    "geometry.overlap",
                    describe(&t.id),
                    format!("text crosses {}", describe(ie)),
                )));
            }
        }
    }
    Ok(f)
}

fn luminance((r, g, b): (u8, u8, u8)) -> f64 {
    let c = |v: u8| {
        let v = v as f64 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * c(r) + 0.7152 * c(g) + 0.0722 * c(b)
}

pub fn contrast_ratio(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Removes every `<text>` element, to render what lies behind the text.
pub fn strip_text(svg: &str) -> Result<String> {
    let doc = roxmltree::Document::parse(svg)?;
    let mut ranges: Vec<std::ops::Range<usize>> = doc
        .descendants()
        .filter(|n| {
            n.has_tag_name("text") && !n.ancestors().skip(1).any(|a| a.has_tag_name("text"))
        })
        .map(|n| n.range())
        .collect();
    ranges.sort_by_key(|r| r.start);
    let mut out = String::with_capacity(svg.len());
    let mut at = 0;
    for r in ranges {
        out.push_str(&svg[at..r.start]);
        at = r.end;
    }
    out.push_str(&svg[at..]);
    Ok(out)
}

pub fn render(tree: &usvg::Tree, scale: f32) -> Result<tiny_skia::Pixmap> {
    let size = tree
        .size()
        .to_int_size()
        .scale_by(scale)
        .ok_or_else(|| anyhow!("empty canvas"))?;
    let mut pm = tiny_skia::Pixmap::new(size.width(), size.height())
        .ok_or_else(|| anyhow!("empty canvas"))?;
    resvg::render(
        tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pm.as_mut(),
    );
    Ok(pm)
}

/// WCAG AA: 4.5:1, or 3:1 for large text (≥24px, or ≥18.66px bold).
pub fn contrast(svg: &str, opt: &usvg::Options, frame: Option<&str>) -> Result<Vec<Finding>> {
    let tree = parse(svg, opt)?;
    let doc = roxmltree::Document::parse(svg)?;
    let hidden = hidden_ids(&doc);
    let bg = render(&parse(&strip_text(svg)?, opt)?, 1.0)?;
    let mut f = Vec::new();
    for t in texts(&tree, &hidden) {
        let (x0, y0) = (t.bbox.l.max(0.0) as u32, t.bbox.t.max(0.0) as u32);
        let (x1, y1) = (
            (t.bbox.r.ceil() as u32).min(bg.width()),
            (t.bbox.b.ceil() as u32).min(bg.height()),
        );
        let mut counts: HashMap<(u8, u8, u8), u32> = HashMap::new();
        for y in y0..y1 {
            for x in x0..x1 {
                let p = bg.pixel(x, y).unwrap().demultiply();
                *counts.entry((p.red(), p.green(), p.blue())).or_default() += 1;
            }
        }
        let Some((&back, _)) = counts.iter().max_by_key(|(_, c)| **c) else {
            continue;
        };
        let ratio = contrast_ratio(t.color, back);
        let large = t.size >= 24.0 || (t.size >= 18.66 && t.weight >= 600);
        let need = if large { 3.0 } else { 4.5 };
        if ratio < need {
            let x = Finding::error(
                "contrast",
                describe(&t.id),
                format!(
                    "contrast {ratio:.2}:1 is below {need}:1 (text #{:02x}{:02x}{:02x} on #{:02x}{:02x}{:02x})",
                    t.color.0, t.color.1, t.color.2, back.0, back.1, back.2
                ),
            );
            f.push(match frame {
                Some(fr) => x.in_frame(fr),
                None => x,
            });
        }
    }
    Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100" viewBox="0 0 200 100"><rect width="200" height="100" fill="#fff"/>"##;

    #[test]
    fn overflowing_text_is_found() {
        let svg = format!(
            r##"{HEAD}<rect id="box" x="10" y="10" width="40" height="30" fill="#eee"/><text id="t" x="15" y="30" font-family="IBM Plex Sans" font-size="14" data-container="box">Much too long</text></svg>"##
        );
        let f = geometry(&svg, &options(), None).unwrap();
        assert!(f.iter().any(|x| x.check == "geometry.overflow"), "{f:?}");
    }

    #[test]
    fn low_contrast_is_found() {
        let svg = format!(
            r##"{HEAD}<text id="t" x="10" y="50" font-family="IBM Plex Sans" font-size="14" fill="#dddddd">faint</text></svg>"##
        );
        let f = contrast(&svg, &options(), None).unwrap();
        assert!(f.iter().any(|x| x.check == "contrast"), "{f:?}");
        let svg = svg.replace("#dddddd", "#222222");
        assert!(contrast(&svg, &options(), None).unwrap().is_empty());
    }

    #[test]
    fn edge_through_an_unrelated_node_is_found() {
        let svg = format!(
            r##"{HEAD}<rect id="n-a" data-role="node" x="0" y="40" width="20" height="20"/><rect id="n-b" data-role="node" x="90" y="40" width="20" height="20"/><rect id="n-c" data-role="node" x="180" y="40" width="20" height="20"/><path id="e-ac" data-role="edge" data-from="n-a" data-to="n-c" d="M20 50 L180 50" stroke="#000"/></svg>"##
        );
        let f = geometry(&svg, &options(), None).unwrap();
        assert!(
            f.iter()
                .any(|x| x.message.contains("passes through node b")),
            "{f:?}"
        );
    }

    #[test]
    fn generic_sans_serif_maps_to_the_pinned_font() {
        let svg = format!(
            r##"{HEAD}<text id="t" x="10" y="50" font-family="sans-serif" font-size="14">laid out</text></svg>"##
        );
        assert!(geometry(&svg, &options(), None).unwrap().is_empty());
    }
}
