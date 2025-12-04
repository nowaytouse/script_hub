use anyhow::Result;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::config::{load_config_sync, save_config_sync};
use crate::subscription::SubscriptionManager;
use crate::types::{Config, MihomoCoreUpdate, Subscription, SingboxCoreUpdate};
use crate::updater::{CoreUpdater, MihomoUpdater};

fn read_input(prompt: &str) -> io::Result<String> {
    print!("{}: ", prompt);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn read_confirm(prompt: &str, default: bool) -> io::Result<bool> {
    let hint = if default { "Y/n" } else { "y/N" };
    print!("{} [{}]: ", prompt, hint);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();
    Ok(match input.as_str() {
        "y" | "yes" => true,
        "n" | "no" => false,
        "" => default,
        _ => default,
    })
}

fn show_menu(items: &[&str]) -> io::Result<usize> {
    for (i, item) in items.iter().enumerate() {
        println!("  [{}] {}", i + 1, item);
    }
    loop {
        let input = read_input("请选择")?;
        if let Ok(choice) = input.parse::<usize>() {
            if choice > 0 && choice <= items.len() {
                return Ok(choice - 1);
            }
        }
        println!("无效选择，请重试");
    }
}

pub struct InteractiveCLI {
    pub config_path: PathBuf,
}

impl InteractiveCLI {
    pub fn new(config_path: PathBuf) -> Self {
        Self { config_path }
    }

    pub async fn run(&self) -> Result<()> {
        println!("\n╔════════════════════════════════════════╗");
        println!("║   Sing-box Manager - 交互式管理工具    ║");
        println!("║   版本: 2.0.0 (Pure Rust CLI)          ║");
        println!("╚════════════════════════════════════════╝\n");

        loop {
            println!("\n主菜单:");
            let items = vec![
                "查看配置",
                "管理订阅",
                "更新订阅",
                "更新 Sing-box 核心",
                "更新 Mihomo 核心",
                "执行所有更新",
                "配置设置",
                "退出",
            ];
            let choice = show_menu(&items)?;

            match choice {
                0 => self.show_config().await?,
                1 => self.manage_subscriptions().await?,
                2 => self.update_subscriptions().await?,
                3 => self.update_singbox_core().await?,
                4 => self.update_mihomo_core().await?,
                5 => self.run_all_updates().await?,
                6 => self.settings_menu().await?,
                7 => {
                    println!("再见！");
                    break;
                }
                _ => {}
            }
        }
        Ok(())
    }

    async fn show_config(&self) -> Result<()> {
        println!("\n═ 当前配置 ═");
        let config = load_config_sync(&self.config_path)?;

        println!("\n订阅源:");
        for (i, sub) in config.subscriptions.iter().enumerate() {
            println!("  [{}] {}", i + 1, sub.name);
            println!("      URL: {}", sub.url);
            println!("      保存路径: {}", sub.save_path.display());
        }

        println!("\n更新设置:");
        println!("  更新间隔: {} 小时", config.update_interval_hours);

        println!("\nSing-box 核心:");
        println!("  启用自动更新: {}", if config.singbox_core_update.enabled { "是" } else { "否" });
        println!("  检查预发布版本: {}", if config.singbox_core_update.check_prerelease { "是" } else { "否" });
        println!("  安装路径: {}", config.singbox_core_update.install_path.display());

        if let Some(mihomo_config) = &config.mihomo_core_update {
            println!("\nMihomo 核心:");
            println!("  启用自动更新: {}", if mihomo_config.enabled { "是" } else { "否" });
            println!("  检查预发布版本: {}", if mihomo_config.check_prerelease { "是" } else { "否" });
            println!("  安装路径:");
            for path in &mihomo_config.install_paths {
                println!("    - {}", path.display());
            }
        }

        println!();
        Ok(())
    }

    async fn manage_subscriptions(&self) -> Result<()> {
        loop {
            println!("\n═ 订阅管理 ═");
            let mut config = load_config_sync(&self.config_path)?;

            let options: Vec<String> = config
                .subscriptions
                .iter()
                .map(|s| format!("{} ({})", s.name, s.url))
                .collect();

            let mut menu_items: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
            menu_items.push("添加新订阅");
            menu_items.push("返回主菜单");

            let choice = show_menu(&menu_items)?;

            if choice == menu_items.len() - 1 {
                break;
            } else if choice == menu_items.len() - 2 {
                self.add_subscription(&mut config).await?;
            } else {
                self.edit_subscription(&mut config, choice).await?;
            }
        }
        Ok(())
    }

    async fn add_subscription(&self, config: &mut Config) -> Result<()> {
        println!("\n添加新订阅");
        let name = read_input("订阅名称")?;
        let url = read_input("订阅 URL")?;
        let save_path = read_input("保存路径 (默认: ./singbox_config.json)")?;
        let save_path = if save_path.is_empty() { "./singbox_config.json".to_string() } else { save_path };

        config.subscriptions.push(Subscription {
            name,
            url,
            save_path: PathBuf::from(save_path),
        });

        save_config_sync(&self.config_path, config)?;
        println!("✓ 订阅已添加");
        Ok(())
    }

    async fn edit_subscription(&self, config: &mut Config, index: usize) -> Result<()> {
        println!("\n编辑订阅");
        let sub = &config.subscriptions[index];
        println!("当前订阅: {}", sub.name);

        let options = vec!["编辑", "删除", "返回"];
        let choice = show_menu(&options)?;

        match choice {
            0 => {
                let name = read_input(&format!("新名称 (当前: {})", sub.name))?;
                let url = read_input(&format!("新 URL (当前: {})", sub.url))?;
                let save_path = read_input(&format!("新保存路径 (当前: {})", sub.save_path.display()))?;

                config.subscriptions[index] = Subscription {
                    name: if name.is_empty() { sub.name.clone() } else { name },
                    url: if url.is_empty() { sub.url.clone() } else { url },
                    save_path: PathBuf::from(if save_path.is_empty() { sub.save_path.display().to_string() } else { save_path }),
                };

                save_config_sync(&self.config_path, config)?;
                println!("✓ 订阅已更新");
            }
            1 => {
                if read_confirm("确认删除此订阅?", false)? {
                    config.subscriptions.remove(index);
                    save_config_sync(&self.config_path, config)?;
                    println!("✓ 订阅已删除");
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn update_subscriptions(&self) -> Result<()> {
        println!("\n═ 更新订阅 ═");
        let config = load_config_sync(&self.config_path)?;
        let manager = SubscriptionManager::new();
        println!("开始下载所有订阅...");
        manager.run_all(&config).await?;
        println!("✓ 所有订阅更新完成\n");
        Ok(())
    }

    async fn update_singbox_core(&self) -> Result<()> {
        println!("\n═ 更新 Sing-box 核心 ═");
        let config = load_config_sync(&self.config_path)?;

        if !config.singbox_core_update.enabled {
            println!("⚠ Sing-box 核心更新已禁用");
            return Ok(());
        }

        let updater = CoreUpdater::new();
        println!("开始检查并更新 Sing-box 核心...");
        updater.run_all(&config).await?;
        println!("✓ Sing-box 核心更新完成\n");
        Ok(())
    }

    async fn update_mihomo_core(&self) -> Result<()> {
        println!("\n═ 更新 Mihomo 核心 ═");
        let config = load_config_sync(&self.config_path)?;

        let mihomo_config = match &config.mihomo_core_update {
            Some(cfg) => cfg,
            None => {
                println!("⚠ Mihomo 核心未配置");
                return Ok(());
            }
        };

        if !mihomo_config.enabled {
            println!("⚠ Mihomo 核心更新已禁用");
            return Ok(());
        }

        let updater = MihomoUpdater::new();
        println!("开始检查并更新 Mihomo 核心...");
        updater.run_all(mihomo_config).await?;
        println!("✓ Mihomo 核心更新完成\n");
        Ok(())
    }

    async fn run_all_updates(&self) -> Result<()> {
        println!("\n═ 执行所有更新 ═");
        let config = load_config_sync(&self.config_path)?;

        println!("1. 更新订阅...");
        let sub_manager = SubscriptionManager::new();
        sub_manager.run_all(&config).await?;

        if config.singbox_core_update.enabled {
            println!("2. 更新 Sing-box 核心...");
            let updater = CoreUpdater::new();
            updater.run_all(&config).await?;
        } else {
            println!("2. Sing-box 核心更新已禁用，跳过");
        }

        if let Some(mihomo_config) = &config.mihomo_core_update {
            if mihomo_config.enabled {
                println!("3. 更新 Mihomo 核心...");
                let mihomo_updater = MihomoUpdater::new();
                mihomo_updater.run_all(mihomo_config).await?;
            } else {
                println!("3. Mihomo 核心更新已禁用，跳过");
            }
        } else {
            println!("3. Mihomo 核心未配置，跳过");
        }

        println!("✓ 所有更新完成\n");
        Ok(())
    }

    async fn settings_menu(&self) -> Result<()> {
        loop {
            println!("\n═ 配置设置 ═");
            let options = vec![
                "更改更新间隔",
                "配置 Sing-box 核心更新",
                "配置 Mihomo 核心更新",
                "返回主菜单",
            ];
            let choice = show_menu(&options)?;

            match choice {
                0 => self.change_update_interval().await?,
                1 => self.configure_singbox_core_update().await?,
                2 => self.configure_mihomo_core_update().await?,
                3 => break,
                _ => {}
            }
        }
        Ok(())
    }

    async fn change_update_interval(&self) -> Result<()> {
        println!("\n更改更新间隔");
        let mut config = load_config_sync(&self.config_path)?;
        let input = read_input(&format!("新的更新间隔（小时） (当前: {})", config.update_interval_hours))?;
        if let Ok(interval) = input.parse::<u64>() {
            config.update_interval_hours = interval;
            save_config_sync(&self.config_path, &config)?;
            println!("✓ 更新间隔已设置为 {} 小时", interval);
        }
        Ok(())
    }

    async fn configure_singbox_core_update(&self) -> Result<()> {
        println!("\n配置 Sing-box 核心更新");
        let mut config = load_config_sync(&self.config_path)?;

        let enabled = read_confirm("启用自动更新?", config.singbox_core_update.enabled)?;
        let check_prerelease = if enabled {
            read_confirm("检查预发布版本?", config.singbox_core_update.check_prerelease)?
        } else {
            config.singbox_core_update.check_prerelease
        };

        let install_path = read_input(&format!("安装路径 (当前: {})", config.singbox_core_update.install_path.display()))?;

        config.singbox_core_update = SingboxCoreUpdate {
            enabled,
            check_prerelease,
            install_path: PathBuf::from(if install_path.is_empty() { 
                config.singbox_core_update.install_path.display().to_string() 
            } else { 
                install_path 
            }),
        };

        save_config_sync(&self.config_path, &config)?;
        println!("✓ 配置已保存");
        Ok(())
    }

    async fn configure_mihomo_core_update(&self) -> Result<()> {
        println!("\n配置 Mihomo 核心更新");
        let mut config = load_config_sync(&self.config_path)?;

        let current_config = config.mihomo_core_update.clone().unwrap_or_else(|| {
            MihomoCoreUpdate {
                enabled: false,
                check_prerelease: false,
                install_paths: vec![PathBuf::from("/usr/local/bin/mihomo")],
            }
        });

        let enabled = read_confirm("启用自动更新?", current_config.enabled)?;
        let check_prerelease = if enabled {
            read_confirm("检查预发布版本?", current_config.check_prerelease)?
        } else {
            current_config.check_prerelease
        };

        println!("\n当前安装路径:");
        for (i, path) in current_config.install_paths.iter().enumerate() {
            println!("  [{}] {}", i + 1, path.display());
        }

        let mut install_paths = current_config.install_paths.clone();
        
        loop {
            println!("\n路径管理:");
            let options = vec!["添加路径", "删除路径", "完成配置"];
            let choice = show_menu(&options)?;

            match choice {
                0 => {
                    let path = read_input("新的安装路径")?;
                    if !path.is_empty() {
                        install_paths.push(PathBuf::from(path));
                        println!("✓ 路径已添加");
                    }
                }
                1 => {
                    if install_paths.is_empty() {
                        println!("⚠ 没有可删除的路径");
                        continue;
                    }
                    let path_strs: Vec<String> = install_paths.iter()
                        .map(|p| p.display().to_string())
                        .collect();
                    let path_refs: Vec<&str> = path_strs.iter().map(|s| s.as_str()).collect();
                    println!("\n选择要删除的路径:");
                    let idx = show_menu(&path_refs)?;
                    install_paths.remove(idx);
                    println!("✓ 路径已删除");
                }
                2 => break,
                _ => {}
            }
        }

        if install_paths.is_empty() {
            println!("⚠ 至少需要一个安装路径");
            install_paths.push(PathBuf::from("/usr/local/bin/mihomo"));
        }

        config.mihomo_core_update = Some(MihomoCoreUpdate {
            enabled,
            check_prerelease,
            install_paths,
        });

        save_config_sync(&self.config_path, &config)?;
        println!("✓ Mihomo 配置已保存");
        Ok(())
    }
}
