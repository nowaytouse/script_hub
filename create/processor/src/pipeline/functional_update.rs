//! Rust-owned upstream refresh for functional modules and PROMAX build inputs.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::merge_bundles::{MergeBundleRequest, SourceInput, process_merge_bundle};
use crate::pipeline::custom_sources::{self, CustomSource, SourceCategory};
use crate::promax::artifact::{ArtifactKind, GeneratedArtifact, publish_generated_artifacts};
use crate::promax::source::{DownloadBatch, DownloadConfig, Downloader};

#[derive(Clone, Copy)]
struct RemoteSource {
    label: &'static str,
    url: &'static str,
    required: bool,
}

const LOCAL_SOURCES: &[(&str, &str)] = &[
    (
        "XWebAds.sgmodule",
        "https://raw.githubusercontent.com/fmz200/wool_scripts/refs/heads/main/Surge/module/XWebAds.module",
    ),
    (
        "Wool.ADBlock.sgmodule",
        "https://raw.githubusercontent.com/fmz200/wool_scripts/refs/heads/main/Surge/module/blockAds.module",
    ),
    (
        "BiliBili.ADBlock.sgmodule",
        "https://github.com/Biliverse/ADBlock/releases/latest/download/BiliBili.ADBlock.sgmodule",
    ),
    (
        "[Sukka] Enhance Better ADBlock for Surge.sgmodule",
        "https://ruleset.skk.moe/Modules/sukka_enhance_adblock.sgmodule",
    ),
    (
        "sukka_url_redirect.sgmodule",
        "https://ruleset.skk.moe/Modules/sukka_url_redirect.sgmodule",
    ),
    (
        "script_hub.surge.sgmodule",
        "https://raw.githubusercontent.com/Script-Hub-Org/Script-Hub/main/modules/script-hub.surge.sgmodule",
    ),
];

const APPLE: &[RemoteSource] = &[
    RemoteSource {
        label: "iRingo.Maps",
        url: "https://github.com/NSRingo/GeoServices/releases/latest/download/iRingo.Maps.sgmodule",
        required: true,
    },
    RemoteSource {
        label: "iRingo.WeatherKit",
        url: "https://github.com/NSRingo/WeatherKit/releases/latest/download/iRingo.WeatherKit.sgmodule",
        required: true,
    },
    RemoteSource {
        label: "iRingo.News",
        url: "https://github.com/NSRingo/News/releases/latest/download/iRingo.News.sgmodule",
        required: true,
    },
    RemoteSource {
        label: "iRingo.TV",
        url: "https://github.com/NSRingo/TV/releases/latest/download/iRingo.TV.sgmodule",
        required: true,
    },
];

const DUALSUBS: &[RemoteSource] = &[RemoteSource {
    label: "DualSubs.Universal",
    url: "https://github.com/DualSubs/Universal/releases/latest/download/DualSubs.Universal.sgmodule",
    required: true,
}];

const BILIBILI: &[RemoteSource] = &[
    RemoteSource {
        label: "Enhanced",
        url: "https://github.com/Biliverse/Enhanced/releases/latest/download/Biliverse.Enhanced.sgmodule",
        required: true,
    },
    RemoteSource {
        label: "Global",
        url: "https://github.com/Biliverse/Global/releases/latest/download/BiliBili.Global.sgmodule",
        required: true,
    },
    RemoteSource {
        label: "Redirect",
        url: "https://github.com/Biliverse/Redirect/releases/latest/download/BiliBili.Redirect.sgmodule",
        required: true,
    },
    RemoteSource {
        label: "Helper",
        url: "https://raw.githubusercontent.com/Maasea/sgmodule/master/Bilibili.Helper.sgmodule",
        required: false,
    },
];

const WEIBO: &[RemoteSource] = &[
    RemoteSource {
        label: "Weibo",
        url: "https://raw.githubusercontent.com/fmz200/wool_scripts/main/Surge/module/weibo.module",
        required: true,
    },
    RemoteSource {
        label: "Intl",
        url: "https://raw.githubusercontent.com/iab0x00/ProxyRules/main/Rewrite/WeiboIntl.sgmodule",
        required: true,
    },
];

const WOOL: &[RemoteSource] = &[
    RemoteSource {
        label: "Xiaohongshu",
        url: "https://raw.githubusercontent.com/fmz200/wool_scripts/main/Surge/module/split/partX/Xiaohongshu.sgmodule",
        required: true,
    },
    RemoteSource {
        label: "Baidu",
        url: "https://raw.githubusercontent.com/fmz200/wool_scripts/main/Surge/module/split/partB/Baidu.sgmodule",
        required: true,
    },
    RemoteSource {
        label: "BaiduTieba",
        url: "https://raw.githubusercontent.com/fmz200/wool_scripts/main/Surge/module/split/partB/BaiduTieba.sgmodule",
        required: true,
    },
    RemoteSource {
        label: "NetEaseCloudMusic",
        url: "https://raw.githubusercontent.com/fmz200/wool_scripts/main/Surge/module/split/partW/NetEaseCloudMusic.sgmodule",
        required: true,
    },
    RemoteSource {
        label: "AutoNavi",
        url: "https://raw.githubusercontent.com/fmz200/wool_scripts/main/Surge/module/split/partG/AutoNavi.sgmodule",
        required: true,
    },
    RemoteSource {
        label: "Douban",
        url: "https://raw.githubusercontent.com/fmz200/wool_scripts/main/Surge/module/split/partD/Douban.sgmodule",
        required: true,
    },
    RemoteSource {
        label: "WeChatOfficial",
        url: "https://raw.githubusercontent.com/fmz200/wool_scripts/main/Surge/module/split/partW/WeChatOfficialAccount.sgmodule",
        required: true,
    },
];

const UTILITIES: &[RemoteSource] = &[
    RemoteSource {
        label: "Timecard",
        url: "https://raw.githubusercontent.com/Rabbit-Spec/Surge/Master/Module/Panel/Timecard/Moore/Timecard.sgmodule",
        required: true,
    },
    RemoteSource {
        label: "net-lsp-x",
        url: "https://raw.githubusercontent.com/xream/scripts/main/surge/modules/network-info/net-lsp-x.sgmodule",
        required: true,
    },
    RemoteSource {
        label: "Sub_Info",
        url: "https://raw.githubusercontent.com/Coldvvater/Mononoke/refs/heads/master/Surge/Module/Tool/Sub_Info.sgmodule",
        required: true,
    },
];

fn downloader() -> Downloader {
    Downloader::new(DownloadConfig {
        attempts: 3,
        timeout: Duration::from_secs(75),
        batch_timeout: Duration::from_secs(5 * 60),
        backoff: Duration::from_secs(1),
        concurrency: 4,
        max_bytes: 16 * 1024 * 1024,
        max_total_bytes: 128 * 1024 * 1024,
    })
}

fn valid_module(content: &str) -> bool {
    let content = content.trim_start();
    if content.len() < 80
        || ["<!doctype html", "<html"].iter().any(|prefix| {
            content
                .get(..prefix.len())
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
        })
    {
        return false;
    }

    let (mut has_name, mut has_section) = (false, false);
    for line in content.lines() {
        let line = line.trim();
        has_name |= line.starts_with("#!name");
        has_section |= line.starts_with('[') && line.ends_with(']');
        if has_name && has_section {
            return true;
        }
    }
    false
}

fn fetch_sources(
    downloader: &Downloader,
    specs: &[RemoteSource],
) -> Result<Vec<SourceInput>, String> {
    let batch = downloader.download_many(specs.iter().map(|spec| spec.url));
    sources_from_batch(specs, batch)
}

fn fetch_custom_sources(
    downloader: &Downloader,
    specs: &[CustomSource],
) -> Result<Vec<SourceInput>, String> {
    if specs.is_empty() {
        return Ok(Vec::new());
    }
    let batch = downloader.download_many(specs.iter().map(|spec| spec.url.as_str()));
    custom_sources_from_batch(specs, batch)
}

fn custom_sources_from_batch(
    specs: &[CustomSource],
    batch: DownloadBatch,
) -> Result<Vec<SourceInput>, String> {
    let DownloadBatch { contents, failures } = batch;
    let failure_by_url: HashMap<&str, &str> = failures
        .iter()
        .map(|failure| (failure.url.as_str(), failure.error.as_str()))
        .collect();
    let mut sources = Vec::with_capacity(specs.len());
    let mut errors = Vec::new();
    for spec in specs {
        match contents.get(spec.url.as_str()) {
            Some(content) if valid_module(content) => sources.push(SourceInput {
                label: spec.name.clone(),
                url: spec.url.clone(),
                content: content.clone(),
            }),
            Some(_) => errors.push(format!("{} returned non-module content", spec.name)),
            None => errors.push(format!(
                "{} ({})",
                spec.name,
                failure_by_url
                    .get(spec.url.as_str())
                    .copied()
                    .unwrap_or("missing download result")
            )),
        }
    }
    if errors.is_empty() {
        Ok(sources)
    } else {
        Err(errors.join("; "))
    }
}

fn sources_from_batch(
    specs: &[RemoteSource],
    batch: DownloadBatch,
) -> Result<Vec<SourceInput>, String> {
    let DownloadBatch {
        mut contents,
        failures,
    } = batch;
    let failure_by_url: HashMap<&str, &str> = failures
        .iter()
        .map(|failure| (failure.url.as_str(), failure.error.as_str()))
        .collect();
    let mut remaining_uses = HashMap::with_capacity(specs.len());
    for spec in specs {
        *remaining_uses.entry(spec.url).or_insert(0usize) += 1;
    }

    let mut sources = Vec::with_capacity(specs.len());
    let mut missing = Vec::new();
    for spec in specs {
        let is_last_use = remaining_uses.get_mut(spec.url).is_none_or(|remaining| {
            *remaining -= 1;
            *remaining == 0
        });
        if contents
            .get(spec.url)
            .is_some_and(|content| valid_module(content))
        {
            let content = if is_last_use {
                contents.remove(spec.url)
            } else {
                contents.get(spec.url).cloned()
            };
            if let Some(content) = content {
                sources.push(SourceInput {
                    label: spec.label.to_string(),
                    url: spec.url.to_string(),
                    content,
                });
                continue;
            }
        }

        let message = if contents.contains_key(spec.url) {
            format!("{} returned non-module content", spec.label)
        } else {
            let error = failure_by_url
                .get(spec.url)
                .copied()
                .unwrap_or("missing download result");
            format!("{} ({error})", spec.label)
        };
        if spec.required {
            missing.push(message);
        } else {
            eprintln!("[WARN] optional functional source skipped: {message}");
        }
    }
    if missing.is_empty() {
        Ok(sources)
    } else {
        Err(missing.join("; "))
    }
}

fn metadata(values: &[(&str, &str)]) -> HashMap<String, String> {
    values
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

type BundlePostprocessor = fn(&str) -> Result<String, String>;

struct MergeJob<'a> {
    op: &'a str,
    output: &'a str,
    header: &'a [(&'a str, &'a str)],
    specs: &'a [RemoteSource],
    custom_sources: Option<&'a [CustomSource]>,
    replacements: &'a [(&'a str, &'a str)],
    youtube_adblock_output: Option<&'a str>,
    postprocess: Option<BundlePostprocessor>,
}

fn merge(root: &Path, downloader: &Downloader, job: MergeJob<'_>) -> Result<(), String> {
    let MergeJob {
        op,
        output,
        header,
        specs,
        custom_sources,
        replacements,
        youtube_adblock_output,
        postprocess,
    } = job;
    let mut sources = fetch_sources(downloader, specs)?;
    if let Some(custom_sources) = custom_sources {
        sources.extend(fetch_custom_sources(downloader, custom_sources)?);
    }
    let request = MergeBundleRequest {
        op: op.to_string(),
        output_path: root.join(output).to_string_lossy().to_string(),
        header_meta: metadata(header),
        provenance_comment: None,
        sources,
        content_replacements: replacements
            .iter()
            .map(|(from, to)| [from.to_string(), to.to_string()])
            .collect(),
        extra_header_lines: Vec::new(),
        youtube_adblock_output: youtube_adblock_output
            .map(|path| root.join(path).to_string_lossy().to_string()),
        section_order: Vec::new(),
        script_path_suffix_map: HashMap::new(),
        script_raw_prefix: None,
        scripts_dir: None,
        script_hub_local: None,
    };
    publish_bundle(root, request, postprocess)
}

fn publish_bundle(
    root: &Path,
    mut request: MergeBundleRequest,
    postprocess: Option<BundlePostprocessor>,
) -> Result<(), String> {
    let target = std::path::PathBuf::from(&request.output_path);
    let adblock_target = request
        .youtube_adblock_output
        .as_ref()
        .map(std::path::PathBuf::from);
    let cache = root.join(".cache");
    fs::create_dir_all(&cache)
        .map_err(|error| format!("cannot create functional cache: {error}"))?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("cannot name functional staging directory: {error}"))?
        .as_nanos();
    let staging = cache.join(format!(
        "functional-bundle-{}-{timestamp}",
        std::process::id()
    ));
    fs::create_dir(&staging)
        .map_err(|error| format!("cannot create functional staging: {error}"))?;
    let result = (|| {
        let staged_output = staging.join("bundle.sgmodule");
        if request.op == "promax_source_merge" && target.exists() {
            fs::copy(&target, &staged_output)
                .map_err(|error| format!("cannot stage preserved {}: {error}", target.display()))?;
        }
        request.output_path = staged_output.to_string_lossy().to_string();
        let staged_adblock = staging.join("youtube-adblock.sgmodule");
        request.youtube_adblock_output = adblock_target
            .as_ref()
            .map(|_| staged_adblock.to_string_lossy().to_string());
        if !process_merge_bundle(&request) {
            return Err(format!("merge failed for {}", target.display()));
        }
        let content = fs::read_to_string(&staged_output)
            .map_err(|error| format!("cannot read staged bundle: {error}"))?;
        let content = match postprocess {
            Some(transform) => transform(&content)?,
            None => content,
        };
        let mut artifacts = vec![GeneratedArtifact {
            target,
            content,
            kind: ArtifactKind::Surge,
        }];
        if let Some(target) = adblock_target {
            artifacts.push(GeneratedArtifact {
                target,
                content: fs::read_to_string(&staged_adblock).map_err(|error| {
                    format!("cannot read staged YouTube adblock source: {error}")
                })?,
                kind: ArtifactKind::Surge,
            });
        }
        publish_generated_artifacts(root, &artifacts)
    })();
    if let Err(error) = fs::remove_dir_all(&staging) {
        eprintln!(
            "[WARN] cannot clean functional staging {}: {error}",
            staging.display()
        );
    }
    result
}

fn sync_local_sources(root: &Path, downloader: &Downloader) {
    let directory = root.join("modules/source/local");
    if let Err(error) = std::fs::create_dir_all(&directory) {
        eprintln!("[WARN] cannot create local functional source directory: {error}");
        return;
    }
    let batch = downloader.download_many(LOCAL_SOURCES.iter().map(|(_, url)| *url));
    for (name, url) in LOCAL_SOURCES {
        match batch.contents.get(*url) {
            Some(content) if valid_module(content) => {
                if !crate::safe_write_file_internal(&directory.join(name), content, true) {
                    eprintln!("[WARN] failed to publish local functional source: {name}");
                }
            }
            Some(_) => eprintln!("[WARN] local functional source returned invalid content: {name}"),
            None => {
                let error = batch
                    .failures
                    .iter()
                    .find(|failure| failure.url == *url)
                    .map(|failure| failure.error.as_str())
                    .unwrap_or("missing download result");
                eprintln!("[WARN] keeping existing {name}: {error}");
            }
        }
    }
}

fn run_step(label: &str, result: Result<(), String>, failures: &mut Vec<String>) {
    match result {
        Ok(()) => println!("[SUCCESS] functional bundle refreshed: {label}"),
        Err(error) => {
            eprintln!("[WARN] functional bundle kept unchanged: {label} ({error})");
            failures.push(label.to_string());
        }
    }
}

fn require_valid_custom_sources<T>(
    source_error: &Option<String>,
    action: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    if let Some(error) = source_error {
        Err(format!("invalid custom source configuration: {error}"))
    } else {
        action()
    }
}

fn strip_rule_section(content: &str) -> String {
    let mut in_rule = false;
    let mut output = String::with_capacity(content.len().saturating_add(1));
    let mut first_line = true;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_rule = trimmed.eq_ignore_ascii_case("[Rule]");
            if in_rule {
                continue;
            }
        }
        if !in_rule {
            push_output_line(&mut output, &mut first_line, line);
        }
    }
    output.push('\n');
    output
}

fn push_output_line(output: &mut String, first_line: &mut bool, line: &str) {
    if !*first_line {
        output.push('\n');
    }
    output.push_str(line);
    *first_line = false;
}

const APPLE_FUNCTIONAL_MITM_HOSTS: &[&str] = &[
    "play-cdn.itunes.apple.com",
    "play-edge-cdn.itunes.apple.com",
    "configuration.ls.apple.com",
    "gspe35-ssl.ls.apple.com",
    "gspe35-ssl.ls.apple.cn",
    "weatherkit.apple.com",
    "news-edge.apple.com",
    "news-todayconfig-edge.apple.com",
    "news-events.apple.com",
    "news-sports-events.apple.com",
    "news-client-search.apple.com",
    "uts-api.itunes.apple.com",
    "umc-tempo-api.apple.com",
];

fn harden_apple_content(content: &str) -> Result<String, String> {
    let exclusions = crate::promax::safety::apple_mitm_exclusions();
    let hostname_count = exclusions
        .len()
        .saturating_add(APPLE_FUNCTIONAL_MITM_HOSTS.len());
    let hostname_bytes = exclusions
        .iter()
        .map(String::len)
        .chain(APPLE_FUNCTIONAL_MITM_HOSTS.iter().map(|host| host.len()))
        .fold("hostname = %INSERT% ".len(), usize::saturating_add)
        .saturating_add(hostname_count.saturating_sub(1).saturating_mul(2));
    let mut output = String::with_capacity(content.len().saturating_add(hostname_bytes));
    let mut first_line = true;
    let mut in_rule = false;
    let mut replaced_hostname = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_rule = trimmed.eq_ignore_ascii_case("[Rule]");
            if in_rule {
                continue;
            }
        }
        if in_rule {
            continue;
        }
        if trimmed.starts_with("hostname =") {
            if !replaced_hostname {
                if !first_line {
                    output.push('\n');
                }
                output.push_str("hostname = %INSERT% ");
                let mut first_host = true;
                for host in exclusions
                    .iter()
                    .map(String::as_str)
                    .chain(APPLE_FUNCTIONAL_MITM_HOSTS.iter().copied())
                {
                    if !first_host {
                        output.push_str(", ");
                    }
                    output.push_str(host);
                    first_host = false;
                }
                first_line = false;
                replaced_hostname = true;
            }
        } else {
            if line.contains("isWorkaroundSSLPinning:true") {
                let line = line.replace(
                    "isWorkaroundSSLPinning:true",
                    "isWorkaroundSSLPinning:false",
                );
                push_output_line(&mut output, &mut first_line, &line);
            } else {
                push_output_line(&mut output, &mut first_line, line);
            }
        }
    }
    if !replaced_hostname {
        return Err("Apple bundle has no MITM hostname entry".to_string());
    }
    output.push('\n');
    Ok(output)
}

fn harden_youtube_content(content: &str) -> Result<String, String> {
    let mut in_script = false;
    let mut in_mitm = false;
    let mut output = String::with_capacity(
        content
            .len()
            .saturating_add("hostname = %APPEND% youtubei.googleapis.com".len()),
    );
    let mut first_line = true;
    let mut replaced_hostname = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let section = &trimmed[1..trimmed.len() - 1];
            in_script = section == "Script";
            in_mitm = section == "MITM";
        }
        if in_script && trimmed.contains("googlevideo\\.com") {
            continue;
        }
        if in_mitm && trimmed.starts_with("hostname =") {
            if !replaced_hostname {
                push_output_line(
                    &mut output,
                    &mut first_line,
                    "hostname = %APPEND% youtubei.googleapis.com",
                );
                replaced_hostname = true;
            }
            continue;
        }
        push_output_line(&mut output, &mut first_line, line);
    }
    if !replaced_hostname {
        return Err("YouTube bundle has no MITM hostname entry".to_string());
    }
    output.push('\n');
    Ok(output)
}

const DEVTOOLS_OUTPUT: &str = "modules/surge/amplify_nexus/🧰 Script Hub 配套工具合集.sgmodule";
const DEVTOOLS_CUSTOM_MARKER: &str = "#!script-hub-custom-sources=merged";

fn custom_devtools_request(
    root: &Path,
    custom_sources: Vec<SourceInput>,
) -> Result<MergeBundleRequest, String> {
    const BASE: &str = include_str!("../../assets/DevTools.Base.sgmodule");
    let body_start = BASE
        .find("\n[General]")
        .ok_or_else(|| "curated DevTools base is missing [General]".to_string())?
        + 1;
    let mut sources = vec![SourceInput {
        label: "Script Hub curated base".to_string(),
        url: "local:create/processor/assets/DevTools.Base.sgmodule".to_string(),
        content: BASE[body_start..].to_string(),
    }];
    sources.extend(custom_sources);
    let mut header_meta = crate::merge_bundles::module_metadata_pub(BASE);
    header_meta.remove("date");
    if sources.len() > 1 {
        header_meta.insert(
            "script-hub-custom-sources".to_string(),
            "merged".to_string(),
        );
    }
    Ok(MergeBundleRequest {
        op: "devtools_merge".to_string(),
        output_path: root.join(DEVTOOLS_OUTPUT).to_string_lossy().to_string(),
        header_meta,
        provenance_comment: None,
        sources,
        content_replacements: Vec::new(),
        extra_header_lines: Vec::new(),
        youtube_adblock_output: None,
        section_order: Vec::new(),
        script_path_suffix_map: HashMap::new(),
        script_raw_prefix: None,
        scripts_dir: None,
        script_hub_local: None,
    })
}

fn publish_custom_devtools_sources(
    root: &Path,
    custom_sources: Vec<SourceInput>,
) -> Result<(), String> {
    let request = custom_devtools_request(root, custom_sources)?;
    // Functional modules must not take over routing policies from the user's
    // rulesets, including when an added upstream source contains [Rule].
    publish_bundle(
        root,
        request,
        Some(|content| Ok(strip_rule_section(content))),
    )
}

fn merge_custom_devtools(
    root: &Path,
    downloader: &Downloader,
    custom_sources: &[CustomSource],
) -> Result<(), String> {
    let custom_sources = fetch_custom_sources(downloader, custom_sources)?;
    publish_custom_devtools_sources(root, custom_sources)
}

fn devtools_output_has_custom_marker(root: &Path) -> Result<bool, String> {
    match fs::read_to_string(root.join(DEVTOOLS_OUTPUT)) {
        Ok(content) => Ok(content.lines().any(|line| line == DEVTOOLS_CUSTOM_MARKER)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("cannot inspect current DevTools bundle: {error}")),
    }
}

pub fn run_functional_updates(root: &Path) -> Vec<String> {
    let downloader = downloader();
    sync_local_sources(root, &downloader);
    let mut failures = Vec::new();
    let configured_sources = custom_sources::load(root);
    let (custom_wool, custom_weibo, custom_devtools, custom_source_error) = match configured_sources
    {
        Ok(sources) => (
            sources
                .iter()
                .filter(|source| source.category == SourceCategory::Wool)
                .cloned()
                .collect::<Vec<_>>(),
            sources
                .iter()
                .filter(|source| source.category == SourceCategory::Weibo)
                .cloned()
                .collect::<Vec<_>>(),
            sources
                .into_iter()
                .filter(|source| source.category == SourceCategory::Devtools)
                .collect::<Vec<_>>(),
            None,
        ),
        Err(error) => (Vec::new(), Vec::new(), Vec::new(), Some(error)),
    };

    run_step(
        "Apple",
        merge(
            root,
            &downloader,
            MergeJob {
                op: "upstream_merge",
                output: "modules/surge/amplify_nexus/🍎 Apple服务增强合集.sgmodule",
                header: &[
                    ("name", "🍎 Apple服务增强合集"),
                    (
                        "desc",
                        "整合 Apple 生态增强：Maps · WeatherKit · News · TV；路由由外部 Apple 规则集与用户策略决定，关键服务排除 MITM",
                    ),
                    ("author", "VirgilClyne[https://github.com/VirgilClyne]"),
                    ("homepage", "https://NSRingo.github.io"),
                    (
                        "icon",
                        "https://developer.apple.com/assets/elements/icons/sf-symbols/sf-symbols-128x128.png",
                    ),
                    ("category", "『 🛠️ Amplify Nexus › 增幅枢纽 』"),
                ],
                specs: APPLE,
                custom_sources: None,
                replacements: &[
                    ("Proxy:🇺🇸美国", "Proxy:\"🍎 Apple 🍏\""),
                    (",🇺🇸美国", ",{{{Proxy}}}"),
                    (
                        "isWorkaroundSSLPinning:true",
                        "isWorkaroundSSLPinning:false",
                    ),
                ],
                youtube_adblock_output: None,
                postprocess: Some(harden_apple_content),
            },
        ),
        &mut failures,
    );
    run_step(
        "DualSubs",
        merge(
            root,
            &downloader,
            MergeJob {
                op: "upstream_merge",
                output: "modules/surge/amplify_nexus/🌐 DualSubs字幕增强合集.sgmodule",
                header: &[
                    ("name", "🌐 DualSubs字幕增强合集"),
                    (
                        "desc",
                        "全平台字幕增强与双语翻译；MITM/脚本覆盖面较大，请按需独立安装，不再混入 Apple 合集",
                    ),
                    ("author", "DualSubs"),
                    ("homepage", "https://github.com/DualSubs/Universal"),
                    ("category", "『 🛠️ Amplify Nexus › 增幅枢纽 』"),
                    ("tag", "Subtitles, Translation, Advanced"),
                ],
                specs: DUALSUBS,
                custom_sources: None,
                replacements: &[],
                youtube_adblock_output: None,
                postprocess: None,
            },
        ),
        &mut failures,
    );
    run_step(
        "BiliBili",
        merge(
            root,
            &downloader,
            MergeJob {
                op: "upstream_merge",
                output: "modules/surge/amplify_nexus/📺 BiliBili增强合集.sgmodule",
                header: &[
                    ("name", "📺 BiliBili增强合集"),
                    (
                        "desc",
                        "合并 Biliverse 与 Maasea 官方源的增强、全球、重定向和广告辅助能力",
                    ),
                    ("author", "Biliverse, Maasea"),
                    ("icon", "https://www.bilibili.com/favicon.ico"),
                    ("category", "『 🛠️ Amplify Nexus › 增幅枢纽 』"),
                    ("tag", "BiliBili, 增强"),
                ],
                specs: BILIBILI,
                custom_sources: None,
                replacements: &[("ForceHost:\"1\"", "ForceHost:\"2\"")],
                youtube_adblock_output: None,
                postprocess: Some(|content| Ok(strip_rule_section(content))),
            },
        ),
        &mut failures,
    );
    run_step(
        "YouTube",
        merge(
            root,
            &downloader,
            MergeJob {
                op: "youtube_merge",
                output: "modules/surge/amplify_nexus/📺 YouTube增强合集.sgmodule",
                header: &[
                    ("name", "📺 YouTube增强合集"),
                    ("desc", "YouTube 功能增强；广告行由 Rust 拆分后并入 PROMAX"),
                    ("author", "Maasea"),
                    (
                        "icon",
                        "https://cdn.jsdelivr.net/gh/Orz-3/mini@master/Color/YouTube.png",
                    ),
                    ("category", "『 🛠️ Amplify Nexus › 增幅枢纽 』"),
                ],
                specs: &[RemoteSource {
                    label: "YouTube.Enhance",
                    url: "https://raw.githubusercontent.com/Maasea/sgmodule/master/YouTube.Enhance.sgmodule",
                    required: true,
                }],
                custom_sources: None,
                replacements: &[],
                youtube_adblock_output: Some("modules/source/local/YouTube.ADBlock.sgmodule"),
                postprocess: Some(harden_youtube_content),
            },
        ),
        &mut failures,
    );
    run_step(
        "Weibo PROMAX source",
        require_valid_custom_sources(&custom_source_error, || {
            merge(
                root,
                &downloader,
                MergeJob {
                    op: "promax_source_merge",
                    output: "rulesets/Sources/LocalModules/Weibo.ADBlock.sgmodule",
                    header: &[
                        ("name", "🐦 微博去广告"),
                        ("desc", "微博与国际版去广告 · PROMAX 构建源"),
                        ("author", "fmz200, iab0x00, ScriptHub"),
                        ("tag", "去广告, 微博, PROMAX-build"),
                    ],
                    specs: WEIBO,
                    custom_sources: Some(&custom_weibo),
                    replacements: &[],
                    youtube_adblock_output: None,
                    postprocess: None,
                },
            )
        }),
        &mut failures,
    );
    let devtools_has_custom_marker = devtools_output_has_custom_marker(root);
    if custom_source_error.is_some() {
        failures.push("DevTools custom sources".to_string());
    } else {
        match devtools_has_custom_marker {
            Ok(has_custom_marker) if !custom_devtools.is_empty() || has_custom_marker => {
                run_step(
                    "DevTools custom sources",
                    merge_custom_devtools(root, &downloader, &custom_devtools),
                    &mut failures,
                );
            }
            Ok(_) => {}
            Err(error) => {
                eprintln!("[WARN] cannot inspect DevTools custom output: {error}");
                failures.push("DevTools custom sources".to_string());
            }
        }
    }
    run_step(
        "Wool PROMAX source",
        require_valid_custom_sources(&custom_source_error, || {
            merge(
                root,
                &downloader,
                MergeJob {
                    op: "promax_source_merge",
                    output: "rulesets/Sources/LocalModules/Wool.ADBlock.sgmodule",
                    header: &[
                        ("name", "🐑 综合去广告"),
                        ("desc", "国内常用 App 去广告集合 · PROMAX 构建源"),
                        ("author", "fmz200, ScriptHub"),
                        ("tag", "去广告, 综合, PROMAX-build"),
                    ],
                    specs: WOOL,
                    custom_sources: Some(&custom_wool),
                    replacements: &[],
                    youtube_adblock_output: None,
                    postprocess: None,
                },
            )
        }),
        &mut failures,
    );
    run_step(
        "Panel utilities",
        merge(
            root,
            &downloader,
            MergeJob {
                op: "upstream_merge",
                output: "modules/surge/amplify_nexus/📊 面板工具合集.sgmodule",
                header: &[
                    ("name", "📊 面板工具合集"),
                    ("desc", "节假日、网络信息与订阅流量面板"),
                    ("author", "Rabbit-Spec, xream, Coldvvater"),
                    (
                        "icon",
                        "https://cdn.jsdelivr.net/gh/Orz-3/mini@master/Color/Auto.png",
                    ),
                    ("category", "『 🛠️ Amplify Nexus › 增幅枢纽 』"),
                    ("tag", "面板, 工具"),
                ],
                specs: UTILITIES,
                custom_sources: None,
                replacements: &[],
                youtube_adblock_output: None,
                postprocess: None,
            },
        ),
        &mut failures,
    );

    failures
}

pub fn sync_shadowrocket_variants(root: &Path) -> Result<usize, String> {
    let source = root.join("modules/surge");
    let mut count = 0;
    for entry in walkdir::WalkDir::new(&source) {
        let entry =
            entry.map_err(|error| format!("cannot traverse Surge module sources: {error}"))?;
        if !entry.file_type().is_file()
            || entry.path().extension().and_then(|value| value.to_str()) != Some("sgmodule")
            || entry
                .path()
                .components()
                .any(|part| part.as_os_str() == "github")
        {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(&source)
            .map_err(|error| error.to_string())?;
        let mut target = root.join("modules/shadowrocket").join(relative);
        target.set_extension("module");
        let content = std::fs::read_to_string(entry.path())
            .map_err(|error| format!("cannot read {}: {error}", entry.path().display()))?;
        let converted = crate::promax::artifact::convert_surge_to_shadowrocket(&content);
        let errors = crate::promax::validation::validate_shadowrocket_module(&converted);
        if !errors.is_empty() {
            let diagnostics = errors
                .iter()
                .take(3)
                .map(|error| format!("line {} {}", error.line, error.code))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(format!(
                "Shadowrocket conversion failed validation for {}: {diagnostics}",
                entry.path().display(),
            ));
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "cannot create Shadowrocket output directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        if !crate::safe_write_file_internal(&target, &converted, true) {
            return Err(format!("cannot publish {}", target.display()));
        }
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::{
        DEVTOOLS_CUSTOM_MARKER, RemoteSource, custom_sources_from_batch,
        devtools_output_has_custom_marker, harden_apple_content, harden_youtube_content,
        publish_bundle, publish_custom_devtools_sources, sources_from_batch, strip_rule_section,
        sync_shadowrocket_variants, valid_module,
    };
    use crate::merge_bundles::{MergeBundleRequest, SourceInput};
    use crate::pipeline::custom_sources::{CustomSource, SourceCategory};
    use crate::promax::source::{DownloadBatch, DownloadFailure};
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    fn temporary_root(label: &str) -> PathBuf {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "script-hub-functional-{label}-{}-{timestamp}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        root
    }

    fn request(root: &Path, op: &str, content: &str) -> MergeBundleRequest {
        serde_json::from_value(serde_json::json!({
            "op": op,
            "output_path": root.join("bundle.sgmodule"),
            "header_meta": {"name": "fixture"},
            "sources": [{"label": "fixture", "url": "https://example.test/source", "content": content}],
            "youtube_adblock_output": (op == "youtube_merge").then(|| root.join("adblock.sgmodule"))
        })).unwrap()
    }

    #[test]
    fn postprocessing_failure_preserves_published_bundles() {
        for op in ["upstream_merge", "youtube_merge"] {
            let root = temporary_root(op);
            let old = "#!name=previous validated publication\n";
            for name in ["bundle.sgmodule", "adblock.sgmodule"] {
                std::fs::write(root.join(name), old).unwrap();
            }
            let input = "#!name=upstream\n[Rule]\nDOMAIN,example.test,DIRECT\n";
            let transform = if op == "youtube_merge" {
                harden_youtube_content
            } else {
                harden_apple_content
            };
            let result = publish_bundle(&root, request(&root, op, input), Some(transform));
            assert!(result.is_err());
            for name in ["bundle.sgmodule", "adblock.sgmodule"] {
                assert_eq!(std::fs::read_to_string(root.join(name)).unwrap(), old);
            }
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn invalid_youtube_secondary_output_preserves_both_publications() {
        let root = temporary_root("invalid-youtube");
        let old = "#!name=previous validated publication\n";
        for name in ["bundle.sgmodule", "adblock.sgmodule"] {
            std::fs::write(root.join(name), old).unwrap();
        }
        let input = "#!name=upstream\n[Map Local]\nNOT_A_VALID_MAP_LOCAL\n[MITM]\nhostname = %APPEND% youtubei.googleapis.com\n";
        let result = publish_bundle(
            &root,
            request(&root, "youtube_merge", input),
            Some(harden_youtube_content),
        );
        assert!(result.is_err());
        for name in ["bundle.sgmodule", "adblock.sgmodule"] {
            assert_eq!(std::fs::read_to_string(root.join(name)).unwrap(), old);
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn secondary_publication_io_failure_rolls_back_the_primary() {
        let root = temporary_root("youtube-rollback");
        let old = "#!name=previous validated publication\n";
        std::fs::write(root.join("bundle.sgmodule"), old).unwrap();
        std::fs::write(root.join("blocked"), "not a directory").unwrap();
        let input = "#!name=upstream\n[MITM]\nhostname = %APPEND% youtubei.googleapis.com\n";
        let mut request = request(&root, "youtube_merge", input);
        request.youtube_adblock_output = Some(
            root.join("blocked/adblock.sgmodule")
                .to_string_lossy()
                .to_string(),
        );
        assert!(publish_bundle(&root, request, Some(harden_youtube_content)).is_err());
        assert_eq!(
            std::fs::read_to_string(root.join("bundle.sgmodule")).unwrap(),
            old
        );
        assert_eq!(
            std::fs::read_to_string(root.join("blocked")).unwrap(),
            "not a directory"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn successful_bundle_staging_preserves_rules_and_quarantine() {
        let root = temporary_root("preserved-source");
        std::fs::write(
            root.join("bundle.sgmodule"),
            "#!name=old\n[Rule]\nDOMAIN,preserved.example,REJECT\n",
        )
        .unwrap();
        let quarantine = root.join("rulesets/AdBlock/quarantine.json");
        std::fs::create_dir_all(quarantine.parent().unwrap()).unwrap();
        std::fs::write(&quarantine, "existing quarantine report\n").unwrap();
        let input = "#!name=upstream\n[MITM]\nhostname = %APPEND% ads.example\n";
        publish_bundle(&root, request(&root, "promax_source_merge", input), None).unwrap();
        let content = std::fs::read_to_string(root.join("bundle.sgmodule")).unwrap();
        assert!(content.contains("DOMAIN,preserved.example,REJECT"));
        assert!(content.contains("#!name=fixture"));
        assert_eq!(
            std::fs::read_to_string(quarantine).unwrap(),
            "existing quarantine report\n"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn youtube_without_ads_clears_the_previous_adblock_source() {
        let root = temporary_root("empty-youtube-ads");
        std::fs::write(
            root.join("adblock.sgmodule"),
            "#!name=old ads\n[Rule]\nDOMAIN,old-ads.example,REJECT\n",
        )
        .unwrap();
        let input = "#!name=upstream\n[MITM]\nhostname = %APPEND% cdn.example\n";
        publish_bundle(
            &root,
            request(&root, "youtube_merge", input),
            Some(harden_youtube_content),
        )
        .unwrap();
        let adblock = std::fs::read_to_string(root.join("adblock.sgmodule")).unwrap();
        assert!(!adblock.contains("old-ads.example"));
        assert!(crate::promax::validation::validate_surge_module(&adblock).is_empty());
        let functional = std::fs::read_to_string(root.join("bundle.sgmodule")).unwrap();
        assert!(functional.contains("hostname = %APPEND% youtubei.googleapis.com"));
        assert!(!functional.contains("cdn.example"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_surge_source_directory_is_not_a_successful_empty_sync() {
        let root = temporary_root("missing-surge");
        assert!(sync_shadowrocket_variants(&root).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn published_promax_round_trips_through_shadowrocket_sync_offline() {
        let root = temporary_root("published-promax-sync");
        let relative = Path::new(
            "modules/surge/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule",
        );
        let published = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(relative);
        let content = std::fs::read_to_string(&published).unwrap();
        assert!(crate::promax::validation::validate_surge_module(&content).is_empty());
        let source = root.join(relative);
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(&source, &content).unwrap();

        assert_eq!(sync_shadowrocket_variants(&root).unwrap(), 1);
        let converted = std::fs::read_to_string(root.join("modules/shadowrocket/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).module")).unwrap();
        assert_eq!(
            converted,
            crate::promax::artifact::convert_surge_to_shadowrocket(&content)
        );
        assert!(crate::promax::validation::validate_shadowrocket_module(&converted).is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn shadowrocket_sync_validates_converted_promax_with_target_rules() {
        let root = temporary_root("shadowrocket-sync-promax");
        let source_dir = root.join("modules/surge/head_expanse");
        std::fs::create_dir_all(&source_dir).unwrap();
        let source = source_dir.join("PROMAX.sgmodule");
        let valid = "#!name=PROMAX\n#!desc=fixture\n[Rule]\nIP-CIDR,192.0.2.0/24,REJECT,no-resolve\nRULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/AdBlock/AdBlock_Local.list,REJECT,no-resolve\n";
        std::fs::write(&source, valid).unwrap();

        assert_eq!(sync_shadowrocket_variants(&root).unwrap(), 1);
        let target = root.join("modules/shadowrocket/head_expanse/PROMAX.module");
        let converted = std::fs::read_to_string(&target).unwrap();
        assert!(converted.contains("IP-CIDR,192.0.2.0/24,REJECT"));
        assert!(converted.contains("RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/AdBlock/AdBlock_Local.list,REJECT"));
        assert!(!converted.contains("no-resolve"));
        assert!(crate::promax::validation::validate_shadowrocket_module(&converted).is_empty());

        for invalid in [
            "#!name=PROMAX\n[Rule]\nDOMAIN,ads.example,PROXY\n",
            "#!name=PROMAX\n[Rule]\nRULE-SET,https://external.example/ads.list,REJECT,no-resolve\n",
        ] {
            std::fs::write(&source, invalid).unwrap();
            assert!(sync_shadowrocket_variants(&root).is_err());
            assert_eq!(std::fs::read_to_string(&target).unwrap(), converted);
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn youtube_adblock_exclusions_precede_inclusions() {
        let root = temporary_root("youtube-mitm-order");
        let input = "#!name=upstream\n[MITM]\nhostname = %APPEND% *.googlevideo.com, youtubei.googleapis.com\n";
        publish_bundle(
            &root,
            request(&root, "youtube_merge", input),
            Some(harden_youtube_content),
        )
        .unwrap();
        let adblock = std::fs::read_to_string(root.join("adblock.sgmodule")).unwrap();
        assert!(adblock.contains("hostname = %APPEND% -github.com, -api.github.com, -*.githubusercontent.com, *.googlevideo.com"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_html_and_accepts_real_module_shape() {
        assert!(!valid_module("<!doctype html><html>not a module</html>"));
        assert!(!valid_module("  <!DOCTYPE HTML><html>not a module</html>"));
        assert!(!valid_module("#!name=too short\n[Script]\n"));
        assert!(valid_module(
            "#!name=Fixture functional module with enough bytes for validation\n#!desc=fixture\n[Script]\nfixture = type=generic,script-path=https://example.test/a.js\n"
        ));
    }

    #[test]
    fn functional_bundle_does_not_publish_routing_rules() {
        let module = "#!name=Fixture\n[Rule]\nDOMAIN,apple.example,DIRECT\n[Script]\nfixture = type=generic,script-path=https://example.test/a.js\n";
        let stripped = strip_rule_section(module);
        assert!(!stripped.contains("[Rule]"));
        assert!(!stripped.contains("apple.example"));
        assert!(stripped.contains("[Script]"));
    }

    #[test]
    fn rule_section_removal_preserves_remaining_lines_and_final_newline() {
        assert_eq!(
            strip_rule_section(
                "#!name=Fixture\n[Rule]\nDOMAIN,ads.example,DIRECT\n[Script]\nscript = ok\n"
            ),
            "#!name=Fixture\n[Script]\nscript = ok\n"
        );
        assert_eq!(strip_rule_section(""), "\n");
    }

    #[test]
    fn repeated_remote_urls_keep_each_label_without_cloning_the_last_body() {
        let url = "https://example.test/shared.sgmodule";
        let specs = [
            RemoteSource {
                label: "first",
                url,
                required: true,
            },
            RemoteSource {
                label: "second",
                url,
                required: true,
            },
        ];
        let content = "#!name=Fixture functional module with enough bytes for validation\n[Script]\nscript = ok\n";
        let batch = crate::promax::source::DownloadBatch {
            contents: HashMap::from([(url.to_string(), content.to_string())]),
            failures: Vec::new(),
        };

        let sources = sources_from_batch(&specs, batch).unwrap();

        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].content, sources[1].content);
    }

    #[test]
    #[ignore = "release-only diagnostic benchmark; run with --release --ignored --nocapture"]
    fn benchmark_module_validation_hot_path() {
        use std::hint::black_box;
        use std::time::Instant;

        fn legacy_validation(content: &str) -> bool {
            let lowered = content.trim_start().to_ascii_lowercase();
            content.len() >= 80
                && !lowered.starts_with("<!doctype html")
                && !lowered.starts_with("<html")
                && content
                    .lines()
                    .any(|line| line.trim().starts_with("#!name"))
                && content
                    .lines()
                    .any(|line| line.trim().starts_with('[') && line.trim().ends_with(']'))
        }

        let filler = "x".repeat(8 * 1024 * 1024);
        let content = format!("#!name=Fixture\n{filler}\n[Script]\nscript = ok\n");
        let iterations = 20;
        assert_eq!(legacy_validation(&content), valid_module(&content));

        let start = Instant::now();
        for _ in 0..iterations {
            black_box(legacy_validation(black_box(&content)));
        }
        let legacy = start.elapsed();

        let start = Instant::now();
        for _ in 0..iterations {
            black_box(valid_module(black_box(&content)));
        }
        let optimized = start.elapsed();
        eprintln!(
            "validation of 8 MiB module x {iterations}: legacy={legacy:?}, optimized={optimized:?}"
        );
    }

    #[test]
    fn apple_hardening_keeps_only_bounded_function_hosts() {
        let input = "#!name=Apple\n#!arguments=isWorkaroundSSLPinning:true\n[Rule]\nDOMAIN,account.apple.com,DIRECT\n[Script]\nweather = type=http-response,pattern=^https://weatherkit\\.apple\\.com,script-path=https://example.test/a.js\n[MITM]\nhostname = %APPEND% *.apple.com, arbitrary.apple.com\n";
        let hardened = harden_apple_content(input).unwrap();
        assert!(!hardened.contains("[Rule]"));
        assert!(!hardened.contains("isWorkaroundSSLPinning:true"));
        assert!(hardened.contains("-account.apple.com"));
        assert!(hardened.contains("weatherkit.apple.com"));
        assert!(hardened.contains("uts-api.itunes.apple.com"));
        assert!(!hardened.contains("arbitrary.apple.com"));
    }

    #[test]
    fn youtube_hardening_moves_googlevideo_work_out_of_feature_bundle() {
        let input = "#!name=YouTube\n[Script]\nyoutube = type=http-response,pattern=^https://youtubei\\.googleapis\\.com,script-path=https://example.test/a.js\ninit = type=http-request,pattern=^https://x\\.googlevideo\\.com/initplayback,script-path=https://example.test/b.js\n[MITM]\nhostname = %APPEND% *.googlevideo.com, youtubei.googleapis.com\n";
        let hardened = harden_youtube_content(input).unwrap();
        assert!(hardened.contains("youtubei.googleapis.com"));
        assert!(!hardened.contains("googlevideo\\.com"));
        assert!(!hardened.contains("*.googlevideo.com"));
        assert!(!hardened.contains("hostname = %APPEND%\n"));
    }

    #[test]
    fn custom_source_batch_requires_every_configured_source_to_be_a_module() {
        let good_url = "https://raw.githubusercontent.com/example/repo/main/good.sgmodule";
        let bad_url = "https://cdn.jsdelivr.net/gh/example/repo/bad.module";
        let sources = [
            CustomSource {
                category: SourceCategory::Wool,
                name: "Good".to_string(),
                url: good_url.to_string(),
            },
            CustomSource {
                category: SourceCategory::Wool,
                name: "Bad".to_string(),
                url: bad_url.to_string(),
            },
        ];
        let batch = DownloadBatch {
            contents: HashMap::from([
                (
                    good_url.to_string(),
                    "#!name=Fixture functional module with enough bytes for validation\n[Script]\nscript = ok\n".to_string(),
                ),
                (bad_url.to_string(), "<!doctype html><html>error</html>".to_string()),
            ]),
            failures: Vec::<DownloadFailure>::new(),
        };

        let error = custom_sources_from_batch(&sources, batch).unwrap_err();
        assert!(error.contains("Bad returned non-module content"));
    }

    #[test]
    fn custom_devtools_marker_survives_finalize_and_empty_sources_restore_curated_base() {
        let root = temporary_root("devtools-custom-marker");
        let custom_rule = "^https://custom-devtools\\.example\\.test/ads - reject";
        publish_custom_devtools_sources(
            &root,
            vec![SourceInput {
                label: "Fixture custom DevTools".to_string(),
                url: "https://raw.githubusercontent.com/example/repo/main/custom.sgmodule"
                    .to_string(),
                content: format!(
                    "#!name=Custom DevTools fixture\n[Rule]\nDOMAIN,custom-devtools.example.test,DIRECT\n[URL Rewrite]\n{custom_rule}\n[MITM]\nhostname = %APPEND% custom-devtools.example.test\n"
                ),
            }],
        )
        .unwrap();

        assert!(devtools_output_has_custom_marker(&root).unwrap());
        let output = root.join(super::DEVTOOLS_OUTPUT);
        let with_custom = std::fs::read_to_string(&output).unwrap();
        assert!(
            with_custom
                .lines()
                .any(|line| line == DEVTOOLS_CUSTOM_MARKER)
        );
        assert!(with_custom.contains(custom_rule));
        assert!(!with_custom.contains("[Rule]"));
        assert!(!with_custom.contains("DOMAIN,custom-devtools.example.test,DIRECT"));

        publish_custom_devtools_sources(&root, Vec::new()).unwrap();

        assert!(!devtools_output_has_custom_marker(&root).unwrap());
        let restored = std::fs::read_to_string(output).unwrap();
        assert!(restored.contains("Sub-Store Core"));
        assert!(!restored.contains(custom_rule));
        let _ = std::fs::remove_dir_all(root);
    }
}
