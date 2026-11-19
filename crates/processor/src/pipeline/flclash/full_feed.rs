//! Consume protected domain rules with the native FlClash Preview contract.

use super::{
    MAIN_MODULE, MAX_PROMAX_BYTES, OWNER, validate_native_rewrites, validate_promax_load_budget,
};
use crate::dns_name::{ancestors, is_dotted_ascii_host};
use crate::filesystem::read_public_bounded;
use crate::promax::heidai_feed::HEIDAI_FULL;
use std::collections::HashSet;
use std::path::Path;

pub(in crate::pipeline) const LOCAL_MARKER: &str =
    "# Script Hub local export · 217heidai complete input";

pub(in crate::pipeline) fn load_base(root: &Path) -> Result<String, String> {
    read_public_bounded(root, &root.join(MAIN_MODULE), MAX_PROMAX_BYTES)
}

pub(in crate::pipeline) fn positive_mitm_hosts(base: &str) -> impl Iterator<Item = &str> {
    base.split_once("\n[MITM]\n")
        .into_iter()
        .flat_map(|(_, section)| section.lines())
        .take_while(|line| !line.starts_with('['))
        .filter_map(|line| line.strip_prefix("hostname = %APPEND% "))
        .flat_map(|hosts| hosts.split(',').map(str::trim))
        .filter(|host| !host.starts_with('-') && is_dotted_ascii_host(host))
}

pub(in crate::pipeline) fn append_full_feed(
    base: &str,
    version: &str,
    domains: &[&str],
) -> Result<(String, usize), String> {
    if !base.lines().any(|line| line == OWNER) {
        return Err("Local export requires the repository-generated FlClash PROMAX".into());
    }
    let rule_start = base.find("\n[Rule]\n").ok_or("Missing PROMAX rules")? + "\n[Rule]\n".len();
    let rule_end = base[rule_start..]
        .find("\n[")
        .map_or(base.len(), |index| rule_start + index);
    let rules = &base[rule_start..rule_end];
    let suffixes = rules
        .lines()
        .filter_map(|row| {
            let (host, policy) = row.strip_prefix("DOMAIN-SUFFIX,")?.split_once(',')?;
            super::is_block_policy(policy).then_some(host)
        })
        .collect::<HashSet<_>>();
    let mut additions = String::new();
    let mut count = 0;
    for host in domains
        .iter()
        .filter(|host| !ancestors(host).any(|name| suffixes.contains(name)))
    {
        additions.push_str("DOMAIN-SUFFIX,");
        additions.push_str(host);
        additions.push_str(",REJECT\n");
        count += 1;
    }
    let (_, rule_prefix) = base[..rule_start]
        .split_once('\n')
        .ok_or("Missing PROMAX metadata")?;
    let header = format!(
        "#!name=FlClash Preview · AdBlock PROMAX (217heidai Full Local)\n{LOCAL_MARKER}\n# Complete upstream: {HEIDAI_FULL}\n# Upstream version: {version}\n"
    );
    let mut result = String::with_capacity(base.len() + additions.len() + header.len());
    result.push_str(&header);
    result.push_str(rule_prefix);
    result.push_str(rules);
    if !rules.ends_with('\n') {
        result.push('\n');
    }
    result.push_str(&additions);
    result.push_str(&base[rule_end..]);
    let active = rules
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .count()
        + count
        + base[rule_end..]
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.starts_with(['#', '[']))
            .count();
    validate_promax_load_budget(result.len(), active)?;
    validate_native_rewrites(&result)?;
    Ok((result, count))
}
