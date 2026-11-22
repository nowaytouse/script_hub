use std::collections::BTreeSet;
use std::path::Path;

use super::SourceInput;

pub(super) const HUB: (&str, &str) = (
    "script_hub.surge.sgmodule",
    "https://raw.githubusercontent.com/Script-Hub-Org/Script-Hub/main/modules/script-hub.surge.sgmodule",
);
pub(super) const STORE: (&str, &str) = (
    "Sub-Store-Beta.sgmodule",
    "https://raw.githubusercontent.com/sub-store-org/Sub-Store/master/config/Surge-Beta.sgmodule",
);

fn scoped_pattern(pattern: &str, store: bool) -> Option<String> {
    let authority = if store {
        r"^https?:\/\/sub\.store"
    } else {
        r"^https?:\/\/script\.hub"
    };
    let suffix = pattern.strip_prefix(authority)?;
    if store && suffix.is_empty() {
        return Some(format!(r"{authority}(?:[/?]|$)"));
    }
    let path = suffix.strip_prefix(r"\/")?;
    // Validate the path in isolation before grouping it. Even a new top-level
    // alternative must stay behind the reviewed authority and slash.
    if path.len() > 2048
        || path.contains("(?") && path.replace("(?:", "").contains("(?")
        || regex::Regex::new(path).is_err()
    {
        return None;
    }
    Some(format!(r"{authority}\/(?:{path})"))
}

// Only the operator's existing endpoints and jobs belong in the bundle.
// Network-wide settings, new cron jobs and MITM permissions are not inherited.
pub(super) fn project(url: &str, body: &str) -> Result<String, String> {
    let store = url == STORE.1;
    let (assets, prefix): (&[&str], &str) = if store {
        (
            &[
                "sub-store-1.min.js",
                "sub-store-0.min.js",
                "cron-sync-artifacts.min.js",
                "cron-sync-artifacts.min.js",
            ],
            "https://github.com/sub-store-org/Sub-Store/releases/latest/download/",
        )
    } else if url == HUB.1 {
        (
            &[
                "script-hub.js",
                "Rewrite-Parser.js",
                "rule-parser.js",
                "script-converter.js",
            ],
            "https://raw.githubusercontent.com/Script-Hub-Org/Script-Hub/main/",
        )
    } else {
        return Err("unrecognized tools upstream".into());
    };
    let mut header = Vec::new();
    let mut scripts = Vec::new();
    let mut seen = BTreeSet::new();
    let mut labels = BTreeSet::new();
    let mut section = "";
    for line in body.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            section = line;
        } else if section.is_empty()
            && line.split_once('=').is_some_and(|(k, _)| {
                matches!(k.trim(), "#!name" | "#!arguments" | "#!arguments-desc")
            })
        {
            header.push(line);
        } else if section == "[Script]"
            && !line.is_empty()
            && !line.starts_with(['#', ';'])
            && !line.starts_with("//")
        {
            let (label, params) = line.split_once('=').ok_or("missing tools script label")?;
            let fields = crate::smart_cleanup::split_script_parameters(params);
            let field = |key: &str| {
                let mut values = fields
                    .iter()
                    .filter_map(|f| f.split_once('='))
                    .filter_map(|(k, v)| (k.trim().eq_ignore_ascii_case(key)).then_some(v.trim()));
                let value = values.next()?;
                values.next().is_none().then_some(value)
            };
            let asset = field("script-path")
                .and_then(|u| u.strip_prefix(prefix))
                .ok_or("unexpected tools script origin")?;
            if !assets.contains(&asset) {
                return Err("unreviewed tools script asset".into());
            }
            let typ = field("type");
            let label = label.trim();
            if label.is_empty() || !labels.insert(label) {
                return Err("empty or duplicated tools script label".into());
            }
            let projected = if store && asset == "cron-sync-artifacts.min.js" {
                if !matches!(label, "{{{sync}}}" | "{{{produce}}}") || typ != Some("cron") {
                    return Err("unexpected tools cron handler".into());
                }
                line.to_owned()
            } else {
                if store && field("argument") != Some(r#""cors={{{cors}}}""#) {
                    return Err(
                        "Sub-Store's request handler no longer uses the configured CORS boundary"
                            .into(),
                    );
                }
                let pattern = field("pattern")
                    .and_then(|p| scoped_pattern(p, store))
                    .filter(|_| typ == Some("http-request"))
                    .ok_or("unexpected tools request scope")?;
                let params = fields
                    .iter()
                    .map(|f| match f.split_once('=') {
                        Some((key, _)) if key.trim().eq_ignore_ascii_case("pattern") => {
                            format!("pattern={pattern}")
                        }
                        _ => f.trim().to_owned(),
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                format!("{label} = {params}")
            };
            if !seen.insert((asset, if store { label } else { "" })) {
                return Err("unexpected or duplicated tools handler".into());
            }
            scripts.push(projected);
        }
    }
    if scripts.len() != assets.len()
        || !header.iter().any(|l| {
            l.split_once('=')
                .is_some_and(|(k, _)| k.trim() == "#!arguments")
        })
    {
        return Err("incomplete tools upstream module".into());
    }
    let projected = format!("{}\n[Script]\n{}\n", header.join("\n"), scripts.join("\n"));
    if store {
        super::verify_substore_resources(&projected)?;
    }
    Ok(projected)
}

pub(super) fn load(root: &Path) -> Result<Vec<SourceInput>, String> {
    [STORE, HUB]
        .into_iter()
        .map(|(file, url)| {
            let path = root.join("modules/source/local").join(file);
            let body = crate::filesystem::read_public(root, &path)?;
            Ok(SourceInput {
                label: file.to_owned(),
                url: url.to_owned(),
                content: project(url, &body).map_err(|e| format!("{file}: {e}"))?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upstream_paths_can_change_without_matching_other_origins() {
        let scoped = scoped_pattern(r"^https?:\/\/script\.hub\/preview|v2", false).unwrap();
        let matcher = regex::Regex::new(&scoped).unwrap();
        assert!(matcher.is_match("https://script.hub/v2"));
        assert!(!matcher.is_match("https://elsewhere.example/v2"));
        let bare =
            regex::Regex::new(&scoped_pattern(r"^https?:\/\/sub\.store", true).unwrap()).unwrap();
        assert!(bare.is_match("https://sub.store/api"));
        assert!(!bare.is_match("https://sub.store.example/api"));
        assert!(scoped_pattern(r"^https?:\/\/script\.hub\/x)|.*", false).is_none());
    }
}
