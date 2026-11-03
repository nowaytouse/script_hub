//! Private, offline subscription workspace.
use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use super::model::{self, editor::Options};

const MAX_CACHE: usize = 4 * 1024 * 1024;
const MAX_DOCUMENT: usize = 8 * 1024 * 1024;
pub const MAX_SUBSCRIPTIONS: usize = 256;
pub const MAX_COLLECTIONS: usize = 128;

#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct Workspace {
    pub version: u8,
    pub next_id: u64,
    pub subscriptions: Vec<Entry>,
    pub collections: Vec<Collection>,
}

#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: u64,
    pub name: String,
    pub source: String,
    pub source_file: String,
    pub content: String,
    pub processing: Options,
    #[serde(default, skip_serializing_if = "PublishSettings::is_default")]
    pub publish: PublishSettings,
}

#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct Collection {
    pub id: u64,
    pub name: String,
    pub members: Vec<u64>,
    pub processing: Options,
    #[serde(default, skip_serializing_if = "PublishSettings::is_default")]
    pub publish: PublishSettings,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct PublishSettings {
    pub output: String,
    pub output_file: String,
    pub title: String,
    pub interval_minutes: u64,
}

impl Default for PublishSettings {
    fn default() -> Self {
        let config = super::RelayConfig::default();
        Self {
            output: config.output,
            output_file: config.output_file,
            title: config.title,
            interval_minutes: super::schedule::DEFAULT_INTERVAL.as_secs() / 60,
        }
    }
}

impl PublishSettings {
    fn is_default(&self) -> bool {
        self == &Self::default()
    }

    pub fn validate(&self) -> Result<(), Error> {
        let seconds = self
            .interval_minutes
            .checked_mul(60)
            .ok_or(Error::Invalid)?;
        if !(super::schedule::MIN_INTERVAL.as_secs()..=super::schedule::MAX_INTERVAL.as_secs())
            .contains(&seconds)
            || self.output.len() > 8192
            || self.output.chars().any(char::is_control)
            || super::gist::validate_filename(&self.output_file).is_err()
            || super::gist::validate_name(&self.title).is_err()
            || (!self.output.trim().is_empty()
                && super::gist::GistFile::parse(self.output.trim(), &self.output_file).is_err())
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }

    fn aliases_source(&self, entry: &Entry) -> bool {
        if self.output.trim().is_empty() || entry.source.trim().is_empty() {
            return false;
        }
        matches!(
            (super::gist::GistFile::parse(self.output.trim(), &self.output_file),
             super::input::target(entry.source.trim(), &entry.source_file)),
            (Ok(output), Ok(super::input::Target::Gist(source))) if output.id == source.id
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Invalid,
    TooLarge,
    NotFound,
    Referenced,
    Unsupported,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Invalid => "Invalid workspace",
            Self::TooLarge => "Workspace is too large",
            Self::NotFound => "Item not found",
            Self::Referenced => "Item is referenced by a collection",
            Self::Unsupported => "Collection cannot be merged safely",
        })
    }
}
impl std::error::Error for Error {}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            version: 1,
            next_id: 1,
            subscriptions: Vec::new(),
            collections: Vec::new(),
        }
    }
}

// JSON values contain subscription credentials and must not survive error paths.
struct PrivateJson(serde_json::Value);
impl Drop for PrivateJson {
    fn drop(&mut self) {
        fn wipe(value: &mut serde_json::Value) {
            match value {
                serde_json::Value::String(s) => s.zeroize(),
                serde_json::Value::Array(a) => a.iter_mut().for_each(wipe),
                serde_json::Value::Object(o) => {
                    for (mut key, mut value) in std::mem::take(o) {
                        key.zeroize();
                        wipe(&mut value);
                    }
                }
                _ => {}
            }
        }
        wipe(&mut self.0);
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 512 && !name.chars().any(char::is_control)
}

impl Workspace {
    pub fn validate(&self) -> Result<(), Error> {
        if self.version != 1 || self.next_id == 0 {
            return Err(Error::Invalid);
        }
        if self.subscriptions.len() > MAX_SUBSCRIPTIONS || self.collections.len() > MAX_COLLECTIONS
        {
            return Err(Error::TooLarge);
        }
        let mut ids = HashSet::new();
        let mut names = HashSet::new();
        let mut bytes = 0usize;
        for entry in &self.subscriptions {
            if entry.id == 0
                || entry.id >= self.next_id
                || !ids.insert(entry.id)
                || !valid_name(&entry.name)
                || !names.insert(&entry.name)
                || entry.source.len() > 8192
                || entry.source_file.len() > 512
                || entry.source.chars().any(char::is_control)
                || entry.source_file.chars().any(char::is_control)
            {
                return Err(Error::Invalid);
            }
            if (!entry.source.trim().is_empty()
                && super::input::target(entry.source.trim(), &entry.source_file).is_err())
                || model::pipeline::validate(&entry.processing.pipeline).is_err()
                || entry.publish.validate().is_err()
                || entry.publish.aliases_source(entry)
            {
                return Err(Error::Invalid);
            }
            bytes = bytes
                .checked_add(entry.content.len())
                .ok_or(Error::TooLarge)?;
        }
        if bytes > MAX_CACHE {
            return Err(Error::TooLarge);
        }
        let entry_ids = ids.clone();
        names.clear();
        for collection in &self.collections {
            if collection.id == 0
                || collection.id >= self.next_id
                || !ids.insert(collection.id)
                || !valid_name(&collection.name)
                || !names.insert(&collection.name)
                || model::pipeline::validate(&collection.processing.pipeline).is_err()
                || collection.publish.validate().is_err()
            {
                return Err(Error::Invalid);
            }
            let mut members = HashSet::new();
            if collection
                .members
                .iter()
                .any(|id| !entry_ids.contains(id) || !members.insert(id))
            {
                return Err(Error::Invalid);
            }
            if collection.members.iter().any(|id| {
                self.subscriptions
                    .iter()
                    .any(|entry| entry.id == *id && collection.publish.aliases_source(entry))
            }) {
                return Err(Error::Invalid);
            }
        }
        Ok(())
    }

    pub fn encode(&self) -> Result<Zeroizing<Vec<u8>>, Error> {
        self.validate()?;
        let mut bytes = Zeroizing::new(Vec::new());
        serde_json::to_writer(&mut *bytes, self).map_err(|_| Error::Invalid)?;
        if bytes.len() > MAX_DOCUMENT {
            return Err(Error::TooLarge);
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_DOCUMENT {
            return Err(Error::TooLarge);
        }
        let value = PrivateJson(
            serde_json::from_slice::<model::UniqueValue>(bytes)
                .map_err(|_| Error::Invalid)?
                .0,
        );
        drop(value);
        let workspace: Self = serde_json::from_slice(bytes).map_err(|_| Error::Invalid)?;
        workspace.validate()?;
        Ok(workspace)
    }

    fn change(&mut self, f: impl FnOnce(&mut Self) -> Result<(), Error>) -> Result<(), Error> {
        let mut candidate = self.clone();
        f(&mut candidate)?;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    pub fn add_entry(&mut self, mut entry: Entry) -> Result<u64, Error> {
        let id = self.next_id;
        self.change(|w| {
            entry.id = id;
            w.next_id = id.checked_add(1).ok_or(Error::Invalid)?;
            w.subscriptions.push(entry);
            Ok(())
        })?;
        Ok(id)
    }
    pub fn update_entry(&mut self, entry: Entry) -> Result<(), Error> {
        self.change(|w| {
            let existing = w
                .subscriptions
                .iter()
                .find(|item| item.id == entry.id)
                .ok_or(Error::NotFound)?;
            if w.collections
                .iter()
                .any(|c| c.members.contains(&entry.id) && !c.processing.nodes.is_empty())
            {
                let mut old = Zeroizing::new(Vec::new());
                let mut new = Zeroizing::new(Vec::new());
                serde_json::to_writer(&mut *old, &existing.processing)
                    .map_err(|_| Error::Invalid)?;
                serde_json::to_writer(&mut *new, &entry.processing).map_err(|_| Error::Invalid)?;
                if existing.content != entry.content || *old != *new {
                    return Err(Error::Referenced);
                }
            }
            let slot = w
                .subscriptions
                .iter_mut()
                .find(|item| item.id == entry.id)
                .ok_or(Error::NotFound)?;
            *slot = entry;
            Ok(())
        })
    }
    pub fn remove_entry(&mut self, id: u64) -> Result<(), Error> {
        self.change(|w| {
            if w.collections.iter().any(|c| c.members.contains(&id)) {
                return Err(Error::Referenced);
            }
            let index = w
                .subscriptions
                .iter()
                .position(|e| e.id == id)
                .ok_or(Error::NotFound)?;
            w.subscriptions.remove(index);
            Ok(())
        })
    }
    pub fn add_collection(&mut self, mut collection: Collection) -> Result<u64, Error> {
        let id = self.next_id;
        self.change(|w| {
            collection.id = id;
            w.next_id = id.checked_add(1).ok_or(Error::Invalid)?;
            w.collections.push(collection);
            Ok(())
        })?;
        Ok(id)
    }
    pub fn update_collection(&mut self, collection: Collection) -> Result<(), Error> {
        self.change(|w| {
            let slot = w
                .collections
                .iter_mut()
                .find(|item| item.id == collection.id)
                .ok_or(Error::NotFound)?;
            *slot = collection;
            Ok(())
        })
    }
    pub fn remove_collection(&mut self, id: u64) -> Result<(), Error> {
        self.change(|w| {
            let index = w
                .collections
                .iter()
                .position(|c| c.id == id)
                .ok_or(Error::NotFound)?;
            w.collections.remove(index);
            Ok(())
        })
    }

    pub fn merge(&self, id: u64) -> Result<Zeroizing<String>, Error> {
        self.validate()?;
        let collection = self
            .collections
            .iter()
            .find(|c| c.id == id)
            .ok_or(Error::NotFound)?;
        if collection.members.is_empty() {
            return Err(Error::Invalid);
        }
        let mut output = Zeroizing::new(vec![b'[']);
        let mut first = true;
        for (member_index, id) in collection.members.iter().enumerate() {
            let entry = self
                .subscriptions
                .iter()
                .find(|e| e.id == *id)
                .ok_or(Error::Invalid)?;
            if entry.processing.pipeline.iter().any(|s| {
                s.enabled
                    && matches!(
                        &s.operation,
                        model::pipeline::Operation::ResolveDomain { .. }
                    )
            }) {
                return Err(Error::Unsupported);
            }
            let sub = model::Subscription::parse(&entry.content).map_err(|_| Error::Unsupported)?;
            let health = vec![model::Health::Unknown; sub.nodes().len()];
            let rendered = sub
                .render_processed(
                    &health,
                    &model::EnhanceOptions {
                        rename: false,
                        sort: false,
                    },
                    &entry.processing,
                )
                .map_err(|_| Error::Unsupported)?;
            let processed =
                model::Subscription::parse(&rendered.content).map_err(|_| Error::Unsupported)?;
            let mut namespaced = Options::default();
            for (index, node) in processed.nodes().iter().enumerate() {
                let name = format!("{}: {}", member_index + 1, node.name());
                if !valid_name(&name) {
                    return Err(Error::Unsupported);
                }
                namespaced.nodes.push(model::editor::NodeEdit {
                    index,
                    included: true,
                    name: Some(name),
                });
            }
            let health = vec![model::Health::Unknown; processed.nodes().len()];
            let rendered = processed
                .render_processed(
                    &health,
                    &model::EnhanceOptions {
                        rename: false,
                        sort: false,
                    },
                    &namespaced,
                )
                .map_err(|_| Error::Unsupported)?;
            let processed =
                model::Subscription::parse(&rendered.content).map_err(|_| Error::Unsupported)?;
            for node in processed.nodes() {
                let value = PrivateJson(node.to_sing_box(false).map_err(|_| Error::Unsupported)?);
                let mut bytes = Zeroizing::new(Vec::new());
                serde_json::to_writer(&mut *bytes, &value.0).map_err(|_| Error::Invalid)?;
                if output
                    .len()
                    .checked_add(bytes.len())
                    .and_then(|n| n.checked_add(2))
                    .is_none_or(|n| n > MAX_CACHE)
                {
                    return Err(Error::TooLarge);
                }
                if !first {
                    output.push(b',');
                }
                first = false;
                output.extend_from_slice(&bytes);
            }
        }
        output.push(b']');
        let text = String::from_utf8(std::mem::take(&mut *output)).map_err(|_| Error::Invalid)?;
        Ok(Zeroizing::new(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, content: &str) -> Entry {
        Entry {
            id: 0,
            name: name.into(),
            source: String::new(),
            source_file: String::new(),
            content: content.into(),
            processing: Options::default(),
            publish: PublishSettings::default(),
        }
    }

    fn collection(members: Vec<u64>) -> Collection {
        Collection {
            id: 0,
            name: "group".into(),
            members,
            processing: Options::default(),
            publish: PublishSettings::default(),
        }
    }

    const NODE: &str =
        r#"[{"type":"socks","tag":"same","server":"example.com","server_port":1080}]"#;

    #[test]
    fn legacy_publish_defaults_and_custom_settings_roundtrip() {
        let mut w = Workspace::default();
        let id = w.add_entry(entry("one", NODE)).unwrap();
        w.add_collection(collection(vec![id])).unwrap();
        let legacy = w.encode().unwrap();
        assert!(
            !std::str::from_utf8(&legacy)
                .unwrap()
                .contains("\"publish\"")
        );
        let mut restored = Workspace::decode(&legacy).unwrap();
        assert!(restored.subscriptions[0].publish.is_default());
        assert!(restored.collections[0].publish.is_default());
        restored.subscriptions[0].publish.interval_minutes = 2;
        restored.collections[0].publish.title = "Custom".into();
        let restored = Workspace::decode(&restored.encode().unwrap()).unwrap();
        assert_eq!(restored.subscriptions[0].publish.interval_minutes, 2);
        assert_eq!(restored.collections[0].publish.title, "Custom");
        assert!(serde_json::from_str::<PublishSettings>(
            r#"{"output":"","output_file":"nodes.list","title":"x","interval_minutes":60,"token":"secret"}"#
        )
        .is_err());
    }

    #[test]
    fn publish_settings_validate_and_reject_source_aliases() {
        let mut w = Workspace::default();
        let mut e = entry("one", NODE);
        e.source = "0123456789abcdef0123456789abcdef".into();
        let id = w.add_entry(e).unwrap();
        w.add_collection(collection(vec![id])).unwrap();
        for interval in [0, 10081, u64::MAX] {
            w.subscriptions[0].publish.interval_minutes = interval;
            assert_eq!(w.validate(), Err(Error::Invalid));
        }
        w.subscriptions[0].publish.interval_minutes = 60;
        w.subscriptions[0].publish.output_file = "../bad".into();
        assert_eq!(w.validate(), Err(Error::Invalid));
        w.subscriptions[0].publish.output_file = "nodes.list".into();
        w.subscriptions[0].publish.output = "invalid".into();
        assert_eq!(w.validate(), Err(Error::Invalid));
        w.subscriptions[0].publish.output = w.subscriptions[0].source.clone();
        assert_eq!(w.validate(), Err(Error::Invalid));
        w.subscriptions[0].publish.output.clear();
        w.collections[0].publish.output = w.subscriptions[0].source.clone();
        assert_eq!(w.validate(), Err(Error::Invalid));
        w.collections[0].publish.output.clear();
        w.collections[0].publish.title = "bad\nname".into();
        assert_eq!(w.validate(), Err(Error::Invalid));
    }

    #[test]
    fn roundtrip_rejects_duplicate_keys() {
        let mut w = Workspace::default();
        w.add_entry(entry("one", NODE)).unwrap();
        let encoded = w.encode().unwrap();
        let restored = Workspace::decode(&encoded).unwrap();
        assert_eq!(restored.subscriptions[0].content, NODE);
        assert!(matches!(
            Workspace::decode(
                br#"{"version":1,"version":1,"next_id":1,"subscriptions":[],"collections":[]}"#
            ),
            Err(Error::Invalid)
        ));
    }

    #[test]
    fn referenced_delete_is_atomic() {
        let mut w = Workspace::default();
        let id = w.add_entry(entry("one", NODE)).unwrap();
        w.add_collection(collection(vec![id])).unwrap();
        assert_eq!(w.remove_entry(id), Err(Error::Referenced));
        assert_eq!(w.subscriptions.len(), 1);
    }

    #[test]
    fn referenced_index_edits_reject_changed_member_content_and_processing() {
        let mut w = Workspace::default();
        let id = w.add_entry(entry("one", NODE)).unwrap();
        let mut group = collection(vec![id]);
        group.processing.nodes.push(model::editor::NodeEdit {
            index: 0,
            included: false,
            name: None,
        });
        w.add_collection(group).unwrap();
        let mut changed = w.subscriptions[0].clone();
        changed.content.push(' ');
        assert_eq!(w.update_entry(changed), Err(Error::Referenced));
        let mut changed = w.subscriptions[0].clone();
        changed.processing.include = "test".into();
        assert_eq!(w.update_entry(changed), Err(Error::Referenced));
        assert_eq!(w.subscriptions[0].content, NODE);
        assert!(w.subscriptions[0].processing.include.is_empty());
        let mut renamed = w.subscriptions[0].clone();
        renamed.name = "renamed".into();
        assert!(w.update_entry(renamed).is_ok());
    }

    #[test]
    fn workspace_limits_are_checked() {
        let mut w = Workspace::default();
        w.subscriptions = (0..=MAX_SUBSCRIPTIONS)
            .map(|_| entry("one", NODE))
            .collect();
        assert_eq!(w.validate(), Err(Error::TooLarge));
        w.subscriptions.clear();
        w.collections = (0..=MAX_COLLECTIONS)
            .map(|_| collection(Vec::new()))
            .collect();
        assert_eq!(w.validate(), Err(Error::TooLarge));
    }

    #[test]
    fn merge_namespaces_collisions_and_rejects_dependencies() {
        let mut w = Workspace::default();
        let a = w.add_entry(entry("one", NODE)).unwrap();
        let b = w.add_entry(entry("two", NODE)).unwrap();
        let c = w.add_collection(collection(vec![a, b])).unwrap();
        let merged = w.merge(c).unwrap();
        let parsed = model::Subscription::parse(&merged).unwrap();
        assert_eq!(parsed.nodes().len(), 2);
        assert_ne!(parsed.nodes()[0].name(), parsed.nodes()[1].name());
        let mut dependent = w.subscriptions[1].clone();
        dependent.content = r#"[{"type":"socks","tag":"child","server":"example.com","server_port":1080,"detour":"same"},{"type":"socks","tag":"same","server":"example.com","server_port":1080}]"#.into();
        w.update_entry(dependent).unwrap();
        let mut group = w.collections[0].clone();
        group.members = vec![b];
        w.update_collection(group).unwrap();
        assert!(matches!(w.merge(c), Err(Error::Unsupported)));
    }

    #[test]
    fn merge_produces_node_array_and_rejects_unsupported_input() {
        let mut w = Workspace::default();
        let id = w.add_entry(entry("one", NODE)).unwrap();
        let c = w.add_collection(collection(vec![id])).unwrap();
        let merged = w.merge(c).unwrap();
        assert_eq!(
            model::Subscription::parse(&merged).unwrap().nodes().len(),
            1
        );
        let mut bad = w.subscriptions[0].clone();
        bad.content = "not a subscription".into();
        w.update_entry(bad).unwrap();
        assert!(matches!(w.merge(c), Err(Error::Unsupported)));
    }

    #[test]
    fn member_script_runs_before_collection_namespace() {
        let mut w = Workspace::default();
        let mut item = entry("one", NODE);
        item.processing.pipeline.push(model::pipeline::Step {
            id: 1,
            enabled: true,
            operation: model::pipeline::Operation::Script {
                source: r#"fn transform(node) { node.name = "script-" + node.name; node }"#.into(),
            },
        });
        let id = w.add_entry(item).unwrap();
        let c = w.add_collection(collection(vec![id])).unwrap();
        let merged = w.merge(c).unwrap();
        let parsed = model::Subscription::parse(&merged).unwrap();
        assert_eq!(parsed.nodes()[0].name(), "1: script-same");
    }
}
