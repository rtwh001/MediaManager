# MediaManager 0.1.0 发布说明

[English](RELEASE.en.md) | **简体中文**

这是 MediaManager 的首个 Windows x64 发布版本。

## 下载

在 GitHub Release 中下载：

```text
MediaManager_0.1.0_x64-setup.exe
MediaManager_0.1.0_windows_x64_portable.zip
```

### 安装版

- 平台：Windows x64
- 安装器：NSIS
- 文件大小：46,099,733 bytes
- SHA-256：`ED5867F14401CD2304C65E9DD10089E5C6721F5485A55BBF0A84BDB9CDF659D8`
- 代码签名：未签名

### 便携版

- 格式：ZIP
- 文件大小：71,486,864 bytes
- SHA-256：`9322AFF093F58CC0E5A5246F5B896ECD0E2AFBA8EE9232D979D36E992D48566E`
- 使用方式：完整解压后双击 `media-manager.exe`

## 本版本内容

- 全新 `#6597ec` 蓝色主题与泉此方应用图标。
- 本地电影、剧集和动画资料库。
- 面向字幕组及非标准动画命名的识别与剧集归组。
- TMDB、Bangumi、AniList 和本地 NFO 元数据。
- 本地及在线海报缓存。
- 已看/未看状态、标签、片单和筛选排序。
- 手动分类、手动合并和重复条目整理。
- 批量选择与批量刷新元数据。
- 删除黑名单和恢复功能。
- 扫描进度、取消、错误历史、日志和诊断。
- 自包含 `.mmbak` 备份、完整性检查及恢复前自动备份。
- 媒体根路径批量迁移。
- 内置 LGPL 静态 `ffprobe`，无需另外安装 FFmpeg。

## 安装后如何打开

安装完成后：

1. 按 Windows 键。
2. 输入 `MediaManager`。
3. 按回车打开。

开始菜单会包含 MediaManager 快捷方式。可以将其固定到任务栏或开始屏幕。正常使用不需要执行 `pnpm`、`cargo` 或其他开发命令。

## 数据保留

用户数据位于：

```text
%LOCALAPPDATA%\com.local.mediamanager\
```

覆盖安装、升级和正常卸载不会主动删除该目录。原始视频文件不会由应用移动或删除。

建议升级前在“管理 → 数据安全”中导出 `.mmbak` 备份。

## 已完成验证

- Rust 测试 17/17 通过。
- 严格 Clippy 检查通过。
- TypeScript 和 Vite 生产构建通过。
- Release 与 NSIS 构建通过。
- 静默安装和卸载通过。
- 安装后的应用成功启动。
- 内置 `ffprobe` 成功运行。
- FFmpeg LGPL 许可证与来源说明随安装落地。
- 卸载后资料库数据仍然保留。

## 已知提示

安装包尚未进行商业代码签名，Windows SmartScreen 可能显示“未知发布者”。请确认下载来源和 SHA-256 后再运行。
