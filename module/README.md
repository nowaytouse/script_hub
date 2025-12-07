# Surge & Shadowrocket Modules

## 📦 Available Modules

All modules are hosted on GitHub and can be referenced directly via URL.

### Main Modules (surge/main)

1. **🚫 Universal Ad-Blocking Rules** - Comprehensive ad blocking
2. **🚀 General Enhanced** - General enhancements and optimizations
3. **🔥 Firewall Port Blocker** - Block dangerous ports
4. **🔒 Encrypted DNS** - DNS encryption configuration
5. **🔄 URL Rewrite** - URL rewriting rules
6. **📺 YouTube Ad Removal** - Remove YouTube ads

## 🔗 How to Use (Recommended)

### For Surge

Add modules to your Surge configuration using GitHub raw URLs:

```ini
[Module]
# Ad Blocking
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge(main)/🚫%20Universal%20Ad-Blocking%20Rules%20Dependency%20Component%20LITE%20(Kali-style).sgmodule

# General Enhanced
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge(main)/🚀💪General%20Enhanced⬆️⬆️%20plus.sgmodule

# Firewall
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge(main)/🔥%20Firewall%20Port%20Blocker%20🛡️🚫.sgmodule

# Encrypted DNS
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge(main)/Encrypted%20DNS%20Module%20🔒🛡️DNS.sgmodule

# URL Rewrite
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge(main)/URL%20Rewrite%20Module%20🔄🌐.sgmodule
```

### For Shadowrocket

Shadowrocket also supports remote modules. Use the same GitHub URLs.

## ✅ Benefits of Using GitHub URLs

1. **Auto-Update** - Modules update automatically when you update the repository
2. **No Manual Sync** - No need to copy files to iCloud
3. **Version Control** - Track changes via Git history
4. **Easy Sharing** - Share modules with others via URL
5. **Centralized Management** - Manage all modules in one place

## 🚫 Deprecated: Manual iCloud Sync

**We no longer recommend copying modules to iCloud directories.**

The old sync scripts (`sync_modules_to_shadowrocket.sh`) are deprecated because:
- ❌ Requires manual sync
- ❌ Duplicates files
- ❌ No automatic updates
- ❌ Harder to maintain

## 📝 Module Development

When developing new modules:

1. Create/edit module in `module/surge(main)/`
2. Test locally
3. Commit to Git
4. Use GitHub URL in your configuration

## 🔄 Update Process

```bash
# Update repository
git pull

# Modules are automatically updated via GitHub URLs
# No manual sync needed!
```

## 📚 Additional Resources

- [Surge Module Documentation](https://manual.nssurge.com/book/understanding-surge/en/)
- [Shadowrocket Module Guide](https://shadowrocket.org/)

