//! Source-level checks on any SVG: profile rules, dangling references, and
//! whether every SMIL animation shares one clock.

use std::collections::HashSet;

use crate::finding::{Finding, describe};
use crate::spec::Profile;

const ANIMATIONS: [&str; 5] = [
    "animate",
    "set",
    "animateMotion",
    "animateTransform",
    "animateColor",
];

fn is_external(url: &str) -> bool {
    let u = url.trim();
    u.starts_with("http:") || u.starts_with("https:") || u.starts_with("//")
}

/// Ids referenced as `url(#id)` in a string.
fn url_refs(s: &str) -> Vec<String> {
    s.match_indices("url(")
        .filter_map(|(i, _)| {
            let rest = &s[i + 4..];
            let inner = rest[..rest.find(')')?].trim().trim_matches(['"', '\'']);
            inner.strip_prefix('#').map(str::to_string)
        })
        .collect()
}

pub fn lint(svg: &str, profile: Profile) -> Vec<Finding> {
    let mut f = Vec::new();
    let doc = match roxmltree::Document::parse(svg) {
        Ok(d) => d,
        Err(e) => {
            return vec![Finding::error(
                "lint",
                "document".into(),
                format!("not well-formed XML: {e}"),
            )];
        }
    };
    let root = doc.root_element();
    let ids: HashSet<&str> = doc
        .descendants()
        .filter_map(|n| n.attribute("id"))
        .collect();
    let style: String = doc
        .descendants()
        .filter(|n| n.has_tag_name("style"))
        .flat_map(|n| n.children().filter_map(|c| c.text()))
        .collect();

    if profile == Profile::Readme {
        let rule = |f: &mut Vec<Finding>, what: &str, why: &str| {
            f.push(Finding::error(
                "lint.readme",
                what.into(),
                format!("{why}; GitHub shows README SVGs as <img>, where this does not work"),
            ))
        };
        if style.contains(":hover") {
            rule(&mut f, "style", ":hover never fires");
        }
        if style.contains("prefers-color-scheme") {
            rule(
                &mut f,
                "style",
                "in-SVG prefers-color-scheme is ignored by WebKit/Safari; ship a -dark.svg and a <picture> instead",
            );
        }
        for n in doc.descendants().filter(|n| n.is_element()) {
            let tag = n.tag_name().name();
            let target = n
                .attribute("id")
                .map(describe)
                .unwrap_or_else(|| format!("<{tag}>"));
            match tag {
                "script" => rule(&mut f, &target, "scripts are stripped"),
                "a" => rule(&mut f, &target, "links are inert"),
                "foreignObject" => rule(
                    &mut f,
                    &target,
                    "foreignObject content is dropped by SVG-only consumers",
                ),
                "title" if n.parent_element() != Some(root) => f.push(Finding::warning(
                    "lint.readme",
                    target,
                    "<title> tooltips never show inside <img>".into(),
                )),
                _ => {}
            }
        }
        for u in style.match_indices("url(").map(|(i, _)| &style[i + 4..]) {
            if is_external(u.trim_start_matches(['"', '\''])) {
                rule(&mut f, "style", "external url() resources are blocked");
            }
        }
    }

    for n in doc.descendants().filter(|n| n.is_element()) {
        let target = n
            .attribute("id")
            .map(describe)
            .unwrap_or_else(|| format!("<{}>", n.tag_name().name()));
        for a in n.attributes() {
            let v = a.value();
            if a.name() == "href" {
                if is_external(v) {
                    if profile == Profile::Readme && !n.has_tag_name("a") {
                        f.push(Finding::error(
                            "lint.readme",
                            target.clone(),
                            format!("external resource `{v}` is blocked inside <img>"),
                        ));
                    }
                } else if let Some(id) = v.strip_prefix('#')
                    && !ids.contains(id)
                {
                    f.push(Finding::error(
                        "lint.refs",
                        target.clone(),
                        format!("href points at missing #{id}"),
                    ));
                }
            }
            for id in url_refs(v) {
                if !ids.contains(id.as_str()) {
                    f.push(Finding::error(
                        "lint.refs",
                        target.clone(),
                        format!("url(#{id}) points at a missing element"),
                    ));
                }
            }
            if a.name() == "begin" || a.name() == "end" {
                for part in v.split(';') {
                    if let Some((id, _event)) = part.trim().split_once('.')
                        && !id.is_empty()
                        && !id.chars().next().unwrap().is_ascii_digit()
                        && !ids.contains(id)
                    {
                        f.push(Finding::error(
                            "lint.refs",
                            target.clone(),
                            format!("{}=\"{v}\" names missing element `{id}`", a.name()),
                        ));
                    }
                }
            }
        }
    }
    for id in url_refs(&style) {
        if !ids.contains(id.as_str()) {
            f.push(Finding::error(
                "lint.refs",
                "style".into(),
                format!("url(#{id}) points at a missing element"),
            ));
        }
    }

    // One clock: every SMIL animation loops with the same duration.
    let cycle = root.attribute("data-cycle").map(str::to_string);
    let mut durs: Vec<(String, String)> = Vec::new();
    for n in doc
        .descendants()
        .filter(|n| ANIMATIONS.iter().any(|t| n.has_tag_name(*t)))
    {
        let owner = n
            .parent_element()
            .and_then(|p| p.attribute("id"))
            .map(describe)
            .unwrap_or_else(|| "animation".into());
        let dur = n
            .attribute("dur")
            .unwrap_or("")
            .trim_end_matches('s')
            .to_string();
        if n.attribute("repeatCount") != Some("indefinite") {
            f.push(Finding::warning(
                "lint.clock",
                owner.clone(),
                "animation does not loop (repeatCount is not indefinite)".into(),
            ));
        }
        durs.push((owner, dur));
    }
    let reference = cycle.or_else(|| durs.first().map(|d| d.1.clone()));
    if let Some(r) = reference {
        for (owner, d) in &durs {
            if *d != r {
                f.push(Finding::error(
                    "lint.clock",
                    owner.clone(),
                    format!("dur={d}s differs from the cycle ({r}s); animations with their own clock drift apart on every loop"),
                ));
            }
        }
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svg(body: &str) -> String {
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">{body}</svg>"#)
    }

    #[test]
    fn readme_profile_rejects_interaction() {
        let s = svg(
            r#"<style>#a:hover{fill:red}</style><rect id="a" width="1" height="1"/><foreignObject/>"#,
        );
        let f = lint(&s, Profile::Readme);
        assert!(f.iter().any(|x| x.message.contains(":hover")));
        assert!(f.iter().any(|x| x.message.contains("foreignObject")));
        assert!(
            lint(&s, Profile::Interactive)
                .iter()
                .all(|x| !x.message.contains(":hover"))
        );
    }

    #[test]
    fn dangling_references_are_found() {
        let s = svg(
            r##"<circle><animateMotion dur="2s" repeatCount="indefinite"><mpath href="#gone"/></animateMotion></circle><rect fill="url(#nope)"/>"##,
        );
        let f = lint(&s, Profile::Readme);
        assert!(f.iter().any(|x| x.message.contains("#gone")));
        assert!(f.iter().any(|x| x.message.contains("#nope")));
    }

    #[test]
    fn animations_with_their_own_clock_are_found() {
        let s = svg(
            r#"<rect><animate dur="2s" repeatCount="indefinite"/></rect><rect><animate dur="3s" repeatCount="indefinite"/></rect>"#,
        );
        assert!(
            lint(&s, Profile::Readme)
                .iter()
                .any(|x| x.check == "lint.clock")
        );
    }
}
