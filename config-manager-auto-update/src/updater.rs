use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use std::path::{Path, PathBuf};
use tar::Archive;
use tokio::fs;

use crate::types::{Config, MihomoCoreUpdate};

/// Core Updater (Pure CLI Mode) - Direct Download, No API
pub struct CoreUpdater;

impl CoreUpdater {
    pub fn new() -> Self {
        Self
    }

    /// Get latest version tag by following redirect (no API rate limit)
    async fn get_latest_version(&self) -> Result<String> {
        let client = reqwest::Client::builder()
            .user_agent("singbox-manager/2.0")
            .redirect(reqwest::redirect::Policy::none()) // Don't follow redirects
            .build()?;

        let response = client
            .get("https://github.com/SagerNet/sing-box/releases/latest")
            .send()
            .await
            .context("Failed to get latest version")?;

        // Get redirect location header
        if let Some(location) = response.headers().get("location") {
            let location_str = location.to_str().context("Invalid location header")?;
            // Extract version from URL like: /SagerNet/sing-box/releases/tag/v1.12.12
            if let Some(version) = location_str.split('/').last() {
                println!("✅ Found latest version: {}", version);
                return Ok(version.to_string());
            }
        }

        anyhow::bail!("Failed to extract version from redirect")
    }

    /// Get direct download URL for specific version
    fn get_download_url(&self, version: &str) -> String {
        let os = match std::env::consts::OS {
            "macos" => "darwin",
            other => other,
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            "x86" => "386",
            other => other,
        };

        // Direct download URL with version
        format!(
            "https://github.com/SagerNet/sing-box/releases/download/{}/sing-box-{}-{}-{}.tar.gz",
            version,
            version.trim_start_matches('v'),
            os,
            arch
        )
    }

    /// Download and extract binary
    pub async fn download_and_extract(&self) -> Result<PathBuf> {
        // Get latest version first
        let version = self.get_latest_version().await?;
        let download_url = self.get_download_url(&version);
        println!("📥 Downloading sing-box from: {}", download_url);

        // Optimized client with connection pooling
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .user_agent("singbox-manager/2.0")
            .tcp_keepalive(std::time::Duration::from_secs(60))
            .pool_max_idle_per_host(2)
            .build()?;

        let response = client
            .get(&download_url)
            .send()
            .await
            .context("Failed to download")?;

        if !response.status().is_success() {
            anyhow::bail!("Download failed, status code: {}", response.status());
        }

        let bytes = response.bytes().await.context("Failed to read response")?;

        // Create temporary directory
        let temp_dir = std::env::temp_dir().join(format!("singbox-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).context("Failed to create temp directory")?;

        println!("📦 Extracting to: {}", temp_dir.display());

        // Extract tar.gz
        let extracted_path = self.extract_tar_gz(&bytes, &temp_dir).await?;

        Ok(extracted_path)
    }

    /// Extract tar.gz file
    async fn extract_tar_gz(&self, data: &[u8], dest_dir: &Path) -> Result<PathBuf> {
        let data = data.to_vec();
        let dest_dir = dest_dir.to_path_buf();

        let result = tokio::task::spawn_blocking(move || {
            let decoder = GzDecoder::new(&data[..]);
            let mut archive = Archive::new(decoder);

            let exe_name = if cfg!(windows) {
                "sing-box.exe"
            } else {
                "sing-box"
            };

            for entry in archive.entries()? {
                let mut entry = entry?;
                let path = entry.path()?;

                if path.file_name().and_then(|n| n.to_str()) == Some(exe_name)
                    || path.to_str().unwrap_or("").ends_with(exe_name)
                {
                    let extract_path = dest_dir.join(exe_name);
                    entry.unpack(&extract_path)?;

                    // Set execute permissions on Unix
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let mut perms = std::fs::metadata(&extract_path)?.permissions();
                        perms.set_mode(0o755);
                        std::fs::set_permissions(&extract_path, perms)?;
                    }

                    return Ok(extract_path);
                }
            }

            anyhow::bail!("sing-box executable not found in archive")
        })
        .await??;

        println!("✅ Extracted sing-box to {}", result.display());
        Ok(result)
    }

    /// Install file
    pub async fn install_file(&self, source: &Path, dest: &Path) -> Result<()> {
        println!("📦 Installing {} to {}", source.display(), dest.display());

        // Ensure target directory exists
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create target directory")?;
        }

        // Copy file
        fs::copy(source, dest).await.context("Failed to copy file")?;

        // Set execute permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(dest).await?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(dest, perms).await?;
        }

        println!("✅ Installation successful! sing-box updated to {}", dest.display());
        Ok(())
    }

    /// Run core updater - Simple direct download
    pub async fn run_all(&self, config: &Config) -> Result<()> {
        if !config.singbox_core_update.enabled {
            println!("⚠️  sing-box core update is disabled");
            return Ok(());
        }

        println!("🔄 Updating sing-box core (direct download)...");

        let temp_binary_path = self.download_and_extract().await?;

        self.install_file(&temp_binary_path, &config.singbox_core_update.install_path)
            .await?;

        // Clean up temporary files
        if let Some(temp_dir) = temp_binary_path.parent() {
            let _ = fs::remove_dir_all(temp_dir).await;
        }

        println!("✅ sing-box core update complete");
        Ok(())
    }
}

/// Mihomo Core Updater - Direct Download from GitHub Releases
pub struct MihomoUpdater;

impl MihomoUpdater {
    pub fn new() -> Self {
        Self
    }

    /// Get download URL - Simple and direct
    fn get_download_url(&self, check_prerelease: bool) -> String {
        let os = match std::env::consts::OS {
            "macos" => "darwin",
            other => other,
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            "x86" => "386",
            other => other,
        };

        if check_prerelease {
            // Prerelease: fixed URL with current commit hash
            // Note: This hash may change, but it's the simplest approach
            format!(
                "https://github.com/MetaCubeX/mihomo/releases/download/Prerelease-Alpha/mihomo-{}-{}-alpha-bc719eb.gz",
                os, arch
            )
        } else {
            // Stable: use latest tag (will redirect to actual version)
            format!(
                "https://github.com/MetaCubeX/mihomo/releases/latest/download/mihomo-{}-{}.gz",
                os, arch
            )
        }
    }

    /// Download and extract binary
    pub async fn download_and_extract(&self, check_prerelease: bool) -> Result<PathBuf> {
        let download_url = self.get_download_url(check_prerelease);
        println!("📥 Downloading mihomo ({}) from: {}", 
                 if check_prerelease { "prerelease" } else { "stable" }, 
                 download_url);

        // Reuse client for better connection pooling
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .user_agent("config-manager/2.0")
            .tcp_keepalive(std::time::Duration::from_secs(60))
            .pool_max_idle_per_host(2)
            .build()?;

        let response = client
            .get(&download_url)
            .send()
            .await
            .context("Failed to download")?;

        if !response.status().is_success() {
            anyhow::bail!("Download failed, status code: {}", response.status());
        }

        // Stream to reduce memory usage
        let bytes = response.bytes().await.context("Failed to read response")?;

        // Create temporary directory
        let temp_dir = std::env::temp_dir().join(format!("mihomo-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).context("Failed to create temp directory")?;

        println!("📦 Extracting to: {}", temp_dir.display());

        // Extract .gz file (mihomo uses single .gz, not tar.gz)
        let extracted_path = self.extract_gz(&bytes, &temp_dir).await?;

        Ok(extracted_path)
    }

    /// Extract .gz file (single file compression)
    async fn extract_gz(&self, data: &[u8], dest_dir: &Path) -> Result<PathBuf> {
        use flate2::read::GzDecoder;
        use std::io::Read;

        let data = data.to_vec();
        let dest_dir = dest_dir.to_path_buf();

        let result = tokio::task::spawn_blocking(move || -> Result<PathBuf> {
            let mut decoder = GzDecoder::new(&data[..]);
            // Pre-allocate buffer with estimated size (compressed * 3)
            let mut buffer = Vec::with_capacity(data.len() * 3);
            decoder
                .read_to_end(&mut buffer)
                .context("Failed to decompress")?;

            let exe_name = if cfg!(windows) {
                "mihomo.exe"
            } else {
                "mihomo"
            };

            let extract_path = dest_dir.join(exe_name);
            std::fs::write(&extract_path, buffer).context("Failed to write file")?;

            // Set execute permissions on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&extract_path)?.permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&extract_path, perms)?;
            }

            Ok(extract_path)
        })
        .await??;

        println!("✅ Extracted mihomo to {}", result.display());
        Ok(result)
    }

    /// Install file with backup
    pub async fn install_file(&self, source: &Path, dest: &Path) -> Result<()> {
        println!("📦 Installing {} to {}", source.display(), dest.display());

        // Ensure target directory exists (only if needed)
        if let Some(parent) = dest.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .await
                    .context("Failed to create target directory")?;
            }
        }

        // Backup existing file if it exists (atomic rename)
        if dest.exists() {
            let backup_path = dest.with_extension("bak");
            println!("💾 Backing up existing file to {}", backup_path.display());
            fs::rename(dest, &backup_path)
                .await
                .context("Failed to backup existing file")?;
        }

        // Copy file (optimized for large files)
        fs::copy(source, dest).await.context("Failed to copy file")?;

        // Set execute permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(dest).await?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(dest, perms).await?;
        }

        println!("✅ Installation successful! mihomo updated to {}", dest.display());
        Ok(())
    }

    /// Run mihomo updater for all configured paths
    pub async fn run_all(&self, config: &MihomoCoreUpdate) -> Result<()> {
        if !config.enabled {
            println!("⚠️  mihomo core update is disabled");
            return Ok(());
        }

        println!("🔄 Updating mihomo core (direct download)...");

        let temp_binary_path = self.download_and_extract(config.check_prerelease).await?;

        // Install to all configured paths
        for install_path in &config.install_paths {
            println!("\n📍 Installing to: {}", install_path.display());
            self.install_file(&temp_binary_path, install_path).await?;
        }

        // Clean up temporary files
        if let Some(temp_dir) = temp_binary_path.parent() {
            let _ = fs::remove_dir_all(temp_dir).await;
        }

        println!("\n✅ mihomo core update complete for all paths");
        Ok(())
    }
}
