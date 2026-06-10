# Publishing MediaManager On GitHub

**English** | [简体中文](PUBLISHING.zh-CN.md)

This guide assumes no previous GitHub experience.

## What Users Need To Download

Regular users only need one of these files:

| File | Installation | Extra requirements |
| --- | --- | --- |
| `MediaManager_0.1.0_x64-setup.exe` | Recommended installer | None |
| `MediaManager_0.1.0_windows_x64_portable.zip` | Extract and run | None |

Users do not need the source code, Node.js, Rust, pnpm, Visual Studio, or FFmpeg.

GitHub's **Code → Download ZIP** option downloads source code. It is intended for developers and cannot be launched directly.

## Create The Repository

1. Create and verify a GitHub account.
2. Select **New repository** from the top-right `+` menu.
3. Name the repository `MediaManager`.
4. Choose **Public**.
5. Do not initialize it with a README, `.gitignore`, or license.
6. Create the repository and copy its HTTPS URL.

## Make The First Source Commit

Open PowerShell:

```powershell
cd A:\CODING\MediaManager
git init
git branch -M main
pnpm.cmd check:secrets
git add .
git status
```

Only continue after the checker prints `Secret check passed`. Check that `node_modules`, `target`, `ffprobe.exe`, `.env` files, local databases, backups, credentials, and the private `记忆` directory are not staged.

Enter the TMDB token only in MediaManager. Never place it in source files, documentation, issues, release notes, logs, or screenshots. Backups forcibly exclude the token, and every user should configure their own token locally.

Commit and push:

```powershell
git commit -m "Initial release of MediaManager"
git remote add origin https://github.com/YOUR_NAME/MediaManager.git
git push -u origin main
```

Complete browser authentication if GitHub requests it.

## Add A Project License

The repository does not currently contain a project license. Without one, public visitors may read the code but do not automatically receive permission to copy, modify, or redistribute it.

If MIT matches your intent, use **Add file → Create new file → Choose a license template** on GitHub and select MIT License.

## Create The Release

Create and push a version tag:

```powershell
git tag v0.1.0
git push origin v0.1.0
```

Then:

1. Open **Releases → Draft a new release**.
2. Select `v0.1.0`.
3. Use `MediaManager 0.1.0` as the title.
4. Paste [RELEASE.en.md](RELEASE.en.md) into the description.
5. Upload:

```text
src-tauri\target\release\bundle\nsis\MediaManager_0.1.0_x64-setup.exe
src-tauri\target\release\bundle\portable\MediaManager_0.1.0_windows_x64_portable.zip
```

6. Publish the release.

Upload binaries as Release Assets. Do not commit them to the Git repository.

## Checksums

Installer:

```text
Size: 46,099,733 bytes
SHA-256: ED5867F14401CD2304C65E9DD10089E5C6721F5485A55BBF0A84BDB9CDF659D8
```

Portable ZIP:

```text
Size: 71,486,864 bytes
SHA-256: 9322AFF093F58CC0E5A5246F5B896ECD0E2AFBA8EE9232D979D36E992D48566E
```

Recalculate both hashes after every rebuild:

```powershell
Get-FileHash .\path\to\file -Algorithm SHA256
```

## Source ZIP Requirements

Developers using GitHub's automatically generated source ZIP must run:

```powershell
pnpm.cmd install
powershell -ExecutionPolicy Bypass -File .\scripts\prepare-ffprobe.ps1
pnpm.cmd tauri dev
```

Source builds require Windows C++ Build Tools, Rust, Node.js, and pnpm.

## Future Updates

```powershell
git status
pnpm.cmd check:secrets
git add .
git commit -m "Describe the update"
git push
```

For every application release, update the version, rebuild the installer and portable ZIP, update checksums, create a new tag, and publish a new GitHub Release.
