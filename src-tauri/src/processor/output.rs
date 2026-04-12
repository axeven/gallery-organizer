use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub enum OutputMode {
    OutputFolder { dir: PathBuf, scan_root: PathBuf },
    InPlace,
}

pub fn write(
    source_path: &Path,
    data: &[u8],
    ext: &str,
    mode: &OutputMode,
) -> Result<PathBuf> {
    match mode {
        OutputMode::InPlace => write_in_place(source_path, data, ext),
        OutputMode::OutputFolder { dir, scan_root } => {
            write_to_folder(source_path, data, ext, dir, scan_root)
        }
    }
}

fn write_in_place(source_path: &Path, data: &[u8], ext: &str) -> Result<PathBuf> {
    let parent = source_path.parent().unwrap_or(Path::new("."));
    let stem = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");

    // Write to temp file first, then atomic rename
    let tmp = tempfile::Builder::new()
        .prefix(".gallery-tmp-")
        .suffix(&format!(".{ext}"))
        .tempfile_in(parent)
        .context("failed to create temp file")?;

    let tmp_path = tmp.path().to_path_buf();
    std::fs::write(&tmp_path, data).context("failed to write temp file")?;

    let out_path = parent.join(format!("{stem}.{ext}"));
    std::fs::rename(&tmp_path, &out_path).context("failed to rename temp file")?;

    // tmp is now closed (renamed), keep file by forgetting the guard
    std::mem::forget(tmp);

    Ok(out_path)
}

fn write_to_folder(
    source_path: &Path,
    data: &[u8],
    ext: &str,
    out_dir: &Path,
    scan_root: &Path,
) -> Result<PathBuf> {
    // Preserve relative path structure
    let rel = source_path
        .strip_prefix(scan_root)
        .unwrap_or(source_path.file_name().map(Path::new).unwrap_or(source_path));

    let stem = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");

    let out_path = out_dir.join(rel.parent().unwrap_or(Path::new("")))
        .join(format!("{stem}.{ext}"));

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).context("failed to create output dir")?;
    }

    std::fs::write(&out_path, data).context("failed to write output file")?;

    Ok(out_path)
}
