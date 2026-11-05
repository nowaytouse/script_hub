//! Rust-owned upstream refresh for functional modules and PROMAX build inputs.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{apple_shadowrocket, apple_surge::harden as harden_apple_content};
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

const APPLE_OUTPUT: &str = "modules/surge/amplify_nexus/🍎 Apple服务增强合集.sgmodule";
const APPLE_HEADER: &[(&str, &str)] = &[
    ("name", "🍎 Apple服务增强合集"),
    (
        "desc",
        "整合 WeatherKit · News · TV；默认使用 Apple 原生天气数据，路由由外部规则集与用户策略决定",
    ),
    ("author", "VirgilClyne[https://github.com/VirgilClyne]"),
    ("homepage", "https://NSRingo.github.io"),
    (
        "icon",
        "https://developer.apple.com/assets/elements/icons/sf-symbols/sf-symbols-128x128.png",
    ),
    ("category", "『 🛠️ Amplify Nexus › 增幅枢纽 』"),
];
const APPLE: &[RemoteSource] = &[
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

const APPLE_SHADOWROCKET: &[RemoteSource] = &[
    RemoteSource {
        label: "iRingo.TV (Shadowrocket)",
        url: apple_shadowrocket::TV_SOURCE,
        required: true,
    },
    RemoteSource {
        label: "iRingo.WeatherKit Rewrite (Shadowrocket)",
        url: apple_shadowrocket::WEATHER_SOURCE,
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

const YOUTUBE: RemoteSource = RemoteSource {
    label: "YouTube.Enhance",
    url: "https://raw.githubusercontent.com/Maasea/sgmodule/master/YouTube.Enhance.sgmodule",
    required: true,
};
const VIDEO_OUTPUT: &str = "modules/surge/amplify_nexus/📺 BiliBili + YouTube 增强合集.sgmodule";
const BILIBILI_BUILD_SOURCE: &str = "rulesets/Sources/LocalModules/BiliBili.Functional.sgmodule";
const YOUTUBE_BUILD_SOURCE: &str = "modules/source/local/YouTube.ADBlock.sgmodule";

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
    let mut sources = fetch_sources(downloader, job.specs)?;
    if let Some(custom_sources) = job.custom_sources {
        sources.extend(fetch_custom_sources(downloader, custom_sources)?);
    }
    publish_merge_job(root, job, sources)
}

fn publish_merge_job(
    root: &Path,
    job: MergeJob<'_>,
    sources: Vec<SourceInput>,
) -> Result<(), String> {
    let MergeJob {
        op,
        output,
        header,
        replacements,
        youtube_adblock_output,
        postprocess,
        ..
    } = job;
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
    with_bundle_staging(root, |staging| {
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
        let content = read_staged_bundle(&staged_output)?;
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
                content: read_staged_bundle(&staged_adblock)?,
                kind: ArtifactKind::Surge,
            });
        }
        publish_generated_artifacts(root, &artifacts)
    })
}

fn read_staged_bundle(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map_err(|error| format!("cannot read staged bundle {}: {error}", path.display()))
}

fn with_bundle_staging(
    root: &Path,
    build: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
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
    let result = build(&staging);
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

fn scope_biliverse_enhanced_storage(content: &str) -> String {
    content
        .lines()
        .map(|line| {
            let directive = line
                .strip_prefix("#!")
                .and_then(|line| line.split_once('='))
                .map(|(key, value)| (key.trim(), value.trim()));
            if let Some(("arguments", args)) = directive {
                let args = crate::smart_cleanup::split_script_parameters(args)
                    .into_iter()
                    .map(|arg| {
                        let arg = arg.trim();
                        match arg.split_once(':') {
                            Some(("Storage", value)) => format!("Enhanced.Storage:{value}"),
                            _ if arg == "Storage" => "Enhanced.Storage".to_string(),
                            _ => arg.to_string(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                format!("#!arguments={args}")
            } else if matches!(directive, Some(("arguments-desc", _))) {
                line.replace("\\nStorage:", "\\nEnhanced.Storage:")
            } else {
                line.replace("{{{Storage}}}", "{{{Enhanced.Storage}}}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn publish_video_sources(
    root: &Path,
    mut bilibili: Vec<SourceInput>,
    youtube: SourceInput,
) -> Result<(), String> {
    for source in &mut bilibili {
        if source.label == "Enhanced" {
            source.content = scope_biliverse_enhanced_storage(&source.content);
        }
        source.content = crate::url_rewriter::normalize_force_http_engine_hosts(&source.content);
    }
    with_bundle_staging(root, |staging| {
        let bili_path = staging.join("bilibili.sgmodule");
        let youtube_path = staging.join("youtube.sgmodule");
        let ads_path = staging.join("youtube-ads.sgmodule");
        let combined_path = staging.join("video.sgmodule");
        let mut feature = MergeBundleRequest {
            op: "upstream_merge".into(),
            output_path: bili_path.to_string_lossy().into_owned(),
            header_meta: metadata(&[
                ("name", "📺 BiliBili增强合集"),
                (
                    "desc",
                    "合并 Biliverse 与 Maasea 官方源的增强、全球、重定向和广告辅助能力",
                ),
                ("author", "Biliverse, Maasea"),
            ]),
            sources: bilibili,
            content_replacements: vec![[r#"ForceHost:"1""#.into(), r#"ForceHost:"2""#.into()]],
            ..MergeBundleRequest::default()
        };
        if !process_merge_bundle(&feature) {
            return Err("cannot build the BiliBili-only extraction input".into());
        }
        let youtube_url = youtube.url.clone();
        let split = MergeBundleRequest {
            op: "youtube_merge".into(),
            output_path: youtube_path.to_string_lossy().into_owned(),
            header_meta: metadata(&[("name", "YouTube.Enhance")]),
            sources: vec![youtube],
            youtube_adblock_output: Some(ads_path.to_string_lossy().into_owned()),
            ..MergeBundleRequest::default()
        };
        if !process_merge_bundle(&split) {
            return Err("cannot separate YouTube features from its PROMAX input".into());
        }
        feature.sources.push(SourceInput {
            label: YOUTUBE.label.into(),
            url: youtube_url,
            content: harden_youtube_content(&read_staged_bundle(&youtube_path)?)?,
        });
        feature.output_path = combined_path.to_string_lossy().into_owned();
        feature.header_meta = metadata(&[
            ("name", "📺 BiliBili + YouTube 增强合集"),
            (
                "desc",
                "BiliBili 与 YouTube 功能增强；YouTube 广告构建源独立维护",
            ),
            ("author", "Biliverse, Maasea"),
            ("icon", "https://www.bilibili.com/favicon.ico"),
            ("category", "『 🛠️ Amplify Nexus › 增幅枢纽 』"),
            ("tag", "BiliBili, YouTube, 增强"),
        ]);
        if !process_merge_bundle(&feature) {
            return Err("cannot build the combined video module".into());
        }
        let functional = |path: &Path| {
            read_staged_bundle(path).map(|content| {
                let content = crate::url_rewriter::normalize_cdn_urls(strip_rule_section(&content));
                crate::mitm_manager::cleaned_mitm_content(&content).unwrap_or(content)
            })
        };
        let combined = functional(&combined_path)?;
        crate::merge_bundles::verify_video_defaults(&combined)?;
        publish_generated_artifacts(
            root,
            &[
                GeneratedArtifact {
                    target: root.join(VIDEO_OUTPUT),
                    content: combined,
                    kind: ArtifactKind::Surge,
                },
                GeneratedArtifact {
                    target: root.join(BILIBILI_BUILD_SOURCE),
                    content: functional(&bili_path)?,
                    kind: ArtifactKind::Surge,
                },
                GeneratedArtifact {
                    target: root.join(YOUTUBE_BUILD_SOURCE),
                    content: read_staged_bundle(&ads_path)?,
                    kind: ArtifactKind::Surge,
                },
            ],
        )
    })
}

fn merge_video(root: &Path, downloader: &Downloader) -> Result<(), String> {
    let bilibili = fetch_sources(downloader, BILIBILI)?;
    let youtube = fetch_sources(downloader, &[YOUTUBE])?
        .pop()
        .ok_or_else(|| "YouTube feature source unavailable".to_string())?;
    publish_video_sources(root, bilibili, youtube)
}

const DEVTOOLS_OUTPUT: &str = "modules/surge/amplify_nexus/🧰 Script Hub 配套工具合集.sgmodule";
fn publish_devtools_sources(
    root: &Path,
    panel_sources: Vec<SourceInput>,
    custom_sources: Vec<SourceInput>,
) -> Result<(), String> {
    const BASE: &str = include_str!("../../assets/DevTools.Base.sgmodule");
    let has_custom_sources = !custom_sources.is_empty();
    let mut sources = vec![SourceInput {
        label: "Script Hub curated base".to_string(),
        url: "local:crates/processor/assets/DevTools.Base.sgmodule".to_string(),
        content: BASE.to_string(),
    }];
    sources.extend(panel_sources);
    sources.extend(custom_sources);
    let mut header_meta = crate::merge_bundles::module_metadata_pub(BASE);
    header_meta.remove("date");
    header_meta.extend(metadata(&[
        (
            "desc",
            "Script Hub 转换 · BoxJs · Sub-Store(β) · 节假日、网络与订阅流量面板",
        ),
        (
            "author",
            "@小白脸 @xream @keywos @ckyb, ChavyLeung, sub-store-org, Rabbit-Spec, Coldvvater",
        ),
        ("tag", "ScriptHub, BoxJs, Sub-Store, 面板, 工具"),
    ]));
    if has_custom_sources {
        header_meta.insert(
            "script-hub-custom-sources".to_string(),
            "merged".to_string(),
        );
    }
    let request = MergeBundleRequest {
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
    };
    // Functional modules must not take over routing policies from the user's
    // rulesets, including when an added upstream source contains [Rule].
    publish_bundle(
        root,
        request,
        Some(|content| {
            crate::merge_bundles::verify_tool_defaults(content)?;
            Ok(crate::url_rewriter::normalize_cdn_urls(strip_rule_section(
                content,
            )))
        }),
    )
}

fn merge_devtools(
    root: &Path,
    downloader: &Downloader,
    custom_sources: &[CustomSource],
) -> Result<(), String> {
    publish_devtools_sources(
        root,
        fetch_sources(downloader, UTILITIES)?,
        fetch_custom_sources(downloader, custom_sources)?,
    )
}

fn publish_apple_surge(root: &Path, mut sources: Vec<SourceInput>) -> Result<(), String> {
    // The shared merge validates its intermediate output before Apple-specific hardening.
    for source in &mut sources {
        source.content = strip_rule_section(&source.content).replace(
            "isWorkaroundSSLPinning:true",
            "isWorkaroundSSLPinning:false",
        );
    }
    publish_merge_job(
        root,
        MergeJob {
            op: "upstream_merge",
            output: APPLE_OUTPUT,
            header: APPLE_HEADER,
            specs: APPLE,
            custom_sources: None,
            replacements: &[],
            youtube_adblock_output: None,
            postprocess: Some(harden_apple_content),
        },
        sources,
    )
}

fn publish_apple_shadowrocket(root: &Path, sources: &[SourceInput]) -> Result<(), String> {
    let [tv, weather] = sources else {
        return Err("missing official Shadowrocket Apple sources".into());
    };
    publish_generated_artifacts(
        root,
        &[
            GeneratedArtifact {
                target: root.join(apple_shadowrocket::TV_OUTPUT),
                content: apple_shadowrocket::render_tv(&tv.content)?,
                kind: ArtifactKind::Shadowrocket,
            },
            GeneratedArtifact {
                target: root.join(apple_shadowrocket::WEATHER_OUTPUT),
                content: apple_shadowrocket::render_weather(&weather.content)?,
                kind: ArtifactKind::Shadowrocket,
            },
        ],
    )
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
        fetch_sources(&downloader, APPLE).and_then(|sources| publish_apple_surge(root, sources)),
        &mut failures,
    );
    run_step(
        "Apple for Shadowrocket",
        fetch_sources(&downloader, APPLE_SHADOWROCKET)
            .and_then(|sources| publish_apple_shadowrocket(root, &sources)),
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
        "BiliBili + YouTube",
        merge_video(root, &downloader),
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
    run_step(
        "Script Hub utilities and panels",
        require_valid_custom_sources(&custom_source_error, || {
            merge_devtools(root, &downloader, &custom_devtools)
        }),
        &mut failures,
    );
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
        if relative == Path::new("amplify_nexus/🍎 Apple服务增强合集.sgmodule") {
            continue;
        }
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
    #[test]
    fn published_video_and_tools_still_use_the_configured_defaults() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let video = std::fs::read_to_string(root.join(super::VIDEO_OUTPUT)).unwrap();
        let tools = std::fs::read_to_string(root.join(super::DEVTOOLS_OUTPUT)).unwrap();
        crate::merge_bundles::verify_video_defaults(&video).unwrap();
        crate::merge_bundles::verify_tool_defaults(&tools).unwrap();
        let duplicated = tools.replace(
            "sync:\"# Sub-Store Sync\"",
            "sync:\"# Sub-Store Sync\",sync:\"# Sub-Store Sync\"",
        );
        assert!(crate::merge_bundles::verify_tool_defaults(&duplicated).is_err());
        let renamed = video.replace("屏蔽Shorts按钮", "上游改名");
        assert!(crate::merge_bundles::verify_video_defaults(&renamed).is_err());
    }

    #[test]
    fn video_bundle_keeps_media_features_separate_from_adblock_inputs() {
        let root = temporary_root("video-bundle");
        let source = |label: &str, content: &str| SourceInput {
            label: label.into(),
            url: "https://example.test/source.sgmodule".into(),
            content: content.into(),
        };
        super::publish_video_sources(
            &root,
            vec![
                source("Enhanced", concat!(
                    "#!name=Enhanced\n#!arguments = Storage:\"PersistentStore\"\n",
                    "[General]\nforce-http-engine-hosts = %APPEND% bili.example.test:80, *:8082\n",
                    "[Rule]\nDOMAIN,bili.example.test,DIRECT\n",
                    "[Script]\nBili = type=http-request,pattern=^https://bili.example.test/,script-path=https://example.test/bili.js,argument=Storage={{{Storage}}}\n",
                        "[MITM]\nhostname = %APPEND% bili.example.test, accounts.google.com\n",
                )),
                source("Global", "#!name=Global\n#!arguments = Storage:\"Argument\"\n[MITM]\nhostname = %APPEND% global.example.test\n"),
            ],
            source("YouTube", concat!(
                "#!name=YouTube\n#!arguments=Language:off,屏蔽上传按钮:true,屏蔽选段按钮:true,屏蔽Shorts按钮:true,字幕翻译语言:off\n",
                "[Rule]\nDOMAIN,yt-ads.example.test,REJECT\n",
                "[Script]\nYT = type=http-request,pattern=^https://youtubei.googleapis.com/,script-path=https://example.test/yt.js,argument=\"{{{屏蔽上传按钮}}},{{{屏蔽选段按钮}}},{{{屏蔽Shorts按钮}}}\"\n",
                "YT Video = type=http-request,pattern=^https://.+\\.googlevideo\\.com/,script-path=https://example.test/video.js\n",
                "[MITM]\nhostname = %APPEND% *.googlevideo.com, youtubei.googleapis.com\n",
            )),
        ).unwrap();
        let combined = std::fs::read_to_string(root.join(super::VIDEO_OUTPUT)).unwrap();
        let bili = std::fs::read_to_string(root.join(super::BILIBILI_BUILD_SOURCE)).unwrap();
        let ads = std::fs::read_to_string(root.join(super::YOUTUBE_BUILD_SOURCE)).unwrap();
        assert!(
            combined.contains("bili.example.test") && combined.contains("youtubei.googleapis.com")
        );
        assert!(combined.contains("Enhanced.Storage:\"PersistentStore\",Storage:\"Argument\""));
        assert!(combined.contains("argument=Storage={{{Enhanced.Storage}}}"));
        assert!(combined.contains("屏蔽上传按钮:false,屏蔽选段按钮:false,屏蔽Shorts按钮:false"));
        assert!(combined.contains("字幕翻译语言:\"zh-Hans\""));
        assert!(combined.contains("force-http-engine-hosts = %APPEND% bili.example.test:80"));
        assert!(!combined.contains("*:8082"));
        assert!(!combined.contains("accounts.google.com"));
        assert!(!combined.contains("[Rule]") && !combined.contains("googlevideo"));
        assert!(!bili.contains("youtubei.googleapis.com"));
        assert!(ads.contains("yt-ads.example.test") && ads.contains("*.googlevideo.com"));
        assert!(!ads.contains("bili.example.test"));
        let _ = std::fs::remove_dir_all(root);
    }

    use super::{
        RemoteSource, custom_sources_from_batch, harden_apple_content, harden_youtube_content,
        publish_bundle, publish_devtools_sources, sources_from_batch, strip_rule_section,
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
    fn apple_refresh_removes_upstream_routing_and_restores_native_weather_defaults() {
        let root = temporary_root("apple-refresh");
        let original =
            include_str!("../../../../modules/surge/amplify_nexus/🍎 Apple服务增强合集.sgmodule");
        let content = original
            .replace(
                "isWorkaroundSSLPinning:false",
                "isWorkaroundSSLPinning:true",
            )
            .replace(
                "Weather.Provider:\"WeatherKit\"",
                "Weather.Provider:\"ColorfulClouds\"",
            );
        let content = format!("{content}\n[Rule]\nDOMAIN,account.apple.com,REJECT\n");
        super::publish_apple_surge(
            &root,
            vec![crate::merge_bundles::SourceInput {
                label: "fixture".into(),
                url: "https://example.test/apple.sgmodule".into(),
                content,
            }],
        )
        .unwrap();
        let refreshed = std::fs::read_to_string(root.join(super::APPLE_OUTPUT)).unwrap();
        assert!(!refreshed.contains("[Rule]"));
        assert!(refreshed.contains("Weather.Provider:\"WeatherKit\""));
        assert!(refreshed.contains("isWorkaroundSSLPinning:false"));
        std::fs::remove_dir_all(root).unwrap();
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
    fn shadowrocket_sync_never_overwrites_native_apple_publication() {
        let root = temporary_root("shadowrocket-native-apple");
        let surge = root.join(super::APPLE_OUTPUT);
        let native = root.join(super::apple_shadowrocket::TV_OUTPUT);
        for path in [&surge, &native] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        }
        std::fs::write(surge, "#!name=Surge-only Apple News\n").unwrap();
        std::fs::write(&native, "#!name=Native Apple TV\n").unwrap();
        assert_eq!(sync_shadowrocket_variants(&root).unwrap(), 0);
        assert_eq!(
            std::fs::read_to_string(native).unwrap(),
            "#!name=Native Apple TV\n"
        );
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
        assert!(converted.contains("IP-CIDR,192.0.2.0/24,REJECT,no-resolve"));
        assert!(converted.contains("RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/AdBlock/AdBlock_Local.list,REJECT,no-resolve"));
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
    fn tools_keep_panels_and_core_when_custom_sources_are_removed() {
        let root = temporary_root("devtools-custom-marker");
        let custom_rule = "^https://custom-devtools\\.example\\.test/ads - reject";
        let panels = || {
            vec![SourceInput {
            label: "Fixture panel".to_string(),
            url: "https://example.test/panel.sgmodule".to_string(),
            content: concat!(
                 "#!name=Fixture panel\n#!arguments=REFRESH:60,TITLE:\"One, two\",EVENT_SCRIPT_NAME:\"Active upstream event\"\n",
                "[Panel]\nFixture = script-name=Fixture Panel,update-interval={{{REFRESH}}}\n",
                     "[Script]\nFixture Panel = type=generic,script-path=https://raw.githubusercontent.com/example/repo/main/panel.js\n",
                     "{{{EVENT_SCRIPT_NAME}}} = type=event,event-name=network-changed,script-path=https://example.test/panel.js\n",
            )
            .to_string(),
        }]
        };
        publish_devtools_sources(
            &root,
            panels(),
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

        let output = root.join(super::DEVTOOLS_OUTPUT);
        let with_custom = std::fs::read_to_string(&output).unwrap();
        assert!(
            with_custom
                .lines()
                .any(|line| line == "#!script-hub-custom-sources=merged")
        );
        assert!(with_custom.contains(custom_rule));
        assert!(!with_custom.contains("[Rule]"));
        assert!(!with_custom.contains("DOMAIN,custom-devtools.example.test,DIRECT"));

        publish_devtools_sources(&root, panels(), Vec::new()).unwrap();

        let restored = std::fs::read_to_string(output).unwrap();
        assert!(!restored.contains("#!script-hub-custom-sources="));
        assert!(restored.contains("Sub-Store Core"));
        let metadata = crate::merge_bundles::module_metadata_pub(&restored);
        assert!(metadata["arguments"].contains("ability:http-client-policy"));
        assert!(metadata["arguments"].contains("sync:\"# Sub-Store Sync\""));
        assert!(metadata["arguments"].contains("EVENT_SCRIPT_NAME:\"# 网络信息变化\""));
        assert!(metadata["arguments"].contains("REFRESH:60"));
        assert!(metadata["arguments"].contains("TITLE:\"One, two\""));
        assert!(restored.contains("[Panel]\nFixture = script-name=Fixture Panel"));
        assert!(restored.lines().any(|line| {
            line.split_once('=').is_some_and(|(name, params)| {
                name.trim() == "Fixture Panel" && params.trim().starts_with("type=generic,")
            })
        }));
        assert!(!restored.contains(custom_rule));
        assert!(restored.contains("https://cdn.jsdelivr.net/gh/example/repo@main/panel.js"));
        let dated = restored
            .lines()
            .map(|line| {
                if line.starts_with("#!date=") {
                    "#!date=2000-01-01 00:00:00"
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let output = root.join(super::DEVTOOLS_OUTPUT);
        std::fs::write(&output, &dated).unwrap();
        publish_devtools_sources(&root, panels(), Vec::new()).unwrap();
        assert_eq!(std::fs::read_to_string(&output).unwrap(), dated);
        let mut changed_upstream = panels();
        changed_upstream[0].content = changed_upstream[0]
            .content
            .replace("{{{EVENT_SCRIPT_NAME}}} =", "Always-on event =");
        let error = publish_devtools_sources(&root, changed_upstream, Vec::new()).unwrap_err();
        assert!(error.contains("EVENT_SCRIPT_NAME"));
        assert_eq!(std::fs::read_to_string(output).unwrap(), dated);
        let _ = std::fs::remove_dir_all(root);
    }
}
