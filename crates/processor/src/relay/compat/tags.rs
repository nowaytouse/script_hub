//! Materialize Sub-Store's tag selectors against the subscriptions in one backup.
use std::collections::HashSet;

use serde_json::Value;
use zeroize::Zeroizing;

use crate::relay::library::MAX_SUBSCRIPTIONS;

pub(super) fn is_supported_metadata(value: &Value) -> bool {
    value.is_null()
        || value.as_array().is_some_and(|tags| {
            tags.len() <= MAX_SUBSCRIPTIONS && tags.iter().all(Value::is_string)
        })
}

pub(super) fn extend_members(
    members: &mut Vec<Zeroizing<String>>,
    selector: Option<&Value>,
    subscriptions: Option<&[Value]>,
) -> bool {
    let Some(selector) = selector.filter(|value| !value.is_null()) else {
        return true;
    };
    let Some(tags) = selector
        .as_array()
        .filter(|tags| tags.len() <= MAX_SUBSCRIPTIONS)
    else {
        return false;
    };
    if tags.is_empty() {
        return true;
    }
    let Some(selected) = tags
        .iter()
        .map(Value::as_str)
        .collect::<Option<HashSet<_>>>()
    else {
        return false;
    };
    let Some(subscriptions) = subscriptions.filter(|subs| subs.len() <= MAX_SUBSCRIPTIONS) else {
        return false;
    };
    if members.len() > MAX_SUBSCRIPTIONS {
        return false;
    }

    let mut seen: HashSet<_> = members.iter().map(|name| name.as_str()).collect();
    let mut additions = Vec::new();
    for subscription in subscriptions {
        let Some(tags) = subscription.get("tag").and_then(Value::as_array) else {
            continue;
        };
        if tags.len() > MAX_SUBSCRIPTIONS {
            return false;
        }
        if !tags
            .iter()
            .filter_map(Value::as_str)
            .any(|tag| selected.contains(tag))
        {
            continue;
        }
        let Some(name) = subscription.get("name").and_then(Value::as_str) else {
            return false;
        };
        if seen.insert(name) {
            let Some(name) = super::name(subscription) else {
                return false;
            };
            additions.push(name);
            if members.len() + additions.len() > MAX_SUBSCRIPTIONS {
                return false;
            }
        }
    }
    members.extend(additions);
    true
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn adds_tagged_members_in_backup_order_without_repeating_explicit_members() {
        let source = json!([
            {"name": "A", "tag": ["Y", "X"]},
            {"name": "B", "tag": ["x"]},
            {"name": "C", "tag": ["X"]},
            {"name": "D", "tag": ["Y", "X"]}
        ]);
        let mut members = vec![Zeroizing::new("D".into()), Zeroizing::new("C".into())];
        assert!(extend_members(
            &mut members,
            Some(&json!(["X", "Y", "X"])),
            source.as_array().map(Vec::as_slice),
        ));
        assert_eq!(
            members.iter().map(|name| name.as_str()).collect::<Vec<_>>(),
            ["D", "C", "A"]
        );
    }

    #[test]
    fn malformed_selector_or_member_does_not_partially_extend_the_list() {
        let source = json!([
            {"name": "A", "tag": ["X"]},
            {"tag": ["X"]}
        ]);
        for selector in [json!("X"), json!(["X", 1]), json!(["X"])] {
            let mut members = vec![Zeroizing::new("explicit".into())];
            assert!(!extend_members(
                &mut members,
                Some(&selector),
                source.as_array().map(Vec::as_slice),
            ));
            assert_eq!(members.len(), 1);
            assert_eq!(members[0].as_str(), "explicit");
        }
    }
}
