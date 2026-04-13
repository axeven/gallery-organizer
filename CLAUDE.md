# Gallery Organizer — Claude Code Guide

Tauri v2 desktop app for scanning, grouping, and batch-processing a photo library.

## Stack

- **Frontend**: React + TypeScript, TanStack Query (server state), Zustand (ephemeral UI state), Tailwind CSS
- **Backend**: Rust, Tauri v2 command system, SQLite via sqlx (WAL mode)
- **Build**: `pnpm` for frontend, Cargo for Rust

## Common commands

```bash
# Dev (runs both Vite and Tauri)
pnpm tauri dev

# Build
pnpm tauri build

# Type-check frontend only
npx tsc --noEmit

# Check Rust only
cd src-tauri && cargo check

# After adding a new sqlx::query! macro, regenerate the offline cache:
cd src-tauri
DATABASE_URL="sqlite:///home/lathif/.local/share/com.gallery-organizer.app/gallery.db" cargo sqlx prepare
```

## Architecture overview

### Rust modules (`src-tauri/src/`)

```
lib.rs              — AppState definition, Tauri plugin setup, command registration
commands/
  scan.rs           — scan_folder, get_scan_status, cancel_scan
  groups.rs         — get_groups, rebuild_groups, get_duplicate_clusters,
                      set_keeper, dismiss_cluster, remove_group, remove_image_from_group
  images.rs         — get_images, get_image_detail, get_thumbnail
  jobs.rs           — create_job, start_job, cancel_job, get_jobs, get_job_detail, retry_failed_images
  settings.rs       — get_settings, update_settings, open_folder_dialog, reveal_in_explorer
db/
  models.rs         — DbImage, NewImage, DbGroup, DbGroupMember, DbProcessingJob, DbJobImage
  queries.rs        — all SQL via sqlx query macros
  migrations/       — 001_initial.sql, 002_settings.sql
grouper/
  mod.rs            — rebuild_groups() dispatcher ("date" | "duplicates" | "all")
  date_grouper.rs   — buckets images by day/month/year (all three in one pass)
  dupe_grouper.rs   — union-find clustering by perceptual hash hamming distance
scanner/
  mod.rs            — two-phase: walkdir collect → rayon parallel decode/hash → async DB upsert
  exif.rs           — EXIF extraction (taken_at, camera make/model, dimensions)
  hash.rs           — gradient perceptual hash (8×8 = 8 bytes, hex); JPEG fast path via zune-jpeg
processor/
  job_runner.rs     — run_job(): rayon parallel compress, tokio::task::block_in_place for DB writes
  compress.rs       — ProcessParams: quality, resize, fit, target_format
  output.rs         — OutputMode: InPlace | OutputFolder
settings/mod.rs     — AppSettings struct, load_all() / save() via app_settings KV table
```

### Frontend (`src/`)

```
api/commands.ts     — all invoke() wrappers, TS types, Tauri event bridge
store/scanStore.ts  — Zustand: scan phase/progress
store/jobStore.ts   — Zustand: active job progress keyed by job_id
pages/
  ScanPage.tsx      — folder picker, recursive toggle, progress bar
  GalleryPage.tsx   — date groups, expandable image grids, remove group/image
  DuplicatesPage.tsx — duplicate clusters, keeper selection, dismiss cluster
  JobsPage.tsx      — job list, retry failed
  SettingsPage.tsx  — settings form
```

### AppState

```rust
pub struct AppState {
    pub db: SqlitePool,
    pub scan_handle: Mutex<Option<JoinHandle<()>>>,      // cancellable scan
    pub job_handles: Mutex<HashMap<i64, JoinHandle<()>>>, // cancellable jobs
    pub settings: RwLock<AppSettings>,                    // loaded at startup
}
```

## Database

SQLite at `~/.local/share/com.gallery-organizer.app/gallery.db`

Key tables:
- **`images`** — one row per scanned file, never deleted by the app
- **`groups`** — named collections (`date_day`, `date_month`, `date_year`, `duplicate_cluster`); rebuilt from scratch on each grouper run
- **`group_members`** — many-to-many; `ON DELETE CASCADE` from groups; `is_keeper` flag for duplicate clusters
- **`processing_jobs`** / **`job_images`** — batch job tracking
- **`app_settings`** — key-value store, values are JSON-encoded strings

**Important**: removing a group or image from a group only affects `groups`/`group_members` rows. The `images` record and the file on disk are never touched.

## Tauri events (backend → frontend)

| Event | When |
|-------|------|
| `scan:progress` | every 100 images or 100 ms during scan |
| `groups:rebuilt` | after `rebuild_groups` completes |
| `job:progress` | after each image is processed |
| `job:complete` / `job:failed` | when a job finishes |

Wired in `setupEventBridge()` in `src/api/commands.ts`, called once on app mount.

## TanStack Query key conventions

| Key | Data |
|-----|------|
| `["groups", "date_day"]` etc. | date groups by granularity |
| `["images", groupId, page]` | paginated images per group |
| `["thumbnail", imageId]` | base64 JPEG thumbnail (`staleTime: Infinity`) |
| `["duplicate-clusters"]` | duplicate cluster list |
| `["jobs"]` | processing job list |

## Adding a new Tauri command

1. Write the handler in the appropriate `src-tauri/src/commands/*.rs` file
2. Add any new SQL to `src-tauri/src/db/queries.rs` using `sqlx::query!`
3. Register the command in the `tauri::generate_handler![]` list in `src-tauri/src/lib.rs`
4. Run `cargo sqlx prepare` (see above) to update the offline query cache
5. Add the `invoke()` wrapper + TypeScript type to `src/api/commands.ts`

## Known issues / notes

- `processor/job_runner.rs` hardcodes output format to `"jpeg"` — `ProcessParams.target_format` is not fully wired through
- Duplicate clustering is O(n²) on perceptual hash comparisons — acceptable up to ~50k images per code comment
- `OutputFolder` mode has `scan_root` hardcoded to `"/"` in `job_runner.rs`; should ideally come from the original scan root
