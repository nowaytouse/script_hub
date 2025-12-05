use anyhow::{Context, Result};
use std::path::Path;
use tokio::fs;

use crate::types::Config;

/// 加载配置文件
pub async fn load_config<P: AsRef<Path>>(path: P) -> Result<Config> {
    let path = path.as_ref();
    
    if !path.exists() {
        log::info!("配置文件不存在，创建默认配置");
        let config = Config::default();
        save_config(path, &config).await?;
        return Ok(config);
    }

    let content = fs::read_to_string(path)
        .await
        .context("读取配置文件失败")?;

    let config: Config = serde_json::from_str(&content)
        .context("解析配置文件失败")?;

    log::info!("配置文件加载成功");
    Ok(config)
}

/// 保存配置文件
pub async fn save_config<P: AsRef<Path>>(path: P, config: &Config) -> Result<()> {
    let content = serde_json::to_string_pretty(config)
        .context("序列化配置失败")?;

    // 确保目录存在
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)
            .await
            .context("创建配置目录失败")?;
    }

    fs::write(path.as_ref(), content)
        .await
        .context("写入配置文件失败")?;

    log::info!("配置文件保存成功");
    Ok(())
}

/// 同步加载配置（用于非异步上下文）
pub fn load_config_sync<P: AsRef<Path>>(path: P) -> Result<Config> {
    let path = path.as_ref();
    
    if !path.exists() {
        log::info!("配置文件不存在，创建默认配置");
        let config = Config::default();
        save_config_sync(path, &config)?;
        return Ok(config);
    }

    let content = std::fs::read_to_string(path)
        .context("读取配置文件失败")?;

    let config: Config = serde_json::from_str(&content)
        .context("解析配置文件失败")?;

    log::info!("配置文件加载成功");
    Ok(config)
}

/// 同步保存配置（用于非异步上下文）
pub fn save_config_sync<P: AsRef<Path>>(path: P, config: &Config) -> Result<()> {
    let content = serde_json::to_string_pretty(config)
        .context("序列化配置失败")?;

    // 确保目录存在
    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent)
            .context("创建配置目录失败")?;
    }

    std::fs::write(path.as_ref(), content)
        .context("写入配置文件失败")?;

    log::info!("配置文件保存成功");
    Ok(())
}

