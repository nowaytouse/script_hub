//! Bounded Rhai node transformations; scripts never receive the original node or credentials.
use super::{ModelError, properties::Setting};
use rhai::{AST, Dynamic, Engine, Scope};
use std::{cell::Cell, rc::Rc};

pub const MAX_SOURCE: usize = 16 * 1024;
const MAX_OPERATIONS: u64 = 1_000_000;

#[derive(Clone)]
struct ScriptNode {
    name: String,
    server: String,
    port: i64,
    protocol: String,
    included: bool,
    udp: Option<bool>,
    tfo: Option<bool>,
    sni: Option<String>,
    verify_tls: bool,
}

pub(super) struct Output {
    pub name: String,
    pub included: bool,
    pub settings: Vec<Setting>,
}

pub(super) struct Script {
    engine: Engine,
    ast: AST,
}

pub(super) fn compile(source: &str) -> Result<Script, ModelError> {
    if source.is_empty() || source.len() > MAX_SOURCE {
        return Err(ModelError::InvalidEdit);
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
    engine.set_max_array_size(128);
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
    engine.register_get("server", |n: &mut ScriptNode| n.server.clone());
    engine.register_get("port", |n: &mut ScriptNode| n.port);
    engine.register_get("protocol", |n: &mut ScriptNode| n.protocol.clone());
    engine.register_get_set(
        "included",
        |n: &mut ScriptNode| n.included,
        |n: &mut ScriptNode, v: bool| n.included = v,
    );
    engine.register_set("udp", |n: &mut ScriptNode, v: bool| n.udp = Some(v));
    engine.register_set("tfo", |n: &mut ScriptNode, v: bool| n.tfo = Some(v));
    engine.register_set("sni", |n: &mut ScriptNode, v: String| n.sni = Some(v));
    engine.register_set("verify_tls", |n: &mut ScriptNode, v: bool| n.verify_tls = v);
    let ast = engine
        .compile(source)
        .map_err(|_| ModelError::InvalidEdit)?;
    Ok(Script { engine, ast })
}

impl Script {
    pub(super) fn transform(
        &self,
        name: &str,
        server: &str,
        port: u16,
        protocol: &str,
    ) -> Result<Output, ModelError> {
        let input = ScriptNode {
            name: name.to_owned(),
            server: server.to_owned(),
            port: i64::from(port),
            protocol: protocol.to_owned(),
            included: true,
            udp: None,
            tfo: None,
            sni: None,
            verify_tls: false,
        };
        let result: ScriptNode = self
            .engine
            .call_fn(&mut Scope::new(), &self.ast, "transform", (input,))
            .map_err(|_| ModelError::InvalidEdit)?;
        let mut settings = Vec::new();
        if let Some(enabled) = result.udp {
            settings.push(Setting::Udp { enabled });
        }
        if let Some(enabled) = result.tfo {
            settings.push(Setting::TcpFastOpen { enabled });
        }
        if let Some(value) = result.sni {
            settings.push(Setting::ServerName { value });
        }
        if result.verify_tls {
            settings.push(Setting::VerifyTls);
        }
        Ok(Output {
            name: result.name,
            included: result.included,
            settings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &str) -> Result<Output, ModelError> {
        compile(source)?.transform("node", "example.com", 443, "trojan")
    }

    #[test]
    fn transforms_only_the_restricted_view() {
        let result = run(r#"fn transform(node) { node.name = "prefix-" + node.name; node.included = false; node.udp = true; node }"#).unwrap();
        assert_eq!(result.name, "prefix-node");
        assert!(!result.included);
        assert!(matches!(
            result.settings.as_slice(),
            [Setting::Udp { enabled: true }]
        ));
        for body in [
            "node.password",
            "node.server = \"changed\"",
            "node.extra = 1",
            "#{ name: \"x\" }",
        ] {
            assert!(run(&format!("fn transform(node) {{ {body} }}")).is_err());
        }
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
        assert!(script.transform("a", "a", 1, "http").is_ok());
        assert!((0..1000).any(|_| script.transform("a", "a", 1, "http").is_err()));
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
