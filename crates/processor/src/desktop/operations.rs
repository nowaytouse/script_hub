//! Labels, defaults and identifiers for desktop processing steps.

use super::i18n::Text;
use crate::relay::model::dns;
use crate::relay::model::pipeline::{
    AddressFamily, Condition, Field, KeepOccurrence, MatchMode, NameCase, Operation,
    RegexSortUnmatched, RegionLabel, RenameRule, SortKey, Step, TextMatch,
};
use crate::relay::model::properties::Setting;

pub(crate) const MAX_STEPS: usize = 64;

pub(crate) fn operation_label(op: &Operation) -> Text {
    match op {
        Operation::Script { .. } => Text::OperationScript,
        Operation::NameFilter { .. } => Text::OperationNameFilter,
        Operation::MemberNameFilter { .. } => Text::OperationMemberNameFilter,
        Operation::ProtocolFilter { .. } => Text::OperationProtocolFilter,
        Operation::ReplaceName { .. } => Text::OperationReplaceName,
        Operation::FormatName { .. } => Text::OperationFormatName,
        Operation::Flags { .. } => Text::OperationFlags,
        Operation::Sort { .. } => Text::OperationSort,
        Operation::RegexSort { .. } => Text::OperationRegexSort,
        Operation::MemberRegexSort { .. } => Text::OperationMemberRegexSort,
        Operation::Deduplicate => Text::OperationDeduplicate,
        Operation::HostFilter { .. } => Text::OperationHostFilter,
        Operation::PortFilter { .. } => Text::OperationPortFilter,
        Operation::RegionFilter { .. } => Text::OperationRegionFilter,
        Operation::IpFilter { .. } => Text::OperationIpFilter,
        Operation::Limit { .. } => Text::OperationLimit,
        Operation::Reverse => Text::OperationReverse,
        Operation::ResolveDomain { .. } => Text::OperationResolveDomain,
        Operation::SetProperty { .. } => Text::OperationSetProperty,
        Operation::ConditionFilter { .. } => Text::OperationConditionFilter,
        Operation::NormalizeName { .. } => Text::OperationNormalizeName,
        Operation::UniqueNames { .. } => Text::OperationUniqueNames,
        Operation::RegionNames { .. } => Text::OperationRegionNames,
        Operation::Shuffle { .. } => Text::OperationShuffle,
        Operation::BatchRename { .. } => Text::OperationBatchRename,
        Operation::Slice { .. } => Text::OperationSlice,
        Operation::DeduplicateName { .. } => Text::OperationDeduplicateName,
        Operation::NetworkFilter { .. } => Text::OperationNetworkFilter,
    }
}

pub(crate) fn new_operations() -> [Operation; 26] {
    [
        Operation::NameFilter {
            pattern: String::new(),
            keep: true,
        },
        Operation::ProtocolFilter {
            protocols: Vec::new(),
            keep: true,
        },
        Operation::ReplaceName {
            pattern: String::new(),
            replacement: String::new(),
            regex: false,
        },
        Operation::FormatName {
            template: "{protocol} | {name}".into(),
        },
        Operation::Flags { add: true },
        Operation::Sort {
            key: SortKey::Name,
            descending: false,
        },
        Operation::RegexSort {
            patterns: Vec::new(),
            unmatched: RegexSortUnmatched::Original,
        },
        Operation::Deduplicate,
        Operation::HostFilter {
            pattern: String::new(),
            keep: true,
        },
        Operation::PortFilter {
            start: 1,
            end: 65535,
            keep: true,
        },
        Operation::RegionFilter {
            regions: vec!["JP".into()],
            keep: true,
            keep_unknown: true,
        },
        Operation::IpFilter {
            family: AddressFamily::V4,
            keep_domains: true,
        },
        Operation::Limit { count: 50 },
        Operation::Reverse,
        Operation::ResolveDomain {
            family: AddressFamily::PreferV4,
            options: dns::Options::default(),
        },
        Operation::SetProperty {
            setting: Setting::TcpFastOpen { enabled: true },
        },
        Operation::ConditionFilter {
            conditions: vec![Condition {
                field: Field::Name,
                matcher: TextMatch::Contains,
                value: String::new(),
                negate: false,
            }],
            mode: MatchMode::All,
            keep: true,
        },
        Operation::NormalizeName {
            case: NameCase::Preserve,
            trim: true,
            collapse_whitespace: true,
        },
        Operation::UniqueNames {
            separator: " · ".into(),
        },
        Operation::RegionNames {
            labels: vec![
                RegionLabel {
                    region: "JP".into(),
                    label: "Japan".into(),
                },
                RegionLabel {
                    region: "US".into(),
                    label: "United States".into(),
                },
            ],
            separator: " | ".into(),
            keep_name: true,
        },
        Operation::Shuffle { seed: 0 },
        Operation::BatchRename {
            trim_each: false,
            rules: vec![RenameRule {
                pattern: String::new(),
                replacement: String::new(),
                regex: false,
            }],
        },
        Operation::Slice { skip: 0, count: 50 },
        Operation::DeduplicateName {
            keep: KeepOccurrence::First,
            ignore_case: false,
        },
        Operation::NetworkFilter {
            networks: Vec::new(),
            keep: true,
            keep_domains: true,
        },
        Operation::Script {
            source: "fn transform(node) { node.name = \"Hub \" + node.name; node }".into(),
            arguments: Default::default(),
        },
    ]
}

pub(crate) fn next_step_id(steps: &[Step]) -> Option<u64> {
    steps
        .iter()
        .map(|step| step.id)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .or_else(|| (0..=MAX_STEPS as u64).find(|id| steps.iter().all(|step| step.id != *id)))
}

pub(crate) fn operation_group(op: &Operation) -> u8 {
    match op {
        Operation::NameFilter { .. }
        | Operation::MemberNameFilter { .. }
        | Operation::ProtocolFilter { .. }
        | Operation::HostFilter { .. }
        | Operation::PortFilter { .. }
        | Operation::RegionFilter { .. }
        | Operation::IpFilter { .. }
        | Operation::NetworkFilter { .. }
        | Operation::ConditionFilter { .. } => 0,
        Operation::ResolveDomain { .. } | Operation::SetProperty { .. } => 2,
        Operation::Script { .. } => 3,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::model::pipeline;

    #[test]
    fn imported_maximum_identifier_does_not_disable_new_operations() {
        let steps = [Step {
            id: u64::MAX,
            enabled: true,
            operation: Operation::Reverse,
        }];
        assert_eq!(next_step_id(&steps), Some(0));
    }

    #[test]
    fn operation_defaults_and_groups_cover_all_choices() {
        let operations = new_operations();
        assert_eq!(operations.len(), 26);
        assert_eq!(
            operations
                .iter()
                .filter(|op| operation_group(op) == 0)
                .count(),
            8
        );
        assert_eq!(
            operations
                .iter()
                .filter(|op| operation_group(op) == 1)
                .count(),
            15
        );
        assert_eq!(
            operations
                .iter()
                .filter(|op| operation_group(op) == 2)
                .count(),
            2
        );
        assert_eq!(
            operations
                .iter()
                .filter(|op| operation_group(op) == 3)
                .count(),
            1
        );
        assert!(
            matches!(&operations[8], Operation::HostFilter { pattern, keep: true } if pattern.is_empty())
        );
        assert!(matches!(
            &operations[9],
            Operation::PortFilter {
                start: 1,
                end: 65535,
                keep: true
            }
        ));
        assert!(
            matches!(&operations[10], Operation::RegionFilter { regions, keep: true, keep_unknown: true } if regions == &["JP"])
        );
        assert!(matches!(
            &operations[11],
            Operation::IpFilter {
                family: AddressFamily::V4,
                keep_domains: true
            }
        ));
        assert!(matches!(&operations[12], Operation::Limit { count: 50 }));
        assert!(matches!(
            &operations[14],
            Operation::ResolveDomain {
                family: AddressFamily::PreferV4,
                ..
            }
        ));
        assert!(matches!(
            &operations[15],
            Operation::SetProperty {
                setting: Setting::TcpFastOpen { enabled: true }
            }
        ));
        assert!(
            matches!(&operations[16], Operation::ConditionFilter { conditions, mode: MatchMode::All, keep: true } if conditions.len() == 1 && conditions[0].value.is_empty() && conditions[0].field == Field::Name && conditions[0].matcher == TextMatch::Contains)
        );
        assert!(matches!(
            &operations[17],
            Operation::NormalizeName {
                case: NameCase::Preserve,
                trim: true,
                collapse_whitespace: true
            }
        ));
        assert!(
            matches!(&operations[18], Operation::UniqueNames { separator } if separator == " · ")
        );
        assert!(
            matches!(&operations[19], Operation::RegionNames { labels, separator, keep_name: true } if labels.len() == 2 && labels[0].region == "JP" && labels[0].label == "Japan" && labels[1].region == "US" && labels[1].label == "United States" && separator == " | ")
        );
        assert!(matches!(&operations[20], Operation::Shuffle { seed: 0 }));
        assert!(
            matches!(&operations[25], Operation::Script { source, arguments } if source == "fn transform(node) { node.name = \"Hub \" + node.name; node }" && arguments.is_empty())
        );
        let condition = Step {
            id: 1,
            enabled: true,
            operation: operations[16].clone(),
        };
        assert!(
            pipeline::validate(&[condition]).is_err(),
            "empty condition must require input"
        );
    }
}
