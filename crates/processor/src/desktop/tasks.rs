use super::i18n::Text;
use crate::runtime::progress::Stage;

pub(crate) fn stage_text(stage: Stage) -> Text {
    match stage {
        Stage::Prepare => Text::StagePrepare,
        Stage::Functional => Text::StageFunctional,
        Stage::General => Text::StageGeneral,
        Stage::Adblock => Text::StageAdblock,
        Stage::Cleanup => Text::StageCleanup,
        Stage::Categories => Text::StageCategories,
        Stage::Gaming => Text::StageGaming,
        Stage::Rewrites => Text::StageRewrites,
        Stage::Mitm => Text::StageMitm,
        Stage::Shadowrocket => Text::StageShadowrocket,
        Stage::Security => Text::StageSecurity,
        Stage::Mirrors => Text::StageMirrors,
        Stage::Catalog => Text::StageCatalog,
        Stage::Srs => Text::StageSrs,
        Stage::Finalize => Text::StageFinalize,
        Stage::Validate => Text::StageValidate,
        Stage::Repository => Text::Repository,
    }
}

pub(crate) fn is_problem(line: &str) -> bool {
    ["[WARN]", "[WARNING]", "[ERROR]", "[GUI ERROR]"]
        .iter()
        .any(|marker| line.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::is_problem;

    #[test]
    fn problem_filter_keeps_worker_warnings_and_failures() {
        for line in [
            "[WARN] stale source",
            "[WARNING] compiler",
            "[ERROR] failed",
            "[GUI ERROR] stopped",
        ] {
            assert!(is_problem(line));
        }
        assert!(!is_problem("[INFO] no errors found"));
    }
}
