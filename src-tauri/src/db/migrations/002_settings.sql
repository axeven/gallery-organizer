CREATE TABLE IF NOT EXISTS app_settings (
    key        TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

INSERT OR IGNORE INTO app_settings (key, value_json, updated_at) VALUES
    ('output_mode',             '"output_folder"', unixepoch()),
    ('output_dir',              'null',            unixepoch()),
    ('default_quality',         '85',              unixepoch()),
    ('default_format',          '"jpeg"',          unixepoch()),
    ('duplicate_hash_distance', '8',               unixepoch()),
    ('date_group_granularity',  '"day"',           unixepoch()),
    ('thumbnail_size_px',       '256',             unixepoch()),
    ('scan_recursive',          'true',            unixepoch());
