CREATE TABLE IF NOT EXISTS media_blacklist (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE COLLATE NOCASE,
    file_name TEXT NOT NULL,
    media_title TEXT,
    scan_source_path TEXT,
    deleted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

ALTER TABLE scan_history ADD COLUMN files_ignored INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_media_blacklist_deleted_at
    ON media_blacklist(deleted_at DESC);

INSERT OR IGNORE INTO schema_migrations (version) VALUES (5);
