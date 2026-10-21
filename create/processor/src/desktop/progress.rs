use serde::{Deserialize, Serialize};
use std::io::Write;

pub const EVENT_PREFIX: &str = "@@script-hub-event:";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Prepare,
    Functional,
    General,
    Adblock,
    Cleanup,
    Categories,
    Gaming,
    Rewrites,
    Mitm,
    Shadowrocket,
    Security,
    Mirrors,
    Catalog,
    Srs,
    Finalize,
    Validate,
    Repository,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProgressEvent {
    pub stage: Stage,
    pub completed: Option<u64>,
    pub total: Option<u64>,
}

pub fn emit_stage(stage: Stage) {
    emit_progress(ProgressEvent {
        stage,
        completed: None,
        total: None,
    });
}

pub fn emit_progress(event: ProgressEvent) {
    if !matches!(std::env::var("SCRIPT_HUB_GUI_EVENTS").as_deref(), Ok("1")) {
        return;
    }
    let Ok(json) = serde_json::to_string(&event) else {
        return;
    };
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    let _ = writeln!(output, "{EVENT_PREFIX}{json}");
    let _ = output.flush();
}

#[cfg(test)]
mod tests {
    use super::{ProgressEvent, Stage};

    #[test]
    fn stage_names_are_stable_snake_case() {
        assert_eq!(
            serde_json::to_string(&Stage::Adblock).unwrap(),
            "\"adblock\""
        );
        assert_eq!(
            serde_json::to_string(&Stage::Shadowrocket).unwrap(),
            "\"shadowrocket\""
        );
    }

    #[test]
    fn progress_event_round_trips_optional_counters() {
        let event = ProgressEvent {
            stage: Stage::Validate,
            completed: Some(3),
            total: Some(8),
        };
        let encoded = serde_json::to_string(&event).unwrap();
        assert_eq!(
            serde_json::from_str::<ProgressEvent>(&encoded).unwrap(),
            event
        );
    }
}
