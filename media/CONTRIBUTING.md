# 开发准则 | Development Guidelines

[English](#english) | [中文](#中文)

---

## 中文

所有处理用户文件的脚本必须遵守以下准则。

### 准则一：完整元数据保留

- **内部元数据**: 使用 `exiftool -tagsfromfile` 迁移 EXIF/IPTC/XMP/ICC
- **系统元数据**: 使用 `touch -r` 保留时间戳
- **动画帧保留**: 100% 帧数保留，使用 `-r $fps -vsync cfr`

### 准则二：安全功能与响亮报错

- 内置危险目录检查 (`/`, `/System`, `~` 等)
- 触发破坏性操作时立即退出并报错

### 准则三：批量处理能力

- 输入参数为目录路径
- 使用 `find -print0` + `while read -r -d '\0'` 安全遍历

### 准则四：明确的原地替换

- 破坏性操作必须通过 `--in-place` 或 `--delete-source` 显式启用
- 默认行为必须安全（创建新文件）

### 准则五：验证后删除

1. 转换到临时文件
2. 验证退出码
3. 复制元数据和时间戳
4. **全部成功后**才删除原文件

---

## English

All scripts handling user files must follow these guidelines.

### Guideline 1: Complete Metadata Preservation

- **Internal**: Use `exiftool -tagsfromfile` for EXIF/IPTC/XMP/ICC
- **System**: Use `touch -r` for timestamps
- **Animation**: 100% frame preservation with `-r $fps -vsync cfr`

### Guideline 2: Safety & Loud Errors

- Built-in dangerous directory check (`/`, `/System`, `~`, etc.)
- Abort immediately with clear error on dangerous operations

### Guideline 3: Batch Processing

- Input parameter is directory path
- Use `find -print0` + `while read -r -d '\0'` for safe traversal

### Guideline 4: Explicit In-Place Replacement

- Destructive operations require `--in-place` or `--delete-source` flag
- Default behavior must be safe (create new files)

### Guideline 5: Verified Safe Deletes

1. Convert to temp file
2. Verify exit code
3. Copy metadata and timestamps
4. Delete original **only after all success**

---

MIT License
