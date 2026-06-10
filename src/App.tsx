import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { useEffect, useMemo, useRef, useState } from "react";
import "./App.css";
import appIcon from "./assets/konata-brand.jpg";

type DatabaseStatus = {
  path: string;
  schemaVersion: number;
  mediaCount: number;
  scanSourceCount: number;
};

type ScanSource = {
  id: number;
  path: string;
  displayName: string;
  enabled: boolean;
  recursive: boolean;
  lastScannedAt: string | null;
};

type ScanProgress = {
  running: boolean;
  cancelled: boolean;
  currentSource: string | null;
  currentFile: string | null;
  filesFound: number;
  filesIgnored: number;
  filesAdded: number;
  filesUpdated: number;
  filesMissing: number;
  errors: number;
};

type MediaCard = {
  id: number;
  title: string;
  year: number | null;
  mediaType: string;
  recognitionStatus: string;
  fileCount: number;
  extension: string | null;
  width: number | null;
  height: number | null;
  isMissing: boolean;
  filePath: string | null;
  fileName: string | null;
  fileSize: number | null;
  modifiedAt: string | null;
  videoCodec: string | null;
  audioCodec: string | null;
  containerFormat: string | null;
  createdAt: string;
  overview: string | null;
  userNotes: string | null;
  watched: boolean;
  seasonNumber: number | null;
  episodeNumber: number | null;
  durationSeconds: number | null;
  hdrFormat: string | null;
  posterPath: string | null;
  tagIds: number[];
  tagNames: string[];
  collectionIds: number[];
  collectionNames: string[];
};

type Tag = { id: number; name: string; color: string | null };
type Collection = { id: number; name: string; description: string | null };

type BlacklistItem = {
  id: number;
  path: string;
  fileName: string;
  mediaTitle: string | null;
  scanSourcePath: string | null;
  deletedAt: string;
};

type ScanHistoryItem = {
  id: number;
  sourceName: string | null;
  startedAt: string;
  finishedAt: string | null;
  status: string;
  filesFound: number;
  filesIgnored: number;
  filesAdded: number;
  filesUpdated: number;
  filesMissing: number;
  errorMessage: string | null;
};

type DiagnosticsReport = {
  appVersion: string;
  databasePath: string;
  databaseSizeBytes: number;
  logDirectory: string;
  schemaVersion: number;
  mediaCount: number;
  fileCount: number;
  missingFileCount: number;
  scanSourceCount: number;
  failedScanCount: number;
  ffprobeAvailable: boolean;
  ffprobeVersion: string | null;
};

type MergeDuplicatesResult = {
  groupsMerged: number;
  itemsRemoved: number;
  filesRelinked: number;
};

type ScrapeResult = {
  provider: string;
  nfoPath: string | null;
  posterPath: string | null;
  fieldsApplied: string[];
  message: string;
};

type TmdbStatus = { configured: boolean };

type TmdbCandidate = {
  provider: "tmdb" | "anilist" | "bangumi";
  tmdbId?: number;
  anilistId?: number;
  bangumiId?: number;
  mediaType: "movie" | "tv" | "anime";
  title: string;
  originalTitle: string | null;
  year: number | null;
  overview: string | null;
  posterUrl: string | null;
  voteAverage: number | null;
};

type ApplyTmdbResult = {
  title: string;
  posterPath: string | null;
  fieldsApplied: string[];
};

type LibraryMutationResult = {
  itemsRemoved: number;
  filesRelinked: number;
};

type BackupResult = {
  path: string;
  sizeBytes: number;
  artworkFiles: number;
};

type RestoreBackupResult = {
  automaticBackupPath: string;
  artworkFiles: number;
  schemaVersion: number;
};

type MigrateMediaPathsResult = {
  scanSourcesUpdated: number;
  mediaFilesUpdated: number;
  blacklistPathsUpdated: number;
};

type NavigationItem = { label: string; count?: number };

type EditForm = {
  title: string;
  year: string;
  mediaType: string;
  overview: string;
  userNotes: string;
  watched: boolean;
  tagIds: number[];
  collectionIds: number[];
};

const libraryItems: NavigationItem[] = [
  { label: "全部影片" },
  { label: "电影" },
  { label: "剧集" },
  { label: "动画" },
];
const smartItems: NavigationItem[] = [
  { label: "最近添加" },
  { label: "未识别" },
  { label: "缺少海报" },
  { label: "文件缺失" },
];
const managementItems: NavigationItem[] = [
  { label: "媒体文件夹" },
  { label: "标签与片单" },
  { label: "在线刮削" },
  { label: "黑名单" },
  { label: "数据安全" },
  { label: "扫描记录" },
  { label: "日志与诊断" },
];

function mediaTypeForLabel(label: string) {
  return ({ 电影: "movie", 剧集: "series", 动画: "animation" } as Record<string, string>)[
    label
  ];
}

function mediaTypeLabel(type: string) {
  return (
    {
      movie: "电影",
      series: "剧集",
      animation: "动画",
      other: "其他",
      unknown: "未分类",
    } as Record<string, string>
  )[type] ?? "未分类";
}

function formatFileSize(bytes: number | null) {
  if (!bytes) return "未知";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  return `${value.toFixed(unitIndex >= 3 ? 2 : 1)} ${units[unitIndex]}`;
}

function formatDuration(seconds: number | null) {
  if (!seconds) return "未知";
  const minutes = Math.round(seconds / 60);
  return minutes >= 60 ? `${Math.floor(minutes / 60)} 小时 ${minutes % 60} 分` : `${minutes} 分钟`;
}

function fileLeaf(path: string | null) {
  return path?.split(/[\\/]/).pop() ?? "";
}

function normalizedMatchTitle(value: string | null) {
  return (value ?? "")
    .toLocaleLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, "");
}

function emptyProgress(): ScanProgress {
  return {
    running: false,
    cancelled: false,
    currentSource: null,
    currentFile: null,
    filesFound: 0,
    filesIgnored: 0,
    filesAdded: 0,
    filesUpdated: 0,
    filesMissing: 0,
    errors: 0,
  };
}

function NavigationGroup({
  title,
  items,
  activeItem,
  onSelect,
}: {
  title: string;
  items: NavigationItem[];
  activeItem: string;
  onSelect: (label: string) => void;
}) {
  return (
    <section className="nav-group">
      <h2>{title}</h2>
      <div className="nav-list">
        {items.map((item) => (
          <button
            className={activeItem === item.label ? "nav-item active" : "nav-item"}
            key={item.label}
            onClick={() => onSelect(item.label)}
            type="button"
          >
            <span>{item.label}</span>
            {item.count !== undefined && <span className="nav-count">{item.count}</span>}
          </button>
        ))}
      </div>
    </section>
  );
}

function App() {
  const [activeItem, setActiveItem] = useState("全部影片");
  const [database, setDatabase] = useState<DatabaseStatus | null>(null);
  const [databaseError, setDatabaseError] = useState("");
  const [scanSources, setScanSources] = useState<ScanSource[]>([]);
  const [mediaItems, setMediaItems] = useState<MediaCard[]>([]);
  const [tags, setTags] = useState<Tag[]>([]);
  const [collections, setCollections] = useState<Collection[]>([]);
  const [blacklistItems, setBlacklistItems] = useState<BlacklistItem[]>([]);
  const [scanHistory, setScanHistory] = useState<ScanHistoryItem[]>([]);
  const [actionError, setActionError] = useState("");
  const [notice, setNotice] = useState("");
  const [isAddingSource, setIsAddingSource] = useState(false);
  const [scanProgress, setScanProgress] = useState<ScanProgress>(emptyProgress());
  const [searchQuery, setSearchQuery] = useState("");
  const [sortMode, setSortMode] = useState("watch");
  const [watchFilter, setWatchFilter] = useState<"all" | "unwatched" | "watched">("all");
  const [selectionMode, setSelectionMode] = useState(false);
  const [selectedMediaIds, setSelectedMediaIds] = useState<number[]>([]);
  const [mergeKeeperId, setMergeKeeperId] = useState<number | null>(null);
  const [batchProgress, setBatchProgress] = useState({ running: false, current: 0, total: 0 });
  const [selectedMedia, setSelectedMedia] = useState<MediaCard | null>(null);
  const [editing, setEditing] = useState(false);
  const [editForm, setEditForm] = useState<EditForm | null>(null);
  const [newTagName, setNewTagName] = useState("");
  const [newCollectionName, setNewCollectionName] = useState("");
  const [diagnostics, setDiagnostics] = useState<DiagnosticsReport | null>(null);
  const [recentLogs, setRecentLogs] = useState("");
  const [diagnosticsBusy, setDiagnosticsBusy] = useState(false);
  const [dataSafetyBusy, setDataSafetyBusy] = useState(false);
  const [oldMediaRoot, setOldMediaRoot] = useState("");
  const [newMediaRoot, setNewMediaRoot] = useState("");
  const [scrapingMediaId, setScrapingMediaId] = useState<number | null>(null);
  const [tmdbConfigured, setTmdbConfigured] = useState(false);
  const [tmdbToken, setTmdbToken] = useState("");
  const [tmdbBusy, setTmdbBusy] = useState(false);
  const [tmdbCandidates, setTmdbCandidates] = useState<TmdbCandidate[]>([]);
  const [tmdbQuery, setTmdbQuery] = useState("");
  const searchInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    refreshLibraryState();
    refreshTmdbStatus();
    invoke<boolean>("scan_running").then((running) => {
      if (running) setScanProgress((current) => ({ ...current, running: true }));
    });

    const unlistenPromise = listen<ScanProgress>("scan-progress", (event) => {
      setScanProgress(event.payload);
      if (!event.payload.running) refreshLibraryState(selectedMedia?.id);
    });
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    const handleSearchShortcut = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLocaleLowerCase() === "f") {
        event.preventDefault();
        setActiveItem("全部影片");
        window.setTimeout(() => searchInputRef.current?.focus(), 0);
      }
    };
    window.addEventListener("keydown", handleSearchShortcut);
    return () => window.removeEventListener("keydown", handleSearchShortcut);
  }, []);

  useEffect(() => {
    if (activeItem === "日志与诊断") refreshDiagnostics();
    if (activeItem === "在线刮削") refreshTmdbStatus();
  }, [activeItem]);

  const filteredMediaItems = useMemo(() => {
    const query = searchQuery.trim().toLocaleLowerCase();
    return mediaItems
      .filter((item) => {
        if (watchFilter === "watched" && !item.watched) return false;
        if (watchFilter === "unwatched" && item.watched) return false;
        const matchesSearch =
          !query ||
          item.title.toLocaleLowerCase().includes(query) ||
          item.fileName?.toLocaleLowerCase().includes(query) ||
          item.tagNames.some((tag) => tag.toLocaleLowerCase().includes(query));
        if (!matchesSearch) return false;
        switch (activeItem) {
          case "电影":
          case "剧集":
          case "动画":
            return item.mediaType === mediaTypeForLabel(activeItem);
          case "未识别":
            return item.recognitionStatus === "unrecognized";
          case "文件缺失":
            return item.isMissing;
          case "缺少海报":
            return !item.posterPath;
          default:
            return true;
        }
      })
      .sort((left, right) => {
        if (sortMode === "watch") {
          const statusOrder = Number(left.watched) - Number(right.watched);
          return statusOrder || right.id - left.id;
        }
        if (sortMode === "title") return left.title.localeCompare(right.title, "zh-CN");
        if (sortMode === "year") return (right.year ?? 0) - (left.year ?? 0);
        return right.id - left.id;
      });
  }, [activeItem, mediaItems, searchQuery, sortMode, watchFilter]);

  const navigationLibraryItems = libraryItems.map((item) => ({
    ...item,
    count:
      item.label === "全部影片"
        ? mediaItems.length
        : mediaItems.filter((media) => media.mediaType === mediaTypeForLabel(item.label)).length,
  }));
  const navigationSmartItems = smartItems.map((item) => ({
    ...item,
    count:
      item.label === "未识别"
        ? mediaItems.filter((media) => media.recognitionStatus === "unrecognized").length
        : item.label === "文件缺失"
          ? mediaItems.filter((media) => media.isMissing).length
          : item.label === "缺少海报"
            ? mediaItems.filter((media) => !media.posterPath).length
            : undefined,
  }));

  async function refreshLibraryState(selectedId?: number) {
    try {
      const [status, sources, items, tagItems, collectionItems, blacklist, historyItems] =
        await Promise.all([
        invoke<DatabaseStatus>("database_status"),
        invoke<ScanSource[]>("list_scan_sources"),
        invoke<MediaCard[]>("list_media_items"),
        invoke<Tag[]>("list_tags"),
        invoke<Collection[]>("list_collections"),
        invoke<BlacklistItem[]>("list_blacklist"),
        invoke<ScanHistoryItem[]>("list_scan_history"),
      ]);
      setDatabase(status);
      setScanSources(sources);
      setMediaItems(items);
      setTags(tagItems);
      setCollections(collectionItems);
      setBlacklistItems(blacklist);
      setScanHistory(historyItems);
      if (selectedId) setSelectedMedia(items.find((item) => item.id === selectedId) ?? null);
      setDatabaseError("");
    } catch (error) {
      setDatabaseError(String(error));
    }
  }

  async function addMediaFolder() {
    setActionError("");
    const selected = await open({ directory: true, multiple: false, title: "选择媒体文件夹" });
    if (!selected || Array.isArray(selected)) return;
    setIsAddingSource(true);
    try {
      await invoke("add_scan_source", { request: { path: selected } });
      await refreshLibraryState();
    } catch (error) {
      setActionError(String(error));
    } finally {
      setIsAddingSource(false);
    }
  }

  async function removeMediaFolder(source: ScanSource) {
    if (!window.confirm(`从资料库中移除“${source.displayName}”？\n\n不会删除任何原始文件。`))
      return;
    try {
      await invoke("remove_scan_source", { id: source.id });
      await refreshLibraryState();
    } catch (error) {
      setActionError(String(error));
    }
  }

  async function scanLibrary() {
    setActionError("");
    setNotice("");
    setScanProgress({ ...emptyProgress(), running: true });
    try {
      const summary = await invoke<ScanProgress>("scan_library");
      setScanProgress(summary);
      setNotice(summary.cancelled ? "扫描已取消" : "扫描完成");
      await refreshLibraryState(selectedMedia?.id);
    } catch (error) {
      setActionError(String(error));
      setScanProgress(emptyProgress());
    }
  }

  async function cancelScan() {
    await invoke("cancel_scan");
  }

  async function refreshDiagnostics() {
    setDiagnosticsBusy(true);
    try {
      const [report, logs] = await Promise.all([
        invoke<DiagnosticsReport>("diagnostics_report"),
        invoke<string>("read_recent_logs"),
      ]);
      setDiagnostics(report);
      setRecentLogs(logs);
    } catch (error) {
      setActionError(String(error));
    } finally {
      setDiagnosticsBusy(false);
    }
  }

  async function exportBackup() {
    const date = new Date().toISOString().slice(0, 10);
    const destination = await save({
      defaultPath: `MediaManager-${date}.mmbak`,
      filters: [{ name: "MediaManager 备份", extensions: ["mmbak"] }],
      title: "导出资料库备份",
    });
    if (!destination) return;
    setDataSafetyBusy(true);
    setActionError("");
    try {
      const result = await invoke<BackupResult>("export_library_backup", {
        request: {
          destination,
        },
      });
      setNotice(
        `备份已保存：${formatFileSize(result.sizeBytes)}，包含 ${result.artworkFiles} 张本地海报`,
      );
    } catch (error) {
      setActionError(String(error));
    } finally {
      setDataSafetyBusy(false);
    }
  }

  async function restoreBackup() {
    const source = await open({
      directory: false,
      multiple: false,
      filters: [{ name: "MediaManager 备份", extensions: ["mmbak"] }],
      title: "选择资料库备份",
    });
    if (!source || Array.isArray(source)) return;
    if (
      !window.confirm(
        "恢复该备份会替换当前资料库。\n\n程序会先自动备份当前数据，原始视频文件不会被修改。是否继续？",
      )
    )
      return;
    setDataSafetyBusy(true);
    setActionError("");
    try {
      const result = await invoke<RestoreBackupResult>("restore_library_backup", {
        request: { source },
      });
      setSelectedMedia(null);
      leaveSelectionMode();
      await Promise.all([refreshLibraryState(), refreshTmdbStatus(), refreshDiagnostics()]);
      setNotice(
        `资料库恢复完成（Schema v${result.schemaVersion}，恢复 ${result.artworkFiles} 张海报）。恢复前备份：${result.automaticBackupPath}`,
      );
    } catch (error) {
      setActionError(String(error));
    } finally {
      setDataSafetyBusy(false);
    }
  }

  async function migrateMediaPaths() {
    if (!oldMediaRoot.trim() || !newMediaRoot.trim()) return;
    if (
      !window.confirm(
        `将所有以\n${oldMediaRoot.trim()}\n开头的媒体路径改为\n${newMediaRoot.trim()}\n\n不会移动或删除磁盘文件。是否继续？`,
      )
    )
      return;
    setDataSafetyBusy(true);
    setActionError("");
    try {
      const result = await invoke<MigrateMediaPathsResult>("migrate_media_paths", {
        request: {
          oldRoot: oldMediaRoot,
          newRoot: newMediaRoot,
        },
      });
      await refreshLibraryState();
      setNotice(
        `路径迁移完成：目录 ${result.scanSourcesUpdated}，媒体文件 ${result.mediaFilesUpdated}，黑名单路径 ${result.blacklistPathsUpdated}`,
      );
    } catch (error) {
      setActionError(String(error));
    } finally {
      setDataSafetyBusy(false);
    }
  }

  async function mergeDuplicates() {
    if (
      !window.confirm(
        "整理高置信度重复条目？\n\n动画和剧集按系列名称合并，电影仅在标题与年份均相同时合并。不会删除原始视频文件。",
      )
    )
      return;
    setDiagnosticsBusy(true);
    setActionError("");
    try {
      const result = await invoke<MergeDuplicatesResult>("merge_duplicate_media");
      setNotice(
        `整理完成：合并 ${result.groupsMerged} 组，移除 ${result.itemsRemoved} 个重复条目，重新关联 ${result.filesRelinked} 个文件`,
      );
      await Promise.all([refreshLibraryState(), refreshDiagnostics()]);
    } catch (error) {
      setActionError(String(error));
    } finally {
      setDiagnosticsBusy(false);
    }
  }

  function openDetails(item: MediaCard) {
    setSelectedMedia(item);
    setEditing(false);
    setEditForm(null);
    setTmdbQuery(item.title);
    setTmdbCandidates([]);
  }

  function beginEditing() {
    if (!selectedMedia) return;
    setEditForm({
      title: selectedMedia.title,
      year: selectedMedia.year?.toString() ?? "",
      mediaType: selectedMedia.mediaType,
      overview: selectedMedia.overview ?? "",
      userNotes: selectedMedia.userNotes ?? "",
      watched: selectedMedia.watched,
      tagIds: [...selectedMedia.tagIds],
      collectionIds: [...selectedMedia.collectionIds],
    });
    setEditing(true);
  }

  async function saveMedia() {
    if (!selectedMedia || !editForm) return;
    try {
      await Promise.all([
        invoke("update_media_item", {
          request: {
            id: selectedMedia.id,
            title: editForm.title,
            year: editForm.year ? Number(editForm.year) : null,
            mediaType: editForm.mediaType,
            overview: editForm.overview,
            userNotes: editForm.userNotes,
            watched: editForm.watched,
          },
        }),
        invoke("set_media_tags", {
          request: { mediaId: selectedMedia.id, ids: editForm.tagIds },
        }),
        invoke("set_media_collections", {
          request: { mediaId: selectedMedia.id, ids: editForm.collectionIds },
        }),
      ]);
      await refreshLibraryState(selectedMedia.id);
      setEditing(false);
      setNotice("资料已保存");
    } catch (error) {
      setActionError(String(error));
    }
  }

  async function choosePoster() {
    if (!selectedMedia) return;
    const selected = await open({
      multiple: false,
      title: "选择海报",
      filters: [{ name: "图片", extensions: ["jpg", "jpeg", "png", "webp"] }],
    });
    if (!selected || Array.isArray(selected)) return;
    try {
      await invoke("set_media_poster", {
        mediaId: selectedMedia.id,
        sourcePath: selected,
      });
      await refreshLibraryState(selectedMedia.id);
      setNotice("海报已更新");
    } catch (error) {
      setActionError(String(error));
    }
  }

  async function scrapeSelectedMedia() {
    if (!selectedMedia) return;
    setScrapingMediaId(selectedMedia.id);
    setActionError("");
    try {
      const result = await invoke<ScrapeResult>("scrape_local_metadata", {
        mediaId: selectedMedia.id,
      });
      await refreshLibraryState(selectedMedia.id);
      setNotice(
        result.fieldsApplied.length
          ? `${result.message} 已更新：${result.fieldsApplied.join("、")}`
          : result.message,
      );
    } catch (error) {
      setActionError(String(error));
    } finally {
      setScrapingMediaId(null);
    }
  }

  async function refreshTmdbStatus() {
    try {
      const status = await invoke<TmdbStatus>("tmdb_status");
      setTmdbConfigured(status.configured);
    } catch (error) {
      setActionError(String(error));
    }
  }

  async function saveTmdbToken() {
    if (!tmdbToken.trim()) return;
    setTmdbBusy(true);
    setActionError("");
    try {
      const status = await invoke<TmdbStatus>("save_tmdb_token", {
        request: { token: tmdbToken },
      });
      setTmdbConfigured(status.configured);
      setTmdbToken("");
      setNotice("TMDB Read Access Token 已保存在本机");
    } catch (error) {
      setActionError(String(error));
    } finally {
      setTmdbBusy(false);
    }
  }

  async function searchOnlineMetadata() {
    if (!selectedMedia) return;
    setTmdbBusy(true);
    setActionError("");
    setTmdbCandidates([]);
    try {
      const query = tmdbQuery.trim() || selectedMedia.title;
      const candidates =
        selectedMedia.mediaType === "animation"
          ? await searchAnimationMetadata(selectedMedia.id, query)
          : await invoke<TmdbCandidate[]>("search_tmdb", {
              request: { mediaId: selectedMedia.id, query },
            });
      setTmdbCandidates(candidates);
      if (!candidates.length) setNotice("没有找到匹配结果，可以修改搜索词重试");
    } catch (error) {
      setActionError(String(error));
    } finally {
      setTmdbBusy(false);
    }
  }

  async function applyOnlineMetadata(candidate: TmdbCandidate) {
    if (!selectedMedia) return;
    setTmdbBusy(true);
    setActionError("");
    try {
      const result =
        candidate.provider === "bangumi"
          ? await invoke<ApplyTmdbResult>("apply_bangumi_metadata", {
              request: {
                mediaId: selectedMedia.id,
                bangumiId: candidate.bangumiId,
              },
            })
          : candidate.provider === "anilist"
            ? await invoke<ApplyTmdbResult>("apply_anilist_metadata", {
              request: {
                mediaId: selectedMedia.id,
                anilistId: candidate.anilistId,
              },
            })
          : await invoke<ApplyTmdbResult>("apply_tmdb_metadata", {
              request: {
                mediaId: selectedMedia.id,
                tmdbId: candidate.tmdbId,
                mediaType: candidate.mediaType,
              },
            });
      await refreshLibraryState(selectedMedia.id);
      setTmdbCandidates([]);
      setNotice(`已应用《${result.title}》：${result.fieldsApplied.join("、")}`);
    } catch (error) {
      setActionError(String(error));
    } finally {
      setTmdbBusy(false);
    }
  }

  async function applyCandidateToMedia(mediaId: number, candidate: TmdbCandidate) {
    return candidate.provider === "bangumi"
      ? invoke<ApplyTmdbResult>("apply_bangumi_metadata", {
          request: { mediaId, bangumiId: candidate.bangumiId },
        })
      : candidate.provider === "anilist"
        ? invoke<ApplyTmdbResult>("apply_anilist_metadata", {
          request: { mediaId, anilistId: candidate.anilistId },
        })
      : invoke<ApplyTmdbResult>("apply_tmdb_metadata", {
          request: {
            mediaId,
            tmdbId: candidate.tmdbId,
            mediaType: candidate.mediaType,
          },
        });
  }

  async function searchAnimationMetadata(mediaId: number, query: string) {
    const [bangumi, anilist] = await Promise.allSettled([
      invoke<TmdbCandidate[]>("search_bangumi", { request: { query } }),
      invoke<TmdbCandidate[]>("search_anilist", { request: { mediaId, query } }),
    ]);
    const candidates = [
      ...(bangumi.status === "fulfilled" ? bangumi.value : []),
      ...(anilist.status === "fulfilled" ? anilist.value : []),
    ];
    if (!candidates.length && bangumi.status === "rejected" && anilist.status === "rejected") {
      throw new Error(`Bangumi: ${bangumi.reason}；AniList: ${anilist.reason}`);
    }
    return candidates;
  }

  async function toggleWatched() {
    if (!selectedMedia) return;
    try {
      await invoke("set_watched_status", {
        request: { mediaId: selectedMedia.id, watched: !selectedMedia.watched },
      });
      await refreshLibraryState(selectedMedia.id);
    } catch (error) {
      setActionError(String(error));
    }
  }

  async function changeMediaType(mediaType: string) {
    if (!selectedMedia || mediaType === selectedMedia.mediaType) return;
    try {
      await invoke("set_media_type", {
        request: { mediaId: selectedMedia.id, mediaType },
      });
      await refreshLibraryState(selectedMedia.id);
      setNotice(`已将《${selectedMedia.title}》分类为${mediaTypeLabel(mediaType)}`);
    } catch (error) {
      setActionError(String(error));
    }
  }

  async function createTag() {
    if (!newTagName.trim()) return;
    try {
      await invoke("create_tag", {
        request: { name: newTagName, color: "#6597ec" },
      });
      setNewTagName("");
      await refreshLibraryState(selectedMedia?.id);
    } catch (error) {
      setActionError(String(error));
    }
  }

  async function createCollection() {
    if (!newCollectionName.trim()) return;
    try {
      await invoke("create_collection", {
        request: { name: newCollectionName, description: null },
      });
      setNewCollectionName("");
      await refreshLibraryState(selectedMedia?.id);
    } catch (error) {
      setActionError(String(error));
    }
  }

  async function revealSelectedFile() {
    if (selectedMedia?.filePath) await revealItemInDir(selectedMedia.filePath);
  }

  function toggleBatchSelection(mediaId: number) {
    setSelectedMediaIds((current) => {
      const next = current.includes(mediaId)
        ? current.filter((id) => id !== mediaId)
        : [...current, mediaId];
      setMergeKeeperId((keeper) => (keeper && next.includes(keeper) ? keeper : next[0] ?? null));
      return next;
    });
  }

  function leaveSelectionMode() {
    setSelectionMode(false);
    setSelectedMediaIds([]);
    setMergeKeeperId(null);
  }

  function selectAllVisible() {
    const ids = filteredMediaItems.map((item) => item.id);
    setSelectedMediaIds(ids);
    setMergeKeeperId(ids[0] ?? null);
  }

  async function deleteMedia(mediaIds: number[]) {
    if (!mediaIds.length) return;
    const names = mediaItems
      .filter((item) => mediaIds.includes(item.id))
      .slice(0, 3)
      .map((item) => `《${item.title}》`)
      .join("、");
    if (
      !window.confirm(
        `从资料库删除 ${mediaIds.length} 个条目${names ? `：${names}` : ""}？\n\n不会删除原始视频文件。文件会加入黑名单，后续扫描不会再次出现，可在“黑名单”中恢复。`,
      )
    )
      return;
    try {
      const result = await invoke<LibraryMutationResult>("delete_media_items", {
        request: { mediaIds },
      });
      setSelectedMedia(null);
      leaveSelectionMode();
      await refreshLibraryState();
      setNotice(`已删除 ${result.itemsRemoved} 个资料库条目，并将对应文件加入黑名单`);
    } catch (error) {
      setActionError(String(error));
    }
  }

  async function restoreBlacklist(ids: number[]) {
    if (!ids.length) return;
    setActionError("");
    try {
      const restored = await invoke<number>("restore_blacklist_items", {
        request: { ids },
      });
      if (scanSources.length) {
        await scanLibrary();
        setNotice(`已从黑名单恢复 ${restored} 个文件并重新扫描`);
      } else {
        await refreshLibraryState();
        setNotice(`已从黑名单恢复 ${restored} 个文件`);
      }
    } catch (error) {
      setActionError(String(error));
    }
  }

  async function restoreAllBlacklist() {
    if (!blacklistItems.length) return;
    if (!window.confirm(`恢复黑名单中的 ${blacklistItems.length} 个文件？`)) return;
    setActionError("");
    try {
      const restored = await invoke<number>("clear_blacklist");
      if (scanSources.length) {
        await scanLibrary();
        setNotice(`已恢复全部 ${restored} 个文件并重新扫描`);
      } else {
        await refreshLibraryState();
        setNotice(`已恢复全部 ${restored} 个文件`);
      }
    } catch (error) {
      setActionError(String(error));
    }
  }

  async function mergeSelectedMedia() {
    if (selectedMediaIds.length < 2 || !mergeKeeperId) return;
    const keeper = mediaItems.find((item) => item.id === mergeKeeperId);
    if (
      !window.confirm(
        `将 ${selectedMediaIds.length} 个条目合并到《${keeper?.title ?? "主条目"}》？\n\n所有媒体文件将显示在同一个海报下，原始视频不会移动。`,
      )
    )
      return;
    try {
      const result = await invoke<LibraryMutationResult>("merge_media_items", {
        request: { keeperId: mergeKeeperId, mediaIds: selectedMediaIds },
      });
      leaveSelectionMode();
      await refreshLibraryState(mergeKeeperId);
      setNotice(
        `合并完成：移除 ${result.itemsRemoved} 个旧条目，归入 ${result.filesRelinked} 个文件`,
      );
    } catch (error) {
      setActionError(String(error));
    }
  }

  async function refreshSelectedMetadata() {
    const items = mediaItems.filter((item) => selectedMediaIds.includes(item.id));
    if (!items.length) return;
    if (
      !window.confirm(
        `自动刷新 ${items.length} 个条目的在线元数据？\n\n每个条目将采用搜索结果中最匹配的首个候选。`,
      )
    )
      return;
    setBatchProgress({ running: true, current: 0, total: items.length });
    setActionError("");
    let updated = 0;
    const failures: string[] = [];
    for (const [index, item] of items.entries()) {
      setBatchProgress({ running: true, current: index + 1, total: items.length });
      try {
        const candidates =
          item.mediaType === "animation"
            ? await searchAnimationMetadata(item.id, item.title)
            : await invoke<TmdbCandidate[]>("search_tmdb", {
                request: { mediaId: item.id, query: item.title },
              });
        const itemTitle = normalizedMatchTitle(item.title);
        const candidate =
          candidates.find(
            (entry) =>
              normalizedMatchTitle(entry.title) === itemTitle ||
              normalizedMatchTitle(entry.originalTitle) === itemTitle,
          ) ??
          candidates.find((entry) => item.year && entry.year === item.year) ??
          candidates[0];
        if (!candidate) {
          failures.push(`${item.title}：无匹配`);
          continue;
        }
        await applyCandidateToMedia(item.id, candidate);
        updated += 1;
      } catch (error) {
        failures.push(`${item.title}：${String(error)}`);
      }
    }
    setBatchProgress({ running: false, current: items.length, total: items.length });
    await refreshLibraryState();
    setNotice(
      `批量刷新完成：成功 ${updated}，失败 ${failures.length}${
        failures.length ? `。${failures.slice(0, 2).join("；")}` : ""
      }`,
    );
  }

  function toggleRelation(field: "tagIds" | "collectionIds", id: number) {
    setEditForm((current) => {
      if (!current) return current;
      const values = current[field];
      return {
        ...current,
        [field]: values.includes(id) ? values.filter((value) => value !== id) : [...values, id],
      };
    });
  }

  const isManagementPage = managementItems.some((item) => item.label === activeItem);

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <img className="brand-mark" src={appIcon} alt="MediaManager" />
          <div>
            <strong>MediaManager</strong>
            <span>本地影视资料库</span>
          </div>
        </div>
        <nav className="navigation" aria-label="主导航">
          <NavigationGroup
            activeItem={activeItem}
            items={navigationLibraryItems}
            onSelect={setActiveItem}
            title="资料库"
          />
          <NavigationGroup
            activeItem={activeItem}
            items={navigationSmartItems}
            onSelect={setActiveItem}
            title="智能分类"
          />
          <NavigationGroup
            activeItem={activeItem}
            items={managementItems}
            onSelect={setActiveItem}
            title="管理"
          />
        </nav>
        <div className={databaseError ? "database-state error" : "database-state"}>
          <span />
          <div>
            <strong>{databaseError ? "数据库连接失败" : "数据库已连接"}</strong>
            <small>
              {databaseError ||
                (database
                  ? `Schema v${database.schemaVersion} · ${database.scanSourceCount} 个目录`
                  : "正在初始化 SQLite")}
            </small>
          </div>
        </div>
      </aside>

      <main className="workspace">
        <header className="topbar">
          <div className="search-wrap">
            <span aria-hidden="true">⌕</span>
            <input
              aria-label="搜索资料库"
              onChange={(event) => {
                setSearchQuery(event.currentTarget.value);
                if (isManagementPage) setActiveItem("全部影片");
              }}
              onFocus={() => {
                if (isManagementPage) setActiveItem("全部影片");
              }}
              placeholder="搜索标题、文件名或标签"
              ref={searchInputRef}
              type="search"
              value={searchQuery}
            />
            {searchQuery && (
              <button
                aria-label="清除搜索"
                className="search-clear"
                onClick={() => {
                  setSearchQuery("");
                  searchInputRef.current?.focus();
                }}
                type="button"
              >
                ×
              </button>
            )}
          </div>
          {scanProgress.running ? (
            <button className="danger-button" onClick={cancelScan} type="button">
              取消扫描
            </button>
          ) : (
            <button
              className="primary-button"
              disabled={scanSources.length === 0}
              onClick={scanLibrary}
              type="button"
            >
              扫描资料库
            </button>
          )}
        </header>

        <section className="content">
          <div className="content-heading">
            <div>
              <p className="eyebrow">{isManagementPage ? "管理" : "资料库"}</p>
              <h1>{activeItem}</h1>
              <p>
                {activeItem === "媒体文件夹"
                  ? `${scanSources.length} 个扫描目录`
                  : activeItem === "标签与片单"
                    ? `${tags.length} 个标签 · ${collections.length} 个片单`
                    : activeItem === "在线刮削"
                      ? tmdbConfigured
                        ? "TMDB 已配置"
                        : "需要配置 TMDB Token"
                    : activeItem === "黑名单"
                      ? `${blacklistItems.length} 个已忽略文件`
                    : activeItem === "数据安全"
                      ? "备份、恢复与媒体路径迁移"
                    : activeItem === "扫描记录"
                      ? `${scanHistory.length} 条记录`
                      : activeItem === "日志与诊断"
                        ? "运行状态、重复整理与最近日志"
                      : `${filteredMediaItems.length} 个影视条目`}
              </p>
            </div>
            {!isManagementPage && (
              <div className="library-controls">
                <div className="watch-filter" aria-label="观看状态筛选">
                  {([
                    ["all", "全部"],
                    ["unwatched", "未看"],
                    ["watched", "已看"],
                  ] as const).map(([value, label]) => (
                    <button
                      className={watchFilter === value ? "active" : ""}
                      key={value}
                      onClick={() => setWatchFilter(value)}
                      type="button"
                    >
                      {label}
                    </button>
                  ))}
                </div>
                <select
                  aria-label="排序方式"
                  onChange={(event) => setSortMode(event.currentTarget.value)}
                  value={sortMode}
                >
                  <option value="watch">未看优先</option>
                  <option value="added">最近添加</option>
                  <option value="title">标题</option>
                  <option value="year">年份</option>
                </select>
                <button
                  className={selectionMode ? "secondary-button active" : "secondary-button"}
                  onClick={() => (selectionMode ? leaveSelectionMode() : setSelectionMode(true))}
                  type="button"
                >
                  {selectionMode ? "退出管理" : "批量管理"}
                </button>
              </div>
            )}
          </div>

          {actionError && <div className="action-error">{actionError}</div>}
          {notice && <div className="notice-banner">{notice}</div>}
          {selectionMode && !isManagementPage && (
            <div className="bulk-toolbar">
              <strong>已选择 {selectedMediaIds.length} 个条目</strong>
              <button onClick={selectAllVisible} type="button">全选当前</button>
              <button
                disabled={!selectedMediaIds.length || batchProgress.running}
                onClick={refreshSelectedMetadata}
                type="button"
              >
                {batchProgress.running
                  ? `刷新中 ${batchProgress.current}/${batchProgress.total}`
                  : "刷新元数据"}
              </button>
              <label>
                主条目
                <select
                  disabled={selectedMediaIds.length < 2}
                  onChange={(event) => setMergeKeeperId(Number(event.currentTarget.value))}
                  value={mergeKeeperId ?? ""}
                >
                  {mediaItems
                    .filter((item) => selectedMediaIds.includes(item.id))
                    .map((item) => (
                      <option key={item.id} value={item.id}>{item.title}</option>
                    ))}
                </select>
              </label>
              <button
                disabled={selectedMediaIds.length < 2}
                onClick={mergeSelectedMedia}
                type="button"
              >
                合并为一个海报
              </button>
              <button
                className="bulk-delete"
                disabled={!selectedMediaIds.length}
                onClick={() => deleteMedia(selectedMediaIds)}
                type="button"
              >
                删除
              </button>
            </div>
          )}
          {(scanProgress.running || scanProgress.filesFound > 0) && (
            <div className="scan-progress-card">
              <div>
                <strong>
                  {scanProgress.running
                    ? `正在扫描 ${scanProgress.currentSource ?? ""}`
                    : scanProgress.cancelled
                      ? "扫描已取消"
                      : "扫描完成"}
                </strong>
                <span title={scanProgress.currentFile ?? ""}>
                  {scanProgress.running
                    ? fileLeaf(scanProgress.currentFile) || "正在读取目录..."
                    : `发现 ${scanProgress.filesFound} 个视频`}
                </span>
              </div>
              <div className="scan-stat-row">
                <span>忽略 {scanProgress.filesIgnored}</span>
                <span>新增 {scanProgress.filesAdded}</span>
                <span>更新 {scanProgress.filesUpdated}</span>
                <span>缺失 {scanProgress.filesMissing}</span>
                <span className={scanProgress.errors ? "scan-errors" : ""}>
                  错误 {scanProgress.errors}
                </span>
              </div>
            </div>
          )}

          {scanSources.length === 0 && !isManagementPage ? (
            <EmptyLibrary isAdding={isAddingSource} onAdd={addMediaFolder} />
          ) : activeItem === "媒体文件夹" ? (
            <SourceManager
              isAdding={isAddingSource}
              onAdd={addMediaFolder}
              onRemove={removeMediaFolder}
              sources={scanSources}
            />
          ) : activeItem === "标签与片单" ? (
            <TaxonomyManager
              collections={collections}
              newCollectionName={newCollectionName}
              newTagName={newTagName}
              onCollectionNameChange={setNewCollectionName}
              onCreateCollection={createCollection}
              onCreateTag={createTag}
              onTagNameChange={setNewTagName}
              tags={tags}
            />
          ) : activeItem === "在线刮削" ? (
            <TmdbSettings
              busy={tmdbBusy}
              configured={tmdbConfigured}
              onSave={saveTmdbToken}
              onTokenChange={setTmdbToken}
              token={tmdbToken}
            />
          ) : activeItem === "黑名单" ? (
            <BlacklistManager
              items={blacklistItems}
              onRestore={restoreBlacklist}
              onRestoreAll={restoreAllBlacklist}
            />
          ) : activeItem === "数据安全" ? (
            <DataSafetyManager
              busy={dataSafetyBusy}
              databasePath={database?.path ?? ""}
              newRoot={newMediaRoot}
              oldRoot={oldMediaRoot}
              onExport={exportBackup}
              onMigrate={migrateMediaPaths}
              onNewRootChange={setNewMediaRoot}
              onOldRootChange={setOldMediaRoot}
              onRestore={restoreBackup}
              scanRunning={scanProgress.running}
            />
          ) : activeItem === "扫描记录" ? (
            <ScanHistory history={scanHistory} />
          ) : activeItem === "日志与诊断" ? (
            <DiagnosticsPanel
              busy={diagnosticsBusy}
              logs={recentLogs}
              onMerge={mergeDuplicates}
              onRefresh={refreshDiagnostics}
              report={diagnostics}
            />
          ) : filteredMediaItems.length === 0 ? (
            <div className="empty-library compact">
              <h2>没有符合条件的条目</h2>
              <p>尝试清除搜索词、切换分类，或者重新扫描资料库。</p>
            </div>
          ) : (
            <div className="poster-grid">
              {filteredMediaItems.map((item, index) => (
                <button
                  className={`media-card ${
                    selectedMediaIds.includes(item.id) ? "selected" : ""
                  }`}
                  key={item.id}
                  onClick={() =>
                    selectionMode ? toggleBatchSelection(item.id) : openDetails(item)
                  }
                  type="button"
                >
                  {selectionMode && (
                    <span className="selection-mark">
                      {selectedMediaIds.includes(item.id) ? "✓" : ""}
                    </span>
                  )}
                  <Poster item={item} tone={index % 6} />
                  <div className="media-card-body">
                    <h2>{item.title}</h2>
                    <p>
                      {item.year ?? "年份未知"} <span>·</span>{" "}
                      {item.extension?.toUpperCase() ?? "VIDEO"}
                      {item.fileCount > 1 ? (
                        <>
                          <span>·</span>
                          {item.fileCount} 个文件
                        </>
                      ) : null}
                      {item.width && item.height ? (
                        <>
                          <span>·</span>
                          {item.width}×{item.height}
                        </>
                      ) : null}
                    </p>
                    <div className="card-tags">
                      {item.watched && <small>已看</small>}
                      {item.tagNames.slice(0, 2).map((tag) => (
                        <small key={tag}>{tag}</small>
                      ))}
                    </div>
                  </div>
                </button>
              ))}
            </div>
          )}
        </section>
      </main>

      {selectedMedia && (
        <div className="detail-backdrop" onClick={() => setSelectedMedia(null)}>
          <aside className="detail-panel" onClick={(event) => event.stopPropagation()}>
            <button
              aria-label="关闭详情"
              className="detail-close"
              onClick={() => setSelectedMedia(null)}
              type="button"
            >
              ×
            </button>
            {editing && editForm ? (
              <MediaEditor
                collections={collections}
                form={editForm}
                item={selectedMedia}
                onCancel={() => setEditing(false)}
                onChange={setEditForm}
                onChoosePoster={choosePoster}
                onSave={saveMedia}
                onToggleRelation={toggleRelation}
                tags={tags}
              />
            ) : (
              <MediaDetails
                item={selectedMedia}
                onChangeMediaType={changeMediaType}
                onChoosePoster={choosePoster}
                onEdit={beginEditing}
                onApplyTmdb={applyOnlineMetadata}
                onDelete={() => deleteMedia([selectedMedia.id])}
                onReveal={revealSelectedFile}
                onSearchTmdb={searchOnlineMetadata}
                onScrape={scrapeSelectedMedia}
                onTmdbQueryChange={setTmdbQuery}
                onToggleWatched={toggleWatched}
                scraping={scrapingMediaId === selectedMedia.id}
                tmdbBusy={tmdbBusy}
                tmdbCandidates={tmdbCandidates}
                tmdbConfigured={tmdbConfigured}
                tmdbQuery={tmdbQuery}
              />
            )}
          </aside>
        </div>
      )}
    </div>
  );
}

function Poster({ item, tone }: { item: MediaCard; tone: number }) {
  return (
    <div
      className={`media-poster tone-${tone} ${item.posterPath ? "has-image" : ""}`}
      style={
        item.posterPath
          ? { backgroundImage: `url("${convertFileSrc(item.posterPath)}")` }
          : undefined
      }
    >
      {!item.posterPath && (
        <div className="poster-monogram">{item.title.trim().slice(0, 1).toUpperCase() || "M"}</div>
      )}
      <span>{item.year ?? "未知年份"}</span>
      {item.isMissing && <strong className="missing-badge">文件缺失</strong>}
      {item.recognitionStatus === "unrecognized" && (
        <strong className="unrecognized-badge">未识别</strong>
      )}
      <strong className={item.watched ? "watch-badge watched" : "watch-badge"}>
        {item.watched ? "已看" : "未看"}
      </strong>
    </div>
  );
}

function EmptyLibrary({ isAdding, onAdd }: { isAdding: boolean; onAdd: () => void }) {
  return (
    <div className="empty-library">
      <div className="poster-placeholder">
        <div />
        <div />
        <div />
      </div>
      <h2>建立你的本地影视资料库</h2>
      <p>添加存放电影、剧集或动画的文件夹，MediaManager 会建立本地资料库。</p>
      <button className="primary-button large" disabled={isAdding} onClick={onAdd} type="button">
        {isAdding ? "正在添加..." : "添加媒体文件夹"}
      </button>
      <span>不会移动或修改你的原始视频文件</span>
    </div>
  );
}

function SourceManager({
  sources,
  isAdding,
  onAdd,
  onRemove,
}: {
  sources: ScanSource[];
  isAdding: boolean;
  onAdd: () => void;
  onRemove: (source: ScanSource) => void;
}) {
  return (
    <div className="source-panel">
      <div className="source-panel-heading">
        <div>
          <h2>媒体文件夹</h2>
          <p>扫描任务将递归检查这些目录中的支持格式。</p>
        </div>
        <button className="secondary-button" disabled={isAdding} onClick={onAdd} type="button">
          添加文件夹
        </button>
      </div>
      <div className="source-list">
        {sources.map((source) => (
          <article className="source-card" key={source.id}>
            <div className="folder-mark">▰</div>
            <div className="source-details">
              <strong>{source.displayName}</strong>
              <span title={source.path}>{source.path}</span>
              <small>
                {source.recursive ? "包含子目录" : "仅当前目录"} ·{" "}
                {source.lastScannedAt ? `上次扫描 ${source.lastScannedAt}` : "尚未扫描"}
              </small>
            </div>
            <span className="source-status">{source.enabled ? "已启用" : "已暂停"}</span>
            <button className="remove-button" onClick={() => onRemove(source)} type="button">
              移除
            </button>
          </article>
        ))}
      </div>
    </div>
  );
}

function TaxonomyManager(props: {
  tags: Tag[];
  collections: Collection[];
  newTagName: string;
  newCollectionName: string;
  onTagNameChange: (value: string) => void;
  onCollectionNameChange: (value: string) => void;
  onCreateTag: () => void;
  onCreateCollection: () => void;
}) {
  return (
    <div className="taxonomy-grid">
      <section className="manage-card">
        <h2>标签</h2>
        <p>标签可用于状态、画质、观看计划或任意自定义分类。</p>
        <div className="inline-create">
          <input
            onChange={(event) => props.onTagNameChange(event.currentTarget.value)}
            placeholder="新标签名称"
            value={props.newTagName}
          />
          <button onClick={props.onCreateTag} type="button">
            添加
          </button>
        </div>
        <div className="chip-list">
          {props.tags.map((tag) => (
            <span key={tag.id} style={{ borderColor: tag.color ?? undefined }}>
              {tag.name}
            </span>
          ))}
          {!props.tags.length && <small>还没有标签</small>}
        </div>
      </section>
      <section className="manage-card">
        <h2>片单</h2>
        <p>同一部影片可以加入多个自定义片单。</p>
        <div className="inline-create">
          <input
            onChange={(event) => props.onCollectionNameChange(event.currentTarget.value)}
            placeholder="新片单名称"
            value={props.newCollectionName}
          />
          <button onClick={props.onCreateCollection} type="button">
            添加
          </button>
        </div>
        <div className="collection-list">
          {props.collections.map((collection) => (
            <div key={collection.id}>
              <strong>{collection.name}</strong>
              <span>{collection.description ?? "暂无说明"}</span>
            </div>
          ))}
          {!props.collections.length && <small>还没有片单</small>}
        </div>
      </section>
    </div>
  );
}

function ScanHistory({ history }: { history: ScanHistoryItem[] }) {
  return (
    <div className="history-list">
      {history.map((item) => (
        <article className="history-card" key={item.id}>
          <div>
            <strong>{item.sourceName ?? "已移除的目录"}</strong>
            <span>{item.startedAt}</span>
          </div>
          <span className={`history-status ${item.status}`}>{item.status}</span>
          <div className="history-stats">
            <span>发现 {item.filesFound}</span>
            <span>忽略 {item.filesIgnored}</span>
            <span>新增 {item.filesAdded}</span>
            <span>更新 {item.filesUpdated}</span>
            <span>缺失 {item.filesMissing}</span>
          </div>
          {item.errorMessage && <pre>{item.errorMessage}</pre>}
        </article>
      ))}
      {!history.length && <div className="empty-library compact"><h2>暂无扫描记录</h2></div>}
    </div>
  );
}

function DataSafetyManager(props: {
  busy: boolean;
  scanRunning: boolean;
  databasePath: string;
  oldRoot: string;
  newRoot: string;
  onOldRootChange: (value: string) => void;
  onNewRootChange: (value: string) => void;
  onExport: () => void;
  onRestore: () => void;
  onMigrate: () => void;
}) {
  return (
    <div className="data-safety-layout">
      <section className="manage-card">
        <h2>资料库备份</h2>
        <p>
          导出单个 `.mmbak` 文件，包含资料库、观看状态、标签、片单、黑名单、刮削资料和本地海报。
        </p>
        <div className="data-safety-actions">
          <button
            className="primary-button"
            disabled={props.busy}
            onClick={props.onExport}
            type="button"
          >
            {props.busy ? "处理中..." : "导出备份"}
          </button>
          <button
            className="secondary-button"
            disabled={props.busy || props.scanRunning}
            onClick={props.onRestore}
            type="button"
          >
            恢复备份
          </button>
        </div>
        <small>
          为防止凭据泄露，任何备份都不会包含 TMDB Token。恢复时会保留本机已有 Token；
          换到新电脑后需要重新填写。原始视频不会被复制或修改。
        </small>
        <span className="data-path" title={props.databasePath}>
          当前数据库：{props.databasePath || "正在读取..."}
        </span>
      </section>

      <section className="manage-card">
        <h2>媒体路径迁移</h2>
        <p>
          用于硬盘盘符或媒体根目录改变，例如把 `D:\Videos` 批量替换为 `E:\Videos`。
        </p>
        <div className="path-migration-fields">
          <label>
            旧根路径
            <input
              onChange={(event) => props.onOldRootChange(event.currentTarget.value)}
              placeholder="D:\Videos"
              value={props.oldRoot}
            />
          </label>
          <label>
            新根路径
            <input
              onChange={(event) => props.onNewRootChange(event.currentTarget.value)}
              placeholder="E:\Videos"
              value={props.newRoot}
            />
          </label>
        </div>
        <button
          className="secondary-button"
          disabled={
            props.busy || props.scanRunning || !props.oldRoot.trim() || !props.newRoot.trim()
          }
          onClick={props.onMigrate}
          type="button"
        >
          执行路径迁移
        </button>
        <small>只修改资料库中的路径，不会移动、重命名或删除真实媒体文件。</small>
      </section>
    </div>
  );
}

function BlacklistManager({
  items,
  onRestore,
  onRestoreAll,
}: {
  items: BlacklistItem[];
  onRestore: (ids: number[]) => void;
  onRestoreAll: () => void;
}) {
  return (
    <div className="blacklist-panel">
      <div className="source-panel-heading">
        <div>
          <h2>已删除文件黑名单</h2>
          <p>这些文件仍保留在磁盘中，但扫描资料库时会被忽略。</p>
        </div>
        <button
          className="secondary-button"
          disabled={!items.length}
          onClick={onRestoreAll}
          type="button"
        >
          全部恢复
        </button>
      </div>
      <div className="blacklist-list">
        {items.map((item) => (
          <article className="blacklist-card" key={item.id}>
            <div>
              <strong>{item.mediaTitle || item.fileName}</strong>
              <span title={item.path}>{item.path}</span>
              <small>删除时间：{item.deletedAt}</small>
            </div>
            <button onClick={() => onRestore([item.id])} type="button">
              恢复并扫描
            </button>
          </article>
        ))}
      </div>
      {!items.length && (
        <div className="empty-library compact">
          <h2>黑名单为空</h2>
          <p>从资料库删除的文件会出现在这里。</p>
        </div>
      )}
    </div>
  );
}

function TmdbSettings(props: {
  configured: boolean;
  token: string;
  busy: boolean;
  onTokenChange: (value: string) => void;
  onSave: () => void;
}) {
  return (
    <section className="manage-card tmdb-settings">
      <p className="eyebrow">在线元数据 Provider</p>
      <h2>TMDB 中文刮削</h2>
      <p>
        配置 TMDB API Read Access Token 后，可以搜索电影、剧集和动画，选择候选项并下载中文资料与海报。
      </p>
      <p>
        动画条目会优先使用 Bangumi 搜索中文标题与中文资料，并同时使用 AniList 补充英文名、罗马字、日文原名和海报，无需额外 Token。
      </p>
      <label>
        Read Access Token
        <input
          autoComplete="off"
          onChange={(event) => props.onTokenChange(event.currentTarget.value)}
          placeholder={props.configured ? "已配置，输入新 Token 可替换" : "粘贴 TMDB Read Access Token"}
          type="password"
          value={props.token}
        />
      </label>
      <div className="tmdb-setting-actions">
        <span className={props.configured ? "provider-ready" : "provider-missing"}>
          {props.configured ? "已配置" : "尚未配置"}
        </span>
        <button
          className="primary-button"
          disabled={props.busy || !props.token.trim()}
          onClick={props.onSave}
          type="button"
        >
          {props.busy ? "保存中..." : "保存 Token"}
        </button>
      </div>
      <small>
        Token 仅保存在本机应用数据库。This product uses the TMDB API but is not endorsed or certified by TMDB.
      </small>
    </section>
  );
}

function DiagnosticsPanel(props: {
  report: DiagnosticsReport | null;
  logs: string;
  busy: boolean;
  onRefresh: () => void;
  onMerge: () => void;
}) {
  const report = props.report;
  return (
    <div className="diagnostics-layout">
      <section className="manage-card diagnostics-card">
        <div className="source-panel-heading">
          <div>
            <h2>运行诊断</h2>
            <p>用于确认数据库、扫描器、ffprobe 和日志状态。</p>
          </div>
          <button
            className="secondary-button"
            disabled={props.busy}
            onClick={props.onRefresh}
            type="button"
          >
            {props.busy ? "读取中..." : "刷新"}
          </button>
        </div>
        {report ? (
          <dl className="diagnostics-grid">
            <div><dt>应用版本</dt><dd>{report.appVersion}</dd></div>
            <div><dt>数据库 Schema</dt><dd>v{report.schemaVersion}</dd></div>
            <div><dt>影视条目</dt><dd>{report.mediaCount}</dd></div>
            <div><dt>媒体文件</dt><dd>{report.fileCount}</dd></div>
            <div><dt>缺失文件</dt><dd>{report.missingFileCount}</dd></div>
            <div><dt>失败扫描</dt><dd>{report.failedScanCount}</dd></div>
            <div><dt>ffprobe</dt><dd>{report.ffprobeAvailable ? "可用" : "不可用"}</dd></div>
            <div><dt>数据库大小</dt><dd>{formatFileSize(report.databaseSizeBytes)}</dd></div>
          </dl>
        ) : (
          <p>正在读取诊断信息...</p>
        )}
        {report && (
          <div className="diagnostic-paths">
            <span title={report.databasePath}>数据库：{report.databasePath}</span>
            <span title={report.logDirectory}>日志：{report.logDirectory}</span>
            {report.ffprobeVersion && <span>{report.ffprobeVersion}</span>}
          </div>
        )}
      </section>

      <section className="manage-card merge-card">
        <h2>重复影片与剧集整理</h2>
        <p>
          重新解析现有文件。动画和剧集会优先使用上级文件夹作为系列名，并将各集保存在同一影视条目下。
        </p>
        <button
          className="primary-button"
          disabled={props.busy}
          onClick={props.onMerge}
          type="button"
        >
          整理重复条目
        </button>
        <small>多个同名手工条目不会自动合并；原始视频文件不会被移动或删除。</small>
      </section>

      <section className="manage-card log-card">
        <div className="source-panel-heading">
          <div>
            <h2>最近日志</h2>
            <p>显示最新日志文件的最后 500 行。</p>
          </div>
        </div>
        <pre>{props.logs || "暂无日志。"}</pre>
      </section>
    </div>
  );
}

function MediaDetails(props: {
  item: MediaCard;
  onChangeMediaType: (mediaType: string) => void;
  onChoosePoster: () => void;
  onEdit: () => void;
  onReveal: () => void;
  onScrape: () => void;
  onToggleWatched: () => void;
  onSearchTmdb: () => void;
  onApplyTmdb: (candidate: TmdbCandidate) => void;
  onDelete: () => void;
  onTmdbQueryChange: (value: string) => void;
  scraping: boolean;
  tmdbConfigured: boolean;
  tmdbBusy: boolean;
  tmdbQuery: string;
  tmdbCandidates: TmdbCandidate[];
}) {
  const item = props.item;
  return (
    <>
      <button className="detail-poster poster-button" onClick={props.onChoosePoster} type="button">
        {item.posterPath ? (
          <img alt={item.title} src={convertFileSrc(item.posterPath)} />
        ) : (
          <span>{item.title.trim().slice(0, 1).toUpperCase() || "M"}</span>
        )}
        <small>更换海报</small>
      </button>
      <p className="eyebrow">影视条目</p>
      <h2>{item.title}</h2>
      <label className="media-type-control">
        <span>分类</span>
        <select
          onChange={(event) => props.onChangeMediaType(event.currentTarget.value)}
          value={item.mediaType}
        >
          <option value="movie">电影</option>
          <option value="series">剧集</option>
          <option value="animation">动画</option>
          <option value="other">其他</option>
        </select>
      </label>
      <div className="detail-chips">
        <span>{item.year ?? "年份未知"}</span>
        <span>{mediaTypeLabel(item.mediaType)}</span>
        <span>{item.fileCount} 个文件</span>
        {item.seasonNumber && <span>S{item.seasonNumber} E{item.episodeNumber}</span>}
        {item.watched && <span>已看</span>}
      </div>
      {item.overview && <p className="detail-overview">{item.overview}</p>}
      <div className="chip-list compact-chips">
        {item.tagNames.map((tag) => <span key={tag}>{tag}</span>)}
        {item.collectionNames.map((name) => <span key={name}>片单：{name}</span>)}
      </div>
      <dl className="detail-list">
        <div><dt>文件名</dt><dd>{item.fileName ?? "未知"}</dd></div>
        <div><dt>路径</dt><dd title={item.filePath ?? ""}>{item.filePath ?? "未知"}</dd></div>
        <div><dt>大小</dt><dd>{formatFileSize(item.fileSize)}</dd></div>
        <div><dt>时长</dt><dd>{formatDuration(item.durationSeconds)}</dd></div>
        <div><dt>画面</dt><dd>{item.width && item.height ? `${item.width} × ${item.height}` : "未知"} {item.hdrFormat ?? ""}</dd></div>
        <div><dt>编码</dt><dd>{item.videoCodec ?? "未知"} / {item.audioCodec ?? "未知"}</dd></div>
      </dl>
      {item.userNotes && <p className="user-notes">{item.userNotes}</p>}
      <button
        className={item.watched ? "watch-toggle watched" : "watch-toggle"}
        onClick={props.onToggleWatched}
        type="button"
      >
        <span>{item.watched ? "已看" : "未看"}</span>
        <small>点击标记为{item.watched ? "未看" : "已看"}</small>
      </button>
      <section className="tmdb-scrape-box">
        <div>
          <strong>{item.mediaType === "animation" ? "Bangumi + AniList 动画刮削" : "TMDB 在线刮削"}</strong>
          <span>
            {item.mediaType === "animation"
              ? "支持中文搜索、中文简介，并补充日文原名与动画海报"
              : props.tmdbConfigured
                ? "搜索中文资料与海报"
                : "请先在管理区配置 Token"}
          </span>
        </div>
        <div className="tmdb-search-row">
          <input
            onChange={(event) => props.onTmdbQueryChange(event.currentTarget.value)}
            placeholder={item.title}
            value={props.tmdbQuery}
          />
          <button
            disabled={
              (item.mediaType !== "animation" && !props.tmdbConfigured) || props.tmdbBusy
            }
            onClick={props.onSearchTmdb}
            type="button"
          >
            {props.tmdbBusy ? "搜索中..." : "在线搜索"}
          </button>
        </div>
        {!!props.tmdbCandidates.length && (
          <div className="tmdb-candidates">
            {props.tmdbCandidates.map((candidate) => (
              <article
                key={`${candidate.provider}-${candidate.tmdbId ?? candidate.anilistId ?? candidate.bangumiId}`}
              >
                {candidate.posterUrl ? (
                  <img alt={candidate.title} src={candidate.posterUrl} />
                ) : (
                  <div className="tmdb-poster-empty">无海报</div>
                )}
                <div>
                  <strong>{candidate.title}</strong>
                  <span>
                    {candidate.provider === "bangumi"
                      ? "Bangumi · "
                      : candidate.provider === "anilist"
                        ? "AniList · "
                        : "TMDB · "}
                    {candidate.mediaType === "movie"
                      ? "电影"
                      : candidate.mediaType === "anime"
                        ? "动画"
                        : "剧集"} ·{" "}
                    {candidate.year ?? "年份未知"}
                    {candidate.voteAverage ? ` · ${candidate.voteAverage.toFixed(1)}` : ""}
                  </span>
                  <p>{candidate.overview || candidate.originalTitle || "暂无简介"}</p>
                </div>
                <button
                  disabled={props.tmdbBusy}
                  onClick={() => props.onApplyTmdb(candidate)}
                  type="button"
                >
                  使用
                </button>
              </article>
            ))}
          </div>
        )}
      </section>
      <div className="detail-button-row">
        <button className="secondary-button" onClick={props.onEdit} type="button">编辑资料</button>
        <button
          className="secondary-button"
          disabled={props.scraping || !item.filePath || item.isMissing}
          onClick={props.onScrape}
          type="button"
        >
          {props.scraping ? "正在刮削..." : "刮削本地资料"}
        </button>
        <button
          className="primary-button"
          disabled={!item.filePath || item.isMissing}
          onClick={props.onReveal}
          type="button"
        >
          在资源管理器中显示
        </button>
      </div>
      <button className="detail-delete-button" onClick={props.onDelete} type="button">
        从资料库删除
      </button>
    </>
  );
}

function MediaEditor(props: {
  item: MediaCard;
  form: EditForm;
  tags: Tag[];
  collections: Collection[];
  onChange: (form: EditForm) => void;
  onToggleRelation: (field: "tagIds" | "collectionIds", id: number) => void;
  onChoosePoster: () => void;
  onSave: () => void;
  onCancel: () => void;
}) {
  const update = <K extends keyof EditForm>(key: K, value: EditForm[K]) =>
    props.onChange({ ...props.form, [key]: value });

  return (
    <div className="edit-form">
      <h2>编辑影视资料</h2>
      <button className="poster-edit-button" onClick={props.onChoosePoster} type="button">
        更换本地海报
      </button>
      <label>标题<input value={props.form.title} onChange={(e) => update("title", e.currentTarget.value)} /></label>
      <div className="form-row">
        <label>年份<input inputMode="numeric" value={props.form.year} onChange={(e) => update("year", e.currentTarget.value)} /></label>
        <label>类型<select value={props.form.mediaType} onChange={(e) => update("mediaType", e.currentTarget.value)}>
          <option value="movie">电影</option><option value="series">剧集</option>
          <option value="animation">动画</option><option value="other">其他</option>
        </select></label>
      </div>
      <label>简介<textarea rows={4} value={props.form.overview} onChange={(e) => update("overview", e.currentTarget.value)} /></label>
      <label>备注<textarea rows={3} value={props.form.userNotes} onChange={(e) => update("userNotes", e.currentTarget.value)} /></label>
      <label className="check-line"><input checked={props.form.watched} onChange={(e) => update("watched", e.currentTarget.checked)} type="checkbox" /> 标记为已看</label>
      <fieldset><legend>标签</legend><div className="relation-grid">
        {props.tags.map((tag) => <label key={tag.id}><input checked={props.form.tagIds.includes(tag.id)} onChange={() => props.onToggleRelation("tagIds", tag.id)} type="checkbox" />{tag.name}</label>)}
        {!props.tags.length && <small>请先在“标签与片单”中创建标签</small>}
      </div></fieldset>
      <fieldset><legend>片单</legend><div className="relation-grid">
        {props.collections.map((collection) => <label key={collection.id}><input checked={props.form.collectionIds.includes(collection.id)} onChange={() => props.onToggleRelation("collectionIds", collection.id)} type="checkbox" />{collection.name}</label>)}
        {!props.collections.length && <small>请先创建片单</small>}
      </div></fieldset>
      <div className="detail-button-row">
        <button className="secondary-button" onClick={props.onCancel} type="button">取消</button>
        <button className="primary-button" onClick={props.onSave} type="button">保存资料</button>
      </div>
    </div>
  );
}

export default App;
