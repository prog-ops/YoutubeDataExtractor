//! Extraction of NewPipe / PipePipe backup ZIP files.

use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use zip::ZipArchive;

/// Extraction result details.
pub struct Extracted {
    /// Target extraction folder.
    pub dir: PathBuf,
    /// Location of `newpipe.db` if found.
    pub db_path: Option<PathBuf>,
    /// List of extracted files (relative path, byte size).
    pub files: Vec<(String, u64)>,
}

/// Extracts all contents of the ZIP archive into `<zip_stem>_extracted` adjacent to the file.
/// If that directory is not writable, extraction falls back to the system temporary directory.
pub fn extract_zip(zip_path: &Path) -> Result<Extracted> {
    let file = File::open(zip_path)
        .with_context(|| format!("Failed to open file {}", zip_path.display()))?;
    let mut archive = ZipArchive::new(BufReader::new(file))
        .context("The specified file is not a valid ZIP archive")?;

    let stem = zip_path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "backup".into());
    let dir_name = format!("{stem}_extracted");

    let preferred = zip_path.parent().unwrap_or(Path::new(".")).join(&dir_name);
    let dir = match fs::create_dir_all(&preferred) {
        Ok(()) => preferred,
        Err(_) => {
            let tmp = std::env::temp_dir().join("youtube_data_extractor").join(&dir_name);
            fs::create_dir_all(&tmp)
                .with_context(|| format!("Failed to create folder {}", tmp.display()))?;
            tmp
        }
    };

    let mut files = Vec::new();
    let mut db_path = None;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).context("Corrupted ZIP entry")?;
        // `enclosed_name` protects against path traversal attacks (e.g. "../../file").
        let Some(rel) = entry.enclosed_name() else { continue };
        let out = dir.join(&rel);

        if entry.is_dir() {
            fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut f = File::create(&out)
            .with_context(|| format!("Failed to write {}", out.display()))?;
        io::copy(&mut entry, &mut f)
            .with_context(|| format!("Failed to extract {}", rel.display()))?;

        let is_db = rel
            .file_name()
            .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("newpipe.db"));
        if is_db {
            db_path = Some(out.clone());
        }
        files.push((rel.display().to_string(), entry.size()));
    }

    Ok(Extracted { dir, db_path, files })
}

/// Automatically searches for a backup ZIP file in the current working directory and executable directory.
pub fn find_default_zip() -> Option<PathBuf> {
    let mut dirs = vec![];
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd);
    }
    if let Some(exe_dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)) {
        dirs.push(exe_dir);
    }

    let mut any_zip = None;
    for dir in dirs {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        let mut zips: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")))
            .collect();
        zips.sort();
        // Prioritize the newest file whose filename contains PipePipe or NewPipe.
        if let Some(p) = zips.iter().rev().find(|p| {
            let n = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
            n.contains("pipepipe") || n.contains("newpipe")
        }) {
            return Some(p.clone());
        }
        if any_zip.is_none() {
            any_zip = zips.pop();
        }
    }
    any_zip
}
