use std::collections::BTreeSet;

use base64::Engine;
use serde::Deserialize;

const REQUIRED_ZONES: &[&str] = &[
    "home.arpa",
    "internal",
    "lan",
    "local",
    "localdomain",
    "localhost",
];

#[derive(Deserialize)]
struct Contents {
    #[serde(rename = "type")]
    kind: String,
    path: String,
    encoding: String,
    size: u64,
    content: String,
}

pub(super) fn filtered(response: &str) -> Result<BTreeSet<String>, String> {
    let file: Contents =
        serde_json::from_str(response).map_err(|_| "invalid V2Fly repository contents response")?;
    if file.kind != "file"
        || file.path != "data/private"
        || file.encoding != "base64"
        || file.size == 0
        || file.size > super::MAX_SOURCE_BYTES
    {
        return Err("unexpected V2Fly repository file metadata".into());
    }
    let encoded: Vec<_> = file
        .content
        .bytes()
        .filter(|byte| !matches!(*byte, b'\r' | b'\n'))
        .collect();
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "invalid V2Fly repository file encoding")?;
    // The raw feed has no EOF marker; the complete API envelope supplies its byte count.
    if decoded.len() as u64 != file.size {
        return Err("incomplete V2Fly repository file".into());
    }
    let source = std::str::from_utf8(&decoded).map_err(|_| "non-UTF-8 V2Fly repository file")?;
    filtered_list(source)
}

fn filtered_list(source: &str) -> Result<BTreeSet<String>, String> {
    let mut hosts = BTreeSet::new();
    let mut zones = BTreeSet::new();
    for line in source.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_ascii_whitespace();
        let rule = parts.next().ok_or("missing V2Fly private rule")?;
        if parts.any(|attribute| {
            !attribute.starts_with(['@', '&'])
                || attribute.len() < 2
                || !attribute.bytes().all(|byte| byte.is_ascii_graphic())
        }) {
            return Err("invalid V2Fly private rule attributes".into());
        }
        let (kind, name) = rule.split_once(':').unwrap_or(("domain", rule));
        let exact = match kind {
            "full" => true,
            "domain" => false,
            // These are never evaluated or expanded into additional downloads.
            "regexp" | "keyword" | "include" => continue,
            _ => return Err("unreviewed V2Fly private rule format".into()),
        };
        let name = name.to_ascii_lowercase();
        if !super::valid_name(&name) {
            return Err("invalid hostname in V2Fly private source".into());
        }
        if !exact && REQUIRED_ZONES.contains(&name.as_str()) {
            zones.insert(name.clone());
        }
        // Private zones come from IANA/Sukka; this feed adds only exact device names.
        if super::DEVICE_NAMES.contains(&name.as_str()) {
            hosts.insert(name);
        }
    }
    if hosts.is_empty() || !REQUIRED_ZONES.iter().all(|zone| zones.contains(*zone)) {
        return Err("V2Fly private source is missing required local names".into());
    }
    Ok(hosts)
}

#[cfg(test)]
pub(super) fn test_source() -> String {
    REQUIRED_ZONES.join("\n") + "\nmy.router\nrouter.ctc\nleike.cc\n"
}

#[cfg(test)]
pub(super) fn test_document(source: &str) -> String {
    serde_json::json!({
        "type": "file",
        "path": "data/private",
        "encoding": "base64",
        "size": source.len(),
        "content": base64::engine::general_purpose::STANDARD.encode(source),
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_only_exact_reviewed_device_names() {
        let source = test_source()
            + "full:unifi\ndomain:routerlogin.net @local\nfull:pi.hole &private\n"
            + "full:home\n8.8.8.in-addr.arpa\nplex.direct\nts.net\n"
            + "localhost.ptlogin2.qq.com\nfull:local.adguard.org\n"
            + "regexp:.*\nkeyword:router\ninclude:google\n";
        let expected = [
            "leike.cc",
            "my.router",
            "pi.hole",
            "router.ctc",
            "routerlogin.net",
            "unifi",
        ]
        .map(String::from)
        .into_iter()
        .collect();
        assert_eq!(filtered_list(&source).unwrap(), expected);
    }

    #[test]
    fn rejects_wrong_documents_incomplete_baselines_and_malformed_rows() {
        for bad in [
            "<html>Access denied</html>".into(),
            test_source().replace("home.arpa\n", ""),
            test_source().replace("lan\n", "full:lan\n"),
            test_source() + "full:*.lan\n",
            test_source() + "full:bad..lan\n",
            test_source() + "full:unifi = server:8.8.8.8\n",
            test_source() + "full:unifi bad-attribute\n",
            test_source() + "host:unifi\n",
        ] {
            assert!(filtered_list(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn canonicalizes_comments_case_crlf_and_duplicates_without_broadening() {
        let source =
            test_source() + "full:MY.ROUTER # local admin\r\nleike.cc\r\ndomain:LEIKE.CC\r\n";
        assert_eq!(
            filtered_list(&source).unwrap(),
            filtered_list(&test_source()).unwrap()
        );
    }

    #[test]
    fn validates_the_complete_file_envelope_before_allowing_updates() {
        let document = test_document(&test_source());
        let mut file: serde_json::Value = serde_json::from_str(&document).unwrap();
        file["content"] = (file["content"].as_str().unwrap().to_string() + "\n").into();
        assert_eq!(filtered(&file.to_string()).unwrap().len(), 3);
        for (key, bad) in [
            ("type", serde_json::json!("symlink")),
            ("path", serde_json::json!("data/google")),
            ("encoding", serde_json::json!("none")),
            ("size", serde_json::json!(0)),
            ("size", serde_json::json!(65537)),
            ("content", serde_json::json!("!!!!")),
            (
                "content",
                serde_json::json!(base64::engine::general_purpose::STANDARD.encode("lan\n")),
            ),
        ] {
            let mut invalid = file.clone();
            invalid[key] = bad;
            assert!(filtered(&invalid.to_string()).is_err(), "{key}");
        }
        assert!(filtered(&document[..document.len() - 1]).is_err());
        let mut truncated = file.clone();
        truncated["content"] = base64::engine::general_purpose::STANDARD
            .encode(test_source().replace("router.ctc\nleike.cc\n", ""))
            .into();
        assert!(filtered(&truncated.to_string()).is_err());
        let removed = test_source().replace("leike.cc\n", "");
        assert_eq!(filtered(&test_document(&removed)).unwrap().len(), 2);
    }
}
