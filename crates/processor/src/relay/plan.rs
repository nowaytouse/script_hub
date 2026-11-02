//! Portable operation chains. Importing a plan never executes it or grants network consent.

use super::model::pipeline::{self, Step};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

const MAX_BYTES: usize = 1024 * 1024;
const FORMAT: &str = "script-hub-operations";

#[derive(Debug, PartialEq, Eq)]
pub enum PlanError {
    Invalid,
    TooLarge,
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Invalid => "Invalid operation plan",
            Self::TooLarge => "Operation plan exceeds size limit",
        })
    }
}
impl std::error::Error for PlanError {}

#[derive(Serialize)]
struct Export<'a> {
    format: &'static str,
    version: u8,
    steps: &'a [Step],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Import {
    format: String,
    version: u8,
    steps: Vec<Step>,
}

pub fn encode(steps: &[Step]) -> Result<Zeroizing<Vec<u8>>, PlanError> {
    pipeline::validate(steps).map_err(|_| PlanError::Invalid)?;
    let bytes = Zeroizing::new(
        serde_json::to_vec_pretty(&Export {
            format: FORMAT,
            version: 1,
            steps,
        })
        .map_err(|_| PlanError::Invalid)?,
    );
    if bytes.len() > MAX_BYTES {
        return Err(PlanError::TooLarge);
    }
    Ok(bytes)
}

pub fn decode(bytes: &[u8]) -> Result<Vec<Step>, PlanError> {
    if bytes.len() > MAX_BYTES {
        return Err(PlanError::TooLarge);
    }
    let plan: Import = serde_json::from_slice(bytes).map_err(|_| PlanError::Invalid)?;
    if plan.format != FORMAT || plan.version != 1 {
        return Err(PlanError::Invalid);
    }
    pipeline::validate(&plan.steps).map_err(|_| PlanError::Invalid)?;
    Ok(plan.steps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::model::pipeline::{AddressFamily, Operation};

    #[test]
    fn roundtrip_preserves_order_disabled_drafts_and_dns_without_running_them() {
        let steps = vec![
            Step {
                id: 4,
                enabled: false,
                operation: Operation::NameFilter {
                    pattern: "[".into(),
                    keep: true,
                },
            },
            Step {
                id: 7,
                enabled: true,
                operation: Operation::ResolveDomain {
                    family: AddressFamily::PreferV4,
                    options: Default::default(),
                },
            },
        ];
        let bytes = encode(&steps).unwrap();
        let restored = decode(&bytes).unwrap();
        assert_eq!(encode(&restored).unwrap(), bytes);
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            value
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["format", "steps", "version"]
        );
    }

    #[test]
    fn rejects_unsupported_schema_extra_credentials_and_duplicate_ids() {
        let steps = [Step {
            id: 1,
            enabled: true,
            operation: Operation::Reverse,
        }];
        let bytes = encode(&steps).unwrap();
        let source = std::str::from_utf8(&bytes).unwrap();
        for invalid in [
            source.replace("\"version\": 1", "\"version\": 2"),
            source.replacen('{', "{\"token\":\"synthetic\",", 1),
            source.replacen('{', "{\"version\":1,", 1),
        ] {
            assert!(matches!(
                decode(invalid.as_bytes()),
                Err(PlanError::Invalid)
            ));
        }
        assert!(matches!(
            encode(&[steps[0].clone(), steps[0].clone()]),
            Err(PlanError::Invalid)
        ));
    }

    #[test]
    fn rejects_oversized_input_and_enabled_invalid_operation() {
        assert!(matches!(
            decode(&vec![b' '; MAX_BYTES + 1]),
            Err(PlanError::TooLarge)
        ));
        assert!(matches!(
            encode(&[Step {
                id: 1,
                enabled: true,
                operation: Operation::NameFilter {
                    pattern: "[".into(),
                    keep: true
                }
            }]),
            Err(PlanError::Invalid)
        ));
    }
}
