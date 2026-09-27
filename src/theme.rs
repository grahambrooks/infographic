//! Colour roles. The scene refers to roles only; a theme resolves them, so
//! light and dark output come from the same scene.

use crate::spec::Tone;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ink {
    Text,
    Muted,
    OnAccent,
    ToneText(Tone),
}

pub struct Theme {
    pub name: &'static str,
    pub background: &'static str,
    pub border: &'static str,
    pub text: &'static str,
    pub muted: &'static str,
    pub node_fill: &'static str,
    pub node_stroke: &'static str,
    pub edge: &'static str,
    pub accent: &'static str,
    pub on_accent: &'static str,
    pub track: &'static str,
    ok: [&'static str; 3],
    fail: [&'static str; 3],
    warn: [&'static str; 3],
    neutral: [&'static str; 3],
}

impl Theme {
    /// `[solid, badge background, badge text]` for a tone.
    fn tone(&self, t: Tone) -> &[&'static str; 3] {
        match t {
            Tone::Ok => &self.ok,
            Tone::Fail => &self.fail,
            Tone::Warn => &self.warn,
            Tone::Neutral => &self.neutral,
        }
    }

    pub fn solid(&self, t: Tone) -> &'static str {
        self.tone(t)[0]
    }

    pub fn badge_fill(&self, t: Tone) -> &'static str {
        self.tone(t)[1]
    }

    pub fn ink(&self, i: Ink) -> &'static str {
        match i {
            Ink::Text => self.text,
            Ink::Muted => self.muted,
            Ink::OnAccent => self.on_accent,
            Ink::ToneText(t) => self.tone(t)[2],
        }
    }
}

pub const LIGHT: Theme = Theme {
    name: "light",
    background: "#ffffff",
    border: "#d0d7de",
    text: "#1f2328",
    muted: "#57606a",
    node_fill: "#f6f8fa",
    node_stroke: "#afb8c1",
    edge: "#8c959f",
    accent: "#0969da",
    on_accent: "#ffffff",
    track: "#d0d7de",
    ok: ["#1a7f37", "#dafbe1", "#116329"],
    fail: ["#cf222e", "#ffebe9", "#a40e26"],
    warn: ["#bf8700", "#fff8c5", "#7d4e00"],
    neutral: ["#6e7781", "#eaeef2", "#424a53"],
};

pub const DARK: Theme = Theme {
    name: "dark",
    background: "#0d1117",
    border: "#30363d",
    text: "#e6edf3",
    muted: "#9198a1",
    node_fill: "#161b22",
    node_stroke: "#3d444d",
    edge: "#656c76",
    accent: "#4493f8",
    on_accent: "#0d1117",
    track: "#3d444d",
    ok: ["#3fb950", "#12321f", "#56d364"],
    fail: ["#f85149", "#3c1618", "#ff7b72"],
    warn: ["#d29922", "#3b2e0c", "#e3b341"],
    neutral: ["#9198a1", "#262c36", "#d1d7e0"],
};

pub const THEMES: [&Theme; 2] = [&LIGHT, &DARK];
