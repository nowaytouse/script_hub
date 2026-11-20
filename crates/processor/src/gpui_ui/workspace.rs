use super::{Hub, Page, Palette, UiStyle, button, heading, panel};
use crate::desktop::i18n::Text;
use crate::desktop::tasks::{is_problem, stage_text};
use crate::runtime::catalog::{Catalog, ModulePlatform};
use crate::runtime::history::HistoryOptions;
use crate::runtime::job::{Job, JobKind, JobOptions, JobOutcome, validate_root};
use crate::runtime::repository;
use crate::runtime::settings::{self, Accent, Density, Language, Theme, WindowSize};
use gpui::{Context, Div, FontWeight, Window, div, prelude::*, px};
use std::path::PathBuf;
use std::time::Duration;

impl Hub {
    pub(super) fn validate_workspace(&mut self, cx: &mut Context<Self>) {
        self.queue_job(JobKind::Validate, cx);
    }

    pub(super) fn load_workspace(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.working(cx) || !self.templates.read(cx).can_close() {
            return;
        }
        self.loading = true;
        self.requested_root = Some(path.clone());
        self.pending_job = None;
        self.message = None;
        let task = cx.background_executor().spawn(async move {
            let root = validate_root(&path)?;
            let surge = Catalog::load_platform(&root, ModulePlatform::Surge);
            let shadowrocket = Catalog::load_platform(&root, ModulePlatform::Shadowrocket);
            let repository = repository::snapshot(&root);
            Ok::<_, String>((root, surge, shadowrocket, repository))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |hub, cx| {
                hub.loading = false;
                match result {
                    Ok((root, surge, shadowrocket, repository)) => {
                        hub.root = Some(root.clone());
                        hub.templates.update(cx, |templates, cx| {
                            templates.set_workspace(Some(root.clone()), cx);
                        });
                        hub.sources
                            .update(cx, |sources, cx| sources.set_workspace(Some(root), cx));
                        let mut warnings = Vec::new();
                        hub.surge = surge.unwrap_or_else(|error| {
                            warnings.push(error);
                            Catalog::bundled(ModulePlatform::Surge)
                        });
                        hub.shadowrocket = shadowrocket.unwrap_or_else(|error| {
                            warnings.push(error);
                            Catalog::bundled(ModulePlatform::Shadowrocket)
                        });
                        hub.repository = repository.map_err(|error| warnings.push(error)).ok();
                        hub.checked_modules.clear();
                        hub.checked_rules.clear();
                        hub.selected_changes.clear();
                        hub.inspect_module = None;
                        hub.active_category = None;
                        hub.message = (!warnings.is_empty()).then(|| warnings.join("\n"));
                    }
                    Err(error) => hub.message = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn reload_repository(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else {
            return;
        };
        self.load_workspace(root, cx);
    }

    fn job_label(&self, job: JobKind) -> &'static str {
        match job {
            JobKind::Validate => {
                self.local("Validate artifacts", "校验发布产物", "公開成果物を検証")
            }
            JobKind::RefreshCatalog => {
                self.local("Validate catalogs", "校验目录", "カタログを検証")
            }
            JobKind::Update => self.local("Update everything", "完整更新", "すべて更新"),
            JobKind::QuickUpdate => self.local("Quick update", "快速更新", "クイック更新"),
            JobKind::GamingOnly => {
                self.local("Update gaming rules", "更新游戏规则", "ゲームルールを更新")
            }
            JobKind::AdBlockOnly => {
                self.local("Update AdBlock", "更新广告规则", "広告ルールを更新")
            }
            JobKind::SrsOnly => self.local(
                "Build sing-box sets",
                "生成 sing-box 规则集",
                "sing-box ルールを作成",
            ),
            JobKind::GitPull => self.local("Pull changes", "拉取更新", "変更を取得"),
            JobKind::GitCommit => self.local("Commit changes", "提交更改", "変更をコミット"),
            JobKind::GitPush => self.local("Push to origin", "推送到远端", "origin にプッシュ"),
        }
    }

    fn queue_job(&mut self, kind: JobKind, cx: &mut Context<Self>) {
        if self.root.is_none() || self.project_action_blocked(cx) {
            return;
        }
        self.message = None;
        if matches!(kind, JobKind::Validate | JobKind::RefreshCatalog) {
            self.start_job(kind, cx);
        } else {
            self.pending_job = Some(kind);
            cx.notify();
        }
    }

    fn start_job(&mut self, kind: JobKind, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else {
            return;
        };
        if self.project_action_blocked(cx) {
            return;
        }
        let engine = self.saved_preferences.singbox.trim();
        let message = self.commit.read(cx).value().trim().to_owned();
        if kind == JobKind::GitCommit && (message.is_empty() || self.selected_changes.is_empty()) {
            self.message = Some(
                self.local(
                    "Select changed files and enter a commit message.",
                    "请选择变更文件并填写提交说明。",
                    "変更したファイルを選び、コミットメッセージを入力してください。",
                )
                .into(),
            );
            self.pending_job = None;
            cx.notify();
            return;
        }
        let history = self
            .preference_path
            .as_ref()
            .filter(|_| self.preferences_ready && self.saved_preferences.record_history)
            .map(|path| HistoryOptions {
                path: path.with_file_name("history.sqlite3"),
                keep_logs: self.saved_preferences.developer_mode
                    && self.saved_preferences.diagnostic_logs,
            });
        let options = JobOptions {
            root,
            singbox: (!engine.is_empty()).then(|| PathBuf::from(engine)),
            commit_message: (kind == JobKind::GitCommit).then_some(message),
            paths: if kind == JobKind::GitCommit {
                self.selected_changes.iter().cloned().collect()
            } else {
                Vec::new()
            },
            history,
        };
        self.pending_job = None;
        match Job::start(kind, options) {
            Ok(job) => {
                self.job = Some(job);
                self.log_query.update(cx, super::input::TextInput::clear);
                self.problems_only = false;
                self.history
                    .update(cx, |history, cx| history.task_running(true, cx));
                self.navigate(Page::Tasks, cx);
                cx.spawn(async move |this, cx| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(220))
                            .await;
                        let active = this
                            .update(cx, |hub, cx| {
                                let active = hub.job.as_ref().is_some_and(Job::is_running);
                                if !active {
                                    let history_open = hub.page == Page::History;
                                    hub.history.update(cx, |history, cx| {
                                        history.task_running(false, cx);
                                        if history_open {
                                            history.refresh(cx);
                                        }
                                    });
                                    if !matches!(kind, JobKind::Validate | JobKind::RefreshCatalog)
                                    {
                                        hub.reload_repository(cx);
                                    }
                                }
                                cx.notify();
                                active
                            })
                            .unwrap_or(false);
                        if !active {
                            break;
                        }
                    }
                })
                .detach();
            }
            Err(error) => self.message = Some(error),
        }
        cx.notify();
    }

    fn choose_workspace_card(&self, palette: Palette, cx: &mut Context<Self>) -> Div {
        panel(palette).p_5().gap_3()
            .child(self.local("Choose the Script Hub repository to manage local files and run tasks.", "选择 Script Hub 仓库，管理本地文件并运行任务。", "ローカルファイルを管理してタスクを実行するには、Script Hub のリポジトリを選択します。"))
            .child(div().flex().child(button("workspace-start", self.local("Open workspace", "打开工作区", "ワークスペースを開く"), palette, true)
                .on_click(cx.listener(|hub, _, _, cx| hub.pick_workspace(cx)))))
    }

    pub(super) fn workspace_page(&self, palette: Palette, cx: &mut Context<Self>) -> Div {
        let mut page = div().flex().flex_col().gap_5().child(heading(
            self.tr(Text::Workspace),
            self.local(
                "Browse local changes, review the remote and choose what to commit.",
                "查看本地变更、远端状态并选择需要提交的文件。",
                "ローカルの変更とリモートの状態を確認し、コミットするファイルを選びます。",
            ),
            palette,
        ));
        let Some(root) = &self.root else {
            return page.child(self.choose_workspace_card(palette, cx));
        };
        page = page.child(
            panel(palette)
                .p_5()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .font_weight(FontWeight::MEDIUM)
                                .child(root.display().to_string()),
                        )
                        .child(
                            button(
                                "repo-reload",
                                self.local("Refresh", "刷新", "更新"),
                                palette,
                                false,
                            )
                            .on_click(cx.listener(|hub, _, _, cx| {
                                if !hub.project_action_blocked(cx) {
                                    hub.reload_repository(cx);
                                }
                            })),
                        ),
                )
                .when(self.loading, |node| {
                    node.child(div().text_color(palette.muted).child(self.local(
                        "Reading workspace…",
                        "正在读取工作区…",
                        "ワークスペースを読み込み中…",
                    )))
                }),
        );
        let Some(repo) = &self.repository else {
            return page;
        };
        page.child(
            panel(palette)
                .p_5()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .bg(palette.subtle)
                                .rounded_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .child(repo.branch.clone()),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(palette.muted)
                                .child(format!("↑ {}    ↓ {}", repo.ahead, repo.behind)),
                        ),
                )
                .when(!repo.remote.is_empty(), |node| {
                    node.child(
                        div()
                            .text_sm()
                            .text_color(palette.muted)
                            .child(repo.remote.clone()),
                    )
                })
                .when(
                    repo.push_remote != repo.remote && !repo.push_remote.is_empty(),
                    |node| {
                        node.child(div().text_sm().text_color(palette.muted).child(format!(
                            "{} · {}",
                            self.job_label(JobKind::GitPush),
                            repo.push_remote
                        )))
                    },
                )
                .child(div().flex().flex_wrap().gap_2().children(
                    [JobKind::GitPull, JobKind::GitPush].map(|kind| {
                        button(
                            ("repo-job", kind as usize),
                            self.job_label(kind),
                            palette,
                            false,
                        )
                        .on_click(cx.listener(move |hub, _, _, cx| hub.queue_job(kind, cx)))
                    }),
                )),
        )
        .child(
            panel(palette)
                .child(
                    div()
                        .px_4()
                        .py_3()
                        .border_b_1()
                        .border_color(palette.border)
                        .font_weight(FontWeight::MEDIUM)
                        .child(self.local("Changed files", "变更文件", "変更ファイル")),
                )
                .when(repo.changes.is_empty(), |node| {
                    node.child(div().p_4().text_color(palette.muted).child(self.local(
                        "Working tree is clean.",
                        "工作树干净。",
                        "作業ツリーに変更はありません。",
                    )))
                })
                .children(repo.changes.iter().enumerate().map(|(index, change)| {
                    let path = change.path.clone();
                    let checked = self.selected_changes.contains(&path);
                    div()
                        .id(("git-path", index))
                        .tab_index(0)
                        .focus(move |style| style.bg(palette.subtle))
                        .flex()
                        .items_center()
                        .gap_3()
                        .px_4()
                        .py_3()
                        .border_b_1()
                        .border_color(palette.border)
                        .cursor_pointer()
                        .hover(move |style| style.bg(palette.subtle))
                        .child(
                            div()
                                .w(px(18.))
                                .text_color(palette.accent)
                                .child(if checked { "✓" } else { "□" }),
                        )
                        .child(
                            div()
                                .w(px(25.))
                                .text_xs()
                                .text_color(palette.muted)
                                .child(change.status.clone()),
                        )
                        .child(div().flex_1().min_w_0().child(match &change.previous_path {
                            Some(previous) => format!("{previous} → {path}"),
                            None => path.clone(),
                        }))
                        .on_click(cx.listener(move |hub, _, _, cx| {
                            if !hub.selected_changes.remove(&path) {
                                hub.selected_changes.insert(path.clone());
                            }
                            cx.notify();
                        }))
                }))
                .when(!repo.changes.is_empty(), |node| {
                    node.child(
                        div()
                            .p_4()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(self.commit.clone())
                            .child(
                                div().flex().child(
                                    button(
                                        "repo-commit",
                                        self.job_label(JobKind::GitCommit),
                                        palette,
                                        true,
                                    )
                                    .on_click(cx.listener(
                                        |hub, _, _, cx| hub.queue_job(JobKind::GitCommit, cx),
                                    )),
                                ),
                            ),
                    )
                }),
        )
        .when_some(self.pending_job, |node, job| {
            node.child(self.confirm_job(job, palette, cx))
        })
    }

    fn confirm_job(&self, job: JobKind, palette: Palette, cx: &mut Context<Self>) -> Div {
        panel(palette)
            .p_5()
            .gap_3()
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.job_label(job)),
            )
            .child(div().text_sm().text_color(palette.muted).child(self.local(
                "Review the selected workspace before running this operation.",
                "确认所选工作区后再执行此操作。",
                "選択したワークスペースを確認してから実行してください。",
            )))
            .when_some(self.root.as_ref(), |node, path| {
                node.child(div().text_sm().child(path.display().to_string()))
            })
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        button(
                            "confirm-job",
                            self.local("Run", "执行", "実行"),
                            palette,
                            true,
                        )
                        .on_click(cx.listener(move |hub, _, _, cx| hub.start_job(job, cx))),
                    )
                    .child(
                        button(
                            "cancel-job",
                            self.local("Cancel", "取消", "キャンセル"),
                            palette,
                            false,
                        )
                        .on_click(cx.listener(|hub, _, _, cx| {
                            hub.pending_job = None;
                            cx.notify();
                        })),
                    ),
            )
    }

    pub(super) fn tasks_page(&self, palette: Palette, cx: &mut Context<Self>) -> Div {
        let mut page = div().flex().flex_col().gap_5().child(heading(
            self.tr(Text::Tasks),
            self.local(
                "Validate and regenerate public artifacts for the selected workspace.",
                "校验并重新生成所选工作区的公开产物。",
                "選択したワークスペースの公開成果物を検証・再生成します。",
            ),
            palette,
        ));
        if self.root.is_none() {
            return page.child(self.choose_workspace_card(palette, cx));
        }
        let jobs = [
            JobKind::Validate,
            JobKind::RefreshCatalog,
            JobKind::Update,
            JobKind::QuickUpdate,
            JobKind::GamingOnly,
            JobKind::AdBlockOnly,
            JobKind::SrsOnly,
        ];
        page = page
            .child(
                panel(palette)
                    .p_5()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child(self.local("Operation", "操作", "操作")),
                    )
                    .child(div().flex().flex_wrap().gap_2().children(jobs.map(|job| {
                        button(
                            ("maintenance-choice", job as usize),
                            self.job_label(job),
                            palette,
                            job == self.job_kind,
                        )
                        .on_click(cx.listener(move |hub, _, _, cx| {
                            hub.job_kind = job;
                            hub.pending_job = None;
                            cx.notify();
                        }))
                    })))
                    .child(div().text_sm().text_color(palette.muted).child(self.tr(
                        match self.job_kind {
                            JobKind::Validate => Text::ValidateDescription,
                            JobKind::RefreshCatalog => Text::CatalogDescription,
                            JobKind::Update => Text::UpdateDescription,
                            JobKind::QuickUpdate => Text::QuickDescription,
                            JobKind::GamingOnly => Text::GamingDescription,
                            JobKind::AdBlockOnly => Text::AdBlockDescription,
                            JobKind::SrsOnly => Text::SrsDescription,
                            _ => Text::HelpGit,
                        },
                    )))
                    .child(
                        div().flex().child(
                            button(
                                "start-task",
                                self.local(
                                    "Run selected task",
                                    "运行所选任务",
                                    "選択したタスクを実行",
                                ),
                                palette,
                                true,
                            )
                            .on_click(cx.listener(|hub, _, _, cx| hub.queue_job(hub.job_kind, cx))),
                        ),
                    ),
            )
            .when_some(self.pending_job, |node, job| {
                node.child(self.confirm_job(job, palette, cx))
            });
        if let Some(job) = &self.job {
            let view = if self.show_logs {
                job.snapshot()
            } else {
                job.summary()
            };
            let outcome = if view.running {
                self.local("Running", "运行中", "実行中")
            } else {
                match view.outcome {
                    Some(JobOutcome::Success) => self.local("Completed", "已完成", "完了"),
                    Some(JobOutcome::Warnings) => self.local(
                        "Completed with warnings",
                        "已完成，有警告",
                        "警告ありで完了",
                    ),
                    _ => self.local("Failed", "失败", "失敗"),
                }
            };
            let elapsed = view.elapsed.as_secs();
            let progress = view
                .progress
                .and_then(|event| event.completed.zip(event.total))
                .filter(|&(_, total)| total > 0);
            let stage = view.running.then(|| {
                view.progress.map_or(self.tr(Text::StagePrepare), |event| {
                    self.tr(stage_text(event.stage))
                })
            });
            let search = self.log_query.read(cx).value().trim().to_lowercase();
            page = page.child(
                panel(palette)
                    .p_5()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(outcome),
                            )
                            .child(div().text_sm().text_color(palette.muted).child(format!(
                                "{:02}:{:02}",
                                elapsed / 60,
                                elapsed % 60
                            ))),
                    )
                    .child(div().text_sm().text_color(palette.muted).child(view.status))
                    .when_some(stage, |card, stage| {
                        card.child(div().text_sm().font_weight(FontWeight::MEDIUM).child(stage))
                    })
                    .when_some(progress, |card, (done, total)| {
                        card.child(
                            div()
                                .text_xs()
                                .text_color(palette.muted)
                                .child(format!("{done} / {total}")),
                        )
                        .child(
                            div().h(px(5.)).rounded_full().bg(palette.subtle).child(
                                div()
                                    .h_full()
                                    .w(gpui::relative(done.min(total) as f32 / total as f32))
                                    .rounded_full()
                                    .bg(palette.accent),
                            ),
                        )
                    })
                    .when_some(view.exit_code, |card, code| {
                        card.child(
                            div()
                                .text_sm()
                                .text_color(palette.muted)
                                .child(format!("{} {code}", self.tr(Text::ExitCode))),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(
                                button(
                                    "toggle-job-logs",
                                    self.tr(if self.show_logs {
                                        Text::HideLogs
                                    } else {
                                        Text::ShowLogs
                                    }),
                                    palette,
                                    false,
                                )
                                .on_click(cx.listener(
                                    |hub, _, _, cx| {
                                        hub.show_logs = !hub.show_logs;
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                button("copy-job-logs", self.tr(Text::CopyLogs), palette, false)
                                    .on_click(cx.listener(|hub, _, _, cx| {
                                        if let Some(job) = &hub.job {
                                            hub.copy(
                                                job.snapshot()
                                                    .logs
                                                    .into_iter()
                                                    .collect::<Vec<_>>()
                                                    .join("\n"),
                                                cx,
                                            );
                                        }
                                    })),
                            )
                            .child(
                                button("clear-job-logs", self.tr(Text::ClearLogs), palette, false)
                                    .on_click(cx.listener(|hub, _, _, cx| {
                                        if let Some(job) = &hub.job {
                                            job.clear_logs();
                                            cx.notify();
                                        }
                                    })),
                            ),
                    )
                    .when(self.show_logs, |node| {
                        node.child(
                            div()
                                .text_xs()
                                .text_color(palette.muted)
                                .child(self.tr(Text::ShareLogsWarning)),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .items_center()
                                .gap_2()
                                .child(div().flex_1().min_w(px(180.)).child(self.log_query.clone()))
                                .child(
                                    button(
                                        "task-problems-only",
                                        self.tr(Text::ProblemsOnly),
                                        palette,
                                        self.problems_only,
                                    )
                                    .on_click(cx.listener(
                                        |hub, _, _, cx| {
                                            hub.problems_only = !hub.problems_only;
                                            cx.notify();
                                        },
                                    )),
                                ),
                        )
                        .child(
                            div()
                                .id("task-logs")
                                .overflow_y_scroll()
                                .max_h(px(350.))
                                .p_3()
                                .rounded_md()
                                .bg(palette.subtle)
                                .text_xs()
                                .children(
                                    view.logs
                                        .into_iter()
                                        .filter(|line| {
                                            (!self.problems_only || is_problem(line))
                                                && (search.is_empty()
                                                    || line.to_lowercase().contains(&search))
                                        })
                                        .map(|line| div().py_1().child(line)),
                                ),
                        )
                    }),
            );
        } else {
            page = page.child(
                panel(palette)
                    .p_5()
                    .text_color(palette.muted)
                    .child(self.tr(Text::NoTask)),
            );
        }
        page
    }

    fn save_preferences(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(path) = &self.preference_path else {
            self.message = Some(
                self.local(
                    "Settings storage is unavailable.",
                    "设置存储不可用。",
                    "設定を保存できません。",
                )
                .into(),
            );
            cx.notify();
            return false;
        };
        let mut preferences = self.preferences.clone();
        preferences.singbox = self.singbox.read(cx).value().trim().into();
        if let Some(root) = &self.root {
            preferences.root = root.to_string_lossy().into();
        }
        let saved = match settings::save(path, &preferences) {
            Ok(()) => {
                self.preferences = preferences;
                self.saved_preferences = self.preferences.clone();
                self.preferences_ready = true;
                self.message = Some(self.tr(Text::SettingsSaved).into());
                true
            }
            Err(error) => {
                self.message = Some(error);
                false
            }
        };
        cx.notify();
        saved
    }

    fn apply_settings_draft(&mut self, preferences: settings::Preferences, cx: &mut Context<Self>) {
        self.singbox.update(cx, |field, cx| {
            field.set_value(preferences.singbox.clone(), cx)
        });
        self.preferences = preferences;
        self.message = None;
        self.reset_language(cx);
        cx.set_global(UiStyle::from(&self.preferences));
        cx.refresh_windows();
    }

    fn finish_settings_quit(&mut self, save: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_blocked(cx) {
            self.can_close(cx);
        } else if !save || self.save_preferences(cx) {
            self.confirm_quit = false;
            window.remove_window();
        }
    }

    fn settings_quit_card(&self, palette: Palette, cx: &mut Context<Self>) -> Div {
        panel(palette)
            .p_4()
            .gap_3()
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.tr(Text::UnsavedChanges)),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .when(self.preference_path.is_some(), |row| {
                        row.child(
                            button(
                                "save-settings-quit",
                                self.tr(Text::SettingsSaveAndQuit),
                                palette,
                                true,
                            )
                            .on_click(cx.listener(
                                |hub, _, window, cx| hub.finish_settings_quit(true, window, cx),
                            )),
                        )
                    })
                    .child(
                        button(
                            "discard-settings-quit",
                            self.tr(Text::SettingsDiscardAndQuit),
                            palette,
                            false,
                        )
                        .on_click(cx.listener(|hub, _, window, cx| {
                            hub.finish_settings_quit(false, window, cx)
                        })),
                    )
                    .child(
                        button(
                            "cancel-settings-quit",
                            self.tr(Text::Cancel),
                            palette,
                            false,
                        )
                        .on_click(cx.listener(|hub, _, _, cx| {
                            hub.confirm_quit = false;
                            cx.notify();
                        })),
                    ),
            )
    }

    pub(super) fn settings_page(&self, palette: Palette, cx: &mut Context<Self>) -> Div {
        let theme = [Theme::System, Theme::Light, Theme::Dark];
        let language = [
            (Language::System, self.tr(Text::System)),
            (Language::En, "English"),
            (Language::ZhCn, "简体中文"),
            (Language::Ja, "日本語"),
        ];
        let accents = [
            (Accent::Purple, self.local("Violet", "紫色", "バイオレット")),
            (Accent::Blue, self.local("Blue", "蓝色", "ブルー")),
            (Accent::Teal, self.local("Teal", "青色", "ティール")),
            (Accent::Amber, self.local("Amber", "琥珀色", "アンバー")),
        ];
        let heading_style = |name| div().text_sm().font_weight(FontWeight::MEDIUM).child(name);
        let mut toggles = vec![
            (
                "setting-reopen",
                self.local(
                    "Reopen workspace",
                    "启动时重开工作区",
                    "ワークスペースを再度開く",
                ),
                self.preferences.reopen_workspace,
            ),
            (
                "setting-history",
                self.local("Keep task history", "保留任务历史", "タスク履歴を保持"),
                self.preferences.record_history,
            ),
            (
                "setting-developer",
                self.local("Developer mode", "开发者模式", "開発者モード"),
                self.preferences.developer_mode,
            ),
        ];
        if self.preferences.developer_mode {
            toggles.push((
                "setting-diagnostics",
                self.local("Keep diagnostic logs", "保留诊断日志", "診断ログを保持"),
                self.preferences.diagnostic_logs,
            ));
        }
        div()
            .flex()
            .flex_col()
            .gap_5()
            .child(heading(
                self.tr(Text::Settings),
                self.local(
                    "Appearance and workspace preferences",
                    "外观和工作区偏好",
                    "外観とワークスペースの設定",
                ),
                palette,
            ))
            .when(self.confirm_quit, |page| {
                page.child(self.settings_quit_card(palette, cx))
            })
            .when(!self.preferences_ready, |page| {
                page.child(
                    panel(palette)
                        .p_4()
                        .child(self.tr(Text::HistorySettingsUnavailable)),
                )
            })
            .child(
                panel(palette)
                    .p_5()
                    .gap_3()
                    .child(heading_style(self.tr(Text::Theme)))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .children(theme.map(|theme| {
                                let label = self.tr(match theme {
                                    Theme::System => Text::System,
                                    Theme::Light => Text::Light,
                                    Theme::Dark => Text::Dark,
                                });
                                button(
                                    ("set-theme", theme as usize),
                                    label,
                                    palette,
                                    self.preferences.theme == theme,
                                )
                                .on_click(cx.listener(
                                    move |hub, _, _, cx| {
                                        hub.preferences.theme = theme;
                                        cx.set_global(UiStyle::from(&hub.preferences));
                                        cx.refresh_windows();
                                        cx.notify();
                                    },
                                ))
                            })),
                    )
                    .child(heading_style(self.local("Accent", "强调色", "アクセント")))
                    .child(div().flex().flex_wrap().gap_2().children(accents.map(
                        |(accent, label)| {
                            button(
                                ("set-accent", accent as usize),
                                label,
                                palette,
                                self.preferences.accent == accent,
                            )
                            .on_click(cx.listener(
                                move |hub, _, _, cx| {
                                    hub.preferences.accent = accent;
                                    cx.set_global(UiStyle::from(&hub.preferences));
                                    cx.refresh_windows();
                                    cx.notify();
                                },
                            ))
                        },
                    )))
                    .child(heading_style(self.tr(Text::Language)))
                    .child(div().flex().flex_wrap().gap_2().children(language.map(
                        |(language, label)| {
                            button(
                                ("set-language", language as usize),
                                label,
                                palette,
                                self.preferences.language == language,
                            )
                            .on_click(cx.listener(
                                move |hub, _, _, cx| {
                                    hub.preferences.language = language;
                                    hub.reset_language(cx);
                                },
                            ))
                        },
                    )))
                    .child(heading_style(self.local(
                        "List density",
                        "列表密度",
                        "リストの密度",
                    )))
                    .child(
                        div().flex().gap_2().children(
                            [
                                (
                                    Density::Comfortable,
                                    self.local("Comfortable", "舒适", "ゆったり"),
                                ),
                                (
                                    Density::Compact,
                                    self.local("Compact", "紧凑", "コンパクト"),
                                ),
                            ]
                            .map(|(density, label)| {
                                button(
                                    ("set-density", density as usize),
                                    label,
                                    palette,
                                    self.preferences.density == density,
                                )
                                .on_click(cx.listener(
                                    move |hub, _, _, cx| {
                                        hub.preferences.density = density;
                                        cx.notify();
                                    },
                                ))
                            }),
                        ),
                    )
                    .child(heading_style(self.local(
                        "Interface scale",
                        "界面缩放",
                        "表示倍率",
                    )))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .children([90u16, 100, 115, 130].map(|scale| {
                                button(
                                    ("set-scale", usize::from(scale)),
                                    format!("{scale}%"),
                                    palette,
                                    self.preferences.scale_percent == scale,
                                )
                                .on_click(cx.listener(
                                    move |hub, _, _, cx| {
                                        hub.preferences.scale_percent = scale;
                                        cx.notify();
                                    },
                                ))
                            })),
                    )
                    .child(heading_style(self.local(
                        "Window size on next launch",
                        "下次启动时的窗口尺寸",
                        "次回起動時のウィンドウサイズ",
                    )))
                    .child(
                        div().flex().gap_2().children(
                            [
                                (
                                    WindowSize::Compact,
                                    self.local("Compact", "小", "コンパクト"),
                                ),
                                (WindowSize::Standard, self.local("Standard", "标准", "標準")),
                                (WindowSize::Wide, self.local("Wide", "宽", "ワイド")),
                            ]
                            .map(|(size, label)| {
                                button(
                                    ("set-window", size as usize),
                                    label,
                                    palette,
                                    self.preferences.window == size,
                                )
                                .on_click(cx.listener(
                                    move |hub, _, _, cx| {
                                        hub.preferences.window = size;
                                        cx.notify();
                                    },
                                ))
                            }),
                        ),
                    ),
            )
            .child(
                panel(palette)
                    .p_5()
                    .gap_3()
                    .child(heading_style(self.local(
                        "Workspace and tasks",
                        "工作区与任务",
                        "ワークスペースとタスク",
                    )))
                    .child(self.singbox.clone())
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .children(toggles.into_iter().map(|(id, label, enabled)| {
                                button(
                                    id,
                                    format!("{} {label}", if enabled { "✓" } else { "□" }),
                                    palette,
                                    false,
                                )
                                .on_click(cx.listener(
                                    move |hub, _, _, cx| {
                                        match id {
                                            "setting-reopen" => {
                                                hub.preferences.reopen_workspace =
                                                    !hub.preferences.reopen_workspace
                                            }
                                            "setting-history" => {
                                                hub.preferences.record_history =
                                                    !hub.preferences.record_history
                                            }
                                            "setting-developer" => {
                                                hub.preferences.developer_mode =
                                                    !hub.preferences.developer_mode
                                            }
                                            _ => {
                                                hub.preferences.diagnostic_logs =
                                                    !hub.preferences.diagnostic_logs
                                            }
                                        }
                                        cx.notify();
                                    },
                                ))
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(
                                button(
                                    "save-preferences",
                                    self.local("Save settings", "保存设置", "設定を保存"),
                                    palette,
                                    true,
                                )
                                .on_click(cx.listener(
                                    |hub, _, _, cx| {
                                        hub.save_preferences(cx);
                                    },
                                )),
                            )
                            .child(
                                button(
                                    "revert-preferences",
                                    self.local("Revert changes", "撤销更改", "変更を取り消す"),
                                    palette,
                                    false,
                                )
                                .on_click(cx.listener(
                                    |hub, _, _, cx| {
                                        hub.apply_settings_draft(hub.saved_preferences.clone(), cx);
                                    },
                                )),
                            )
                            .child(
                                button(
                                    "reset-settings",
                                    self.tr(Text::ResetSettings),
                                    palette,
                                    false,
                                )
                                .on_click(cx.listener(
                                    |hub, _, _, cx| {
                                        hub.apply_settings_draft(
                                            settings::Preferences {
                                                root: hub.preferences.root.clone(),
                                                ..Default::default()
                                            },
                                            cx,
                                        );
                                    },
                                )),
                            )
                            .child(
                                button(
                                    "quit-settings",
                                    self.tr(Text::SettingsQuit),
                                    palette,
                                    false,
                                )
                                .on_click(cx.listener(
                                    |hub, _, window, cx| {
                                        if hub.can_close(cx) {
                                            window.remove_window();
                                        }
                                    },
                                )),
                            ),
                    ),
            )
            .child(self.settings_tools(palette, cx))
    }

    fn settings_tools(&self, palette: Palette, cx: &mut Context<Self>) -> Div {
        panel(palette)
            .p_5()
            .gap_3()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.local("Offline demo", "离线演示", "オフラインデモ")),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(palette.muted)
                    .child(self.tr(Text::RelayDemoOffline)),
            )
            .child(
                div().flex().flex_wrap().gap_2().child(
                    button(
                        "open-demo",
                        self.tr(Text::RelayDemoOpenWindow),
                        palette,
                        true,
                    )
                    .on_click(cx.listener(|hub, _, _, cx| hub.open_demo(cx))),
                ),
            )
    }
}
