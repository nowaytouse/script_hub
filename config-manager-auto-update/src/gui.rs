use eframe::egui;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::{load_config_sync, save_config_sync};
use crate::subscription::SubscriptionManager;
use crate::types::{Config, LogMessage, Subscription};
use crate::updater::CoreUpdater;

/// GUI 应用程序状态
pub struct SingboxManagerApp {
    config: Config,
    config_path: PathBuf,
    logs: Arc<Mutex<Vec<LogMessage>>>,
    log_display: String,
    status: String,
    
    // 运行时
    runtime: tokio::runtime::Runtime,
    
    // 订阅编辑
    show_add_subscription: bool,
    edit_subscription_name: String,
    edit_subscription_url: String,
    edit_subscription_path: String,
    edit_subscription_index: Option<usize>,
    
    // 自动更新
    auto_update_enabled: bool,
}

impl Default for SingboxManagerApp {
    fn default() -> Self {
        let config_path = PathBuf::from("config.json");
        let config = load_config_sync(&config_path).unwrap_or_else(|_| Config::default());
        
        Self {
            config,
            config_path,
            logs: Arc::new(Mutex::new(Vec::new())),
            log_display: String::new(),
            status: "就绪".to_string(),
            runtime: tokio::runtime::Runtime::new().unwrap(),
            show_add_subscription: false,
            edit_subscription_name: String::new(),
            edit_subscription_url: String::new(),
            edit_subscription_path: String::from("./singbox_config.json"),
            edit_subscription_index: None,
            auto_update_enabled: false,
        }
    }
}

impl SingboxManagerApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }

    fn add_log(&mut self, message: String) {
        let log_msg = LogMessage::new(message);
        self.log_display.push_str(&format!("{}\n", log_msg.formatted()));
        
        let logs = self.logs.clone();
        self.runtime.spawn(async move {
            let mut logs = logs.lock().await;
            logs.push(log_msg);
        });
    }

    fn set_status(&mut self, status: String) {
        self.status = status.clone();
        self.add_log(status);
    }

    fn save_config(&mut self) {
        if let Err(e) = save_config_sync(&self.config_path, &self.config) {
            self.add_log(format!("保存配置失败: {}", e));
        }
    }

    fn update_subscriptions(&mut self) {
        self.set_status("正在更新订阅...".to_string());
        
        let config = self.config.clone();
        let logs = self.logs.clone();
        
        self.runtime.spawn(async move {
            let manager = SubscriptionManager::new(logs);
            if let Err(e) = manager.run_subscription_downloader(&config).await {
                log::error!("订阅更新失败: {}", e);
            }
        });
    }

    fn update_core(&mut self) {
        self.set_status("正在更新 Sing-box 核心...".to_string());
        
        let config = self.config.clone();
        let logs = self.logs.clone();
        
        self.runtime.spawn(async move {
            let updater = CoreUpdater::new(logs);
            if let Err(e) = updater.run_core_updater(&config).await {
                log::error!("核心更新失败: {}", e);
            }
        });
    }

    fn render_status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("状态:").strong());
            ui.label(&self.status);
        });
        ui.separator();
    }

    fn render_subscriptions(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("📋 订阅管理")
            .default_open(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .show(ui, |ui| {
                        let mut to_delete: Option<usize> = None;
                        
                        for (idx, sub) in self.config.subscriptions.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(&sub.name);
                                ui.label("-");
                                ui.label(&sub.url);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button("🗑 删除").clicked() {
                                        to_delete = Some(idx);
                                    }
                                    if ui.button("✏ 编辑").clicked() {
                                        self.edit_subscription_index = Some(idx);
                                        self.edit_subscription_name = sub.name.clone();
                                        self.edit_subscription_url = sub.url.clone();
                                        self.edit_subscription_path = sub.save_path.display().to_string();
                                        self.show_add_subscription = true;
                                    }
                                });
                            });
                            ui.separator();
                        }
                        
                        if let Some(idx) = to_delete {
                            self.config.subscriptions.remove(idx);
                            self.save_config();
                        }
                    });
                
                if ui.button("➕ 添加订阅").clicked() {
                    self.show_add_subscription = true;
                    self.edit_subscription_index = None;
                    self.edit_subscription_name.clear();
                    self.edit_subscription_url.clear();
                    self.edit_subscription_path = "./singbox_config.json".to_string();
                }
            });
    }

    fn render_core_update(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("🚀 Sing-box 核心更新")
            .default_open(true)
            .show(ui, |ui| {
                let mut enabled = self.config.singbox_core_update.enabled;
                if ui.checkbox(&mut enabled, "启用核心自动更新").changed() {
                    self.config.singbox_core_update.enabled = enabled;
                    self.save_config();
                }
                
                let mut check_prerelease = self.config.singbox_core_update.check_prerelease;
                if ui.checkbox(&mut check_prerelease, "包含预发布版本").changed() {
                    self.config.singbox_core_update.check_prerelease = check_prerelease;
                    self.save_config();
                }
                
                ui.horizontal(|ui| {
                    ui.label("安装路径:");
                    let mut path_str = self.config.singbox_core_update.install_path.display().to_string();
                    if ui.text_edit_singleline(&mut path_str).changed() {
                        self.config.singbox_core_update.install_path = PathBuf::from(path_str);
                        self.save_config();
                    }
                });
            });
    }

    fn render_interval_config(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("⏰ 定时更新设置")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("更新间隔 (小时):");
                    let mut hours = self.config.update_interval_hours as i32;
                    if ui.add(egui::DragValue::new(&mut hours).speed(1.0)).changed() {
                        if hours >= 0 {
                            self.config.update_interval_hours = hours as u64;
                            self.save_config();
                        }
                    }
                });
                
                ui.label("(设置为 0 表示仅手动更新)");
                
                ui.checkbox(&mut self.auto_update_enabled, "启用自动定时更新");
            });
    }

    fn render_logs(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("📝 日志")
            .default_open(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.log_display.as_str())
                                .desired_width(f32::INFINITY)
                                .font(egui::TextStyle::Monospace)
                        );
                    });
            });
    }

    fn render_action_buttons(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("🔄 立即更新订阅").clicked() {
                self.update_subscriptions();
            }
            
            if ui.button("⬆ 立即更新核心").clicked() {
                self.update_core();
            }
            
            if ui.button("🔄 全部更新").clicked() {
                self.update_subscriptions();
                self.update_core();
            }
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🗑 清空日志").clicked() {
                    self.log_display.clear();
                }
            });
        });
    }

    fn render_subscription_dialog(&mut self, ctx: &egui::Context) {
        if self.show_add_subscription {
            egui::Window::new(if self.edit_subscription_index.is_some() {
                "编辑订阅"
            } else {
                "添加订阅"
            })
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("名称:");
                    ui.text_edit_singleline(&mut self.edit_subscription_name);
                });
                
                ui.horizontal(|ui| {
                    ui.label("URL:");
                    ui.text_edit_singleline(&mut self.edit_subscription_url);
                });
                
                ui.horizontal(|ui| {
                    ui.label("保存路径:");
                    ui.text_edit_singleline(&mut self.edit_subscription_path);
                });
                
                ui.horizontal(|ui| {
                    if ui.button("💾 保存").clicked() {
                        let sub = Subscription {
                            name: self.edit_subscription_name.clone(),
                            url: self.edit_subscription_url.clone(),
                            save_path: PathBuf::from(&self.edit_subscription_path),
                        };
                        
                        if let Some(idx) = self.edit_subscription_index {
                            self.config.subscriptions[idx] = sub;
                        } else {
                            self.config.subscriptions.push(sub);
                        }
                        
                        self.save_config();
                        self.show_add_subscription = false;
                    }
                    
                    if ui.button("❌ 取消").clicked() {
                        self.show_add_subscription = false;
                    }
                });
            });
        }
    }
}

impl eframe::App for SingboxManagerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Sing-box 配置管理器");
            ui.separator();
            
            self.render_status_bar(ui);
            
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.render_subscriptions(ui);
                ui.add_space(10.0);
                
                self.render_core_update(ui);
                ui.add_space(10.0);
                
                self.render_interval_config(ui);
                ui.add_space(10.0);
                
                self.render_logs(ui);
            });
            
            ui.separator();
            self.render_action_buttons(ui);
        });
        
        self.render_subscription_dialog(ctx);
        
        // 请求重绘以更新日志
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }
}

/// 运行 GUI
pub fn run_gui() -> Result<(), eframe::Error> {
    env_logger::init();
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    
    eframe::run_native(
        "Sing-box Manager",
        options,
        Box::new(|cc| Box::new(SingboxManagerApp::new(cc))),
    )
}

