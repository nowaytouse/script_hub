//! Explicit, all-or-nothing export of a prepared node list.

use zeroize::Zeroizing;

use super::RelayError;
use super::model::{ModelError, Subscription};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    #[default]
    Original,
    /// Checked native Surge policy list, without conversion.
    Surge,
    /// Existing mihomo provider document, without conversion.
    Mihomo,
    /// Strict projection into a sing-box `outbounds` fragment.
    SingBox,
}

impl OutputFormat {
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Original | Self::Surge => "txt",
            Self::Mihomo => "yaml",
            Self::SingBox => "json",
        }
    }
}

/// No network or file access. A failed projection returns no output or partial list.
pub fn export_subscription(
    content: &str,
    format: OutputFormat,
) -> Result<Zeroizing<String>, RelayError> {
    let subscription = Subscription::parse(content).map_err(RelayError::Model)?;
    subscription
        .validate_export_dependencies()
        .map_err(RelayError::Model)?;
    match format {
        OutputFormat::Surge if !subscription.is_surge_policy_list() => {
            return Err(RelayError::ExportFormat);
        }
        OutputFormat::Mihomo if !subscription.is_mihomo_provider() => {
            return Err(RelayError::ExportFormat);
        }
        OutputFormat::Surge => {
            for (index, node) in subscription.nodes().iter().enumerate() {
                node.validate_native_surge_export()
                    .map_err(|error| RelayError::ExportNode {
                        index: index + 1,
                        error,
                    })?;
            }
        }
        OutputFormat::SingBox => return sing_box(&subscription),
        _ => {}
    }
    Ok(Zeroizing::new(content.to_owned()))
}

fn sing_box(subscription: &Subscription) -> Result<Zeroizing<String>, RelayError> {
    let mut output = Zeroizing::new(br#"{"outbounds":["#.to_vec());
    for (index, node) in subscription.nodes().iter().enumerate() {
        let mut bytes = Zeroizing::new(Vec::new());
        node.write_export_sing_box(&mut bytes)
            .map_err(|error| RelayError::ExportNode {
                index: index + 1,
                error,
            })?;
        if output.len() + bytes.len() + 3 > super::model::MAX_CONTENT {
            return Err(RelayError::Model(ModelError::Render));
        }
        if index != 0 {
            output.push(b',');
        }
        output.extend_from_slice(&bytes);
    }
    output.extend_from_slice(b"]}\n");
    std::str::from_utf8(&output)
        .map(|text| Zeroizing::new(text.to_owned()))
        .map_err(|_| RelayError::Model(ModelError::Render))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SURGE: &str = "[Proxy]\r\n# example\r\nalpha = https, example.com, 443\r\n";

    #[test]
    fn preserves_native_client_documents_byte_for_byte() {
        let mihomo =
            "proxies:\n  - {name: beta, type: http, server: example.net, port: 80, tfo: true}\n";
        for (format, text) in [
            (OutputFormat::Original, SURGE),
            (OutputFormat::Surge, SURGE),
            (OutputFormat::Mihomo, mihomo),
        ] {
            assert_eq!(export_subscription(text, format).unwrap().as_str(), text);
        }
    }

    #[test]
    fn rejects_wrong_native_client_dialect() {
        assert!(matches!(
            export_subscription(SURGE, OutputFormat::Mihomo),
            Err(RelayError::ExportFormat)
        ));
    }

    #[test]
    fn rejects_incomplete_node_chains() {
        let input = "alpha = https, example.com, 443, underlying-proxy=missing\n";
        assert!(matches!(
            export_subscription(input, OutputFormat::Original),
            Err(RelayError::Model(ModelError::InvalidDependency))
        ));
    }

    #[test]
    fn surge_export_rejects_shadowrocket_only_policy() {
        let foreign = "alpha = vless, example.com, 443, username=synthetic\n";
        assert!(matches!(
            export_subscription(foreign, OutputFormat::Surge),
            Err(RelayError::ExportNode { index: 1, .. })
        ));
    }

    #[test]
    fn refuses_surge_external_directives_even_in_original() {
        for directive in [
            "#!include https://example.com/outside.conf\n",
            "beta = http, example.com, 80 #!REQUIREMENT IOS\n",
        ] {
            let source = format!("{SURGE}{directive}");
            assert!(matches!(
                export_subscription(&source, OutputFormat::Original),
                Err(RelayError::Model(ModelError::InvalidInput))
            ));
        }
    }

    #[test]
    fn exports_all_nodes_and_preserves_udp_and_tls_semantics() {
        let input = format!(
            "{SURGE}beta = socks5, example.net, 1080\ngamma = socks5, example.org, 1080, udp-relay=true\n"
        );
        let output = export_subscription(&input, OutputFormat::SingBox).unwrap();
        let expected = serde_json::json!({"outbounds":[
            {"type":"http","tag":"alpha","server":"example.com","server_port":443,"tls":{"enabled":true}},
            {"type":"socks","tag":"beta","server":"example.net","server_port":1080,"version":"5","network":"tcp"},
            {"type":"socks","tag":"gamma","server":"example.org","server_port":1080,"version":"5"}
        ]});
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&output).unwrap(),
            expected
        );
    }

    #[test]
    fn conversion_rejects_unrepresentable_options_at_their_node_without_leaking_values() {
        let input = format!(
            "{SURGE}private-name = https, example.net, 443, test-url=https://example.net/private-token\n"
        );
        let error = export_subscription(&input, OutputFormat::SingBox).unwrap_err();
        assert!(matches!(
            error,
            RelayError::ExportNode {
                index: 2,
                error: ModelError::Unsupported
            }
        ));
        assert!(!error.to_string().contains("private-"));
    }

    #[test]
    fn conversion_refuses_to_silently_downgrade_tls_verification() {
        let input = "alpha = https, example.net, 443, skip-cert-verify=true\n";
        assert!(matches!(
            export_subscription(input, OutputFormat::SingBox),
            Err(RelayError::ExportNode {
                index: 1,
                error: ModelError::SecurityBlocked
            })
        ));
    }
}
