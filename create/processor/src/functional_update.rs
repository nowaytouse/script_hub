//! Rust-owned upstream refresh for functional modules and PROMAX build inputs.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use crate::merge_bundles::{MergeBundleRequest, SourceInput, process_merge_bundle};
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

struct MergeJob<'a> {
    op: &'a str,
    output: &'a str,
    header: &'a [(&'a str, &'a str)],
    specs: &'a [RemoteSource],
    replacements: &'a [(&'a str, &'a str)],
    youtube_adblock_output: Option<&'a str>,
}

fn merge(root: &Path, downloader: &Downloader, job: MergeJob<'_>) -> Result<(), String> {
    let MergeJob {
        op,
        output,
        header,
        specs,
        replacements,
        youtube_adblock_output,
    } = job;
    let sources = fetch_sources(downloader, specs)?;
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
    process_merge_bundle(&request)
        .then_some(())
        .ok_or_else(|| format!("merge failed for {output}"))
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

fn strip_routing_rules(root: &Path, relative: &str) -> Result<(), String> {
    let path = root.join(relative);
    let content = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    if !crate::safe_write_file_internal(&path, &strip_rule_section(&content), true) {
        return Err(format!(
            "failed to remove routing rules from {}",
            path.display()
        ));
    }
    Ok(())
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

fn harden_apple_bundle(root: &Path) -> Result<(), String> {
    let path = root.join("modules/surge/amplify_nexus/🍎 Apple服务增强合集.sgmodule");
    let content = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let hardened = harden_apple_content(&content)?;
    if !crate::safe_write_file_internal(&path, &hardened, true) {
        return Err(format!("failed to harden {}", path.display()));
    }
    Ok(())
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

fn harden_youtube_bundle(root: &Path) -> Result<(), String> {
    let path = root.join("modules/surge/amplify_nexus/📺 YouTube增强合集.sgmodule");
    let content = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let hardened = harden_youtube_content(&content)?;
    if !crate::safe_write_file_internal(&path, &hardened, true) {
        return Err(format!("failed to harden {}", path.display()));
    }
    Ok(())
}

pub fn run_functional_updates(root: &Path) -> Vec<String> {
    let downloader = downloader();
    sync_local_sources(root, &downloader);
    let mut failures = Vec::new();

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
                replacements: &[
                ("Proxy:🇺🇸美国", "Proxy:\"🍎 Apple 🍏\""),
                (",🇺🇸美国", ",{{{Proxy}}}"),
                (
                    "isWorkaroundSSLPinning:true",
                    "isWorkaroundSSLPinning:false",
                ),
            ],
                youtube_adblock_output: None,
            },
        )
        .and_then(|_| harden_apple_bundle(root)),
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
                replacements: &[],
                youtube_adblock_output: None,
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
                replacements: &[("ForceHost:\"1\"", "ForceHost:\"2\"")],
                youtube_adblock_output: None,
            },
        )
        .and_then(|_| {
            strip_routing_rules(
                root,
                "modules/surge/amplify_nexus/📺 BiliBili增强合集.sgmodule",
            )
        }),
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
                replacements: &[],
                youtube_adblock_output: Some("modules/source/local/YouTube.ADBlock.sgmodule"),
            },
        )
        .and_then(|_| harden_youtube_bundle(root)),
        &mut failures,
    );
    run_step(
        "Weibo PROMAX source",
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
                replacements: &[],
                youtube_adblock_output: None,
            },
        ),
        &mut failures,
    );
    run_step(
        "Wool PROMAX source",
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
                replacements: &[],
                youtube_adblock_output: None,
            },
        ),
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
                replacements: &[],
                youtube_adblock_output: None,
            },
        ),
        &mut failures,
    );

    failures
}

pub fn sync_shadowrocket_variants(root: &Path) -> Result<usize, String> {
    let source = root.join("modules/surge");
    let mut count = 0;
    for entry in walkdir::WalkDir::new(&source)
        .into_iter()
        .filter_map(Result::ok)
    {
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
        let errors = crate::promax::validation::validate_surge_module(&converted);
        if !errors.is_empty() {
            return Err(format!(
                "Shadowrocket conversion failed validation for {}",
                entry.path().display()
            ));
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
        RemoteSource, harden_apple_content, harden_youtube_content, sources_from_batch,
        strip_rule_section, valid_module,
    };
    use std::collections::HashMap;

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
}
