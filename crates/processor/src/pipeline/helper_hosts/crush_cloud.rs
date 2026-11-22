//! Project a single reviewed Host scope, never the upstream module as a whole.
use std::path::Path;

pub(crate) const SOURCE: &str =
    "https://raw.githubusercontent.com/crushcloud91/Crush-Cloud/main/crushcloud-dns.sgmodule";
pub(super) const SNAPSHOT: &str = "modules/source/local/crush-cloud-dns.txt";
pub(crate) const ZONE: &str = "*.node.daacc.org";

pub(crate) struct Mapping {
    pub(crate) resolver: String,
    path: String,
}

impl Mapping {
    pub(crate) fn host_line(&self) -> String {
        format!("{ZONE} = server:https://{}/{}", self.resolver, self.path)
    }
}

pub(crate) fn parse(source: &str) -> Result<Mapping, String> {
    if source.len() as u64 > super::MAX_SOURCE_BYTES {
        return Err("Crush Cloud source exceeds its byte budget".into());
    }
    let mut in_hosts = false;
    let mut mapping = None;
    for line in source.lines().map(str::trim) {
        if line.starts_with('[') {
            in_hosts = line.eq_ignore_ascii_case("[Host]");
            continue;
        }
        let Some((zone, rhs)) = line.split_once('=') else {
            continue;
        };
        if !in_hosts || !zone.trim().eq_ignore_ascii_case(ZONE) {
            continue;
        }
        if mapping.is_some() {
            return Err("ambiguous Crush Cloud Host mapping".into());
        }
        let (host, path) = rhs
            .trim()
            .strip_prefix("server:https://")
            .and_then(|url| url.split_once('/'))
            .ok_or("expected an HTTPS DoH endpoint")?;
        let host = host.to_ascii_lowercase();
        // A resolver inside the delegated zone would depend on itself for bootstrap.
        if !super::valid_name(&host)
            || !host.ends_with(".daacc.org")
            || host == "node.daacc.org"
            || host.ends_with(".node.daacc.org")
            || path.len() > 256
            || !path.split('/').all(|part| {
                !part.is_empty()
                    && part != "."
                    && part != ".."
                    && part
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
            })
        {
            return Err("Crush Cloud endpoint is outside the reviewed resolver scope".into());
        }
        mapping = Some(Mapping {
            resolver: host,
            path: path.to_owned(),
        });
    }
    mapping.ok_or_else(|| "missing reviewed Crush Cloud Host mapping".into())
}

pub(super) fn publish(root: &Path, source: &str) -> Result<(), String> {
    let mapping = parse(source)?;
    super::write_snapshot(
        root,
        SNAPSHOT,
        &format!(
            "# Reviewed split DNS from {SOURCE}\n[Host]\n{}\n",
            mapping.host_line()
        ),
    )
}

pub(crate) fn load(root: &Path) -> Result<Mapping, String> {
    let source = crate::filesystem::read_public_bounded(
        root,
        &root.join(SNAPSHOT),
        super::MAX_SOURCE_BYTES as usize,
    )?;
    parse(&source)
}
