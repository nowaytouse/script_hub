//! Upstream bundle merge — local-only processing for Rust-provided sources.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use chrono::Local;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;

use crate::smart_cleanup::{
    ModuleSectionPub, format_header_pub, format_module_pub, merge_mitm_hosts_pub,
};

static META_LINE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)^#!\s*([A-Za-z0-9_-]+)\s*[=:]\s*(.*)$").unwrap());
static SCRIPT_LABEL_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^(.+?)\s*=\s*type=").unwrap());
static HOSTNAME_PREFIX_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^hostname\s*=\s*").unwrap());
static HOSTNAME_APPEND_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)^(%APPEND%|%INSERT%)\s*").unwrap());

const WEIBO_SECTION_ORDER: &[&str] = &[
    "Rule",
    "URL Rewrite",
    "Body Rewrite",
    "Map Local",
    "Script",
    "MITM",
];

const DEVTOOLS_MITM_EXCLUSIONS: &[&str] =
    &["-github.com", "-api.github.com", "-*.githubusercontent.com"];

#[derive(Debug, Deserialize)]
pub struct SourceInput {
    pub label: String,
    pub url: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct MergeBundleRequest {
    pub op: String,
    pub output_path: String,
    pub header_meta: HashMap<String, String>,
    pub provenance_comment: Option<String>,
    pub sources: Vec<SourceInput>,
    #[serde(default)]
    pub content_replacements: Vec<[String; 2]>,
    #[serde(default)]
    pub extra_header_lines: Vec<String>,
    pub youtube_adblock_output: Option<String>,
    #[serde(default)]
    pub section_order: Vec<String>,
    #[serde(default)]
    pub script_path_suffix_map: HashMap<String, String>,
    pub script_raw_prefix: Option<String>,
    pub scripts_dir: Option<String>,
    pub script_hub_local: Option<String>,
}

fn parse_module(text: &str) -> (HashMap<String, String>, Vec<ModuleSectionPub>) {
    let mut header_meta = HashMap::new();
    let mut sections = Vec::new();
    let mut current_name = String::new();
    let mut current_lines: Vec<String> = Vec::new();
    let mut in_body = false;

    for raw in text.lines() {
        let stripped = raw.trim();
        if stripped.starts_with('[') && stripped.ends_with(']') && !stripped.starts_with('#') {
            if !current_name.is_empty() {
                sections.push(ModuleSectionPub {
                    Name: current_name.clone(),
                    Lines: current_lines.clone(),
                });
            }
            current_name = stripped[1..stripped.len() - 1].to_string();
            current_lines.clear();
            in_body = true;
            continue;
        }
        if !in_body {
            if let Some(caps) = META_LINE_RE.captures(stripped) {
                header_meta.insert(caps[1].to_lowercase(), caps[2].trim().to_string());
            }
            continue;
        }
        if !current_name.is_empty() {
            current_lines.push(raw.trim_end_matches('\r').to_string());
        }
    }
    if !current_name.is_empty() {
        sections.push(ModuleSectionPub {
            Name: current_name,
            Lines: current_lines,
        });
    }
    (header_meta, sections)
}

pub fn module_metadata_pub(text: &str) -> HashMap<String, String> {
    parse_module(text).0
}

fn apply_replacements(mut text: String, replacements: &[[String; 2]]) -> String {
    for pair in replacements {
        if pair.len() == 2 {
            text = text.replace(&pair[0], &pair[1]);
        }
    }
    text
}

fn combine_metadata(parts: &[(String, HashMap<String, String>)]) -> (String, String, String) {
    let mut arg_tokens: Vec<String> = Vec::new();
    let mut args_desc_blocks: Vec<String> = Vec::new();
    let mut desc_blocks: Vec<String> = Vec::new();
    let nl_re = Regex::new(r"\n+").unwrap();

    for (label, meta) in parts {
        let mod_name = meta.get("name").map(|s| s.as_str()).unwrap_or(label);

        if let Some(raw_args) = meta.get("arguments") {
            for token in raw_args.split(',') {
                let token = token.trim();
                if !token.is_empty() && !arg_tokens.iter().any(|t| t == token) {
                    arg_tokens.push(token.to_string());
                }
            }
        }
        if let Some(raw) = meta.get("arguments-desc") {
            let clean = nl_re.replace_all(raw.trim(), r"\n");
            args_desc_blocks.push(format!("[{}]\\n{}", mod_name, clean));
        }
        if let Some(raw) = meta.get("desc") {
            let clean = nl_re.replace_all(raw.trim(), r"\n");
            desc_blocks.push(format!("✦ {}: {}", mod_name, clean));
        }
    }

    let beautify = |blocks: &[String], max_len: usize, prefix: &str| -> String {
        if blocks.is_empty() {
            return String::new();
        }
        let combined = format!("{}{}", prefix, blocks.join("\\n\\n"));
        if combined.len() > max_len {
            format!(
                "{}…\\n(see upstream docs for full details)",
                &combined[..max_len]
            )
        } else {
            combined
        }
    };

    (
        arg_tokens.join(","),
        beautify(&args_desc_blocks, 3800, ""),
        beautify(&desc_blocks, 3800, "【Bundled Modules】\\n"),
    )
}

fn order_sections(merged: &HashMap<String, Vec<String>>, order: &[&str]) -> Vec<ModuleSectionPub> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for &name in order {
        if let Some(lines) = merged.get(name) {
            if !lines.is_empty() {
                result.push(ModuleSectionPub {
                    Name: name.to_string(),
                    Lines: lines.clone(),
                });
                seen.insert(name.to_string());
            }
        }
    }
    let mut extra: Vec<String> = merged
        .keys()
        .filter(|k| !seen.contains(*k) && !merged[*k].is_empty())
        .cloned()
        .collect();
    extra.sort();
    for name in extra {
        result.push(ModuleSectionPub {
            Name: name.clone(),
            Lines: merged[&name].clone(),
        });
    }
    result
}

fn load_preserved_rules(path: &Path) -> HashMap<String, Vec<String>> {
    if !path.is_file() {
        return HashMap::new();
    }
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let (_, sections) = parse_module(&text);
    let mut preserved = HashMap::new();
    for sec in sections {
        if sec.Name == "Rule" && !sec.Lines.is_empty() {
            preserved.insert(sec.Name.clone(), sec.Lines);
        }
    }
    preserved
}

fn write_module(
    path: &Path,
    header_lines: &[String],
    sections: &[ModuleSectionPub],
    dedupe: bool,
) -> bool {
    let content = format_module_pub(header_lines, sections, dedupe);
    if content.contains("%INSERT%") {
        eprintln!(
            "\x1b[0;31m[ERROR]\x1b[0m merged module contains %%INSERT%%: {:?}",
            path
        );
        return false;
    }
    let validation = crate::promax::validation::validate_surge_module(&content);
    if !validation.is_empty() {
        let summary = validation
            .iter()
            .take(5)
            .map(|error| format!("line {} {}", error.line, error.code))
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!(
            "\x1b[0;31m[ERROR]\x1b[0m refusing invalid merged module {:?}: {}",
            path, summary
        );
        return false;
    }
    crate::safe_write_file_internal(path, &content, true)
}

fn merge_upstream(req: &MergeBundleRequest) -> bool {
    let mut parsed_parts: Vec<(String, HashMap<String, String>)> = Vec::new();
    let mut merged_sections: HashMap<String, Vec<String>> = HashMap::new();
    let mut used_sources: Vec<(String, String)> = Vec::new();

    for src in &req.sources {
        let text = apply_replacements(src.content.clone(), &req.content_replacements);
        let (meta, sections) = parse_module(&text);
        parsed_parts.push((src.label.clone(), meta));
        used_sources.push((src.label.clone(), src.url.clone()));

        for sec in sections {
            let filtered: Vec<String> = sec
                .Lines
                .iter()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            if let Some(existing) = merged_sections.get_mut(&sec.Name) {
                if !existing.is_empty()
                    && !filtered.is_empty()
                    && existing.last() != Some(&String::new())
                {
                    existing.push(String::new());
                }
                existing.extend(filtered);
            } else {
                merged_sections.insert(sec.Name, filtered);
            }
        }
    }

    if parsed_parts.is_empty() {
        eprintln!(
            "\x1b[0;31m[ERROR]\x1b[0m no upstream modules for {:?}",
            req.output_path
        );
        return false;
    }

    let (combined_args, combined_args_desc, combined_desc) = combine_metadata(&parsed_parts);
    let mut out_meta = req.header_meta.clone();
    out_meta.insert(
        "date".to_string(),
        Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    );
    if !combined_args.is_empty() {
        out_meta.insert("arguments".to_string(), combined_args);
    }
    if !combined_args_desc.is_empty() {
        out_meta.insert("arguments-desc".to_string(), combined_args_desc);
    }
    if !combined_desc.is_empty() {
        let existing = out_meta.get("desc").cloned().unwrap_or_default();
        let desc = if existing.trim().is_empty() {
            combined_desc
        } else {
            format!("{}\\n\\n{}", existing, combined_desc)
        };
        out_meta.insert("desc".to_string(), desc);
    }

    let mut section_list: Vec<ModuleSectionPub> = merged_sections
        .into_iter()
        .map(|(name, lines)| ModuleSectionPub {
            Name: name,
            Lines: lines,
        })
        .collect();
    section_list = merge_mitm_hosts_pub(&section_list);

    let mut extra = req.extra_header_lines.clone();
    if let Some(ref c) = req.provenance_comment {
        extra.push(c.clone());
    }
    extra.push("# Upstream modules merged by create/processor/merge_bundles.rs (Rust)".to_string());
    for (label, url) in &used_sources {
        extra.push(format!("# - {}: {}", label, url));
    }

    let header_lines = format_header_with_extra(&out_meta, &extra);
    if let Some(parent) = Path::new(&req.output_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    write_module(
        Path::new(&req.output_path),
        &header_lines,
        &section_list,
        true,
    )
}

fn format_header_with_extra(meta: &HashMap<String, String>, extra: &[String]) -> Vec<String> {
    let mut lines = format_header_pub(meta);
    lines.extend(extra.iter().cloned());
    lines
}

fn promax_source_merge(req: &MergeBundleRequest) -> bool {
    let out = Path::new(&req.output_path);
    let preserved = load_preserved_rules(out);
    if !merge_upstream(req) {
        return false;
    }
    if preserved.is_empty() {
        return true;
    }
    rewrite_preserved_internal(out, &preserved, WEIBO_SECTION_ORDER)
}

fn rewrite_preserved_internal(
    path: &Path,
    preserved: &HashMap<String, Vec<String>>,
    section_order: &[&str],
) -> bool {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let (meta, sections) = parse_module(&text);
    let mut merged: HashMap<String, Vec<String>> = HashMap::new();
    for sec in sections {
        merged.insert(sec.Name, sec.Lines);
    }
    for (name, lines) in preserved {
        merged.insert(name.clone(), lines.clone());
    }
    let ordered = order_sections(&merged, section_order);
    let ordered = merge_mitm_hosts_pub(&ordered);
    let extra = vec!["# PROMAX build source — install head_expanse/PROMAX only".to_string()];
    let header = format_header_with_extra(&meta, &extra);
    write_module(path, &header, &ordered, true)
}

fn merge_youtube(req: &MergeBundleRequest) -> bool {
    if req.sources.is_empty() {
        return false;
    }
    let text = req.sources[0].content.clone();
    let (meta, sections) = parse_module(&text);

    let mut cleaned_sections: Vec<ModuleSectionPub> = Vec::new();
    let mut mitm_hosts: HashSet<String> = HashSet::new();
    let mut adblock_rules: Vec<String> = Vec::new();
    let mut adblock_map_locals: Vec<String> = Vec::new();
    let mut adblock_mitm_hosts: HashSet<String> = HashSet::new();

    for sec in sections {
        if sec.Name == "Rule" {
            adblock_rules.extend(sec.Lines);
            continue;
        }
        if sec.Name == "Map Local" {
            adblock_map_locals.extend(sec.Lines);
            continue;
        }
        cleaned_sections.push(sec.clone());
        if sec.Name == "MITM" {
            for line in &sec.Lines {
                let trimmed = line.trim();
                if trimmed.to_lowercase().starts_with("hostname") {
                    let hosts_str = HOSTNAME_PREFIX_RE.replace(trimmed, "");
                    for h in hosts_str.split(',') {
                        let h_clean = h.trim();
                        if h_clean.is_empty() {
                            continue;
                        }
                        if h_clean == "*.googlevideo.com" {
                            adblock_mitm_hosts.insert(h_clean.to_string());
                        } else {
                            mitm_hosts.insert(h_clean.to_string());
                            if h_clean.contains("youtubei") {
                                adblock_mitm_hosts.insert(h_clean.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    let mut final_sections: Vec<ModuleSectionPub> = Vec::new();
    let mut mitm_merged = false;
    for sec in cleaned_sections {
        if sec.Name == "MITM" {
            if !mitm_merged && !mitm_hosts.is_empty() {
                let mut sorted: Vec<String> = mitm_hosts.iter().cloned().collect();
                sorted.sort();
                final_sections.push(ModuleSectionPub {
                    Name: "MITM".to_string(),
                    Lines: vec![format!("hostname = %APPEND% {}", sorted.join(", "))],
                });
                mitm_merged = true;
            }
        } else {
            final_sections.push(sec);
        }
    }

    let mut header = req.header_meta.clone();
    if let Some(args) = meta.get("arguments") {
        let mut a = args.clone();
        a = a.replace("字幕翻译语言:off", "字幕翻译语言:\"zh-Hans\"");
        a = a.replace("歌词翻译语言:off", "歌词翻译语言:\"zh-Hans\"");
        a = a.replace("屏蔽Shorts按钮:false", "屏蔽Shorts按钮:true");
        a = a.replace("屏蔽上传按钮:false", "屏蔽上传按钮:true");
        a = a.replace("屏蔽选段按钮:false", "屏蔽选段按钮:true");
        a = a.replace("增加画中画:false", "增加画中画:true");
        header.insert("arguments".to_string(), a);
    }
    if let Some(v) = meta.get("arguments-desc") {
        header.insert("arguments-desc".to_string(), v.clone());
    }

    let extra = vec![
        "# Upstream module processed by create/processor/merge_bundles.rs".to_string(),
        "# - Stripped: [Rule], [Map Local] and *.googlevideo.com from MITM".to_string(),
    ];
    let header_lines = format_header_with_extra(&header, &extra);
    if let Some(parent) = Path::new(&req.output_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let content = format_module_pub(&header_lines, &final_sections, true);
    if content.trim().len() < 100 {
        eprintln!("\x1b[0;31m[ERROR]\x1b[0m YouTube bundle too small");
        return false;
    }
    if !crate::safe_write_file_internal(Path::new(&req.output_path), &content, true) {
        return false;
    }

    if let Some(ref adblock_out) = req.youtube_adblock_output {
        let mut adblock_lines: Vec<String> = vec![
            "#!name=YouTube 去广告".to_string(),
            "#!desc=YouTube 广告过滤 (自动从 Maasea 上游提取并合并进入 PROMAX)".to_string(),
            "#!category=🪐 local".to_string(),
            String::new(),
        ];
        if !adblock_rules.is_empty() {
            adblock_lines.push("[Rule]".to_string());
            adblock_lines.extend(adblock_rules);
            adblock_lines.push(String::new());
        }
        if !adblock_map_locals.is_empty() {
            adblock_lines.push("[Map Local]".to_string());
            adblock_lines.extend(adblock_map_locals);
            adblock_lines.push(String::new());
        }
        if !adblock_mitm_hosts.is_empty() {
            let mut sorted: Vec<String> = adblock_mitm_hosts.into_iter().collect();
            sorted.sort();
            adblock_lines.push("[MITM]".to_string());
            adblock_lines.push(format!(
                "hostname = %APPEND% {}, -github.com, -api.github.com, -*.githubusercontent.com",
                sorted.join(", ")
            ));
            adblock_lines.push(String::new());
        }
        if adblock_lines.len() > 4 {
            let adblock_content = adblock_lines.join("\n").trim().to_string() + "\n";
            if let Some(parent) = Path::new(adblock_out).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            crate::safe_write_file_internal(Path::new(adblock_out), &adblock_content, true);
        }
    }
    true
}

fn pin_script_paths_in_text(
    text: &str,
    suffix_map: &HashMap<String, String>,
    raw_prefix: &str,
    scripts_dir: &str,
) -> String {
    let mut result = text.to_string();
    for (upstream_suffix, local_name) in suffix_map {
        let local_path = Path::new(scripts_dir).join(local_name);
        if !local_path.is_file() {
            continue;
        }
        let canonical = format!("{}{}", raw_prefix, local_name);
        let re_str = format!(
            r"script-path=https?://[^\s,]*{}",
            regex::escape(upstream_suffix)
        );
        if let Ok(re) = Regex::new(&re_str) {
            result = re
                .replace_all(&result, format!("script-path={}", canonical))
                .to_string();
        }
        let re_str2 = format!(r"script-path=https?://[^\s,]*{}", regex::escape(local_name));
        if let Ok(re) = Regex::new(&re_str2) {
            result = re
                .replace_all(&result, format!("script-path={}", canonical))
                .to_string();
        }
    }
    result
}

fn finalize_devtools(path: &Path) -> bool {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let (meta, sections) = parse_module(&text);
    let mut merged: HashMap<String, Vec<String>> = HashMap::new();
    for sec in sections {
        merged.insert(sec.Name, sec.Lines);
    }

    let script_lines = merged.get("Script").cloned().unwrap_or_default();
    let mut script_labels = HashSet::new();
    for line in &script_lines {
        if let Some(caps) = SCRIPT_LABEL_RE.captures(line.trim()) {
            script_labels.insert(caps[1].trim().to_string());
        }
    }
    for required in [
        "Sub-Store Core",
        "Sub-Store Simple",
        "{{{sync}}}",
        "{{{produce}}}",
    ] {
        if !script_labels.contains(required) {
            eprintln!(
                "\x1b[0;33m[WARN]\x1b[0m Sub-Store scripts incomplete (missing: {})",
                required
            );
        }
    }

    let mut hosts: HashSet<String> = HashSet::new();
    if let Some(mitm) = merged.get("MITM") {
        for line in mitm {
            let trimmed = line.trim();
            if trimmed.to_lowercase().starts_with("hostname") {
                let stripped = HOSTNAME_PREFIX_RE.replace(trimmed, "");
                let part = HOSTNAME_APPEND_RE.replace_all(&stripped, "");
                for token in part.split(',') {
                    let token = token.trim();
                    if !token.is_empty() && token != "%INSERT%" && token != "%APPEND%" {
                        hosts.insert(token.to_string());
                    }
                }
            }
        }
    }
    for h in DEVTOOLS_MITM_EXCLUSIONS {
        hosts.insert(h.to_string());
    }
    let mut inclusions: Vec<String> = hosts
        .iter()
        .filter(|h| !h.starts_with('-'))
        .cloned()
        .collect();
    let mut exclusions: Vec<String> = hosts
        .iter()
        .filter(|h| h.starts_with('-'))
        .cloned()
        .collect();
    inclusions.sort();
    exclusions.sort();
    inclusions.extend(exclusions);
    merged.insert(
        "MITM".to_string(),
        vec![format!("hostname = %APPEND% {}", inclusions.join(", "))],
    );

    let order = &[
        "General",
        "Host",
        "Rule",
        "URL Rewrite",
        "Map Local",
        "Script",
        "Body Rewrite",
        "Header Rewrite",
        "MITM",
    ];
    let section_list = order_sections(&merged, order);
    let header = format_header_pub(&meta);
    write_module(path, &header, &section_list, false)
}

fn devtools_merge(req: &MergeBundleRequest) -> bool {
    if let Some(ref local) = req.script_hub_local {
        let path = Path::new(local);
        if path.is_file() {
            let raw_prefix = req.script_raw_prefix.as_deref().unwrap_or("");
            let scripts_dir = req.scripts_dir.as_deref().unwrap_or("");
            if !req.script_path_suffix_map.is_empty()
                && !raw_prefix.is_empty()
                && !scripts_dir.is_empty()
            {
                let text = std::fs::read_to_string(path).unwrap_or_default();
                let pinned = pin_script_paths_in_text(
                    &text,
                    &req.script_path_suffix_map,
                    raw_prefix,
                    scripts_dir,
                );
                let _ = crate::safe_write_file_internal(path, &pinned, true);
            }
        }
    }
    if !merge_upstream(req) {
        return false;
    }
    let path = Path::new(&req.output_path);
    let raw_prefix = req.script_raw_prefix.as_deref().unwrap_or("");
    let scripts_dir = req.scripts_dir.as_deref().unwrap_or("");
    if !req.script_path_suffix_map.is_empty() && !raw_prefix.is_empty() && !scripts_dir.is_empty() {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let pinned =
            pin_script_paths_in_text(&text, &req.script_path_suffix_map, raw_prefix, scripts_dir);
        if !crate::safe_write_file_internal(path, &pinned, true) {
            return false;
        }
    }
    finalize_devtools(path)
}

pub fn process_merge_bundle(req: &MergeBundleRequest) -> bool {
    match req.op.as_str() {
        "upstream_merge" => merge_upstream(req),
        "promax_source_merge" => promax_source_merge(req),
        "youtube_merge" => merge_youtube(req),
        "devtools_merge" => devtools_merge(req),
        other => {
            eprintln!(
                "\x1b[0;31m[ERROR]\x1b[0m unknown merge_bundles op: {}",
                other
            );
            false
        }
    }
}

pub fn run_merge_bundle_json(json: &str) -> bool {
    match serde_json::from_str::<MergeBundleRequest>(json) {
        Ok(req) => process_merge_bundle(&req),
        Err(e) => {
            eprintln!("\x1b[0;31m[ERROR]\x1b[0m merge_bundles JSON: {}", e);
            false
        }
    }
}
