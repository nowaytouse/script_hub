# Sing-box Manager

🚀 **Cross-platform Sing-box Configuration and Core Auto-Update Tool** - Pure Rust CLI

[中文文档](README_CN.md)

## ✨ Features

- ✅ **Dual Core Update** - Download and install latest sing-box and mihomo automatically
- ✅ **Multi-path Support** - Update mihomo to multiple locations simultaneously
- ✅ **Smart Backup** - Auto backup old versions before replacement
- ✅ **Permission Management** - Auto set executable permissions
- ✅ **Subscription Management** - Auto download and update config files
- ✅ **Interactive Menu** - User-friendly CLI interface
- ✅ **Scheduled Tasks** - Auto update at specified intervals
- ✅ **Cross-platform** - macOS, Linux, Windows
- ✅ **Zero Dependencies** - Pure Rust, no Python or other runtime needed

## 🚀 Quick Start

### 1. Build

```bash
cd config-manager
cargo build --release
```

Binary location: `target/release/singbox-manager`

### 2. Configure

Edit `config.json`:

```json
{
  "subscriptions": [
    {
      "name": "My Subscription",
      "url": "https://your-subscription-url",
      "save_path": "/path/to/save/config.json"
    }
  ],
  "update_interval_hours": 24,
  "singbox_core_update": {
    "enabled": true,
    "check_prerelease": false,
    "install_path": "/usr/local/bin/sing-box"
  },
  "mihomo_core_update": {
    "enabled": true,
    "check_prerelease": false,
    "install_paths": [
      "/usr/local/bin/mihomo",
      "/path/to/v2rayN/bin/mihomo"
    ]
  }
}
```

### 3. Run

#### Interactive Mode (Recommended)

```bash
./target/release/singbox-manager --interactive
```

#### CLI Mode

```bash
# One-time update
./target/release/singbox-manager --once

# Scheduled update (based on config interval)
./target/release/singbox-manager

# Custom config file
./target/release/singbox-manager --config my_config.json
```

#### Quick Scripts

```bash
# One-click setup
chmod +x enable.sh
./enable.sh

# One-click update
chmod +x update.sh
./update.sh
```

## 📋 Features

### Subscription Management

- Auto download config files
- Support multiple subscriptions
- Auto create save directories
- Error handling and retry

### Core Update

- **Sing-box Core**
  - Auto detect latest version (no API rate limit)
  - Direct download from GitHub releases
  - Auto extract tar.gz and install
  - Cross-platform support (macOS/Linux/Windows)
  - Auto set execute permissions

- **Mihomo Core** (NEW!)
  - Auto detect latest version from MetaCubeX/mihomo
  - Support stable and prerelease versions
  - Auto extract .gz and install
  - Multi-path installation support
  - Auto backup old versions (.bak)
  - Auto set executable permissions

### Interactive Menu

- View current config
- Manage subscriptions (add/edit/delete)
- Manual trigger updates
- Config settings

## 🔧 Configuration

### Config File Structure

```json
{
  "subscriptions": [
    {
      "name": "Subscription Name",
      "url": "Subscription URL",
      "save_path": "Save Path"
    }
  ],
  "update_interval_hours": 24,
  "singbox_core_update": {
    "enabled": true,
    "check_prerelease": false,
    "install_path": "/usr/local/bin/sing-box"
  }
}
```

### Config Options

- `subscriptions`: Subscription list
  - `name`: Subscription name (for display)
  - `url`: Subscription URL
  - `save_path`: Config file save path
  
- `update_interval_hours`: Auto update interval (hours), set to 0 to disable

- `singbox_core_update`: Sing-box core update config
  - `enabled`: Enable core update
  - `check_prerelease`: Check prerelease versions
  - `install_path`: Core installation path

- `mihomo_core_update`: Mihomo core update config (optional)
  - `enabled`: Enable mihomo core update
  - `check_prerelease`: Check prerelease versions
  - `install_paths`: Array of installation paths (supports multiple locations)

## 📦 CLI Arguments

```
singbox-manager [OPTIONS]

OPTIONS:
    -i, --interactive       Interactive menu mode
    -c, --config <FILE>     Config file path [default: config.json]
    -o, --once              Execute once and exit
    -v, --version           Show version info
    -h, --help              Show help info
```

## 🎯 Use Cases

### Daily Use

```bash
# Start interactive menu for manual management
./target/release/singbox-manager --interactive
```

### Scheduled Task

```bash
# Run in background, auto update every 24 hours
nohup ./target/release/singbox-manager &
```

### One-time Update

```bash
# Quick update all and exit
./target/release/singbox-manager --once
```

### Custom Config

```bash
# Use custom config file
./target/release/singbox-manager --config /path/to/my_config.json --once
```

## ⚠️ Notes

1. **First Use** - Edit `config.json` first
2. **Permissions** - Ensure write permissions to config and install directories
3. **Network** - Stable network connection required for GitHub access
4. **Backup** - Backup important config files before update

## 🐛 Troubleshooting

### Build Failed

```bash
# Clean and rebuild
cargo clean
cargo build --release
```

### Download Failed

- Check network connection
- Check GitHub accessibility
- Try again later

### Permission Error

```bash
# Check directory permissions
ls -la /path/to/install/dir

# Fix permissions
chmod 755 /path/to/install/dir
```

### Config File Error

```bash
# Validate config
./verify-config.sh
```

## 📊 Performance

- ✅ **Zero compilation warnings**
- ✅ **Build time**: ~20s (Release)
- ✅ **Binary size**: 2.8MB (optimized)
- ✅ **Startup time**: <100ms
- ✅ **Memory usage**: ~10MB

## 🎉 Test Results

```bash
✅ Subscription download: SUCCESS (104KB)
✅ Core update: SUCCESS (sing-box 1.12.12)
✅ Zero compilation warnings
✅ Zero runtime errors
```

## 📚 Documentation

- `README.md` - Main documentation (English)
- `README_CN.md` - Chinese documentation

## 🤝 Contributing

Issues and Pull Requests are welcome!

## 📄 License

MIT License

---

**Version**: 2.0.0  
**Platform**: macOS / Linux / Windows  
**Language**: Rust  
**Last Updated**: 2024-12-02

## 🔒 Security Note

**Important**: `config.json` contains sensitive information (subscription URLs, personal paths) and should **NEVER** be committed to Git.

### First Time Setup

1. Copy the example config:
   ```bash
   cp config.example.json config.json
   ```

2. Edit `config.json` with your settings:
   - Update subscription URLs
   - Update installation paths
   - Configure update intervals

3. The `config.json` file is already in `.gitignore` and will not be tracked by Git.

### Configuration Security

- ✅ `config.example.json` - Safe template (committed to Git)
- ❌ `config.json` - Your personal config (NOT committed to Git)
- ✅ All sensitive data removed from Git history

