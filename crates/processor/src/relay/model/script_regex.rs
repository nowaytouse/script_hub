//! Bounded Rust-regex helpers for node scripts; no external execution.
use regex::{Regex, RegexBuilder};
use rhai::{Engine, EvalAltResult};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

const MAX_TEXT: usize = 1024;
const MAX_OUTPUT: usize = 4096;
const MAX_PATTERN: usize = 256;
const MAX_REPLACEMENT: usize = 128;
const MAX_PATTERNS: usize = 32;
const MAX_CALLS: usize = 8192;

type Result<T> = std::result::Result<T, Box<EvalAltResult>>;

#[derive(Default)]
struct Cache {
    patterns: HashMap<String, Regex>,
    calls: usize,
}

impl Cache {
    fn regex(&mut self, text: &str, pattern: &str) -> Result<&Regex> {
        self.calls = self.calls.saturating_add(1);
        if self.calls > MAX_CALLS || text.len() > MAX_TEXT || pattern.len() > MAX_PATTERN {
            return Err("Regex budget exceeded".into());
        }
        if !self.patterns.contains_key(pattern) {
            if self.patterns.len() == MAX_PATTERNS {
                return Err("Too many script patterns".into());
            }
            let regex = RegexBuilder::new(pattern)
                .size_limit(128 * 1024)
                .dfa_size_limit(128 * 1024)
                .build()
                .map_err(|_| "Invalid script pattern")?;
            self.patterns.insert(pattern.to_owned(), regex);
        }
        Ok(&self.patterns[pattern])
    }
}

pub(super) fn register(engine: &mut Engine) {
    let cache = Rc::new(RefCell::new(Cache::default()));
    let matching = Rc::clone(&cache);
    engine.register_fn(
        "regex_match",
        move |text: &str, pattern: &str| -> Result<bool> {
            Ok(matching.borrow_mut().regex(text, pattern)?.is_match(text))
        },
    );
    engine.register_fn(
        "regex_replace",
        move |text: &str, pattern: &str, replacement: &str| -> Result<String> {
            if replacement.len() > MAX_REPLACEMENT {
                return Err("Regex replacement is too long".into());
            }
            let mut cache = cache.borrow_mut();
            let regex = cache.regex(text, pattern)?;
            let mut output = String::new();
            let mut cursor = 0;
            for captures in regex.captures_iter(text) {
                let found = captures.get(0).ok_or("Missing regex match")?;
                output.push_str(&text[cursor..found.start()]);
                captures.expand(replacement, &mut output);
                if output.len() > MAX_OUTPUT {
                    return Err("Regex output is too long".into());
                }
                cursor = found.end();
            }
            output.push_str(&text[cursor..]);
            if output.len() > MAX_OUTPUT {
                return Err("Regex output is too long".into());
            }
            Ok(output)
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regex_helpers_match_unicode_replace_captures_and_bound_results() {
        let mut engine = Engine::new();
        register(&mut engine);
        assert!(
            engine
                .eval::<bool>(r#"regex_match("香港 03", "^香港 [0-9]+$")"#)
                .unwrap()
        );
        assert_eq!(
            engine
                .eval::<String>(r#"regex_replace("香港 03", "([0-9]+)", "HK-$1")"#)
                .unwrap(),
            "香港 HK-03"
        );
        assert!(engine.eval::<bool>(r#"regex_match("name", "(")"#).is_err());
        assert!(
            engine
                .eval::<String>(r#"regex_replace("a" * 1024, "(.+)", "$1$1$1$1$1")"#)
                .is_err()
        );
        let mut cache = Cache::default();
        for n in 0..MAX_PATTERNS {
            assert!(cache.regex("x", &format!("^x{n}$")).is_ok());
        }
        assert!(cache.regex("x", "^new$").is_err());
        assert!(cache.regex("x", "^x0$").is_ok());
    }
}
