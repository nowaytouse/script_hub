use std::{collections::BTreeMap, fmt};

use serde::de::{self, Deserialize, Deserializer, MapAccess, Visitor};

pub const LOCALES: [&str; 3] = ["en", "zh-CN", "ja"];

struct Catalog(BTreeMap<String, String>);

impl<'de> Deserialize<'de> for Catalog {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct UniqueEntries;
        impl<'de> Visitor<'de> for UniqueEntries {
            type Value = Catalog;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an object of unique translation keys and string values")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Catalog, M::Error> {
                let mut entries = BTreeMap::new();
                while let Some((key, value)) = map.next_entry::<String, String>()? {
                    if entries.insert(key.clone(), value).is_some() {
                        return Err(de::Error::custom(format!(
                            "duplicate translation key {key}"
                        )));
                    }
                }
                Ok(Catalog(entries))
            }
        }
        deserializer.deserialize_map(UniqueEntries)
    }
}

fn placeholders(text: &str) -> Result<BTreeMap<&str, usize>, String> {
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut fields = BTreeMap::new();
    while index < bytes.len() {
        match bytes[index] {
            b'{' | b'}' if bytes.get(index + 1) == Some(&bytes[index]) => index += 2,
            b'{' => {
                let start = index + 1;
                let end = text[start..]
                    .find('}')
                    .map(|offset| start + offset)
                    .ok_or_else(|| "unclosed placeholder".to_owned())?;
                let field = &text[start..end];
                if field.contains('{') || field.chars().any(char::is_control) {
                    return Err("invalid placeholder".to_owned());
                }
                *fields.entry(field).or_default() += 1;
                index = end + 1;
            }
            b'}' => return Err("unmatched closing brace".to_owned()),
            _ => index += 1,
        }
    }
    Ok(fields)
}

fn parse_catalog(locale: &str, source: &str) -> Result<BTreeMap<String, String>, String> {
    let Catalog(entries) =
        serde_json::from_str(source).map_err(|error| format!("{locale}: {error}"))?;
    if entries.is_empty() {
        return Err(format!("{locale}: the translation catalog is empty"));
    }
    for (key, value) in &entries {
        if !key.starts_with(|ch: char| ch.is_ascii_uppercase())
            || !key
                .bytes()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == b'_')
            || key == "Self"
        {
            return Err(format!("{locale}: invalid typed translation key {key}"));
        }
        if value.trim().is_empty() {
            return Err(format!("{locale}: empty translation for {key}"));
        }
        placeholders(value).map_err(|error| format!("{locale}: {key}: {error}"))?;
    }
    Ok(entries)
}

pub fn compile_catalogs(sources: [&str; 3]) -> Result<String, String> {
    let catalogs = LOCALES
        .iter()
        .zip(sources)
        .map(|(locale, source)| parse_catalog(locale, source))
        .collect::<Result<Vec<_>, _>>()?;
    let english = &catalogs[0];
    for (locale, catalog) in LOCALES.iter().zip(&catalogs).skip(1) {
        if let Some(key) = english.keys().find(|key| !catalog.contains_key(*key)) {
            return Err(format!("{locale}: missing translation key {key}"));
        }
        if let Some(key) = catalog.keys().find(|key| !english.contains_key(*key)) {
            return Err(format!("{locale}: unknown translation key {key}"));
        }
        for (key, value) in catalog {
            if placeholders(value)? != placeholders(&english[key])? {
                return Err(format!("{locale}: placeholder mismatch for {key}"));
            }
        }
    }
    let mut output = "#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub enum Text {\n".to_owned();
    for key in english.keys() {
        output.push_str(&format!("    {key},\n"));
    }
    output.push_str(&format!(
        "}}\n\nstatic TRANSLATIONS: [[&str; 3]; {}] = [\n",
        english.len()
    ));
    for key in english.keys() {
        output.push_str(&format!(
            "    [{:?}, {:?}, {:?}],\n",
            english[key], catalogs[1][key], catalogs[2][key]
        ));
    }
    output.push_str("];\n\n#[cfg(test)]\nconst ALL_TEXT: &[Text] = &[\n");
    for key in english.keys() {
        output.push_str(&format!("    Text::{key},\n"));
    }
    output.push_str("];\n");
    Ok(output)
}
