#[path = "../build/locales.rs"]
mod catalogs;

use catalogs::compile_catalogs;

const EN: &str = r#"{"Overview":"Overview","Settings":"Settings"}"#;
const ZH: &str = r#"{"Overview":"概览","Settings":"设置"}"#;
const JA: &str = r#"{"Overview":"概要","Settings":"設定"}"#;

#[test]
fn catalogs_generate_typed_keys_and_static_translations() {
    let output = compile_catalogs([EN, ZH, JA]).unwrap();
    assert!(output.contains("pub enum Text"));
    assert!(output.contains("Overview") && output.contains("概览") && output.contains("概要"));
    assert!(output.contains("ALL_TEXT"));
    assert!(output.contains("static TRANSLATIONS"));
}

#[test]
fn missing_or_unknown_translation_keys_fail_instead_of_falling_back() {
    for changed in [
        r#"{"Overview":"概览"}"#,
        r#"{"Overview":"概览","Settings":"设置","Extra":"多余"}"#,
    ] {
        assert!(compile_catalogs([EN, changed, JA]).is_err());
    }
}

#[test]
fn duplicate_translation_keys_are_not_silently_overwritten() {
    let duplicate = r#"{"Overview":"Overview","Overview":"Changed","Settings":"Settings"}"#;
    assert!(compile_catalogs([duplicate, ZH, JA]).is_err());
}

#[test]
fn blank_translations_and_invalid_typed_keys_are_rejected() {
    for invalid in [
        r#"{"Overview":"  ","Settings":"Settings"}"#,
        r#"{"Not-A-Rust-Key":"text"}"#,
        r#"{"Self":"text"}"#,
        "{}",
    ] {
        assert!(compile_catalogs([invalid, invalid, invalid]).is_err());
    }
}

#[test]
fn placeholder_names_and_occurrence_counts_match_the_english_contract() {
    let english = r#"{"Count":"{count} items from {source}"}"#;
    let translated = r#"{"Count":"{source}：{count} 项"}"#;
    assert!(compile_catalogs([english, translated, translated]).is_ok());
    for invalid in [
        r#"{"Count":"{total} from {source}"}"#,
        r#"{"Count":"{count} {count} {source}"}"#,
        r#"{"Count":"{count"}"#,
        r#"{"Count":"{count}}"}"#,
    ] {
        assert!(compile_catalogs([english, invalid, translated]).is_err());
    }
}

#[test]
fn escaped_literal_braces_are_not_translation_parameters() {
    let english = r#"{"Count":"{count} {{literal}}"}"#;
    let translated = r#"{"Count":"{{字面量}} {count}"}"#;
    assert!(compile_catalogs([english, translated, translated]).is_ok());
}

#[test]
fn malformed_json_and_non_string_translations_fail() {
    for invalid in ["not JSON", r#"{"Overview":7}"#, "[]"] {
        assert!(compile_catalogs([invalid, ZH, JA]).is_err());
    }
}

#[test]
fn key_order_and_json_layout_do_not_change_generated_output() {
    let reordered = r#"{ "Settings": "Settings", "Overview": "Overview" }"#;
    assert_eq!(
        compile_catalogs([EN, ZH, JA]).unwrap(),
        compile_catalogs([reordered, ZH, JA]).unwrap()
    );
}

#[test]
fn the_embedded_catalogs_share_one_complete_nonempty_key_set() {
    let sources = [
        include_str!("../assets/locales/en.json"),
        include_str!("../assets/locales/zh-CN.json"),
        include_str!("../assets/locales/ja.json"),
    ];
    assert!(compile_catalogs(sources).is_ok());
}
