ALTER TABLE media_items ADD COLUMN season_number INTEGER;
ALTER TABLE media_items ADD COLUMN episode_number INTEGER;

CREATE INDEX IF NOT EXISTS idx_media_items_recognition
    ON media_items(recognition_status);

INSERT OR IGNORE INTO schema_migrations (version) VALUES (2);
