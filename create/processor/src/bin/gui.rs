#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use eframe::egui::{self, Color32, RichText};
use rust_processor::gui_job::{Job, JobKind, JobOptions};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

#[derive(Clone, Deserialize)]
struct ModuleItem {
    name: String,
    desc: String,
    category: String,
    install_url: String,
    github_install_url: String,
    merged_into: String,
}

struct RuleItem {
    name: String,
    category: String,
    cdn: String,
    github: String,
    count: Option<u64>,
}

fn subscription_url(path: &str, github: bool) -> String {
    let path = path
        .split('/')
        .map(|part| {
            percent_encoding::utf8_percent_encode(part, percent_encoding::NON_ALPHANUMERIC)
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("/");
    let prefix = if github {
        "https://raw.githubusercontent.com/nowaytouse/script_hub/master/"
    } else {
        "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/"
    };
    format!("{prefix}{path}")
}

fn load_rules(root: &Path) -> Result<Vec<RuleItem>, String> {
    let catalog_path = root.join("rulesets/AdBlock/catalog.json");
    let content = std::fs::read_to_string(&catalog_path)
        .map_err(|error| format!("读取 {} 失败：{error}", catalog_path.display()))?;
    let catalog: serde_json::Value =
        serde_json::from_str(&content).map_err(|error| format!("广告规则索引无效：{error}"))?;
    let shards = catalog["shards"]
        .as_array()
        .ok_or("广告规则索引缺少 shards 数组")?;
    let mut rules = Vec::new();
    for shard in shards {
        let name = shard["file"]
            .as_str()
            .or_else(|| shard["filename"].as_str())
            .ok_or("广告规则分片缺少文件名")?;
        rules.push(RuleItem {
            name: name.to_string(),
            category: shard["label_zh"].as_str().unwrap_or("广告拦截").to_string(),
            cdn: shard["cdn_url"]
                .as_str()
                .ok_or("分片缺少 CDN 地址")?
                .to_string(),
            github: shard["github_url"]
                .as_str()
                .ok_or("分片缺少 GitHub 地址")?
                .to_string(),
            count: shard["rule_count"].as_u64(),
        });
    }
    let directory = root.join("rulesets/RULE-SET");
    let entries = std::fs::read_dir(&directory)
        .map_err(|error| format!("读取 {} 失败：{error}", directory.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("读取规则集目录项失败：{error}"))?;
        if !entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_file()
        {
            continue;
        }
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "list")
        {
            let name = entry.file_name().to_string_lossy().into_owned();
            let relative = format!("rulesets/RULE-SET/{name}");
            rules.push(RuleItem {
                name,
                category: "分流规则".to_string(),
                cdn: subscription_url(&relative, false),
                github: subscription_url(&relative, true),
                count: None,
            });
        }
    }
    rules.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(rules)
}

fn default_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_default();
    for start in [
        Some(cwd.as_path()),
        std::env::current_exe()
            .ok()
            .as_deref()
            .and_then(Path::parent),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(root) = start.ancestors().find(|path| {
            path.join("rulesets/Sources/adblock_whitelist.list")
                .is_file()
        }) {
            return root.to_path_buf();
        }
    }
    cwd
}

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Modules,
    Rules,
    Maintenance,
}

struct HubApp {
    root: String,
    loaded_root: Option<PathBuf>,
    singbox: String,
    shadowrocket: bool,
    search: String,
    page: Page,
    modules: Vec<ModuleItem>,
    rules: Vec<RuleItem>,
    error: Option<String>,
    notice: String,
    job: Option<Job>,
    job_finished_handled: bool,
    pending: Option<JobKind>,
}

impl HubApp {
    fn new(root: PathBuf) -> Self {
        let mut app = Self {
            root: root.to_string_lossy().into_owned(),
            loaded_root: None,
            singbox: String::new(),
            shadowrocket: false,
            search: String::new(),
            page: Page::Modules,
            modules: Vec::new(),
            rules: Vec::new(),
            error: None,
            notice: "尚未执行更新；浏览目录不会联网。".to_string(),
            job: None,
            job_finished_handled: true,
            pending: None,
        };
        app.reload();
        app
    }

    fn reload(&mut self) {
        let result = (|| {
            let root = rust_processor::gui_job::validate_root(Path::new(self.root.trim()))?;
            let modules =
                rust_processor::module_catalog::catalog_snapshot(&root, self.shadowrocket)?;
            let modules = serde_json::from_value(modules)
                .map_err(|error| format!("模块目录无效：{error}"))?;
            let rules = load_rules(&root)?;
            Ok::<_, String>((root, modules, rules))
        })();
        match result {
            Ok((root, modules, rules)) => {
                self.root = root.to_string_lossy().into_owned();
                self.loaded_root = Some(root);
                self.modules = modules;
                self.rules = rules;
                self.error = None;
            }
            Err(error) => {
                self.loaded_root = None;
                self.modules.clear();
                self.rules.clear();
                self.error = Some(error);
            }
        }
    }

    fn active(&self) -> bool {
        self.job.as_ref().is_some_and(Job::is_running)
    }

    fn start(&mut self, kind: JobKind) {
        if self.active() {
            return;
        }
        let options = JobOptions {
            root: PathBuf::from(self.root.trim()),
            singbox: (!self.singbox.trim().is_empty()).then(|| PathBuf::from(self.singbox.trim())),
        };
        match Job::start(kind, options) {
            Ok(job) => {
                self.job = Some(job);
                self.job_finished_handled = false;
                self.notice = "任务正在执行；完成前请保留窗口。".to_string();
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn copy_button(ui: &mut egui::Ui, label: &str, url: &str, notice: &mut String) {
        if ui.button(label).clicked() {
            ui.ctx().copy_text(url.to_string());
            *notice = "订阅地址已复制，请在对应客户端中添加。".to_string();
        }
    }

    fn module_page(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let old = self.shadowrocket;
            ui.selectable_value(&mut self.shadowrocket, false, "Surge");
            ui.selectable_value(&mut self.shadowrocket, true, "Shadowrocket");
            if old != self.shadowrocket {
                self.reload();
            }
            ui.label(format!("{} 个模块", self.modules.len()));
        });
        ui.add_space(8.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text("搜索名称、说明或分类")
                .desired_width(f32::INFINITY),
        );
        let search = self.search.to_lowercase();
        egui::ScrollArea::vertical()
            .id_salt("modules")
            .show(ui, |ui| {
                let mut found = 0;
                for module in self.modules.iter().filter(|module| {
                    format!("{} {} {}", module.name, module.desc, module.category)
                        .to_lowercase()
                        .contains(&search)
                }) {
                    found += 1;
                    egui::Frame::group(ui.style())
                        .inner_margin(14.0)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(RichText::new(&module.name).size(17.0).strong());
                            ui.weak(&module.category);
                            ui.label(module.desc.replace("\\n", "\n"));
                            ui.add_space(6.0);
                            if module.merged_into.is_empty() {
                                ui.horizontal_wrapped(|ui| {
                                    Self::copy_button(
                                        ui,
                                        "复制 CDN 地址",
                                        &module.install_url,
                                        &mut self.notice,
                                    );
                                    Self::copy_button(
                                        ui,
                                        "复制 GitHub 地址",
                                        &module.github_install_url,
                                        &mut self.notice,
                                    );
                                });
                            } else {
                                ui.label(format!(
                                    "已整合至 {}，仅保留旧链接兼容，不重复安装。",
                                    module.merged_into
                                ));
                            }
                        });
                    ui.add_space(8.0);
                }
                if found == 0 {
                    ui.label("没有符合条件的模块。请检查项目目录、平台或搜索词。");
                }
            });
    }

    fn rules_page(&mut self, ui: &mut egui::Ui) {
        ui.label("广告规则按活动 catalog 展示；分流规则的策略由客户端配置决定。");
        ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text("搜索规则集")
                .desired_width(f32::INFINITY),
        );
        let search = self.search.to_lowercase();
        egui::ScrollArea::vertical()
            .id_salt("rules")
            .show(ui, |ui| {
                let mut found = 0;
                for rule in self.rules.iter().filter(|rule| {
                    format!("{} {}", rule.name, rule.category)
                        .to_lowercase()
                        .contains(&search)
                }) {
                    found += 1;
                    ui.group(|ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new(&rule.name).strong());
                        ui.weak(match rule.count {
                            Some(count) => format!("{} · {count} 条规则", rule.category),
                            None => rule.category.clone(),
                        });
                        ui.horizontal_wrapped(|ui| {
                            Self::copy_button(ui, "复制 CDN 地址", &rule.cdn, &mut self.notice);
                            Self::copy_button(
                                ui,
                                "复制 GitHub 地址",
                                &rule.github,
                                &mut self.notice,
                            );
                        });
                    });
                    ui.add_space(6.0);
                }
                if found == 0 {
                    ui.label("没有符合条件的规则集。");
                }
            });
    }

    fn maintenance_page(&mut self, ui: &mut egui::Ui) {
        ui.heading("本地维护");
        ui.label("与命令行共用同一套 Rust 流程。不会自动提交、推送或更改系统代理。");
        ui.add_space(10.0);
        ui.label("sing-box 可执行文件（可选，仅生成 SRS 时需要）");
        ui.add_enabled(
            !self.active(),
            egui::TextEdit::singleline(&mut self.singbox).desired_width(f32::INFINITY),
        );
        if self.singbox.trim().is_empty() {
            ui.weak("未指定 sing-box：更新不会重新生成 SRS，已有 SRS 保持不变。");
        }
        ui.add_space(10.0);
        let current_root_loaded = self.loaded_root.as_deref() == Some(Path::new(self.root.trim()));
        ui.add_enabled_ui(!self.active() && current_root_loaded, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("校验全部产物（只读）").clicked() {
                    self.start(JobKind::Validate);
                }
                if ui.button("刷新安装目录（本地）").clicked() {
                    self.pending = Some(JobKind::RefreshCatalog);
                }
                if ui.button("全量更新（联网）").clicked() {
                    self.pending = Some(JobKind::Update);
                }
            });
        });
        ui.add_space(12.0);
        if let Some(job) = &self.job {
            let snapshot = job.snapshot();
            ui.horizontal(|ui| {
                if snapshot.running {
                    ui.label("● 运行中");
                }
                ui.label(&snapshot.status);
            });
            ui.weak("仅保留最近的有界日志；窗口空闲时不持续刷新。");
            egui::ScrollArea::both()
                .stick_to_bottom(true)
                .id_salt("logs")
                .show(ui, |ui| {
                    for line in &snapshot.logs {
                        ui.monospace(line);
                    }
                });
        } else {
            ui.weak("尚未启动任务。建议先执行只读校验。");
        }
    }
}

impl eframe::App for HubApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.job_finished_handled && !self.active() {
            self.job_finished_handled = true;
            if let Some(job) = &self.job {
                self.notice = job.snapshot().status;
            }
            self.reload();
        }
        if self.active() {
            ctx.request_repaint_after(Duration::from_millis(250));
            if ctx.input(|input| input.viewport().close_requested()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.notice = "任务尚未完成，为避免中断发布，请等待结果后关闭。".to_string();
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("SCRIPT HUB")
                        .size(25.0)
                        .strong()
                        .color(Color32::from_rgb(72, 160, 190)),
                );
                ui.weak("规则与模块控制台");
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("项目目录");
                ui.add_enabled(
                    !self.active(),
                    egui::TextEdit::singleline(&mut self.root)
                        .desired_width(ui.available_width() - 85.0),
                );
                if ui
                    .add_enabled(!self.active(), egui::Button::new("加载目录"))
                    .clicked()
                {
                    self.reload();
                }
            });
            if self
                .loaded_root
                .as_ref()
                .is_some_and(|root| Path::new(self.root.trim()) != root)
            {
                ui.colored_label(
                    Color32::from_rgb(190, 130, 40),
                    "目录已修改，请先加载新目录；当前维护操作已暂停。",
                );
            }
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut self.page, Page::Modules, "模块目录");
                ui.selectable_value(&mut self.page, Page::Rules, "规则集");
                ui.selectable_value(&mut self.page, Page::Maintenance, "更新与校验");
            });
            ui.separator();
            ui.label(&self.notice);
            if let Some(error) = &self.error {
                ui.colored_label(Color32::from_rgb(210, 90, 70), error);
            }
            ui.add_space(8.0);
            match self.page {
                Page::Modules => self.module_page(ui),
                Page::Rules => self.rules_page(ui),
                Page::Maintenance => self.maintenance_page(ui),
            }
        });
        if let Some(kind) = self.pending {
            egui::Window::new("确认写入项目文件").collapsible(false).resizable(false).show(ui.ctx(), |ui| {
                ui.label("此操作会更新所选工作区中的生成文件；请先保存自己的改动。");
                if matches!(kind, JobKind::Update | JobKind::QuickUpdate) {
                    ui.label("将从已配置的上游联网下载，沿用进程的代理环境变量。不会自动执行 Git 提交或推送。");
                }
                ui.horizontal(|ui| {
                    if ui.button("确认执行").clicked() {
                        self.pending = None;
                        self.start(kind);
                    }
                    if ui.button("取消").clicked() {
                        self.pending = None;
                    }
                });
            });
        }
    }
}

fn worker(mut args: impl Iterator<Item = String>) -> ExitCode {
    let operation = args.next();
    let arguments: Vec<String> = args.collect();
    let _write_lock = if matches!(operation.as_deref(), Some("update" | "catalog")) {
        let root = arguments.windows(2).find(|pair| pair[0] == "--root");
        let result = root
            .ok_or_else(|| "missing --root <repository>".to_string())
            .and_then(|pair| rust_processor::gui_job::validate_root(Path::new(&pair[1])))
            .and_then(|root| rust_processor::gui_job::acquire_write_lock(&root));
        match result {
            Ok(lock) => Some(lock),
            Err(error) => {
                eprintln!("[ERROR] {error}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        None
    };
    let mut args = arguments.into_iter();
    match operation.as_deref() {
        Some("validate") => rust_processor::validate_cli::run_cli(args),
        Some("update") => rust_processor::update_cli::run_cli(args),
        Some("catalog") => {
            if args.next().as_deref() != Some("--root") {
                eprintln!("[ERROR] catalog requires --root <repository>");
                return ExitCode::from(2);
            }
            let Some(root) = args.next() else {
                return ExitCode::from(2);
            };
            let result = rust_processor::gui_job::validate_root(Path::new(&root))
                .and_then(|root| rust_processor::module_catalog::refresh(&root));
            match result {
                Ok(()) => {
                    println!("[SUCCESS] refreshed module catalog and helper");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("[ERROR] {error}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("[ERROR] unknown worker operation");
            ExitCode::from(2)
        }
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let root = match args.next().as_deref() {
        Some("--worker") => return worker(args),
        Some("--root") => match args.next() {
            Some(root) if args.next().is_none() => PathBuf::from(root),
            _ => {
                eprintln!("Usage: script-hub-gui [--root <repository>]");
                return ExitCode::from(2);
            }
        },
        Some("--help" | "-h") => {
            println!("Usage: script-hub-gui [--root <repository>]");
            return ExitCode::SUCCESS;
        }
        Some(_) => {
            eprintln!("Usage: script-hub-gui [--root <repository>]");
            return ExitCode::from(2);
        }
        None => default_root(),
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1040.0, 760.0])
            .with_min_inner_size([640.0, 480.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    match eframe::run_native(
        "Script Hub",
        options,
        Box::new(move |_cc| Ok(Box::new(HubApp::new(root)))),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Cannot start Script Hub GUI: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_paths_encode_each_component_without_losing_separators() {
        let url = subscription_url("rulesets/RULE-SET/Apple Media.list", true);
        assert!(url.ends_with("rulesets/RULE%2DSET/Apple%20Media%2Elist"));
    }

    #[test]
    fn missing_catalog_is_an_error_instead_of_a_silent_empty_view() {
        assert!(load_rules(Path::new("nonexistent-script-hub-root")).is_err());
    }

    #[test]
    fn repository_catalog_loads_for_both_platforms_without_network() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let mut app = HubApp::new(root.to_path_buf());
        assert!(app.error.is_none(), "{:?}", app.error);
        assert!(!app.modules.is_empty());
        assert!(app.rules.iter().any(|rule| rule.count.is_some()));
        app.shadowrocket = true;
        app.reload();
        assert!(app.error.is_none(), "{:?}", app.error);
        assert!(!app.modules.is_empty());
    }

    #[test]
    fn invalid_root_clears_previous_catalog_and_surfaces_the_error() {
        let app = HubApp::new(PathBuf::from("nonexistent-script-hub-root"));
        assert!(app.error.is_some());
        assert!(app.modules.is_empty() && app.rules.is_empty());
    }
}
