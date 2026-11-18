use std::collections::{BTreeMap, HashSet};

const HEIDAI_SUPPLEMENT: &str =
    "https://raw.githubusercontent.com/217heidai/adblockfilters/main/rules/myblock.txt";
const HEIDAI_ADS: &[&str] = &[
    "adx-data.yfanads.com",
    "api.yfanads.com",
    "tracker.yfanads.com",
];
pub(super) const ADGUARD_MOBILE_ADSERVERS: &str = "https://raw.githubusercontent.com/AdguardTeam/AdguardFilters/master/MobileFilter/sections/adservers.txt";
const REVIEWED_MOBILE_ADSERVERS: &[&str] = &[
    "ad-creatives-public.commondatastorage.googleapis.com",
    "adhub.porcino-kinoko.workers.dev",
    "admarvel.s3.amazonaws.com",
    "adpublisher.s3.amazonaws.com",
    "ads-bidder-api.twitter.com",
    "adstrategy.biz.weibo.com",
    "appgiftwall.oss-us-west-1.aliyuncs.com",
    "appnext-a.akamaihd.net",
    "goshare-fleet-ads.s3.ap-southeast-1.amazonaws.com",
    "inmobisdk-a.akamaihd.net",
    "mads.amazon.com",
    "nativead.s3.amazonaws.com",
    "p16-ad.byteoversea.com",
    "p3-ad-sign.byteimg.com",
    "supersonicads-a.akamaihd.net",
    "tapdaq-adverts.global.ssl.fastly.net",
    "v16-ad.byteoversea.com",
    "yc-ads.s3.amazonaws.com",
];
pub(super) const AWAVENUE_ADS_ONLY: &str = "https://raw.githubusercontent.com/TG-Twilight/AWAvenue-Ads-Rule/main/Filters/AWAvenue-Ads-Rule-Adguard-Only.Ads.txt";
pub(super) const ADGUARD_TRACKING_SERVERS: &str = "https://raw.githubusercontent.com/AdguardTeam/AdguardFilters/master/SpywareFilter/sections/tracking_servers.txt";
const REVIEWED_ADGUARD_TRACKERS: &[&str] = &[
    "analytics.authoritymedia.com",
    "analytics.m7g.twitch.tv",
    "analytics.pgncs.notion.so",
    "log.mediacategory.com",
    "pixel.nudgify.com",
    "track.fusionmedia.io",
    "trackszn.median.eu",
];

const BRAVE_FIRST_PARTY: &str =
    "https://raw.githubusercontent.com/brave/adblock-lists/master/brave-lists/brave-firstparty.txt";
const BRAVE_REVIEWED_AD_HOSTS: &[&str] = &[
    "adserver.cgchannel.com",
    "ads.coingecko.com",
    "dmads.dailymotion.com",
    "edge.ads.twitch.tv",
];
const UBO_PRIVACY: &str = "https://ublockorigin.github.io/uAssetsCDN/filters/privacy.min.txt";
const UBO_BADWARE: &str = "https://ublockorigin.github.io/uAssetsCDN/filters/badware.min.txt";
const UBO_REVIEWED_TRACKERS: &[&str] = &[
    "analytics-service.api.linkme.global",
    "analytics.dawg.network",
    "analytics.fmovies4u.com",
    "analytics.freezehost.pro",
    "analytics.fsicomics.com",
    "analytics.fullstak.pl",
    "analytics.meaww.com",
    "analytics.strapi.io",
    "analytics.tiendaenoferta.com",
    "metrics.surinenglish.com",
    "stats.index.hr",
    "stats.pronouns.page",
    "stats.webgames.io",
    "track.vtcnews.vn",
    "umami.webtor.io",
];
const GREEK_FILTER: &str =
    "https://raw.githubusercontent.com/kargig/greek-adblockplus-filter/master/void-gr-filters.txt";
const GREEK_REVIEWED_AD_HOSTS: &[&str] = &[
    "ad.gr.doubleclick.net",
    "ad2.linkwise.gr",
    "adman.kathimerini.gr",
    "adman.otenet.gr",
    "ads.24media.gr",
    "ads.naftemporiki.gr",
    "adserver.linkwise.gr",
    "banners.dol.gr",
    "gazzetta.adman.gr",
    "obj.adman.gr",
    "phaistos-ssp.adman.gr",
    "serve.ads.gr",
    "static.adman.gr",
];

fn host(text: &str) -> Option<String> {
    let canonical = text.to_ascii_lowercase();
    (canonical.len() <= 253
        && canonical.parse::<std::net::IpAddr>().is_err()
        && canonical.contains('.')
        && canonical.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        }))
    .then_some(canonical)
}

fn suffixes(host: &str) -> impl Iterator<Item = &str> {
    std::iter::successors(Some(host), |name| {
        name.split_once('.').map(|(_, tail)| tail)
    })
}

pub(super) fn category(source: &str) -> Option<&'static str> {
    match source {
        BRAVE_FIRST_PARTY
        | GREEK_FILTER
        | HEIDAI_SUPPLEMENT
        | ADGUARD_MOBILE_ADSERVERS
        | AWAVENUE_ADS_ONLY => Some("Advertising"),
        UBO_PRIVACY | ADGUARD_TRACKING_SERVERS => Some("Privacy"),
        UBO_BADWARE => Some("ThreatIntel"),
        _ => None,
    }
}

fn exception_host(rule: &str) -> Result<Option<(String, bool)>, &'static str> {
    let (target, options) = rule.split_once('$').unwrap_or((rule, ""));
    if !options.is_empty() {
        let options = options.split(',').collect::<Vec<_>>();
        let modifier = |option: &str| option == "removeparam" || option.starts_with("removeparam=");
        if options.iter().any(|option| modifier(option))
            && options.iter().all(|option| {
                modifier(option)
                    || matches!(*option, "doc" | "document")
                    || option.starts_with("to=")
                    || option.starts_with("from=")
                    || option.starts_with("domain=")
            })
            || options
                .iter()
                .any(|option| matches!(*option, "elemhide" | "generichide" | "specifichide"))
                && options.iter().all(|option| {
                    matches!(*option, "elemhide" | "generichide" | "specifichide")
                        || option.starts_with("domain=")
                })
        {
            return Ok(None);
        }
    }
    let anchor = target
        .strip_prefix("||")
        .or_else(|| target.strip_prefix("|https://"))
        .or_else(|| target.strip_prefix("|http://"))
        .ok_or("network exception has no bounded request host")?;
    let boundary = anchor.find(['/', '^', '|', '?']);
    let authority = boundary.map_or(anchor, |index| &anchor[..index]);
    let (name, has_port) = if let Some((name, port)) = authority.split_once(':') {
        if !port.bytes().all(|byte| byte.is_ascii_digit()) || port.parse::<u16>().is_err() {
            return Err("network exception has an unsupported request port");
        }
        (name, true)
    } else {
        (authority, false)
    };
    // For wildcard exceptions preserve the fixed suffix. Candidates inside
    // that suffix must not become unconditional blocks.
    if let Some((_, pattern)) = name.rsplit_once('*') {
        return pattern
            .split_once('.')
            .and_then(|(_, suffix)| host(suffix))
            .map(|suffix| Some((suffix, true)))
            .ok_or("network exception has no bounded wildcard suffix");
    }
    host(name.strip_suffix('.').unwrap_or(name))
        .map(|name| Some((name, boundary.is_some() || has_port)))
        .ok_or("network exception has an unsupported request host")
}

pub(super) fn project(source: &str, text: &str) -> Result<Vec<(usize, String)>, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let reviewed = match source {
        BRAVE_FIRST_PARTY => Some(BRAVE_REVIEWED_AD_HOSTS),
        UBO_PRIVACY => Some(UBO_REVIEWED_TRACKERS),
        GREEK_FILTER => Some(GREEK_REVIEWED_AD_HOSTS),
        HEIDAI_SUPPLEMENT => Some(HEIDAI_ADS),
        ADGUARD_MOBILE_ADSERVERS => Some(REVIEWED_MOBILE_ADSERVERS),
        ADGUARD_TRACKING_SERVERS => Some(REVIEWED_ADGUARD_TRACKERS),
        UBO_BADWARE | AWAVENUE_ADS_ONLY => None,
        _ => return Err("browser filter source is not reviewed".into()),
    };
    let mut blocks = BTreeMap::new();
    let mut exceptions = HashSet::new();
    let mut prefixes = HashSet::new();
    let mut conditions = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        let error = |reason| format!("browser filter line {}: {reason}", index + 1);
        if let Some(directive) = line.strip_prefix("!#") {
            // CNAME sections have their own source policy. This projection uses only
            // the reviewed hosts present in the tracker file itself.
            let separate_cname_source = directive
                .strip_prefix("include https://adguardteam.github.io/cname-trackers/data/")
                .and_then(|path| path.split_once('/'))
                .is_some_and(|(kind, file)| {
                    matches!(kind, "trackers" | "mail_trackers" | "microsites")
                        && file.ends_with(".txt")
                        && file
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.()".contains(&byte))
                });
            match directive.split_ascii_whitespace().next() {
                Some("include") if source == ADGUARD_TRACKING_SERVERS && separate_cname_source => {}
                Some("if") => conditions.push(false),
                Some("else") if conditions.last() == Some(&false) => {
                    if let Some(branch) = conditions.last_mut() {
                        *branch = true;
                    }
                }
                Some("endif") if conditions.pop().is_some() => {}
                _ => return Err(error("unsupported or unmatched preprocessor directive")),
            }
        } else if let Some(rule) = line.strip_prefix("@@") {
            if let Some((name, bounded)) = exception_host(rule).map_err(error)? {
                if bounded {
                    exceptions.insert(name);
                } else if reviewed.is_some() {
                    prefixes.insert(name);
                } else {
                    return Err(error(
                        "unbounded exception in a non-curated feed needs review",
                    ));
                }
            }
        } else if conditions.is_empty()
            && let Some(name) = line
                .strip_prefix("||")
                .and_then(|line| line.strip_suffix('^'))
                .and_then(host)
            && psl::suffix(name.as_bytes()).is_some_and(|suffix| suffix.is_known())
            && psl::domain_str(&name).is_some()
            && reviewed.is_none_or(|names| names.contains(&name.as_str()))
        {
            blocks.entry(name).or_insert(index + 1);
        }
    }
    if !conditions.is_empty() {
        return Err("browser filter has an unclosed conditional".into());
    }
    // Keep the generic Badware projection conservative when a child is excepted.
    let exception_parents: HashSet<_> = exceptions.iter().flat_map(|name| suffixes(name)).collect();
    Ok(blocks
        .into_iter()
        .filter(|(name, _)| {
            !suffixes(name).any(|suffix| exceptions.contains(suffix))
                && (reviewed.is_some() || !exception_parents.contains(name.as_str()))
                && !suffixes(name)
                    .any(|suffix| prefixes.iter().any(|prefix| suffix.starts_with(prefix)))
        })
        .map(|(name, line)| (line, format!("DOMAIN,{name}")))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_browser_rules_and_exceptions_never_become_network_blocks() {
        let text = "||AD.valid.com^\n||safe.com^\n@@||www.safe.com/image$domain=site.com\n\
                    ||ads.nested.com^\n@@||nested.com^\n||option.com^$third-party\n\
                    ||paths.com/path\n||wild*.com^\n||end.com^|\n||co.uk^\n||printer.local^\n||127.1^\n\
                    site.com##.advert\n!#if env_chromium\n||browser.com^\n!#else\n\
                    ||else.com^\n!#endif\n@@||archive.*/http$removeparam,doc,to=archive.test\n";
        assert_eq!(
            project(UBO_BADWARE, text).unwrap(),
            [(1, "DOMAIN,ad.valid.com".into())]
        );
    }

    #[test]
    fn supplemental_ads_keep_exceptions_scoped_to_their_feed() {
        assert_eq!(
            project(
                HEIDAI_SUPPLEMENT,
                "||api.yfanads.com^\n||ordinary.example.com^\n@@||r.*.awstrack.me^\n\
                 @@||lients2.googleusercontent.com/crx/blobs/Ad_*\n",
            )
            .unwrap(),
            [(1, "DOMAIN,api.yfanads.com".into())],
        );
        assert_eq!(
            project(
                AWAVENUE_ADS_ONLY,
                "||ad.example.com^\n||mail.awstrack.me^\n@@||r.*.awstrack.me^\n\
                 ||thirdparty.example.com^$third-party\n||wild*.example.com^\n",
            )
            .unwrap(),
            [(1, "DOMAIN,ad.example.com".into())],
        );
        assert_eq!(
            project(
                ADGUARD_MOBILE_ADSERVERS,
                "||mads.amazon.com^\n||iadsdk.apple.com^\n||another.example.com^",
            )
            .unwrap(),
            [(1, "DOMAIN,mads.amazon.com".into())],
        );
    }

    #[test]
    fn unsupported_exceptions_and_preprocessing_fail_closed() {
        for line in [
            "@@/script.js$domain=site.test",
            "@@||ad.test$script",
            "@@|https://login.example.test:p@ad.test/path",
            "!#include nested.txt",
            "!#if env_firefox",
            "!#endif",
        ] {
            assert!(project(UBO_BADWARE, &format!("||ad.test^\n{line}")).is_err());
        }
        assert!(project("https://example.test/unknown", "||ad.test^").is_err());
    }

    #[test]
    fn brave_mixed_source_can_only_supply_reviewed_exact_ads() {
        assert_eq!(
            project(
                BRAVE_FIRST_PARTY,
                "||edge.ads.twitch.tv^\n||login.example.com^\n"
            )
            .unwrap(),
            [(1, "DOMAIN,edge.ads.twitch.tv".into())]
        );
    }

    #[test]
    fn mixed_regional_and_privacy_feeds_do_not_block_unreviewed_sites() {
        assert_eq!(
            project(GREEK_FILTER, "||ads.24media.gr^\n||gr.partypoker.com^\n").unwrap(),
            [(1, "DOMAIN,ads.24media.gr".into())]
        );
        assert_eq!(
            project(
                UBO_PRIVACY,
                "||analytics.fsicomics.com^\n||copyhomework.com^\n"
            )
            .unwrap(),
            [(1, "DOMAIN,analytics.fsicomics.com".into())]
        );
    }

    #[test]
    fn unbounded_exceptions_veto_reviewed_names_at_label_boundaries() {
        let text = "||edge.ads.twitch.tv^\n@@||ads.twitch.t$script\n";
        assert!(project(BRAVE_FIRST_PARTY, text).unwrap().is_empty());
    }

    #[test]
    fn adguard_tracking_projects_reviewed_exact_hosts_without_service_traffic() {
        let text = "||analytics.authoritymedia.com^\n\
                    ||analytics.m7g.twitch.tv^\n@@||analytics.m7g.twitch.tv^\n\
                    ||analytics.pgncs.notion.so^$third-party\n\
                    ||api.twitch.tv^\n||www.notion.so^\n||doh.dns.apple.com^\n\
                    ||extra.analytics.authoritymedia.com^\n\
                    !#include https://adguardteam.github.io/cname-trackers/data/trackers/actioniq.txt\n";
        assert_eq!(category(ADGUARD_TRACKING_SERVERS), Some("Privacy"));
        assert_eq!(
            project(ADGUARD_TRACKING_SERVERS, text).unwrap(),
            [(1, "DOMAIN,analytics.authoritymedia.com".into())],
        );
        assert!(
            project(
                ADGUARD_TRACKING_SERVERS,
                "!#include https://unreviewed.example.com/tracking.txt\n"
            )
            .is_err()
        );
    }

    #[test]
    fn bom_cannot_hide_the_first_exception() {
        let text = "\u{feff}@@||ads.example.com^\n||ads.example.com^\n";
        assert!(project(UBO_BADWARE, text).unwrap().is_empty());
    }

    #[test]
    fn shared_public_suffixes_can_veto_blocks_but_cannot_be_blocked() {
        let text =
            "||cloudfront.net^\n||ads.cloudfront.net^\n@@||cloudfront.net^$domain=example.com\n";
        assert!(project(UBO_BADWARE, text).unwrap().is_empty());
    }

    #[test]
    #[ignore = "fetches current public browser filter sources"]
    fn live_reviewed_feeds_project_without_scope_errors() {
        use crate::promax::source::{DownloadConfig, Downloader};
        use std::time::Duration;

        let downloader = Downloader::new(DownloadConfig {
            attempts: 2,
            timeout: Duration::from_secs(25),
            batch_timeout: Duration::from_secs(55),
            concurrency: 1,
            max_bytes: 2 * 1024 * 1024,
            ..DownloadConfig::default()
        });
        for source in [
            BRAVE_FIRST_PARTY,
            UBO_PRIVACY,
            UBO_BADWARE,
            GREEK_FILTER,
            ADGUARD_TRACKING_SERVERS,
        ] {
            let body = downloader.get(source).unwrap();
            let rules = project(source, &body).unwrap_or_else(|error| panic!("{source}: {error}"));
            assert!(!rules.is_empty(), "{source}");
            println!(
                "{source}: {} projected candidates before shared safety and deduplication",
                rules.len()
            );
        }
    }
}
