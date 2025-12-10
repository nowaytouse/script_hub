# Sing-box Manager

Cross-platform Sing-box/Mihomo configuration and core auto-update tool.

## Features

- ✅ Dual core update (sing-box + mihomo)
- ✅ Multi-path installation support
- ✅ Auto backup before replacement
- ✅ Subscription management
- ✅ Interactive CLI menu
- ✅ Scheduled auto-update

## Quick Start

```bash
# Build
cargo build --release

# Interactive mode
./target/release/singbox-manager --interactive

# One-time update
./target/release/singbox-manager --once
```

## Configuration

Edit `config.json`:

```json
{
  "subscriptions": [
    {
      "name": "My Sub",
      "url": "https://your-subscription-url",
      "save_path": "/path/to/config.json"
    }
  ],
  "update_interval_hours": 24,
  "singbox_core_update": {
    "enabled": true,
    "install_path": "/usr/local/bin/sing-box"
  },
  "mihomo_core_update": {
    "enabled": true,
    "install_paths": ["/usr/local/bin/mihomo"]
  }
}
```

## Security Note

`config.json` contains sensitive data - never commit to Git.

```bash
cp config.example.json config.json
# Edit with your settings
```

---

MIT License
