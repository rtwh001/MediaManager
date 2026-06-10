ALTER TABLE media_items ADD COLUMN group_key TEXT;
ALTER TABLE media_files ADD COLUMN season_number INTEGER;
ALTER TABLE media_files ADD COLUMN episode_number INTEGER;

CREATE INDEX IF NOT EXISTS idx_media_items_group_key
    ON media_items(media_type, group_key);

INSERT OR IGNORE INTO schema_migrations (version) VALUES (3);
