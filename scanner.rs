use std::path::{Path, PathBuf};

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::Accessor;
use rusqlite::Connection;

use crate::library::{self, NewTrack};

/// Return the raw bytes of a track's embedded cover art, if any.
pub fn read_cover(path: &Path) -> Option<Vec<u8>> {
    let tagged = lofty::read_from_path(path).ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    tag.pictures().first().map(|p| p.data().to_vec())
}

/// Extensions we hand to lofty. playbin can decode more, but these cover the
/// common lossy/lossless formats and keep us from probing random files.
const AUDIO_EXTS: &[&str] = &[
    "mp3", "flac", "ogg", "opus", "m4a", "aac", "wav", "wma", "aiff",
];

/// Recursively walk `dir`, read tags for every audio file, and upsert them into
/// the library. Returns how many files were inserted or updated.
pub fn scan_dir(conn: &Connection, dir: &Path) -> usize {
    let mut count = 0;
    let mut stack: Vec<PathBuf> = vec![dir.to_path_buf()];

    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if is_audio(&path) {
                if let Some(track) = read_track(&path) {
                    if library::insert_track(conn, &track).is_ok() {
                        count += 1;
                    }
                }
            }
        }
    }
    count
}

fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Read metadata for a single file, falling back to the file name when a tag is
/// missing so nothing shows up blank.
fn read_track(path: &Path) -> Option<NewTrack> {
    let tagged = lofty::read_from_path(path).ok()?;
    let duration = tagged.properties().duration().as_secs() as i64;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());

    let title = tag
        .and_then(|t| t.title())
        .map(|c| c.to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Unknown".to_owned())
        });
    let artist = tag
        .and_then(|t| t.artist())
        .map(|c| c.to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown Artist".to_owned());
    let album = tag
        .and_then(|t| t.album())
        .map(|c| c.to_string())
        .unwrap_or_default();

    Some(NewTrack {
        path: path.to_string_lossy().into_owned(),
        title,
        artist,
        album,
        duration,
    })
}
