//! Bound the Apple bundle to the HTTPS endpoints its features actually use.

use std::collections::HashSet;

use crate::smart_cleanup::split_script_parameters;

const WEATHER_DEFAULTS: &[(&str, &str)] = &[
    (
        "DataSets",
        r#""currentWeather,forecastDaily,forecastHourly,forecastNextHour,weatherAlerts""#,
    ),
    ("Weather.Provider", r#""WeatherKit""#),
    ("WeatherAlerts.Provider", r#""WeatherKit""#),
    ("NextHour.Provider", r#""WeatherKit""#),
    ("AirQuality.Calculate.Algorithm", r#""None""#),
    ("API.ColorfulClouds.Token", r#""""#),
    ("API.QWeather.Token", r#""""#),
    ("API.WAQI.Token", r#""""#),
    ("Storage", r#""Argument""#),
];

const TV_PINNING: (&str, &str) = ("isWorkaroundSSLPinning", "false");

pub(super) const TV_REWRITES: &[(&str, &str)] = &[
    (
        "play-cdn.itunes.apple.com",
        r"^https:\/\/play-cdn\.itunes\.apple\.com(?::443)?([/?].*)?$ https://play.itunes.apple.com$1 header",
    ),
    (
        "play-edge-cdn.itunes.apple.com",
        r"^https:\/\/play-edge-cdn\.itunes\.apple\.com(?::443)?([/?].*)?$ https://play-edge.itunes.apple.com$1 header",
    ),
];

const MITM_HOSTS: &[&str] = &[
    "play-cdn.itunes.apple.com",
    "play-edge-cdn.itunes.apple.com",
    "weatherkit.apple.com",
    "news-edge.apple.com",
    "news-todayconfig-edge.apple.com",
    "news-client-search.apple.com",
    "uts-api.itunes.apple.com",
    "umc-tempo-api.apple.com",
];

pub(super) const OLD_TV_REWRITES: &[&str] = &[
    r"^https?:\/\/play-cdn\.itunes\.apple\.com https://play.itunes.apple.com header",
    r"^https?:\/\/play-edge-cdn\.itunes\.apple\.com https://play-edge.itunes.apple.com header",
];

const SCRIPT_SCOPES: &[(&str, &str, &str)] = &[
    (
        "WeatherKit",
        "api.v1.availability.response",
        r"weatherkit\.apple\.com\/api\/v1\/availability\/",
    ),
    (
        "WeatherKit",
        "api.v1.airQualityScale.request",
        r"weatherkit\.apple\.com\/api\/v1\/airQualityScale\/",
    ),
    (
        "WeatherKit",
        "api.v1.weatherAlerts.html.request",
        r"weatherkit\.apple\.com\/api\/v1\/weatherAlerts\?[^#]*&ids=[^&#]*-[0-9]{9}(?:&|$)",
    ),
    (
        "WeatherKit",
        "api.v1.weatherAlerts.api.request",
        r"weatherkit\.apple\.com\/api\/v1\/weatherAlerts\?[^#]*&ids=-?[0-9]+(?:\.[0-9]+)?,-?[0-9]+(?:\.[0-9]+)?(?:&|$)",
    ),
    (
        "WeatherKit",
        "api.v2.weather.response",
        r"weatherkit\.apple\.com\/api\/v2\/weather\/",
    ),
    (
        "News",
        "v1.configs.request",
        r"news(-todayconfig)?-edge\.apple\.com\/v1\/configs",
    ),
    (
        "News",
        "v1.search.request",
        r"news-client-search\.apple\.com\/v1\/search",
    ),
    (
        "TV",
        "uts.request",
        r"uts-api\.itunes\.apple\.com\/uts\/(v1|v2|v3)\/",
    ),
    (
        "TV",
        "uts.response",
        r"uts-api\.itunes\.apple\.com\/uts\/(v1|v2|v3)\/",
    ),
    (
        "TV",
        "umc.request",
        r"umc-tempo-api\.apple\.com\/(v1|v2|v3)\/",
    ),
];

fn native_defaults(args: &str) -> Result<String, String> {
    let mut seen = HashSet::new();
    let mut output = Vec::new();
    for arg in split_script_parameters(args) {
        let arg = arg.trim();
        let name = arg.split_once(':').map_or(arg, |(key, _)| key.trim());
        if name == "Proxy" {
            continue;
        }
        if let Some(&(name, value)) = WEATHER_DEFAULTS
            .iter()
            .chain([&TV_PINNING])
            .find(|(key, _)| *key == name)
        {
            if !seen.insert(name) {
                return Err(format!("Apple bundle duplicates the {name} argument"));
            }
            output.push(format!("{name}:{value}"));
        } else {
            output.push(arg.to_string());
        }
    }
    if seen.len() != WEATHER_DEFAULTS.len() + 1 {
        return Err("Apple bundle is missing native WeatherKit or TV safety arguments".into());
    }
    Ok(output.join(","))
}

fn scoped_script(line: &str, seen: &mut HashSet<usize>) -> Result<Option<String>, String> {
    let (assignment, parameters) = line.split_once('=').ok_or("invalid Apple script")?;
    let label = assignment
        .split_whitespace()
        .last()
        .ok_or("missing Apple script label")?;
    let label = label.trim_start_matches('');
    if label == "News.analyticseventsv2.async.request" {
        return Ok(None);
    }
    let (idx, &(app, hook, scope)) = SCRIPT_SCOPES
        .iter()
        .enumerate()
        .find(|(_, (app, hook, _))| label.split_once('.') == Some((*app, *hook)))
        .ok_or_else(|| format!("unreviewed Apple script hook: {label}"))?;
    if !seen.insert(idx) {
        return Err(format!("duplicate Apple script hook: {label}"));
    }
    let params = split_script_parameters(parameters);
    let get = |key| {
        let mut values = params.iter().filter_map(|field| {
            let (name, value) = field.split_once('=')?;
            (name.trim() == key).then(|| value.trim())
        });
        let first = values.next()?;
        values.next().is_none().then_some(first)
    };
    let direction = if hook.ends_with(".response") {
        "response"
    } else {
        "request"
    };
    let bundle = format!("{direction}.bundle.js");
    let prefix = format!("https://github.com/NSRingo/{app}/releases/download/");
    let valid_source = get("script-path")
        .and_then(|path| path.strip_prefix(&prefix))
        .and_then(|path| path.split_once('/'))
        .is_some_and(|(version, file)| {
            file == bundle
                && version.as_bytes().get(1).is_some_and(u8::is_ascii_digit)
                && version.starts_with('v')
                && version
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b".-".contains(&byte))
        });
    if !valid_source
        || get("type")
            != Some(if direction == "response" {
                "http-response"
            } else {
                "http-request"
            })
    {
        return Err(format!("unexpected Apple script source or type: {label}"));
    }
    let defaults: &[(&str, &str)] = match app {
        "WeatherKit" => WEATHER_DEFAULTS,
        "TV" if hook.starts_with("uts.") => std::slice::from_ref(&TV_PINNING),
        _ => &[],
    };
    let argument = get("argument").unwrap_or_default();
    if defaults.iter().any(|(key, _)| {
        let reference = format!(r#"{key}="{{{{{{{key}}}}}}}""#);
        argument
            .split('&')
            .filter(|field| *field == reference)
            .count()
            != 1
    }) {
        return Err(format!("Apple safety defaults are not wired to {label}"));
    }
    let pattern = get("pattern").ok_or("missing Apple script pattern")?;
    let boundary = if app == "News" { r"(?:\?|$)" } else { "" };
    let expected = format!(r"^https?:\/\/{scope}");
    let canonical = format!(r"^https:\/\/{scope}{boundary}");
    if ![expected.as_str(), canonical.as_str()].contains(&pattern.trim_matches('"')) {
        return Err(format!("unreviewed Apple script scope: {label}"));
    }
    let parameters = params
        .iter()
        .map(|field| {
            if field
                .split_once('=')
                .is_some_and(|(key, _)| key.trim() == "pattern")
            {
                format!(r#"pattern="{canonical}""#)
            } else {
                field.trim().to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    Ok(Some(format!("{} = {parameters}", assignment.trim())))
}

pub(super) fn harden(content: &str) -> Result<String, String> {
    let mut output = Vec::new();
    let mut section = "";
    let mut hooks = HashSet::new();
    let mut rewrites = HashSet::new();
    let mut has_defaults = false;
    let mut has_mitm = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            section = trimmed;
            match section {
                "[Rule]" => continue,
                "[Script]" | "[URL Rewrite]" | "[MITM]" => output.push(section.to_string()),
                _ => return Err(format!("unreviewed Apple module section: {section}")),
            }
        } else if section == "[Rule]" {
            continue;
        } else if section.is_empty() && trimmed.starts_with("#!arguments=") {
            if has_defaults {
                return Err("duplicate Apple module arguments".into());
            }
            output.push(format!(
                "#!arguments={}",
                native_defaults(&trimmed["#!arguments=".len()..])?
            ));
            has_defaults = true;
        } else if trimmed.starts_with('#') || trimmed.is_empty() || section.is_empty() {
            output.push(line.to_string());
        } else if section == "[Script]" {
            output.extend(scoped_script(line, &mut hooks)?);
        } else if section == "[URL Rewrite]" {
            let idx = TV_REWRITES
                .iter()
                .zip(OLD_TV_REWRITES)
                .position(|((_, canonical), old)| trimmed == *old || trimmed == *canonical)
                .ok_or("unreviewed Apple URL rewrite")?;
            if !rewrites.insert(idx) {
                return Err("duplicate Apple TV rewrite".into());
            }
            output.push(TV_REWRITES[idx].1.to_string());
        } else if section == "[MITM]"
            && !has_mitm
            && trimmed
                .split_once('=')
                .is_some_and(|(key, _)| key.trim() == "hostname")
        {
            let mut hosts = crate::promax::safety::apple_mitm_exclusions();
            hosts.extend(MITM_HOSTS.iter().map(|host| (*host).to_string()));
            output.push(format!("hostname = %INSERT% {}", hosts.join(", ")));
            has_mitm = true;
        } else {
            return Err("unreviewed Apple MITM setting".into());
        }
    }
    if !has_defaults
        || !has_mitm
        || hooks.len() != SCRIPT_SCOPES.len()
        || rewrites.len() != TV_REWRITES.len()
    {
        return Err("Apple module is missing required arguments, hooks, or rewrites".into());
    }
    Ok(output.join("\n") + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tv_rewrites_only_the_exact_https_authority_and_preserves_path_and_query() {
        for (host, rule) in TV_REWRITES {
            let mut fields = rule.split_whitespace();
            let matcher = regex::Regex::new(fields.next().unwrap()).unwrap();
            let replacement = fields.next().unwrap();
            for url in [
                format!("http://{host}/media"),
                format!("https://{host}.example.com/media"),
                format!("https://{host}@example.com/media"),
                format!("https://{host}:444/media"),
            ] {
                assert!(!matcher.is_match(&url), "{url}");
            }
            let sample = format!("https://{host}:443/media/clip?token=fixture");
            let rewritten = matcher.replace(&sample, replacement);
            assert_eq!(
                rewritten,
                replacement.trim_end_matches("$1").to_string() + "/media/clip?token=fixture"
            );
        }
    }

    #[test]
    fn checked_in_bundle_keeps_native_defaults_and_only_reviewed_mitm_hosts() {
        let published =
            include_str!("../../../../modules/surge/amplify_nexus/🍎 Apple服务增强合集.sgmodule");
        assert_eq!(harden(published).unwrap(), published);
        assert!(!published.contains("[Rule]"));
        assert!(!published.contains("news-events.apple.com"));
        assert!(!published.contains("configuration.ls.apple.com"));
        assert!(published.contains("WeatherAlerts.Provider:\"WeatherKit\""));
        assert!(published.contains("-account.apple.com"));

        let drift = published.replace(
            r"weatherkit\.apple\.com\/api\/v1\/availability\/",
            r"weatherkit\.apple\.com\/",
        );
        assert!(harden(&drift).is_err());
        let drift = published.replace("{{{WeatherAlerts.Provider}}}", "QWeatherWeb");
        assert!(harden(&drift).is_err());
        let drift = published.replace(
            "script-path=https://github.com/",
            "script-path=https://github.com.example.net/",
        );
        assert!(harden(&drift).is_err());
        assert!(harden(&(published.to_string() + "skip-server-cert-verify = true\n")).is_err());
    }
}
