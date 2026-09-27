//! `build` and `check`: produce outputs, run the static gate, and report.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use resvg::tiny_skia;
use serde::Serialize;

use crate::check::{geometry, lint};
use crate::emit::{self, Mode};
use crate::finding::{Finding, dedup, has_errors};
use crate::font::{self, Fonts};
use crate::layout;
use crate::spec::{self, Profile, Spec};
use crate::theme::{THEMES, Theme};
use crate::timeline::{self, Timeline};

#[derive(Serialize)]
pub struct PhaseReport {
    pub n: usize,
    pub name: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Serialize)]
pub struct Gates {
    #[serde(rename = "static")]
    pub static_gate: String,
    pub dynamic: String,
}

#[derive(Serialize)]
pub struct Report {
    pub ok: bool,
    pub outputs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cycle_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub phases: Vec<PhaseReport>,
    pub gates: Gates,
    pub findings: Vec<Finding>,
}

impl Report {
    fn failed(findings: Vec<Finding>) -> Self {
        Report {
            ok: false,
            outputs: vec![],
            embed: None,
            cycle_seconds: None,
            phases: vec![],
            gates: Gates {
                static_gate: "failed".into(),
                dynamic: "not run".into(),
            },
            findings,
        }
    }
}

const DYNAMIC_PENDING: &str =
    "not run: the browser gate is milestone 2; frames come from the timeline, not from a browser";

fn description(spec: &Spec) -> String {
    let mut d = spec
        .story
        .takeaway
        .clone()
        .unwrap_or_else(|| spec.title.clone());
    if !d.ends_with('.') {
        d.push('.');
    }
    if let Some(n) = &spec.story.notes {
        d.push_str(&format!(" {n}"));
    }
    for (i, p) in spec.phases.iter().enumerate() {
        d.push_str(&format!(" {}. {}", i + 1, p.name));
        if let Some(c) = &p.caption {
            d.push_str(&format!(": {c}"));
        }
        d.push('.');
    }
    d
}

fn frame_svg(
    scene: &layout::Scene,
    tl: &Timeline,
    theme: &Theme,
    t: f64,
    spec: &Spec,
    desc: &str,
) -> String {
    emit::emit(
        scene,
        tl,
        &emit::Options {
            theme,
            mode: Mode::Frame(t),
            font_css: None,
            text: true,
            title: &spec.title,
            desc,
        },
    )
}

/// Stacks frames: one row per phase, one column per theme.
fn storyboard(rows: &[Vec<tiny_skia::Pixmap>], path: &Path) -> Result<()> {
    const GAP: u32 = 16;
    let cols = rows.first().map_or(0, Vec::len) as u32;
    let (w, h) = rows
        .first()
        .and_then(|r| r.first())
        .map(|p| (p.width(), p.height()))
        .unwrap_or((1, 1));
    let mut sheet =
        tiny_skia::Pixmap::new(GAP + cols * (w + GAP), GAP + rows.len() as u32 * (h + GAP))
            .context("storyboard size")?;
    sheet.fill(tiny_skia::Color::from_rgba8(128, 128, 128, 255));
    for (r, row) in rows.iter().enumerate() {
        for (c, px) in row.iter().enumerate() {
            let (x, y) = (GAP + c as u32 * (w + GAP), GAP + r as u32 * (h + GAP));
            sheet.draw_pixmap(
                x as i32,
                y as i32,
                px.as_ref(),
                &tiny_skia::PixmapPaint::default(),
                tiny_skia::Transform::identity(),
                None,
            );
        }
    }
    sheet.save_png(path).context("writing storyboard")?;
    Ok(())
}

/// `frames`: also write each storyboard frame as SVG, for comparison against a browser.
pub fn build(spec_path: &Path, out_dir: Option<&Path>, frames: bool) -> Result<Report> {
    let yaml = std::fs::read_to_string(spec_path)
        .with_context(|| format!("reading {}", spec_path.display()))?;
    let spec = match spec::parse(&yaml) {
        Ok(s) => s,
        Err(e) => {
            return Ok(Report::failed(vec![Finding::error(
                "spec",
                "document".into(),
                e,
            )]));
        }
    };
    let mut findings = spec.validate();
    if has_errors(&findings) {
        return Ok(Report::failed(findings));
    }

    let fonts = Fonts::new();
    let (scene, lf) = layout::layout(&spec, &fonts);
    findings.extend(lf);
    let (tl, tf) = timeline::schedule(&spec, &scene);
    findings.extend(tf);

    let slug = spec_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("infographic")
        .to_string();
    let dir: PathBuf = out_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| spec_path.parent().unwrap_or(Path::new(".")).to_path_buf());
    std::fs::create_dir_all(&dir)?;
    let desc = description(&spec);
    let font_css = font::font_face_css(&scene.used_chars())?;
    let opt = geometry::options();

    let mut outputs = Vec::new();
    let mut rows: Vec<Vec<tiny_skia::Pixmap>> = tl.phases.iter().map(|_| Vec::new()).collect();
    for theme in THEMES {
        let svg = emit::emit(
            &scene,
            &tl,
            &emit::Options {
                theme,
                mode: Mode::Animated,
                font_css: Some(&font_css),
                text: true,
                title: &spec.title,
                desc: &desc,
            },
        );
        findings.extend(lint::lint(&svg, Profile::Readme));
        geometry::parse(&svg, &opt)?; // must parse; its layout is checked frame by frame below
        let name = if theme.name == "light" {
            format!("{slug}.svg")
        } else {
            format!("{slug}-{}.svg", theme.name)
        };
        let path = dir.join(&name);
        std::fs::write(&path, &svg)?;
        outputs.push(path.display().to_string());

        for (i, span) in tl.phases.iter().enumerate() {
            let frame = frame_svg(&scene, &tl, theme, span.mid(), &spec, &desc);
            let label = format!("{}, phase {} at {:.1}s", theme.name, i + 1, span.mid());
            if frames {
                let fd = dir.join(format!("{slug}.frames"));
                std::fs::create_dir_all(&fd)?;
                std::fs::write(
                    fd.join(format!("{}-phase-{}.svg", theme.name, i + 1)),
                    &frame,
                )?;
            }
            findings.extend(geometry::geometry(&frame, &opt, Some(&label))?);
            findings.extend(geometry::contrast(&frame, &opt, Some(&label))?);
            rows[i].push(geometry::render(&geometry::parse(&frame, &opt)?, 1.0)?);
        }
    }
    let sb = dir.join(format!("{slug}.storyboard.png"));
    storyboard(&rows, &sb)?;
    outputs.push(sb.display().to_string());

    let alt = match &spec.story.takeaway {
        Some(t) => format!("{}: {t}", spec.title),
        None => spec.title.clone(),
    };
    let embed = format!(
        "<picture>\n  <source media=\"(prefers-color-scheme: dark)\" srcset=\"{slug}-dark.svg\">\n  <img alt=\"{}\" src=\"{slug}.svg\">\n</picture>",
        alt.replace('"', "&quot;")
    );
    let findings = dedup(findings);
    let ok = !has_errors(&findings);
    Ok(Report {
        ok,
        outputs,
        embed: Some(embed),
        cycle_seconds: Some((tl.cycle * 100.0).round() / 100.0),
        phases: tl
            .phases
            .iter()
            .enumerate()
            .map(|(i, p)| PhaseReport {
                n: i + 1,
                name: p.name.clone(),
                start: (p.start * 100.0).round() / 100.0,
                end: (p.end * 100.0).round() / 100.0,
            })
            .collect(),
        gates: Gates {
            static_gate: if ok { "passed" } else { "failed" }.into(),
            dynamic: DYNAMIC_PENDING.into(),
        },
        findings,
    })
}

/// Checks any SVG, hand-written or generated, and renders it to PNG for review.
pub fn check(svg_path: &Path, profile: Profile, png: Option<&Path>) -> Result<Report> {
    let svg = std::fs::read_to_string(svg_path)
        .with_context(|| format!("reading {}", svg_path.display()))?;
    let opt = geometry::options();
    let mut findings = lint::lint(&svg, profile);
    let mut outputs = Vec::new();
    match geometry::parse(&svg, &opt) {
        Err(e) => findings.push(Finding::error("parse", "document".into(), e.to_string())),
        Ok(tree) => {
            findings.extend(geometry::geometry(&svg, &opt, None)?);
            findings.extend(geometry::contrast(&svg, &opt, None)?);
            let out = png
                .map(Path::to_path_buf)
                .unwrap_or_else(|| svg_path.with_extension("png"));
            geometry::render(&tree, 2.0)?
                .save_png(&out)
                .context("writing png")?;
            outputs.push(out.display().to_string());
        }
    }
    let animated = svg.contains("<animate") || svg.contains("@keyframes") || svg.contains(":hover");
    let ok = !has_errors(&findings);
    Ok(Report {
        ok,
        outputs,
        embed: None,
        cycle_seconds: None,
        phases: vec![],
        gates: Gates {
            static_gate: if ok { "passed" } else { "failed" }.into(),
            dynamic: if animated {
                "not run: animation and hover need the browser gate (milestone 2); the PNG shows the un-animated state".into()
            } else {
                "not needed".into()
            },
        },
        findings,
    })
}
