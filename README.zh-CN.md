# MediaManager

[English](README.en.md) | **简体中文**

MediaManager 是一款面向 Windows 的本地影视资料库应用，用于整理电影、剧集和动画。它不会移动或修改原始视频文件，所有资料都保存在本机。

## 主要功能

- 扫描一个或多个本地媒体文件夹。
- 识别电影、剧集和非标准命名动画。
- 将同一动画或剧集的多个文件归入同一海报。
- 使用 `ffprobe` 读取时长、分辨率、编码、容器和 HDR 信息。
- 使用 TMDB 获取电影及剧集的中文资料。
- 使用 Bangumi 和 AniList 获取动画中文资料、原名、评分和封面。
- 编辑标题、年份、分类、简介和备注。
- 管理已看、未看、标签和片单。
- 手动合并条目、整理重复项目和批量刷新元数据。
- 删除资料库条目并通过黑名单阻止其再次出现。
- 查看扫描历史、错误记录和运行诊断。
- 导出及恢复包含数据库与本地海报的 `.mmbak` 备份。
- 在硬盘盘符或媒体根目录变化后批量迁移路径。

## 下载与安装

1. 打开 GitHub 仓库右侧的 **Releases** 页面。
2. 下载 `MediaManager_0.1.0_x64-setup.exe`。
3. 运行安装程序并完成安装。
4. 在 Windows 开始菜单搜索 **MediaManager** 并打开。

当前安装包适用于 Windows x64。由于尚未进行商业代码签名，Windows SmartScreen 可能显示“未知发布者”。确认文件来自本仓库 Release 后，可选择“更多信息”并继续运行。

## 日常打开方式

安装后不需要再运行开发命令。

- 按 Windows 键，输入 `MediaManager`，然后按回车。
- 可以右键开始菜单中的 MediaManager，选择固定到开始屏幕或任务栏。
- 也可以从安装目录运行 `media-manager.exe`，但通常没有必要。

`pnpm tauri dev` 仅用于开发源码，不是日常使用方式。

## 数据保存

资料库默认保存在：

```text
%LOCALAPPDATA%\com.local.mediamanager\
├── library.db
├── backups\
└── cache\posters\
```

其中包括影视资料、观看状态、标签、片单、黑名单、扫描目录和刮削记录。原始视频仍保存在用户选择的媒体文件夹中。

正常升级或卸载不会主动删除资料库。重新安装后，应用会继续读取相同数据。

## 备份建议

在应用中打开：

```text
管理 → 数据安全 → 导出备份
```

`.mmbak` 文件包含资料库和本地海报。TMDB Token 永远不会写入手动或自动备份；恢复时只保留当前电脑已有 Token。换电脑后请重新填写。

换硬盘或更改盘符后，可使用“媒体路径迁移”把旧根路径批量替换为新路径。

## 从源码开发

要求：

- Windows 10/11 x64
- Node.js
- pnpm
- Rust MSVC 工具链
- Visual Studio Build Tools，包含 Desktop development with C++

运行：

```powershell
pnpm.cmd install
powershell -ExecutionPolicy Bypass -File .\scripts\prepare-ffprobe.ps1
pnpm.cmd tauri dev
```

构建 NSIS 安装包：

```powershell
pnpm.cmd tauri build
```

产物位于：

```text
src-tauri\target\release\bundle\nsis\
```

## 技术栈

- Tauri 2
- React
- TypeScript
- Rust
- SQLite

## 第三方组件

安装包包含 BtbN 提供的 Windows x64 LGPL 静态 `ffprobe`。许可证、来源和 SHA-256 信息位于：

```text
src-tauri\third-party\ffmpeg\
```

`ffprobe.exe` 超过 GitHub 单文件大小限制，因此不会提交到源码仓库。首次从源码构建前，请运行 `scripts/prepare-ffprobe.ps1` 下载并校验固定版本。

## 许可证

项目目前尚未选定开源许可证。公开仓库发布前应补充项目许可证。FFmpeg/ffprobe 使用其单独的 LGPL 许可证。
