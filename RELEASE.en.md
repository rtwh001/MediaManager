# MediaManager 0.1.0 Release Notes

**English** | [简体中文](RELEASE.zh-CN.md)

This is the first Windows x64 release of MediaManager.

## Download

Download the following asset from the GitHub Release:

```text
MediaManager_0.1.0_x64-setup.exe
MediaManager_0.1.0_windows_x64_portable.zip
```

### Installer

- Platform: Windows x64
- Installer: NSIS
- File size: 46,099,733 bytes
- SHA-256: `ED5867F14401CD2304C65E9DD10089E5C6721F5485A55BBF0A84BDB9CDF659D8`
- Code signing: Unsigned

### Portable ZIP

- Format: ZIP
- File size: 71,486,864 bytes
- SHA-256: `9322AFF093F58CC0E5A5246F5B896ECD0E2AFBA8EE9232D979D36E992D48566E`
- Usage: Extract the complete archive and double-click `media-manager.exe`

## Included In This Release

- New `#6597ec` blue theme and Konata application icon.
- Local movie, TV series, and anime library.
- Episode grouping designed for release-group and non-standard anime filenames.
- TMDB, Bangumi, AniList, and local NFO metadata.
- Local and online poster caching.
- Watched status, tags, collections, filtering, and sorting.
- Manual categorization, manual grouping, and duplicate organization.
- Batch selection and metadata refresh.
- Delete blacklist and restoration.
- Scan progress, cancellation, error history, logs, and diagnostics.
- Self-contained `.mmbak` backups with integrity checks and automatic pre-restore backups.
- Media root path migration.
- Bundled static LGPL `ffprobe`; no separate FFmpeg installation is required.

## Opening The Installed App

After installation:

1. Press the Windows key.
2. Type `MediaManager`.
3. Press Enter.

The installer creates a Windows Start menu shortcut. You can pin it to Start or the taskbar. Normal use does not require `pnpm`, `cargo`, or any other development command.

## Data Preservation

User data is stored under:

```text
%LOCALAPPDATA%\com.local.mediamanager\
```

In-place installation, upgrades, and normal uninstallation do not remove this directory. MediaManager does not move or delete original video files.

Export a `.mmbak` file from **Manage → Data Safety** before upgrading.

## Verification

- All 17 Rust tests passed.
- Strict Clippy checks passed.
- TypeScript and Vite production builds passed.
- Release and NSIS builds passed.
- Silent installation and uninstallation passed.
- The installed application launched successfully.
- The bundled `ffprobe` executed successfully.
- FFmpeg LGPL license and source notices were installed.
- Library data remained after uninstallation.

## Known Notice

The installer is not commercially code-signed yet. Windows SmartScreen may show an unknown publisher warning. Verify the download source and SHA-256 before running it.
