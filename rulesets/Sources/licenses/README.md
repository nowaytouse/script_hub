# Sources & licenses

[English](README.md) · [简体中文](README.zh-CN.md)

| Source | License |
| --- | --- |
| [Sub-Store Beta module](https://github.com/sub-store-org/Sub-Store/blob/master/config/Surge-Beta.sgmodule) (upstream snapshot; the tools bundle selects its handlers and overrides local defaults) | [AGPL-3.0](Sub-Store-AGPL-3.0.txt) |
| [app2smile/rules](https://github.com/app2smile/rules) | [MIT](app2smile-MIT.txt) |
| [AdGuard CNAME](https://github.com/AdguardTeam/cname-trackers) | [MIT](AdGuard-CNAME-MIT.txt) |
| [217heidai/adblockfilters maintainer supplement](https://github.com/217heidai/adblockfilters/blob/main/rules/myblock.txt) | [GPL-3.0](https://github.com/217heidai/adblockfilters/blob/main/LICENSE) |
| [AdGuard Mobile Ads ad-server section](https://github.com/AdguardTeam/AdguardFilters/blob/master/MobileFilter/sections/adservers.txt) | [GPL-3.0](https://github.com/AdguardTeam/AdguardFilters/blob/master/LICENSE) |
| [AdGuard Tracking Protection tracker-server section](https://github.com/AdguardTeam/AdguardFilters/blob/master/SpywareFilter/sections/tracking_servers.txt), reviewed exact analytics hosts | [GPL-3.0](https://github.com/AdguardTeam/AdguardFilters/blob/master/LICENSE) |
| [AWAvenue Ads Rule Only.Ads](https://github.com/TG-Twilight/AWAvenue-Ads-Rule/blob/main/Filters/AWAvenue-Ads-Rule-Adguard-Only.Ads.txt) | [GPL-3.0](https://github.com/TG-Twilight/AWAvenue-Ads-Rule/blob/main/LICENSE) |
| [Steven Black — own hosts only](https://github.com/StevenBlack/hosts/tree/master/data/StevenBlack) | [MIT](StevenBlack-MIT.txt) |
| [MetaCubeX](https://github.com/MetaCubeX/meta-rules-dat) | [GPL-3.0](https://github.com/MetaCubeX/meta-rules-dat/blob/master/LICENSE) |
| [GeoIP All in One country snapshots](https://github.com/daijro/geoip-all-in-one) | [Upstream license and source attributions](https://github.com/daijro/geoip-all-in-one#licenses) |
| [laincat GeoIP ASN database](https://github.com/laincat/GeoIP), built from [IPtoASN](https://iptoasn.com/) | [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) for the release data; the FlClash template links to the upstream file |
| [fmz200 — X Web Ads endpoint and response schema](https://github.com/fmz200/wool_scripts/blob/main/Shadowrocket/module/XWebAds.srmodule) | [GPL-3.0](https://github.com/fmz200/wool_scripts/blob/main/LICENSE) |
| [HaGeZi](https://github.com/hagezi/dns-blocklists) | [GPL-3.0](https://github.com/hagezi/dns-blocklists/blob/main/LICENSE) |
| [V2Fly Domain List Community](https://github.com/v2fly/domain-list-community), private devices and reviewed service domains | [MIT](V2Fly-MIT.txt) |
| [Brave First-Party filters — reviewed advertising hosts only](https://github.com/brave/adblock-lists/blob/master/brave-lists/brave-firstparty.txt) | [MPL-2.0](https://github.com/brave/adblock-lists/blob/master/LICENSE) |
| [uBlock Origin uAssets — reviewed Privacy hosts and Badware network projection](https://github.com/uBlockOrigin/uAssets/tree/master/filters) | [GPL-3.0](https://github.com/uBlockOrigin/uAssets/blob/master/LICENSE) |
| [Greek AdBlock Filter — reviewed advertising hosts only](https://github.com/kargig/greek-adblockplus-filter/blob/master/void-gr-filters.txt) | [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) |

The [source manifests](../Links/) define scope. Conversion, deduplication, classification and quarantine preserve upstream licenses; app settings retain normal video posts, and the full CNAME clickthrough / microsites lists remain disabled.

The advertising compiler reads 217heidai's maintained `myblock.txt` supplement and the AdGuard and AWAvenue originals directly. It projects unconditional domain blocks to exact hostname rules and evaluates each source's exceptions before merging. Shared service protections also apply to the projected rules. The published selection and format conversion retain the source attributions and GPL-3.0 terms.

The `compiled/geoip` snapshots select country codes from the AIO MMDB by `country.iso_code` and aggregate adjacent CIDRs with the same country. Each list records its source URL, build epoch and SHA-256. These snapshots include IP2Location LITE data for [IP geolocation](https://lite.ip2location.com/), GeoLite2 Data created by [MaxMind](https://www.maxmind.com/), and [IP Geolocation by DB-IP](https://db-ip.com/). The upstream project documents the additional country sources and merge procedure.

## Brave browser selections

The [Brave catalog](https://github.com/brave/adblock-resources/blob/master/filter_lists/list_catalog.json), [uBO catalog](https://github.com/gorhill/uBlock/blob/master/assets/assets.json), and [AdGuard DNS source inventory](https://github.com/AdguardTeam/AdGuardSDNSFilter#dns-filter-structure) describe browser/DNS products, not interchangeable proxy policies.

| Checked options | Network intake |
| --- | --- |
| Brave default ad blocking, default privacy, and first-party filters | The existing AdGuard DNS aggregate includes EasyList and EasyPrivacy. The manifest adds reviewed first-party Brave ads and uBO privacy hosts, plus the separate uBO Badware threat category. It does not import the full Brave/uBO default bundles or Brave's first-party regional file. |
| Greek; AdGuard Chinese, Dutch, French, Japanese, Russian, Spanish/Portuguese, Turkish; EasyList Germany, Hebrew, Italy; ABPindo, List-KR, Swedish and Persian | The Greek list supplies reviewed advertising hosts. The existing DNS aggregate already includes relevant regional components for the other named languages; this is not full browser-list parity. |
| Arabic, Bulgarian, Chinese annoyances, Czech/Slovak, Estonian, Finnish, Hindi, Hungarian, Icelandic, Latvian, Lithuanian, Macedonian, Nordic, Polish, Romanian, RU AdList, Slovenian, EasyList Spanish, Thai, Vietnamese ABPVN | Not newly imported as whole network lists. A browser subscription may combine path rules, contextual exceptions, non-ad categories or distinct licensing; the Polish source is non-commercial. |
| X/Twitter embeds; cookie notices, distractions, AI suggestions, newsletters, mobile app promos, social widgets, chat apps and experimental filters | Browser-only. Embed exceptions are not blocks or `DIRECT` rules, and page element changes are not network rejections. |
| YouTube Shorts, mobile recommendations, autodubbed videos, end-screen elements, members-only promotions and mobile distractions; URL Tracking Protection | Browser-only. Element selectors, scriptlets and query-parameter removal are not converted to host rejections. |
| Paywall blocker | Not imported. |

Greek source: Greek AdBlock Filter contributors, [original list](https://github.com/kargig/greek-adblockplus-filter/blob/master/void-gr-filters.txt). The generated subset is modified (selection and format conversion); its source-derived data remains under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/).

Only some sources have been reviewed; future maintenance is not guaranteed. [Full notice](../../../THIRD_PARTY_NOTICES.md)
