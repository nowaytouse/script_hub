use std::collections::HashSet;

use super::apple_surge::{OLD_TV_REWRITES, TV_REWRITES};

pub(super) const TV_SOURCE: &str =
    "https://github.com/NSRingo/TV/releases/latest/download/iRingo.TV.srmodule";
pub(super) const WEATHER_SOURCE: &str = "https://raw.githubusercontent.com/NSRingo/WeatherKit/main/modules/iRingo.WeatherKit.Rewrite.srmodule";
pub(super) const TV_OUTPUT: &str = "modules/shadowrocket/amplify_nexus/🍎 Apple服务增强合集.module";
pub(super) const WEATHER_OUTPUT: &str =
    "modules/shadowrocket/amplify_nexus/🌤 WeatherKit Rewrite (Opt-In).module";

const TV_HOSTS: &[&str] = &[
    "uts-api.itunes.apple.com",
    "umc-tempo-api.apple.com",
    "play-cdn.itunes.apple.com",
    "play-edge-cdn.itunes.apple.com",
];
const WEATHER_HOSTS: &[&str] = &["weatherkit.apple.com"];

const TV_UTS_PATTERN: &str = r"^https?:\/\/uts-api\.itunes\.apple\.com\/uts\/(v1|v2|v3)\/";
const TV_UMC_PATTERN: &str = r"^https?:\/\/umc-tempo-api\.apple\.com\/(v1|v2|v3)\/";

const WEATHER_ARGUMENTS: &str = "endpoint:weatherkit.pages.dev";
const WEATHER_REWRITES: &[&str] = &[
    r"^https?:\/\/weatherkit\.apple\.com\/api\/v1\/availability\/ https://{{{endpoint}}}/api/v1/availability/ header",
    r"^https?:\/\/weatherkit\.apple\.com\/api\/v2\/weather\/ https://{{{endpoint}}}/api/v2/weather/ header",
    r"^https?:\/\/weatherkit\.apple\.com\/api\/v1\/weatherAlerts\?([^#]*&ids=[^&#]*-[0-9]{9}(?:&[^#]*)?)$ https://{{{endpoint}}}/api/v1/weatherAlerts?$1 header",
    r"^https?:\/\/weatherkit\.apple\.com\/api\/v1\/weatherAlerts\?([^#]*&ids=-?[0-9]+(?:\.[0-9]+)?,-?[0-9]+(?:\.[0-9]+)?(?:&[^#]*)?)$ https://{{{endpoint}}}/api/v1/weatherAlerts?$1 header",
];

#[derive(Default)]
struct NativeParts<'a> {
    rewrites: Vec<&'a str>,
    scripts: Vec<&'a str>,
    author: Option<&'a str>,
    arguments: Option<&'a str>,
}

// Accept only the native client sections we publish; upstream routing is never copied.
fn native_parts<'a>(content: &'a str, expected_hosts: &[&str]) -> Result<NativeParts<'a>, String> {
    let mut parts = NativeParts::default();
    let mut section = "";
    let mut sections = HashSet::new();
    let mut saw_hostname = false;
    for line in content.lines().map(str::trim) {
        if let Some((key, value)) = line
            .strip_prefix("#!")
            .and_then(|line| line.split_once('='))
        {
            let field = match key.trim() {
                "author" => &mut parts.author,
                "arguments" => &mut parts.arguments,
                _ => continue,
            };
            if !section.is_empty() || field.replace(value.trim()).is_some() {
                return Err("unexpected Shadowrocket metadata placement or duplication".into());
            }
        } else if line.is_empty() || line.starts_with('#') {
            continue;
        } else if line.starts_with('[') && line.ends_with(']') {
            section = line;
            if !matches!(section, "[Rule]" | "[URL Rewrite]" | "[Script]" | "[MITM]")
                || !sections.insert(section)
            {
                return Err(format!("unexpected Shadowrocket section {section}"));
            }
        } else {
            match section {
                "[Rule]" => {}
                "[URL Rewrite]" => parts.rewrites.push(line),
                "[Script]" => parts.scripts.push(line),
                "[MITM]" if !saw_hostname => {
                    let Some(hosts) = line.split_once('=').and_then(|(key, value)| {
                        (key.trim() == "hostname")
                            .then(|| value.trim().strip_prefix("%APPEND%"))
                            .flatten()
                    }) else {
                        return Err("unexpected Shadowrocket MITM directive".into());
                    };
                    let hosts: Vec<_> = hosts.split(',').map(str::trim).collect();
                    if !same_entries(&hosts, expected_hosts) {
                        return Err("upstream Shadowrocket MITM host list changed".into());
                    }
                    saw_hostname = true;
                }
                _ => return Err("unexpected Shadowrocket content".into()),
            }
        }
    }
    if !saw_hostname {
        return Err("upstream Shadowrocket module has no MITM hosts".into());
    }
    Ok(parts)
}

fn same_entries(actual: &[&str], expected: &[&str]) -> bool {
    actual.len() == expected.len() && expected.iter().all(|entry| actual.contains(entry))
}

fn tv_hook(line: &str) -> Option<(&str, &str)> {
    let (_, params) = line.split_once('=')?;
    let mut hook_type = None;
    let mut pattern = None;
    let mut script = None;
    let mut requires_body = None;
    let mut argument = None;
    for part in crate::smart_cleanup::split_script_parameters(params) {
        let (key, value) = part.split_once('=')?;
        let slot = match key.trim() {
            "type" => &mut hook_type,
            "pattern" => &mut pattern,
            "script-path" => &mut script,
            "requires-body" => &mut requires_body,
            "argument" => &mut argument,
            _ => return None,
        };
        if slot.replace(value.trim()).is_some() {
            return None;
        }
    }
    let key = (hook_type?, pattern?);
    let expected_script = match key {
        ("http-request", TV_UTS_PATTERN | TV_UMC_PATTERN) => "request.bundle.js",
        ("http-response", TV_UTS_PATTERN) => "response.bundle.js",
        _ => return None,
    };
    let (version, filename) = script?
        .strip_prefix("https://github.com/NSRingo/TV/releases/download/")?
        .split_once('/')?;
    (filename == expected_script
        && !version.is_empty()
        && version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".-".contains(&byte))
        && requires_body == Some("1")
        && argument.unwrap_or("").is_empty())
    .then_some(key)
}

pub(super) fn render_tv(content: &str) -> Result<String, String> {
    let parts = native_parts(content, TV_HOSTS)?;
    let author = parts.author.unwrap_or("NSRingo");
    let hooks: HashSet<_> = parts
        .scripts
        .iter()
        .filter_map(|line| tv_hook(line))
        .collect();
    if parts.arguments.is_some()
        || parts.rewrites.len() != TV_REWRITES.len()
        || !OLD_TV_REWRITES
            .iter()
            .zip(TV_REWRITES)
            .all(|(old, (_, new))| parts.rewrites.contains(old) || parts.rewrites.contains(new))
        || parts.scripts.len() != 3
        || hooks.len() != 3
    {
        return Err("upstream Shadowrocket TV hooks changed; review before publishing".into());
    }

    let rewrites = TV_REWRITES
        .iter()
        .map(|(_, line)| *line)
        .collect::<Vec<_>>()
        .join("\n");
    let scripts = parts.scripts.join("\n");
    Ok(format!(
        "#!name=🍎 Apple服务增强合集\n#!desc=[🚀SR] 仅 Apple TV 官方支持的功能；不含 News、MapKit 或天气改写，不改分流。\n#!author={author}, ScriptHub\n#!homepage=https://nsringo.github.io/guide/apple-tv.html\n#!category=『 🌐 Amplify Nexus › 增益中枢 』\n#!tag=Apple, TV\n# Source: {TV_SOURCE}\n\n[URL Rewrite]\n{rewrites}\n\n[Script]\n{scripts}\n\n[MITM]\nhostname = %APPEND% {}\n",
        TV_HOSTS.join(", ")
    ))
}

pub(super) fn render_weather(content: &str) -> Result<String, String> {
    let parts = native_parts(content, WEATHER_HOSTS)?;
    if parts.arguments != Some(WEATHER_ARGUMENTS)
        || !parts.scripts.is_empty()
        || !same_entries(&parts.rewrites, WEATHER_REWRITES)
    {
        return Err(
            "upstream Shadowrocket WeatherKit rewrites changed; review before publishing".into(),
        );
    }
    let author = parts.author.unwrap_or("NSRingo");
    let rewrites = parts.rewrites.join("\n");
    Ok(format!(
        "#!name=🌤 WeatherKit Rewrite (Opt-In)\n#!desc=[🚀SR] 可选：将天气、可用性和预警请求发送到第三方端点。保持禁用即可继续使用 Apple 原生天气；不改分流。\n#!author={author}, ScriptHub\n#!homepage=https://nsringo.github.io/guide/Weather/weather-kit.html\n#!category=『 🌐 Amplify Nexus › 增益中枢 』\n#!tag=Apple, WeatherKit, Opt-In\n#!arguments={WEATHER_ARGUMENTS}\n#!arguments-desc=endpoint: iRingo 天气改写服务（默认 weatherkit.pages.dev）\n# Source: {WEATHER_SOURCE}\n\n[URL Rewrite]\n{rewrites}\n\n[MITM]\nhostname = %APPEND% {}\n",
        WEATHER_HOSTS.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weather_source() -> String {
        format!(
            "#!name = Weather\n#!arguments = {WEATHER_ARGUMENTS}\n[Rule]\nDOMAIN,weather-analytics-events.apple.com,REJECT-DROP\n[URL Rewrite]\n{}\n[MITM]\nhostname = %APPEND% weatherkit.apple.com\n",
            WEATHER_REWRITES.join("\n")
        )
    }

    fn tv_source() -> String {
        let script = "https://github.com/NSRingo/TV/releases/download/v3.4.4";
        format!(
            "#!name = TV\n[Rule]\nDOMAIN,known-issues.apple.com,REJECT\n[URL Rewrite]\n{}\n[Script]\nreq = type=http-request, pattern={TV_UTS_PATTERN}, requires-body=1, script-path={script}/request.bundle.js, argument=\nresp = type=http-response, pattern={TV_UTS_PATTERN}, requires-body=1, script-path={script}/response.bundle.js, argument=\numc = type=http-request, pattern={TV_UMC_PATTERN}, requires-body=1, script-path={script}/request.bundle.js, argument=\n[MITM]\nhostname = %APPEND% {}\n",
            OLD_TV_REWRITES.join("\n"),
            TV_HOSTS.join(", ")
        )
    }

    #[test]
    fn native_tv_keeps_the_three_hooks_and_no_weather_news_or_routing() {
        let output = render_tv(&tv_source()).unwrap();
        assert!(crate::promax::validation::validate_shadowrocket_module(&output).is_empty());
        assert!(!output.contains("[Rule]"));
        assert!(!output.contains("weatherkit.apple.com"));
        assert!(!output.contains("news-edge.apple.com"));
        for (_, line) in TV_REWRITES {
            assert!(output.contains(line));
        }
        assert_eq!(output.matches("script-path=").count(), 3);
        assert!(render_tv(&tv_source().replace("NSRingo/TV", "NSRingo/News")).is_err());
    }

    #[test]
    fn weather_forwarding_is_separate_and_rejects_risky_upstream_drift() {
        let source = weather_source();
        let output = render_weather(&source).unwrap();
        assert!(crate::promax::validation::validate_shadowrocket_module(&output).is_empty());
        assert!(!output.contains("[Rule]"));
        assert!(!output.contains("[Script]"));
        assert!(output.contains("第三方端点"));
        assert!(render_weather(&source.replace("[Rule]", "[Host]")).is_err());
        assert!(
            render_weather(&source.replace("weatherkit.apple.com\n", "*.apple.com\n")).is_err()
        );
        assert!(
            render_weather(&source.replace("[MITM]\n", "[MITM]\nskip-server-cert-verify = true\n"))
                .is_err()
        );
        assert!(render_weather(&source.replace("{{{endpoint}}}", "unreviewed.example")).is_err());
    }
}
