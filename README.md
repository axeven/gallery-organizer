# Gallery Organizer

A desktop app for scanning a photo library, grouping images by date or similarity, and batch-processing them (compress, resize, convert).

Built with Tauri v2 (Rust backend) + React (TypeScript frontend).

## Features

- **Scan** a folder (recursively or flat) — extracts EXIF metadata and computes a perceptual hash for every image
- **Gallery view** — browse images grouped by day, month, or year; remove groups or individual images from a group without touching the files on disk
- **Duplicate detection** — clusters near-identical images using perceptual hash + hamming distance; pick a keeper or dismiss a cluster
- **Batch processing** — compress, resize, and convert images; output in-place or to a separate folder; retry failed images
- **Settings** — configure output mode, quality, format, thumbnail size, duplicate sensitivity, and more

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) + [pnpm](https://pnpm.io/)
- Tauri v2 system dependencies for your platform — see the [Tauri prerequisites guide](https://tauri.app/start/prerequisites/)

## Getting started

```bash
# Install frontend dependencies
pnpm install

# Run in development mode (hot-reload for both frontend and Rust)
pnpm tauri dev

# Build a release binary
pnpm tauri build
```

## Project structure

```
src/                    React + TypeScript frontend
  api/commands.ts       All Tauri invoke() wrappers and TypeScript types
  pages/                One file per page (Scan, Gallery, Duplicates, Jobs, Settings)
  store/                Zustand stores for scan and job progress
src-tauri/              Rust backend
  src/
    commands/           Tauri command handlers (scan, groups, images, jobs, settings)
    db/                 SQLite models, queries, and migrations
    grouper/            Date bucketing and duplicate clustering logic
    scanner/            Folder walking, EXIF extraction, perceptual hashing
    processor/          Image compression/resize and job runner
    settings/           App settings persistence
```

## How it works

1. **Scan** — walkdir collects image paths, then a rayon thread pool decodes each image (JPEG fast path via `zune-jpeg`) to extract dimensions and a perceptual hash. Results are upserted into SQLite.

2. **Group** — "Rebuild Groups" buckets scanned images by their `taken_at` timestamp into day/month/year groups. "Find Duplicates" runs union-find clustering on perceptual hash hamming distances.

3. **Process** — select images or a group, configure compress/resize params, create a job. Jobs run in a background rayon thread pool with per-image progress events sent to the frontend.

## Data storage

| Path | Contents |
|------|----------|
| `~/.local/share/com.gallery-organizer.app/gallery.db` | SQLite database |
| `~/.local/share/com.gallery-organizer.app/thumbnails/` | Cached JPEG thumbnails |

Removing a group or excluding an image only removes database records — files on disk are never deleted or modified unless you run a processing job with "in-place" output mode.

## Development notes

See [`CLAUDE.md`](CLAUDE.md) for architecture details, query cache workflow, and a guide to adding new Tauri commands.
