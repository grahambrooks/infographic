use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// One problem, named in spec terms (`node breaker`, `phase 3`) wherever the
/// SVG came from a spec, so it can be fixed in the spec.
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub severity: Severity,
    pub check: String,
    pub target: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<String>,
}

impl Finding {
    pub fn error(check: &str, target: String, message: String) -> Self {
        Self {
            severity: Severity::Error,
            check: check.into(),
            target,
            message,
            frame: None,
        }
    }

    pub fn warning(check: &str, target: String, message: String) -> Self {
        Self {
            severity: Severity::Warning,
            check: check.into(),
            target,
            message,
            frame: None,
        }
    }

    pub fn in_frame(mut self, frame: &str) -> Self {
        self.frame = Some(frame.into());
        self
    }
}

/// Collapses the same problem found in several frames into one finding.
pub fn dedup(findings: Vec<Finding>) -> Vec<Finding> {
    let mut out: Vec<(Finding, usize)> = Vec::new();
    for f in findings {
        match out.iter_mut().find(|(g, _)| {
            g.severity == f.severity
                && g.check == f.check
                && g.target == f.target
                && g.message == f.message
        }) {
            Some((_, n)) => *n += 1,
            None => out.push((f, 0)),
        }
    }
    out.into_iter()
        .map(|(mut f, more)| {
            if more > 0 {
                f.frame = f.frame.map(|fr| format!("{fr} (and {more} more)"));
            }
            f
        })
        .collect()
}

pub fn has_errors(findings: &[Finding]) -> bool {
    findings.iter().any(|f| f.severity == Severity::Error)
}

/// Translates an emitted element id back into the spec name it came from.
pub fn describe(id: &str) -> String {
    let parts: Vec<&str> = id.splitn(3, '-').collect();
    match parts.as_slice() {
        ["n", rest @ ..] => {
            let rest = rest.join("-");
            let (node, detail) = rest.split_once("--").unwrap_or((&rest, ""));
            if detail.is_empty() {
                format!("node {node}")
            } else {
                format!("node {node} ({detail})")
            }
        }
        ["e", rest @ ..] => {
            let rest = rest.join("-");
            match rest.strip_suffix("--label") {
                Some(e) => format!("edge {e} label"),
                None => format!("edge {rest}"),
            }
        }
        ["cap", n, ..] => format!("phase {n} caption"),
        ["pill", n, ..] => format!("phase {n} indicator"),
        ["legend", ..] => "legend".into(),
        ["title"] => "title".into(),
        ["takeaway"] => "story takeaway".into(),
        _ => id.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::describe;

    #[test]
    fn ids_map_back_to_spec_names() {
        assert_eq!(describe("n-breaker"), "node breaker");
        assert_eq!(
            describe("n-breaker--state-OPEN"),
            "node breaker (state-OPEN)"
        );
        assert_eq!(describe("e-call"), "edge call");
        assert_eq!(describe("e-call--label"), "edge call label");
        assert_eq!(describe("cap-3-name"), "phase 3 caption");
    }
}
