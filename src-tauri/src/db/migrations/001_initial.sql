CREATE TABLE IF NOT EXISTS images (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path       TEXT NOT NULL UNIQUE,
    file_name       TEXT NOT NULL,
    file_size_bytes INTEGER NOT NULL,
    width_px        INTEGER,
    height_px       INTEGER,
    format          TEXT,
    taken_at        INTEGER,
    taken_at_source TEXT NOT NULL DEFAULT 'exif',
    camera_make     TEXT,
    camera_model    TEXT,
    perceptual_hash TEXT,
    scanned_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_images_taken_at ON images(taken_at);
CREATE INDEX IF NOT EXISTS idx_images_perceptual_hash ON images(perceptual_hash);

CREATE TABLE IF NOT EXISTS groups (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    group_type TEXT NOT NULL,
    label      TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS group_members (
    group_id  INTEGER NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    image_id  INTEGER NOT NULL REFERENCES images(id) ON DELETE CASCADE,
    is_keeper INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (group_id, image_id)
);
CREATE INDEX IF NOT EXISTS idx_group_members_image ON group_members(image_id);

CREATE TABLE IF NOT EXISTS processing_jobs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    status          TEXT NOT NULL DEFAULT 'queued',
    operation       TEXT NOT NULL,
    params_json     TEXT NOT NULL,
    output_mode     TEXT NOT NULL,
    output_dir      TEXT,
    total_images    INTEGER NOT NULL DEFAULT 0,
    processed_count INTEGER NOT NULL DEFAULT 0,
    failed_count    INTEGER NOT NULL DEFAULT 0,
    error_message   TEXT,
    created_at      INTEGER NOT NULL,
    started_at      INTEGER,
    finished_at     INTEGER
);

CREATE TABLE IF NOT EXISTS job_images (
    job_id   INTEGER NOT NULL REFERENCES processing_jobs(id) ON DELETE CASCADE,
    image_id INTEGER NOT NULL REFERENCES images(id) ON DELETE CASCADE,
    status   TEXT NOT NULL DEFAULT 'queued',
    error    TEXT,
    PRIMARY KEY (job_id, image_id)
);
