//! Editable native operations inspired by Entrance, not a JavaScript compatibility layer.
use super::{
    dns,
    pipeline::{AddressFamily, NameCase, Operation, SortKey, Step},
    properties::Setting,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Safe,
    Compatible,
}

/// IDs are unique within the returned recipe; reassign them when appending to another pipeline.
/// Filtering and network resolution are disabled until explicitly enabled by the user.
pub fn steps(preset: Preset) -> Vec<Step> {
    let operations = [
        (
            preset == Preset::Safe,
            Operation::SetProperty {
                setting: Setting::VerifyTls,
            },
        ),
        (
            false,
            Operation::ProtocolFilter {
                protocols: vec!["anytls".into(), "vless".into(), "hysteria2".into()],
                keep: true,
            },
        ),
        (
            false,
            Operation::ResolveDomain {
                family: AddressFamily::V4,
                options: dns::Options {
                    cache: false,
                    ..dns::Options::default()
                },
            },
        ),
        (true, Operation::Deduplicate),
        (
            true,
            Operation::NormalizeName {
                case: NameCase::Preserve,
                trim: true,
                collapse_whitespace: true,
            },
        ),
        (true, Operation::Flags { add: true }),
        (
            true,
            Operation::UniqueNames {
                separator: " ".into(),
            },
        ),
        (
            true,
            Operation::Sort {
                key: SortKey::NaturalName,
                descending: false,
            },
        ),
    ];
    operations
        .into_iter()
        .enumerate()
        .map(|(i, (enabled, operation))| Step {
            id: i as u64 + 1,
            enabled,
            operation,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::model::{EnhanceOptions, Health, Subscription, editor, pipeline};

    #[test]
    fn presets_are_valid_offline_and_distinguish_tls_policy() {
        for preset in [Preset::Safe, Preset::Compatible] {
            let recipe = steps(preset);
            assert!(pipeline::validate(&recipe).is_ok());
            assert!(!pipeline::has_resolution(&recipe));
            assert_eq!(recipe[0].enabled, preset == Preset::Safe);
            assert!(!recipe[1].enabled);
            assert!(!recipe[2].enabled);
            assert!(matches!(
                recipe[2].operation,
                Operation::ResolveDomain {
                    family: AddressFamily::V4,
                    ..
                }
            ));
        }
    }

    #[test]
    fn safe_verifies_supported_tls_and_compatible_preserves_it() {
        let source = "B = trojan, b.example, 443, password=secret, skip-cert-verify=true\nA = trojan, a.example, 443, password=secret, skip-cert-verify=true\n";
        for preset in [Preset::Safe, Preset::Compatible] {
            let sub = Subscription::parse(source).unwrap();
            let result = sub
                .render_processed(
                    &vec![Health::Unknown; sub.nodes().len()],
                    &EnhanceOptions {
                        rename: false,
                        sort: false,
                    },
                    &editor::Options {
                        pipeline: steps(preset),
                        include: String::new(),
                        exclude: String::new(),
                        find: String::new(),
                        replace: String::new(),
                        nodes: Vec::new(),
                        deduplicate: false,
                    },
                )
                .unwrap();
            assert_eq!(
                result.content.contains("skip-cert-verify=false"),
                preset == Preset::Safe
            );
            assert_eq!(
                result.content.contains("skip-cert-verify=true"),
                preset == Preset::Compatible
            );
            assert!(result.content.find("A =").unwrap() < result.content.find("B =").unwrap());
        }
    }
}
