//! Bounded Rhai node transformations; scripts never receive the original node or credentials.
use super::{
    MAX_CONTENT, ModelError,
    properties::{ScriptFields, Setting, validate_setting},
};
use rhai::{AST, Array, Dynamic, Engine, EvalAltResult, Map, Scope};
use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap},
    rc::Rc,
};
use zeroize::Zeroize;

pub const MAX_SOURCE: usize = 16 * 1024;
const MAX_OPERATIONS: u64 = 1_000_000;
const MAX_TRANSFORM_ARRAY: usize = 128;
const MAX_BATCH_NODES: usize = 4096;
const MAX_ALPN_PROTOCOLS: usize = 32;
pub(crate) const MAX_ARGUMENTS: usize = 16;

pub(crate) fn valid_argument_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    name.len() <= 32
        && bytes
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

pub(crate) fn valid_argument_value(value: &str) -> bool {
    value.len() <= 256 && !value.chars().any(char::is_control)
}

pub(crate) fn validate_arguments(arguments: &BTreeMap<String, String>) -> Result<(), ModelError> {
    if arguments.len() > MAX_ARGUMENTS
        || arguments
            .iter()
            .any(|(name, value)| !valid_argument_name(name) || !valid_argument_value(value))
    {
        return Err(ModelError::InvalidEdit);
    }
    Ok(())
}

pub(super) struct Input {
    pub index: usize,
    pub name: String,
    pub server: String,
    pub port: u16,
    pub protocol: String,
    pub fields: ScriptFields,
    pub underlying_proxy: Option<String>,
}

#[derive(Clone, Default)]
pub(crate) struct SourceContext {
    pub kind: &'static str,
    pub label: Option<String>,
    pub member_index: Option<usize>,
}

#[derive(Clone)]
pub(super) struct Context {
    pub source_format: String,
    pub target_format: String,
    pub arguments: BTreeMap<String, String>,
    pub source: SourceContext,
}

impl Drop for Context {
    fn drop(&mut self) {
        for (mut key, mut value) in std::mem::take(&mut self.arguments) {
            key.zeroize();
            value.zeroize();
        }
        self.source.label.zeroize();
    }
}

#[derive(Clone)]
struct RuntimeContext(Rc<Context>);

#[derive(Clone)]
struct ScriptNode {
    index: usize,
    name: String,
    server: String,
    port: i64,
    protocol: String,
    included: bool,
    fields: ScriptFields,
    original_proxy: Option<String>,
    underlying_proxy: Option<Option<String>>,
    udp: Option<bool>,
    udp_over_tcp: Option<bool>,
    tfo: Option<bool>,
    sni: Option<String>,
    alpn: Option<Setting>,
    verify_tls: bool,
}

impl From<&Input> for ScriptNode {
    fn from(input: &Input) -> Self {
        Self {
            index: input.index,
            name: input.name.clone(),
            server: input.server.clone(),
            port: i64::from(input.port),
            protocol: input.protocol.clone(),
            included: true,
            fields: input.fields.clone(),
            original_proxy: input.underlying_proxy.clone(),
            underlying_proxy: None,
            udp: None,
            udp_over_tcp: None,
            tfo: None,
            sni: None,
            alpn: None,
            verify_tls: false,
        }
    }
}

pub(super) struct Output {
    pub index: usize,
    pub name: String,
    pub included: bool,
    pub settings: Vec<Setting>,
    pub underlying_proxy: Option<Option<String>>,
}

#[derive(Clone, Copy)]
enum EntryPoint {
    Transform,
    TransformWithContext,
    Operator,
    Filter,
}

pub(super) struct Script {
    engine: Engine,
    ast: AST,
    entry: EntryPoint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SyntaxError {
    Size,
    Parse {
        line: Option<usize>,
        column: Option<usize>,
    },
    EntryPoint,
}

/// Checks syntax and the entry point without evaluating statements or reading any nodes.
pub(crate) fn check_syntax(source: &str) -> Result<(), SyntaxError> {
    compile_with_diagnostics(source).map(drop)
}

pub(super) fn compile(source: &str) -> Result<Script, ModelError> {
    compile_with_diagnostics(source).map_err(|_| ModelError::InvalidEdit)
}

fn compile_with_diagnostics(source: &str) -> Result<Script, SyntaxError> {
    if source.is_empty() || source.len() > MAX_SOURCE {
        return Err(SyntaxError::Size);
    }
    let mut engine = Engine::new();
    engine.disable_symbol("eval");
    engine.set_optimization_level(rhai::OptimizationLevel::None);
    engine.set_max_operations(MAX_OPERATIONS);
    engine.set_max_call_levels(32);
    engine.set_max_expr_depths(64, 32);
    engine.set_max_variables(128);
    engine.set_max_functions(32);
    engine.set_max_string_size(4096);
    engine.set_max_array_size(MAX_TRANSFORM_ARRAY);
    engine.set_max_map_size(128);
    engine.on_print(|_| {});
    engine.on_debug(|_, _, _| {});
    let operations = Rc::new(Cell::new(0u64));
    engine.on_progress(move |_| {
        let next = operations.get().saturating_add(1);
        operations.set(next);
        (next > MAX_OPERATIONS).then_some(Dynamic::UNIT)
    });
    engine.register_type_with_name::<ScriptNode>("Node");
    engine.register_get_set(
        "name",
        |n: &mut ScriptNode| n.name.clone(),
        |n: &mut ScriptNode, v: String| n.name = v,
    );
    engine.register_get_set(
        "server",
        |n: &mut ScriptNode| n.server.clone(),
        |n: &mut ScriptNode, v: String| n.server = v,
    );
    engine.register_get_set(
        "port",
        |n: &mut ScriptNode| n.port,
        |n: &mut ScriptNode, v: i64| n.port = v,
    );
    engine.register_get("protocol", |n: &mut ScriptNode| n.protocol.clone());
    engine.register_get_set(
        "included",
        |n: &mut ScriptNode| n.included,
        |n: &mut ScriptNode, v: bool| n.included = v,
    );
    engine.register_get("udp", |n: &mut ScriptNode| {
        optional_bool(n.udp.or(n.fields.udp))
    });
    engine.register_set("udp", |n: &mut ScriptNode, v: bool| n.udp = Some(v));
    engine.register_get("udp_over_tcp", |n: &mut ScriptNode| {
        optional_bool(n.udp_over_tcp.or(n.fields.udp_over_tcp))
    });
    engine.register_set("udp_over_tcp", |n: &mut ScriptNode, v: bool| {
        n.udp_over_tcp = Some(v)
    });
    engine.register_get("tfo", |n: &mut ScriptNode| {
        optional_bool(n.tfo.or(n.fields.tfo))
    });
    engine.register_set("tfo", |n: &mut ScriptNode, v: bool| n.tfo = Some(v));
    engine.register_get("sni", |n: &mut ScriptNode| {
        optional_text(n.sni.as_deref().or(n.fields.sni.as_deref()))
    });
    engine.register_set("sni", |n: &mut ScriptNode, v: String| n.sni = Some(v));
    engine.register_get("alpn", ScriptNode::get_alpn);
    engine.register_set("alpn", ScriptNode::set_alpn);
    engine.register_get("verify_tls", |n: &mut ScriptNode| {
        optional_bool(if n.verify_tls {
            Some(true)
        } else {
            n.fields.verify_tls
        })
    });
    engine.register_set("verify_tls", |n: &mut ScriptNode, v: bool| n.verify_tls = v);
    engine.register_get("tls", |n: &mut ScriptNode| optional_bool(n.fields.tls));
    engine.register_get("transport", |n: &mut ScriptNode| {
        optional_text(n.fields.transport.as_deref())
    });
    engine.register_get("underlying_proxy", |n: &mut ScriptNode| {
        optional_text(
            n.underlying_proxy
                .as_ref()
                .unwrap_or(&n.original_proxy)
                .as_deref(),
        )
    });
    engine.register_set("underlying_proxy", ScriptNode::set_underlying_proxy);
    engine.register_type_with_name::<RuntimeContext>("Context");
    engine.register_get("source_format", |context: &mut RuntimeContext| {
        context.0.source_format.clone()
    });
    engine.register_get("target_format", |context: &mut RuntimeContext| {
        context.0.target_format.clone()
    });
    engine.register_get("source_kind", |context: &mut RuntimeContext| {
        let kind = context.0.source.kind;
        if kind.is_empty() {
            "unknown".to_owned()
        } else {
            kind.to_owned()
        }
    });
    engine.register_get("source_label", |context: &mut RuntimeContext| {
        optional_text(context.0.source.label.as_deref().filter(|label| {
            !label.is_empty() && label.len() <= 512 && !label.chars().any(char::is_control)
        }))
    });
    engine.register_get("member_index", |context: &mut RuntimeContext| {
        context
            .0
            .source
            .member_index
            .and_then(|index| i64::try_from(index).ok())
            .map_or(Dynamic::UNIT, Dynamic::from)
    });
    engine.register_get("arguments", |context: &mut RuntimeContext| {
        context
            .0
            .arguments
            .iter()
            .map(|(key, value)| (key.as_str().into(), Dynamic::from(value.clone())))
            .collect::<Map>()
    });
    super::script_regex::register(&mut engine);
    let ast = engine.compile(source).map_err(|error| {
        let position = error.position();
        SyntaxError::Parse {
            line: position.line(),
            column: position.position(),
        }
    })?;
    let mut transform = false;
    let mut transform_with_context = false;
    let mut operator = false;
    let mut filter = false;
    for function in ast
        .iter_functions()
        .filter(|function| function.this_type.is_none())
    {
        match (function.name, function.params.len()) {
            ("transform", 1) => transform = true,
            ("transform", 2) => transform_with_context = true,
            ("operator", 2) => operator = true,
            ("filter", 2) => filter = true,
            _ => {}
        }
    }
    let entry = match (transform, transform_with_context, operator, filter) {
        // Keep legacy selection for scripts with a transform overload used as a helper.
        (true, _, false, false) => EntryPoint::Transform,
        (false, true, false, false) => EntryPoint::TransformWithContext,
        (false, false, true, false) => EntryPoint::Operator,
        (false, false, false, true) => EntryPoint::Filter,
        _ => return Err(SyntaxError::EntryPoint),
    };
    if matches!(entry, EntryPoint::Operator | EntryPoint::Filter) {
        engine.set_max_array_size(MAX_BATCH_NODES);
    }
    Ok(Script { engine, ast, entry })
}

fn optional_bool(value: Option<bool>) -> Dynamic {
    value.map_or(Dynamic::UNIT, Dynamic::from)
}

fn optional_text(value: Option<&str>) -> Dynamic {
    value.map_or(Dynamic::UNIT, |value| Dynamic::from(value.to_owned()))
}

impl Script {
    pub(super) fn apply(
        &self,
        inputs: impl ExactSizeIterator<Item = Input>,
        context: &Context,
    ) -> Result<Vec<Output>, ModelError> {
        validate_arguments(&context.arguments)?;
        let context = RuntimeContext(Rc::new(context.clone()));
        if matches!(self.entry, EntryPoint::Operator | EntryPoint::Filter) {
            if inputs.len() > MAX_BATCH_NODES {
                return Err(ModelError::InvalidEdit);
            }
            return match self.entry {
                EntryPoint::Filter => self.apply_filter(inputs, context),
                _ => self.apply_batch(inputs, context),
            };
        }
        collect_outputs(inputs.map(|input| {
            let node = ScriptNode::from(&input);
            let result: ScriptNode = match self.entry {
                EntryPoint::Transform => {
                    self.engine
                        .call_fn(&mut Scope::new(), &self.ast, "transform", (node,))
                }
                EntryPoint::TransformWithContext => self.engine.call_fn(
                    &mut Scope::new(),
                    &self.ast,
                    "transform",
                    (node, context.clone()),
                ),
                EntryPoint::Operator | EntryPoint::Filter => {
                    unreachable!("batch entries are dispatched separately")
                }
            }
            .map_err(|_| ModelError::InvalidEdit)?;
            result.output(&input)
        }))
    }

    fn apply_batch(
        &self,
        inputs: impl ExactSizeIterator<Item = Input>,
        context: RuntimeContext,
    ) -> Result<Vec<Output>, ModelError> {
        let count = inputs.len();
        let mut originals = HashMap::with_capacity(count);
        let mut nodes = Array::with_capacity(count);
        for input in inputs {
            nodes.push(Dynamic::from(ScriptNode::from(&input)));
            if originals.insert(input.index, input).is_some() {
                return Err(ModelError::InvalidEdit);
            }
        }
        let returned: Array = self
            .engine
            .call_fn(&mut Scope::new(), &self.ast, "operator", (nodes, context))
            .map_err(|_| ModelError::InvalidEdit)?;
        if returned.len() > count {
            return Err(ModelError::InvalidEdit);
        }
        collect_outputs(returned.into_iter().map(|value| {
            let node = value
                .try_cast::<ScriptNode>()
                .ok_or(ModelError::InvalidEdit)?;
            // Removing the identity also rejects a second copy of the same input.
            let input = originals
                .remove(&node.index)
                .ok_or(ModelError::InvalidEdit)?;
            node.output(&input)
        }))
    }

    fn apply_filter(
        &self,
        inputs: impl ExactSizeIterator<Item = Input>,
        context: RuntimeContext,
    ) -> Result<Vec<Output>, ModelError> {
        let inputs: Vec<_> = inputs.collect();
        let nodes: Array = inputs
            .iter()
            .map(|input| Dynamic::from(ScriptNode::from(input)))
            .collect();
        let mask: Array = self
            .engine
            .call_fn(&mut Scope::new(), &self.ast, "filter", (nodes, context))
            .map_err(|_| ModelError::InvalidEdit)?;
        if mask.len() != inputs.len() {
            return Err(ModelError::InvalidEdit);
        }
        collect_outputs(inputs.into_iter().zip(mask).filter_map(|(input, keep)| {
            match keep.try_cast::<bool>() {
                // The mask never applies changes made to the script's node copies.
                Some(true) => Some(ScriptNode::from(&input).output(&input)),
                Some(false) => None,
                None => Some(Err(ModelError::InvalidEdit)),
            }
        }))
    }
}

fn collect_outputs(
    items: impl Iterator<Item = Result<Output, ModelError>>,
) -> Result<Vec<Output>, ModelError> {
    let mut bytes = 0usize;
    items
        .map(|item| {
            let output = item?;
            let settings: usize = output
                .settings
                .iter()
                .map(|setting| match setting {
                    Setting::Server { value } | Setting::ServerName { value } => value.len(),
                    Setting::Alpn { values } => values.iter().map(String::len).sum(),
                    _ => 0,
                })
                .sum();
            bytes = bytes
                .checked_add(
                    output.name.len()
                        + settings
                        + output
                            .underlying_proxy
                            .as_ref()
                            .and_then(Option::as_ref)
                            .map_or(0, String::len),
                )
                .filter(|size| *size <= MAX_CONTENT)
                .ok_or(ModelError::InvalidEdit)?;
            Ok(output)
        })
        .collect()
}

impl ScriptNode {
    fn get_alpn(&mut self) -> Dynamic {
        let current = match &self.alpn {
            Some(Setting::Alpn { values }) => Some(values),
            _ => self.fields.alpn.as_ref(),
        };
        current.map_or(Dynamic::UNIT, |values| {
            Dynamic::from(values.iter().cloned().map(Dynamic::from).collect::<Array>())
        })
    }

    fn set_underlying_proxy(&mut self, value: Dynamic) -> Result<(), Box<EvalAltResult>> {
        let target = if value.is_unit() {
            None
        } else {
            let name = value
                .into_string()
                .map_err(|_| "Invalid underlying proxy name")?;
            if name.len() > 512
                || name.chars().any(char::is_control)
                || !name.is_empty() && name.trim().is_empty()
            {
                return Err("Invalid underlying proxy name".into());
            }
            (!name.is_empty()).then_some(name)
        };
        self.underlying_proxy = Some(target);
        Ok(())
    }

    fn set_alpn(&mut self, values: Array) -> Result<(), Box<EvalAltResult>> {
        // A custom Node is opaque to Rhai's nested array accounting. Never store
        // the script's array, even temporarily, where cloned nodes can multiply it.
        if !(1..=MAX_ALPN_PROTOCOLS).contains(&values.len()) {
            return Err("Invalid node ALPN".into());
        }
        let values = values
            .into_iter()
            .map(|value| value.into_string().map_err(|_| "Invalid node ALPN"))
            .collect::<Result<Vec<_>, _>>()?;
        let setting = Setting::Alpn { values };
        validate_setting(&setting).map_err(|_| "Invalid node ALPN")?;
        self.alpn = Some(setting);
        Ok(())
    }

    fn output(self, input: &Input) -> Result<Output, ModelError> {
        if self.index != input.index
            || self.name.is_empty()
            || self.name.len() > 512
            || self.name.chars().any(char::is_control)
        {
            return Err(ModelError::InvalidEdit);
        }
        let mut settings = Vec::new();
        if self.server != input.server {
            settings.push(Setting::Server { value: self.server });
        }
        let port = u16::try_from(self.port).map_err(|_| ModelError::InvalidEdit)?;
        if port != input.port {
            settings.push(Setting::Port { value: port });
        }
        if let Some(enabled) = self.udp {
            settings.push(Setting::Udp { enabled });
        }
        if let Some(enabled) = self.udp_over_tcp {
            settings.push(Setting::UdpOverTcp { enabled });
        }
        if let Some(enabled) = self.tfo {
            settings.push(Setting::TcpFastOpen { enabled });
        }
        if let Some(value) = self.sni {
            settings.push(Setting::ServerName { value });
        }
        if let Some(alpn) = self.alpn {
            settings.push(alpn);
        }
        if self.verify_tls {
            settings.push(Setting::VerifyTls);
        }
        for setting in &settings {
            validate_setting(setting)?;
        }
        Ok(Output {
            index: self.index,
            name: self.name,
            included: self.included,
            settings,
            underlying_proxy: self
                .underlying_proxy
                .filter(|value| value != &input.underlying_proxy),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(index: usize) -> Input {
        Input {
            index,
            name: format!("node-{index}"),
            server: "example.com".to_owned(),
            port: 443,
            protocol: "trojan".to_owned(),
            fields: ScriptFields::default(),
            underlying_proxy: None,
        }
    }

    fn context() -> Context {
        Context {
            source_format: "uri".into(),
            target_format: "surge".into(),
            arguments: BTreeMap::new(),
            source: SourceContext::default(),
        }
    }

    fn run(source: &str) -> Result<Output, ModelError> {
        compile(source)?
            .apply(std::iter::once(input(0)), &context())?
            .pop()
            .ok_or(ModelError::InvalidEdit)
    }

    #[test]
    fn syntax_check_reports_safe_locations_and_checks_entry_without_running() {
        assert_eq!(check_syntax(""), Err(SyntaxError::Size));
        assert_eq!(
            check_syntax(&"猫".repeat(MAX_SOURCE / 2)),
            Err(SyntaxError::Size)
        );
        assert_eq!(
            check_syntax("fn transform() { 0 }"),
            Err(SyntaxError::EntryPoint)
        );
        assert_eq!(
            check_syntax("fn helper(node) { node }"),
            Err(SyntaxError::EntryPoint)
        );
        assert!(matches!(
            check_syntax("fn transform(node) {\nlet x = ;\nnode\n}"),
            Err(SyntaxError::Parse {
                line: Some(2),
                column: Some(_)
            })
        ));
        assert!(check_syntax("throw \"not evaluated\"; fn transform(node) { node }").is_ok());
        assert!(check_syntax("fn transform(node, context) { node }").is_ok());
        assert!(check_syntax("fn operator(nodes, context) { nodes }").is_ok());
        assert!(check_syntax("fn filter(nodes, context) { [] }").is_ok());
        assert_eq!(
            check_syntax("fn filter(nodes, context) { [] } fn operator(nodes, context) { nodes }"),
            Err(SyntaxError::EntryPoint)
        );
        assert_eq!(
            check_syntax("fn transform(node) { node } fn operator(nodes, context) { nodes }"),
            Err(SyntaxError::EntryPoint),
        );
    }

    #[test]
    fn transforms_only_the_restricted_view() {
        let result = run(r#"fn transform(node) { node.name = "prefix-" + node.name; node.included = false; node.udp = true; node }"#).unwrap();
        assert_eq!(result.name, "prefix-node-0");
        assert!(!result.included);
        assert!(matches!(
            result.settings.as_slice(),
            [Setting::Udp { enabled: true }]
        ));
        for body in [
            "node.password",
            "node.index = 4; node",
            "node.protocol = \"ss\"; node",
            "node.extra = 1",
            "#{ name: \"x\" }",
        ] {
            assert!(run(&format!("fn transform(node) {{ {body} }}")).is_err());
        }
    }

    #[test]
    fn transform_with_context_updates_only_validated_properties() {
        let result = run(r#"fn transform(node, context) {
            node.name = context.source_format + "-" + context.target_format;
            node.server = "updated.example"; node.port = 8443;
            node.tfo = false; node.sni = "tls.example"; node.alpn = ["h2", "http/1.1"];
            node.verify_tls = true;
            node
        }"#)
        .unwrap();
        assert_eq!((result.index, result.name.as_str()), (0, "uri-surge"));
        assert!(
            result.settings
                == vec![
                    Setting::Server {
                        value: "updated.example".into()
                    },
                    Setting::Port { value: 8443 },
                    Setting::TcpFastOpen { enabled: false },
                    Setting::ServerName {
                        value: "tls.example".into()
                    },
                    Setting::Alpn {
                        values: vec!["h2".into(), "http/1.1".into()]
                    },
                    Setting::VerifyTls,
                ]
        );
        for body in [
            "context.target_format = \"other\"",
            "node.server = \"https://invalid.example/path\"",
            "node.port = 0",
            "node.port = 65536",
            "node.alpn = [\"h2\", 7]",
            "node.alpn = [\"h2,http/1.1\"]",
            "node.alpn = []",
            "node.skip_cert_verify = true",
        ] {
            assert!(run(&format!("fn transform(node, context) {{ {body}; node }}")).is_err());
        }
    }

    #[test]
    fn reads_and_updates_typed_fields_and_proxy_without_exposing_raw_config() {
        let mut node = input(0);
        node.fields = ScriptFields {
            udp: Some(false),
            udp_over_tcp: Some(false),
            tfo: Some(true),
            tls: Some(true),
            verify_tls: Some(false),
            sni: Some("effective.example".into()),
            alpn: Some(vec!["h2".into()]),
            transport: Some("ws".into()),
        };
        node.underlying_proxy = Some("previous".into());
        let mut context = context();
        context.arguments.insert("prefix".into(), "shown".into());
        context.source = SourceContext {
            kind: "local",
            label: Some("label".into()),
            member_index: Some(2),
        };
        let script = compile(r#"fn transform(node, context) {
            if node.udp != false || node.udp_over_tcp != false || node.tfo != true || node.tls != true
                || node.verify_tls != false || node.sni != "effective.example" || node.alpn[0] != "h2"
                || node.transport != "ws" || node.underlying_proxy != "previous"
                || context.source_kind != "local" || context.source_label != "label" || context.member_index != 2
            { throw "wrong view"; }
            node.name = context.arguments.prefix;
            node.underlying_proxy = "next";
            if node.underlying_proxy != "next" { throw "wrong proxy"; }
            node.underlying_proxy = ();
            node.udp_over_tcp = true;
            if node.udp_over_tcp != true { throw "wrong setting"; }
            node
        }"#).unwrap();
        let output = script
            .apply(std::iter::once(node), &context)
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(output.name, "shown");
        assert_eq!(output.underlying_proxy, Some(None));
        assert!(matches!(
            output.settings.as_slice(),
            [Setting::UdpOverTcp { enabled: true }]
        ));
        for property in [
            "node.tls = false",
            "node.transport = \"quic\"",
            "node.underlying_proxy = 3",
            "node.password",
        ] {
            assert!(run(&format!("fn transform(node) {{ {property}; node }}")).is_err());
        }
    }

    #[test]
    fn script_argument_names_and_values_are_bounded() {
        assert!(valid_argument_name("_name42"));
        assert!(valid_argument_name(&"a".repeat(32)));
        assert!(!valid_argument_name(&"a".repeat(33)));
        for name in ["", "first-name", "7name", "名字", "a.b"] {
            assert!(!valid_argument_name(name));
        }
        assert!(valid_argument_value(""));
        assert!(valid_argument_value(&"é".repeat(128)));
        assert!(!valid_argument_value(&"é".repeat(129)));
        assert!(!valid_argument_value("test\n"));
        let arguments = (0..=MAX_ARGUMENTS)
            .map(|index| (format!("arg{index}"), String::new()))
            .collect();
        assert_eq!(validate_arguments(&arguments), Err(ModelError::InvalidEdit));
    }

    #[test]
    fn batch_can_filter_and_reorder_only_existing_typed_nodes() {
        let script = compile(
            r#"fn operator(nodes, context) {
            let last = nodes[2];
            last.name = context.target_format + "-" + last.name;
            [last, nodes[0]]
        }"#,
        )
        .unwrap();
        let result = script.apply((0..3).map(input), &context()).unwrap();
        assert_eq!(
            result
                .iter()
                .map(|node| (node.index, node.name.as_str()))
                .collect::<Vec<_>>(),
            [(2, "surge-node-2"), (0, "node-0")]
        );

        for body in [
            "nodes[0]",
            "[nodes[0], nodes[0]]",
            "[#{ index: 7, name: \"fake\" }]",
            "[0]",
        ] {
            let script = compile(&format!("fn operator(nodes, context) {{ {body} }}")).unwrap();
            assert!(script.apply((0..3).map(input), &context()).is_err());
        }
        let script = compile("fn operator(nodes, context) { [] }").unwrap();
        assert!(
            script
                .apply((0..3).map(input), &context())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn filter_keeps_original_nodes_in_order_and_requires_a_boolean_per_input() {
        let script = compile(
            r#"fn filter(nodes, context) {
                let edited = nodes[2];
                edited.name = "ignored";
                [context.source_format == "uri", false, true]
            }"#,
        )
        .unwrap();
        let outputs = script.apply((0..3).map(input), &context()).unwrap();
        assert_eq!(
            outputs
                .iter()
                .map(|node| (node.index, node.name.as_str()))
                .collect::<Vec<_>>(),
            [(0, "node-0"), (2, "node-2")]
        );
        for mask in ["[]", "[true, true, true, true]", "[true, false, 1]"] {
            let script = compile(&format!("fn filter(nodes, context) {{ {mask} }}")).unwrap();
            assert!(matches!(
                script.apply((0..3).map(input), &context()),
                Err(ModelError::InvalidEdit)
            ));
        }
        assert!(matches!(
            script.apply((0..MAX_BATCH_NODES + 1).map(input), &context()),
            Err(ModelError::InvalidEdit)
        ));
    }

    #[test]
    fn only_batch_mode_increases_array_capacity_and_has_an_input_limit() {
        let script = compile("fn operator(nodes, context) { nodes }").unwrap();
        assert_eq!(script.engine.max_array_size(), MAX_BATCH_NODES);
        assert_eq!(
            script.apply((0..129).map(input), &context()).unwrap().len(),
            129
        );
        assert!(
            script
                .apply((0..MAX_BATCH_NODES + 1).map(input), &context())
                .is_err()
        );
        for source in [
            "fn transform(node) { node }",
            "fn transform(node, context) { node }",
        ] {
            assert_eq!(
                compile(source).unwrap().engine.max_array_size(),
                MAX_TRANSFORM_ARRAY
            );
        }
    }

    #[test]
    fn invalid_alpn_cannot_be_stored_even_if_later_replaced() {
        let script = compile(
            r#"fn operator(nodes, context) {
            let node = nodes[0];
            let too_many = [];
            for n in 0..33 { too_many.push("h2"); }
            node.alpn = too_many;
            node.alpn = ["h2"];
            [node]
        }"#,
        )
        .unwrap();
        assert!(script.apply((0..1).map(input), &context()).is_err());
        assert!(
            run(r#"fn transform(node) {
            node.alpn = ["h2", 7]; node.alpn = ["h2"]; node
        }"#)
            .is_err()
        );
    }

    #[test]
    fn bounds_and_disallowed_features_fail_closed() {
        for source in [
            "fn transform(node) { loop {} node }",
            "fn recurse() { recurse() } fn transform(node) { recurse(); node }",
            "fn transform(node) { eval(\"1\"); node }",
            "import \"foo\"; fn transform(node) { node }",
            "fn transform(node) { read_file(\"/etc/passwd\"); node }",
            "fn transform(node) { run(\"true\"); node }",
            "fn transform(node) { fetch(\"https://example.com\"); node }",
            "fn transform(node) { node.name = \"x\" * 5000; node }",
        ] {
            assert!(run(source).is_err());
        }
        assert!(compile(&"x".repeat(MAX_SOURCE + 1)).is_err());
    }

    #[test]
    fn budget_is_shared_across_node_calls() {
        let script =
            compile("fn transform(node) { let x = 0; for n in 0..1000 { x += n; } node }").unwrap();
        assert!(script.apply(std::iter::once(input(0)), &context()).is_ok());
        assert!(script.apply((0..1000).map(input), &context()).is_err());
    }

    #[test]
    fn existing_file_cannot_be_imported() {
        use std::io::Write;
        let path =
            std::env::temp_dir().join(format!("script-hub-rhai-{}.rhai", std::process::id()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        writeln!(file, "fn secret() {{ 42 }}").unwrap();
        drop(file);
        let source = format!(
            "import {:?}; fn transform(node) {{ node }}",
            path.to_string_lossy()
        );
        let denied = compile(&source).is_err();
        std::fs::remove_file(path).unwrap();
        assert!(denied);
    }
}
