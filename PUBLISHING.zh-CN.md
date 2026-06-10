# 从零开始发布到 GitHub

[English](PUBLISHING.en.md) | **简体中文**

这份教程按“从未使用过 GitHub”的情况编写。完成后，其他用户可以在项目首页阅读说明，并从 Releases 下载安装版或便携版。

## 用户到底需要下载什么

普通用户只需要以下两个文件之一：

| 下载文件 | 是否需要安装 | 其他环境 |
| --- | --- | --- |
| `MediaManager_0.1.0_x64-setup.exe` | 需要，推荐 | 什么都不需要 |
| `MediaManager_0.1.0_windows_x64_portable.zip` | 不需要，解压即用 | 什么都不需要 |

用户不需要下载源码，不需要安装 Node.js、Rust、pnpm、Visual Studio 或 FFmpeg。

GitHub 页面上的 **Code → Download ZIP** 下载的是源码，不是可直接运行的软件。它主要提供给开发者。

## 第一步：注册 GitHub

1. 打开 <https://github.com/>。
2. 点击 **Sign up**。
3. 注册账号并验证邮箱。
4. 登录 GitHub。

## 第二步：创建空仓库

1. 点击右上角的 `+`。
2. 选择 **New repository**。
3. Repository name 填写：

```text
MediaManager
```

4. Description 可以填写：

```text
A local movie, TV series, and anime library for Windows.
```

5. 选择 **Public**，其他人才能看到和下载。
6. 不要勾选 Add a README file。
7. `.gitignore` 和 License 暂时选择 None。
8. 点击 **Create repository**。

此时 GitHub 会显示一个仓库地址，例如：

```text
https://github.com/你的用户名/MediaManager.git
```

## 第三步：公开前检查本地文件

项目已经配置 `.gitignore`，以下内容不会上传：

- `node_modules`
- `dist`
- Rust `target`
- 大型 `ffprobe.exe`
- 本机数据库与 `.mmbak` 备份
- `.env`、凭据文件和 TMDB Token
- 内部开发记录 `记忆/`
- 临时安装测试目录

打开 PowerShell：

```powershell
cd A:\CODING\MediaManager
```

确认大型 `ffprobe` 下载脚本可用：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\prepare-ffprobe.ps1
```

运行密钥检查：

```powershell
pnpm.cmd check:secrets
```

看到 `Secret check passed` 才继续。推送后 GitHub Actions 还会再次执行同一检查。

Token 只应在 MediaManager 的 TMDB 设置界面填写。不要把它写进源码、`.env`、README、Issue、Release 描述、日志或截图，也不要上传 `library.db` 或 `.mmbak`。所有备份都会强制排除 Token，其他用户应配置自己的 Token。

## 第四步：第一次提交源码

在项目目录依次执行：

```powershell
git init
git branch -M main
pnpm.cmd check:secrets
git add .
git status
```

仔细查看 `git status`：

- 不应出现 `node_modules`。
- 不应出现 `src-tauri/target`。
- 不应出现 `src-tauri/binaries/ffprobe.exe`。
- 不应出现 `记忆/`。
- 不应出现私人数据库或备份。
- 不应出现 `.env`、Token 或凭据文件。

确认无误后提交：

```powershell
git commit -m "Initial release of MediaManager"
```

如果 Git 提示没有配置姓名和邮箱：

```powershell
git config --global user.name "你的 GitHub 用户名"
git config --global user.email "你的 GitHub 邮箱"
git commit -m "Initial release of MediaManager"
```

## 第五步：连接并推送到 GitHub

把下面地址替换成你的真实仓库地址：

```powershell
git remote add origin https://github.com/你的用户名/MediaManager.git
git push -u origin main
```

GitHub 可能弹出浏览器要求登录和授权，按页面提示完成即可。

刷新 GitHub 仓库页面后，就能看到源码和项目主页 README。

## 第六步：选择项目许可证

没有许可证时，其他人可以阅读公开代码，但默认没有复制、修改和重新发布的授权。

如果希望别人自由使用和修改，初学者通常可以选择 MIT：

1. 在 GitHub 仓库点击 **Add file → Create new file**。
2. 文件名输入 `LICENSE`。
3. 点击右侧 **Choose a license template**。
4. 选择 MIT License。
5. 检查年份和姓名。
6. 点击 **Commit changes**。

选择许可证涉及你的发布意愿，请自行确认 MIT 是否符合要求。

## 第七步：创建版本标签

在本地执行：

```powershell
git tag v0.1.0
git push origin v0.1.0
```

标签代表一个确定版本。以后发布新版本可以使用 `v0.1.1`、`v0.2.0` 等。

## 第八步：创建 GitHub Release

1. 打开 GitHub 仓库。
2. 点击右侧 **Releases**。
3. 点击 **Draft a new release**。
4. Choose a tag 选择 `v0.1.0`。
5. Release title 填写：

```text
MediaManager 0.1.0
```

6. 将 [中文发布说明](RELEASE.zh-CN.md) 或 [英文发布说明](RELEASE.en.md) 粘贴到描述框。
7. 上传两个 Release Asset：

```text
src-tauri\target\release\bundle\nsis\MediaManager_0.1.0_x64-setup.exe
src-tauri\target\release\bundle\portable\MediaManager_0.1.0_windows_x64_portable.zip
```

8. 点击 **Publish release**。

不要使用 `git add` 上传安装包和便携 ZIP。二进制发布文件应该作为 Release Asset 上传。

## 第九步：用户如何下载

发布后，用户可以：

1. 打开仓库首页。
2. 点击右侧 **Releases**。
3. 打开 `MediaManager 0.1.0`。
4. 在 **Assets** 中下载：
   - 安装版 EXE，或
   - 便携版 ZIP。

GitHub 还会自动显示：

- Source code (zip)
- Source code (tar.gz)

这两个是源码压缩包，只适合开发者。

## 当前发布文件校验

### 安装版

```text
文件大小：46,099,733 bytes
SHA-256：ED5867F14401CD2304C65E9DD10089E5C6721F5485A55BBF0A84BDB9CDF659D8
```

### 便携版

```text
文件大小：71,486,864 bytes
SHA-256：9322AFF093F58CC0E5A5246F5B896ECD0E2AFBA8EE9232D979D36E992D48566E
```

重新构建后，大小和 SHA-256 可能改变。发布前重新计算：

```powershell
Get-FileHash `
  .\src-tauri\target\release\bundle\nsis\MediaManager_0.1.0_x64-setup.exe `
  -Algorithm SHA256

Get-FileHash `
  .\src-tauri\target\release\bundle\portable\MediaManager_0.1.0_windows_x64_portable.zip `
  -Algorithm SHA256
```

## 以后如何更新源码

修改项目后执行：

```powershell
git status
pnpm.cmd check:secrets
git add .
git commit -m "描述本次修改"
git push
```

发布新版本时：

1. 修改 `package.json`、`Cargo.toml` 和 `tauri.conf.json` 中的版本号。
2. 重新运行测试和 `pnpm.cmd tauri build`。
3. 重新生成便携 ZIP。
4. 更新中英文 Release Notes 中的文件大小和 SHA-256。
5. 创建新标签和 GitHub Release。

## 源码 ZIP 如何构建

开发者下载 GitHub 自动生成的 Source code ZIP 后，需要：

```powershell
pnpm.cmd install
powershell -ExecutionPolicy Bypass -File .\scripts\prepare-ffprobe.ps1
pnpm.cmd tauri dev
```

构建安装包：

```powershell
pnpm.cmd tauri build
```

源码方式需要 Windows C++ Build Tools、Rust、Node.js 和 pnpm。

## 发布前检查清单

- README 中英文内容正确。
- 截图不包含私人路径、邮箱或 Token。
- `pnpm.cmd check:secrets` 已通过。
- `.gitignore` 没有遗漏隐私文件。
- 项目已经添加合适的许可证。
- 安装版和便携版均能运行。
- Release Assets 已上传。
- 文件大小和 SHA-256 已更新。
- 已说明安装包尚未签名和 SmartScreen 提示。
