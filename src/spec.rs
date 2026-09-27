//! The YAML spec: what happens in the infographic, never where things go.

use indexmap::IndexMap;
use serde::Deserialize;

use crate::finding::Finding;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub title: String,
    #[serde(default)]
    pub story: Story,
    #[serde(default = "default_profiles")]
    pub profile: Vec<Profile>,
    pub nodes: IndexMap<String, NodeSpec>,
    #[serde(default)]
    pub edges: IndexMap<String, EdgeSpec>,
    #[serde(default)]
    pub states: IndexMap<String, StateSpec>,
    #[serde(default)]
    pub counters: IndexMap<String, CounterSpec>,
    pub phases: Vec<PhaseSpec>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Story {
    pub takeaway: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    Readme,
    Interactive,
}

fn default_profiles() -> Vec<Profile> {
    vec![Profile::Readme]
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeSpec {
    pub label: String,
    pub sublabel: Option<String>,
    pub state: Option<String>,
    pub counter: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EdgeSpec {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum StateSpec {
    List(Vec<String>),
    Full {
        values: Vec<String>,
        #[serde(default)]
        tones: IndexMap<String, Tone>,
    },
}

impl StateSpec {
    pub fn values(&self) -> &[String] {
        match self {
            StateSpec::List(v) => v,
            StateSpec::Full { values, .. } => values,
        }
    }

    pub fn tone(&self, value: &str) -> Tone {
        match self {
            StateSpec::Full { tones, .. } => tones.get(value).copied().unwrap_or(Tone::Neutral),
            StateSpec::List(_) => Tone::Neutral,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
    Ok,
    Fail,
    Warn,
    Neutral,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CounterSpec {
    pub label: String,
    pub max: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseSpec {
    pub name: String,
    pub caption: Option<String>,
    /// Seconds. Derived from the flows and caption length when absent.
    pub duration: Option<f64>,
    #[serde(default)]
    pub set: IndexMap<String, SetValue>,
    #[serde(default)]
    pub flows: Vec<FlowSpec>,
    /// Counter ramps, `failures: 0..5`: one step per token arriving back at the counter's node.
    #[serde(default)]
    pub ramp: IndexMap<String, String>,
    pub timer: Option<TimerSpec>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SetValue {
    Int(i64),
    Str(String),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowSpec {
    pub path: Vec<String>,
    pub outcome: Outcome,
    pub rate: Option<Rate>,
    pub count: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Ok,
    Fail,
    Fallback,
}

impl Outcome {
    pub fn tone(self) -> Tone {
        match self {
            Outcome::Ok => Tone::Ok,
            Outcome::Fail => Tone::Fail,
            Outcome::Fallback => Tone::Warn,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Outcome::Ok => "success",
            Outcome::Fail => "failure",
            Outcome::Fallback => "fallback",
        }
    }
}

/// Tokens per second, written `2/s` or `2`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Rate {
    Number(f64),
    Text(String),
}

impl Rate {
    pub fn per_second(&self) -> Option<f64> {
        let v = match self {
            Rate::Number(n) => *n,
            Rate::Text(s) => s.trim().trim_end_matches("/s").trim().parse().ok()?,
        };
        (v > 0.0 && v.is_finite()).then_some(v)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimerSpec {
    /// Defaults to the first node that shows a state.
    pub node: Option<String>,
    pub label: String,
}

pub fn parse(yaml: &str) -> Result<Spec, String> {
    serde_saphyr::from_str(yaml).map_err(|e| e.to_string())
}

/// Parses `a..b` into its endpoints.
pub fn parse_ramp(s: &str) -> Option<(i64, i64)> {
    let (a, b) = s.split_once("..")?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

impl Spec {
    pub fn timer_node<'a>(&'a self, timer: &'a TimerSpec) -> Option<&'a str> {
        match &timer.node {
            Some(n) => Some(n.as_str()),
            None => self
                .nodes
                .iter()
                .find(|(_, n)| n.state.is_some())
                .map(|(id, _)| id.as_str()),
        }
    }

    /// The node that shows a given counter.
    pub fn counter_node(&self, counter: &str) -> Option<&str> {
        self.nodes
            .iter()
            .find(|(_, n)| n.counter.as_deref() == Some(counter))
            .map(|(id, _)| id.as_str())
    }

    /// Referential and structural checks. Timing checks live in the timeline.
    pub fn validate(&self) -> Vec<Finding> {
        let mut f = Vec::new();
        let err = |f: &mut Vec<Finding>, target: String, msg: String| {
            f.push(Finding::error("spec", target, msg))
        };

        if self.nodes.is_empty() {
            err(
                &mut f,
                "nodes".into(),
                "at least one node is required".into(),
            );
        }
        if self.phases.is_empty() {
            err(
                &mut f,
                "phases".into(),
                "at least one phase is required".into(),
            );
        }
        if self.profile.contains(&Profile::Interactive) {
            f.push(Finding::warning(
                "spec",
                "profile".into(),
                "the interactive profile is not implemented yet (milestone 3); only readme output is produced".into(),
            ));
        }

        let ids = self
            .nodes
            .keys()
            .map(|k| ("node", k))
            .chain(self.edges.keys().map(|k| ("edge", k)));
        for (kind, id) in ids {
            let valid = !id.is_empty()
                && id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                && !id.contains("--")
                && id.chars().next().is_some_and(|c| c.is_ascii_alphabetic());
            if !valid {
                err(
                    &mut f,
                    format!("{kind} {id}"),
                    "ids must start with a letter and use only letters, digits, `_` and single `-`"
                        .into(),
                );
            }
        }

        for (id, e) in &self.edges {
            for end in [&e.from, &e.to] {
                if !self.nodes.contains_key(end) {
                    err(
                        &mut f,
                        format!("edge {id}"),
                        format!("unknown node `{end}`"),
                    );
                }
            }
            if e.from == e.to {
                err(
                    &mut f,
                    format!("edge {id}"),
                    "self-loops are not supported".into(),
                );
            }
        }

        for (id, n) in &self.nodes {
            if let Some(s) = &n.state
                && !self.states.contains_key(s)
            {
                err(&mut f, format!("node {id}"), format!("unknown state `{s}`"));
            }
            if let Some(c) = &n.counter
                && !self.counters.contains_key(c)
            {
                err(
                    &mut f,
                    format!("node {id}"),
                    format!("unknown counter `{c}`"),
                );
            }
        }
        for (id, s) in &self.states {
            if s.values().is_empty() {
                err(
                    &mut f,
                    format!("state {id}"),
                    "needs at least one value".into(),
                );
            }
            if let StateSpec::Full { tones, .. } = s {
                for k in tones.keys() {
                    if !s.values().contains(k) {
                        err(
                            &mut f,
                            format!("state {id}"),
                            format!("tone given for undeclared value `{k}`"),
                        );
                    }
                }
            }
            if !self.nodes.values().any(|n| n.state.as_deref() == Some(id)) {
                f.push(Finding::warning(
                    "spec",
                    format!("state {id}"),
                    "not shown on any node".into(),
                ));
            }
        }
        for id in self.counters.keys() {
            if self.counter_node(id).is_none() {
                f.push(Finding::warning(
                    "spec",
                    format!("counter {id}"),
                    "not shown on any node".into(),
                ));
            }
        }

        for (i, p) in self.phases.iter().enumerate() {
            let target = format!("phase {} ({})", i + 1, p.name);
            if p.flows.is_empty() && p.set.is_empty() && p.ramp.is_empty() && p.timer.is_none() {
                err(&mut f, target.clone(), "phase changes nothing".into());
            }
            if let Some(d) = p.duration
                && !(d > 0.0 && d.is_finite())
            {
                err(
                    &mut f,
                    target.clone(),
                    "duration must be a positive number of seconds".into(),
                );
            }
            for (k, v) in &p.set {
                match (self.states.get(k), self.counters.contains_key(k), v) {
                    (Some(s), _, SetValue::Str(val)) => {
                        if !s.values().contains(val) {
                            err(
                                &mut f,
                                target.clone(),
                                format!("`{val}` is not a value of state `{k}`"),
                            );
                        }
                    }
                    (Some(_), _, SetValue::Int(n)) => {
                        err(
                            &mut f,
                            target.clone(),
                            format!("state `{k}` set to number {n}"),
                        );
                    }
                    (None, true, SetValue::Int(_)) => {}
                    (None, true, SetValue::Str(s)) => {
                        err(
                            &mut f,
                            target.clone(),
                            format!("counter `{k}` set to non-number `{s}`"),
                        );
                    }
                    (None, false, _) => {
                        err(
                            &mut f,
                            target.clone(),
                            format!("`set` names unknown state or counter `{k}`"),
                        );
                    }
                }
            }
            for (k, r) in &p.ramp {
                if !self.counters.contains_key(k) {
                    err(
                        &mut f,
                        target.clone(),
                        format!("`ramp` names unknown counter `{k}`"),
                    );
                }
                match parse_ramp(r) {
                    None => err(
                        &mut f,
                        target.clone(),
                        format!("ramp `{r}` must look like `0..5`"),
                    ),
                    Some((a, b)) if a == b => {
                        err(&mut f, target.clone(), format!("ramp `{r}` has no steps"))
                    }
                    _ => {}
                }
            }
            if let Some(t) = &p.timer {
                match self.timer_node(t) {
                    Some(n) if self.nodes.contains_key(n) => {}
                    Some(n) => err(
                        &mut f,
                        target.clone(),
                        format!("timer on unknown node `{n}`"),
                    ),
                    None => err(
                        &mut f,
                        target.clone(),
                        "timer needs `node` (no node shows a state)".into(),
                    ),
                }
            }
            for (j, flow) in p.flows.iter().enumerate() {
                let ft = format!("{target} flow {}", j + 1);
                if flow.path.is_empty() {
                    err(&mut f, ft.clone(), "path is empty".into());
                }
                let mut prev_to: Option<&str> = None;
                for e in &flow.path {
                    match self.edges.get(e) {
                        None => err(&mut f, ft.clone(), format!("unknown edge `{e}`")),
                        Some(edge) => {
                            if let Some(pt) = prev_to
                                && pt != edge.from
                            {
                                err(
                                    &mut f,
                                    ft.clone(),
                                    format!(
                                        "path is not connected: `{e}` starts at `{}` but the previous edge ends at `{pt}`",
                                        edge.from
                                    ),
                                );
                            }
                            prev_to = Some(&edge.to);
                        }
                    }
                }
                match (&flow.rate, flow.count) {
                    (Some(_), Some(_)) => err(
                        &mut f,
                        ft.clone(),
                        "give `rate` or `count`, not both".into(),
                    ),
                    (Some(r), None) if r.per_second().is_none() => err(
                        &mut f,
                        ft.clone(),
                        "rate must be positive, e.g. `2/s`".into(),
                    ),
                    (None, Some(0)) => err(&mut f, ft.clone(), "count must be at least 1".into()),
                    _ => {}
                }
            }
        }
        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
title: T
nodes:
  a: { label: A }
  b: { label: B }
edges:
  ab: { from: a, to: b }
phases:
  - name: go
    flows: [{ path: [ab], outcome: ok }]
"#;

    #[test]
    fn minimal_spec_is_valid() {
        let s = parse(MINIMAL).unwrap();
        assert!(s.validate().is_empty(), "{:?}", s.validate());
    }

    #[test]
    fn disconnected_path_is_reported() {
        let y = MINIMAL.replace("path: [ab]", "path: [ab, ab]");
        let f = parse(&y).unwrap().validate();
        assert!(
            f.iter().any(|x| x.message.contains("not connected")),
            "{f:?}"
        );
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let y = MINIMAL.replace("title: T", "title: T\ncolour: red");
        assert!(parse(&y).is_err());
    }

    #[test]
    fn rate_parses_both_forms() {
        assert_eq!(Rate::Text("2/s".into()).per_second(), Some(2.0));
        assert_eq!(Rate::Number(1.5).per_second(), Some(1.5));
        assert_eq!(Rate::Text("0/s".into()).per_second(), None);
    }
}
