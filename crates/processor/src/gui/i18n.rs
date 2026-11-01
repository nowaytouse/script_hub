use crate::runtime::settings::Language;

include!(concat!(env!("OUT_DIR"), "/locales.rs"));

pub fn text(language: Language, key: Text) -> &'static str {
    let language = match language.resolved() {
        Language::En | Language::System => 0,
        Language::ZhCn => 1,
        Language::Ja => 2,
    };
    TRANSLATIONS[key as usize][language]
}

#[cfg(test)]
mod tests {
    use super::{ALL_TEXT, Text, text};
    use crate::runtime::settings::Language;

    #[test]
    fn every_catalog_entry_is_nonempty_in_all_languages() {
        for key in ALL_TEXT {
            for language in [Language::En, Language::ZhCn, Language::Ja] {
                assert!(
                    !text(language, *key).trim().is_empty(),
                    "missing translation for {key:?} in {language:?}"
                );
            }
        }
    }

    #[test]
    fn system_language_resolves_before_lookup() {
        for (language, source) in [
            (Language::En, include_str!("../../assets/locales/en.json")),
            (
                Language::ZhCn,
                include_str!("../../assets/locales/zh-CN.json"),
            ),
            (Language::Ja, include_str!("../../assets/locales/ja.json")),
        ] {
            let catalog: std::collections::BTreeMap<String, String> =
                serde_json::from_str(source).unwrap();
            assert_eq!(text(language, Text::Overview), catalog["Overview"]);
        }
        assert_eq!(
            text(Language::System, Text::Overview),
            text(Language::System.resolved(), Text::Overview)
        );
    }

    #[test]
    fn default_language_is_english_and_chinese_is_explicit() {
        assert_eq!(Language::default(), Language::En);
        assert_eq!(
            text(Language::default(), Text::Settings),
            text(Language::En, Text::Settings)
        );
    }
}
