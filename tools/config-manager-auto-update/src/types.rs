use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 配置文件主结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 订阅源列表
    pub subscriptions: Vec<Subscription>,
    /// 更新间隔（小时）
    pub update_interval_hours: u64,
    /// Sing-box 核心更新配置
    pub singbox_core_update: SingboxCoreUpdate,
    /// Mihomo 核心更新配置（可选）
    #[serde(default)]
    pub mihomo_core_update: Option<MihomoCoreUpdate>,
}

/// 单个订阅源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    /// 订阅名称
    pub name: String,
    /// 订阅 URL
    pub url: String,
    /// 保存路径
    pub save_path: PathBuf,
}

/// Sing-box 核心更新配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingboxCoreUpdate {
    /// 是否启用自动更新
    pub enabled: bool,
    /// 是否检查预发布版本
    pub check_prerelease: bool,
    /// 安装路径
    pub install_path: PathBuf,
}

/// Mihomo 核心更新配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MihomoCoreUpdate {
    /// 是否启用自动更新
    pub enabled: bool,
    /// 是否检查预发布版本
    pub check_prerelease: bool,
    /// 安装路径列表（支持多个路径同时更新）
    pub install_paths: Vec<PathBuf>,
}

// GitHub API types removed - using direct download instead

impl Config {
    /// 创建默认配置
    pub fn default() -> Self {
        let singbox_install_path = if cfg!(windows) {
            PathBuf::from("C:\\Program Files\\sing-box\\sing-box.exe")
        } else {
            PathBuf::from("/usr/local/bin/sing-box")
        };

        let mihomo_install_paths = if cfg!(windows) {
            vec![PathBuf::from("C:\\Program Files\\mihomo\\mihomo.exe")]
        } else {
            vec![
                PathBuf::from("/usr/local/bin/mihomo"),
                // 可以添加更多路径
            ]
        };

        Self {
            subscriptions: vec![Subscription {
                name: "默认订阅".to_string(),
                url: "https://example.com/your-singbox-config.json".to_string(),
                save_path: PathBuf::from("./singbox_config.json"),
            }],
            update_interval_hours: 24,
            singbox_core_update: SingboxCoreUpdate {
                enabled: true,
                check_prerelease: false,
                install_path: singbox_install_path,
            },
            mihomo_core_update: Some(MihomoCoreUpdate {
                enabled: false, // 默认禁用，用户需要手动启用
                check_prerelease: false,
                install_paths: mihomo_install_paths,
            }),
        }
    }
}
