# 🔧 Setup Guide for Sync Scripts

## User-Specific Configuration

Some scripts contain user-specific paths (like iCloud directories) and are excluded from git tracking. You need to create your own copies from the `.example` templates.

### 📋 Required Setup

#### 1. Module Sync Script

**File**: `sync_modules_to_icloud.sh`

**Setup Steps**:
```bash
# 1. Copy the example file
cp sync_modules_to_icloud.sh.example sync_modules_to_icloud.sh

# 2. Edit the file and update these paths:
# - SURGE_ICLOUD_DIR
# - SHADOWROCKET_ICLOUD_DIR

# 3. Find your iCloud paths:
# Open Finder → Go to ~/Library/Mobile Documents/
# Look for folders starting with "iCloud~"

# 4. Make it executable
chmod +x sync_modules_to_icloud.sh

# 5. Test it
./sync_modules_to_icloud.sh --list
```

**Example iCloud Paths**:
```bash
# Surge
SURGE_ICLOUD_DIR="$HOME/Library/Mobile Documents/iCloud~com~nssurge~inc/Documents"

# Shadowrocket
SHADOWROCKET_ICLOUD_DIR="$HOME/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents"
```

### 🔒 Privacy & Security

**Files excluded from git** (in `.gitignore`):
- `sync_modules_to_icloud.sh` - Contains user paths
- `test-full.sh` - Contains user-specific test paths
- `zip_output/` - Test media files
- `**/Menthako/` - Example media collections
- `**/compressed/` - Conversion outputs

**Why?**
- Prevents accidental exposure of personal directory structures
- Keeps repository clean from user-specific configurations
- Protects privacy (usernames, file paths)

### 📝 Best Practices

1. **Never commit user-specific files**
   - Always use `.example` templates
   - Add your custom files to `.gitignore`

2. **Use environment variables** (optional)
   ```bash
   # In your ~/.zshrc or ~/.bashrc
   export SURGE_ICLOUD_DIR="$HOME/Library/Mobile Documents/iCloud~com~nssurge~inc/Documents"
   export SHADOWROCKET_ICLOUD_DIR="$HOME/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents"
   ```

3. **Keep sensitive data separate**
   - Use `conf隐私🔏/` for private configurations
   - Files with keywords like `private`, `secret`, `password` are auto-excluded

### 🆘 Troubleshooting

**Q: Script says "iCloud not found"**
- Check if the app is installed
- Verify iCloud Drive is enabled in System Settings
- Confirm the app has iCloud sync enabled

**Q: How to find my iCloud path?**
```bash
# List all iCloud containers
ls -la ~/Library/Mobile\ Documents/ | grep iCloud
```

**Q: Can I use a different sync location?**
- Yes! Just update the paths in your custom script
- You can sync to Dropbox, Google Drive, or any folder

### 📚 Related Documentation

- [QUICKSTART.md](QUICKSTART.md) - Quick start guide
- [README_MERGE_ADBLOCK.md](README_MERGE_ADBLOCK.md) - Ad-block merge guide
- [README_MERGE_RULES.md](README_MERGE_RULES.md) - Rule merge guide
