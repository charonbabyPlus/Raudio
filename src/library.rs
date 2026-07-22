use rusqlite::{Connection, Result};

/// A track as stored in the library.
#[derive(Debug, Clone)]
pub struct Track {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: i64, // seconds
    pub liked: bool,
}

/// A track freshly read from disk, before it has a database id.
pub struct NewTrack {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: i64,
}

/// A user playlist plus how many tracks it holds.
#[derive(Debug, Clone)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub count: i64,
    pub image: Option<String>,
}

/// Open (creating if needed) the library database and ensure the schema exists.
///
/// The "liked" state lives as a boolean flag on the track — a like is just a
/// fast filter, while `playlists`/`playlist_tracks` cover user playlists.
pub fn open(path: &str) -> Result<Connection> {
    let conn = Connection::open(path)?;
    // Needed for the ON DELETE CASCADE on playlist_tracks to actually fire.
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.execute_batch(SCHEMA)?;
    // Migration for databases created before playlist cover art existed.
    let _ = conn.execute("ALTER TABLE playlists ADD COLUMN image TEXT", []);
    Ok(conn)
}

/// Read a persisted setting (e.g. the chosen theme or volume).
pub fn get_setting(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
        .ok()
}

/// Store a persisted setting.
pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (key, value),
    )?;
    Ok(())
}

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS tracks (
        id       INTEGER PRIMARY KEY,
        path     TEXT NOT NULL UNIQUE,
        title    TEXT NOT NULL DEFAULT '',
        artist   TEXT NOT NULL DEFAULT '',
        album    TEXT NOT NULL DEFAULT '',
        duration INTEGER NOT NULL DEFAULT 0,
        liked    INTEGER NOT NULL DEFAULT 0
    );

    CREATE TABLE IF NOT EXISTS playlists (
        id    INTEGER PRIMARY KEY,
        name  TEXT NOT NULL,
        image TEXT
    );

    CREATE TABLE IF NOT EXISTS playlist_tracks (
        playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
        track_id    INTEGER NOT NULL REFERENCES tracks(id)    ON DELETE CASCADE,
        position    INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (playlist_id, track_id)
    );

    CREATE TABLE IF NOT EXISTS settings (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
";

/// Insert a track, or refresh its tags if the path is already known.
/// The `liked` flag is deliberately left untouched on re-scan.
pub fn insert_track(conn: &Connection, t: &NewTrack) -> Result<()> {
    conn.execute(
        "INSERT INTO tracks (path, title, artist, album, duration)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(path) DO UPDATE SET
             title = excluded.title,
             artist = excluded.artist,
             album = excluded.album,
             duration = excluded.duration",
        (&t.path, &t.title, &t.artist, &t.album, t.duration),
    )?;
    Ok(())
}

/// Look up a track's id by its file path (used right after inserting).
pub fn track_id_by_path(conn: &Connection, path: &str) -> Option<i64> {
    conn.query_row("SELECT id FROM tracks WHERE path = ?1", [path], |r| r.get(0))
        .ok()
}

/// Delete a track from the library entirely. Its playlist memberships and liked
/// flag go with it (playlist rows cascade).
pub fn delete_track(conn: &Connection, track_id: i64) -> Result<()> {
    conn.execute("DELETE FROM tracks WHERE id = ?1", (track_id,))?;
    Ok(())
}

/// Every track in the library, grouped by artist then album.
pub fn all_tracks(conn: &Connection) -> Result<Vec<Track>> {
    let mut stmt = conn.prepare(
        "SELECT id, path, title, artist, album, duration, liked
         FROM tracks ORDER BY artist, album, title",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Track {
                id: r.get(0)?,
                path: r.get(1)?,
                title: r.get(2)?,
                artist: r.get(3)?,
                album: r.get(4)?,
                duration: r.get(5)?,
                liked: r.get::<_, i64>(6)? != 0,
            })
        })?
        .collect::<Result<Vec<_>>>()?;
    Ok(rows)
}

/// Create a new empty playlist and return its id.
pub fn create_playlist(conn: &Connection, name: &str) -> Result<i64> {
    conn.execute("INSERT INTO playlists (name) VALUES (?1)", (name,))?;
    Ok(conn.last_insert_rowid())
}

/// All playlists with their track counts, alphabetical.
pub fn all_playlists(conn: &Connection) -> Result<Vec<Playlist>> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, COUNT(pt.track_id), p.image
         FROM playlists p
         LEFT JOIN playlist_tracks pt ON pt.playlist_id = p.id
         GROUP BY p.id, p.name, p.image
         ORDER BY p.name COLLATE NOCASE",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Playlist {
                id: r.get(0)?,
                name: r.get(1)?,
                count: r.get(2)?,
                image: r.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?;
    Ok(rows)
}

/// Set (or clear) a playlist's cover image path.
pub fn set_playlist_image(conn: &Connection, playlist_id: i64, image: Option<&str>) -> Result<()> {
    conn.execute(
        "UPDATE playlists SET image = ?1 WHERE id = ?2",
        (image, playlist_id),
    )?;
    Ok(())
}

/// Append a track to a playlist. A duplicate (same track) is ignored.
pub fn add_to_playlist(conn: &Connection, playlist_id: i64, track_id: i64) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_id, position)
         VALUES (?1, ?2,
             COALESCE((SELECT MAX(position) + 1 FROM playlist_tracks WHERE playlist_id = ?1), 0))",
        (playlist_id, track_id),
    )?;
    Ok(())
}

/// Remove a track from a playlist.
pub fn remove_from_playlist(conn: &Connection, playlist_id: i64, track_id: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
        (playlist_id, track_id),
    )?;
    Ok(())
}

/// Delete a playlist (its membership rows cascade away).
pub fn delete_playlist(conn: &Connection, playlist_id: i64) -> Result<()> {
    conn.execute("DELETE FROM playlists WHERE id = ?1", (playlist_id,))?;
    Ok(())
}

/// Tracks in a playlist, in their stored order.
pub fn playlist_tracks(conn: &Connection, playlist_id: i64) -> Result<Vec<Track>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.path, t.title, t.artist, t.album, t.duration, t.liked
         FROM tracks t
         JOIN playlist_tracks pt ON pt.track_id = t.id
         WHERE pt.playlist_id = ?1
         ORDER BY pt.position",
    )?;
    let rows = stmt
        .query_map([playlist_id], |r| {
            Ok(Track {
                id: r.get(0)?,
                path: r.get(1)?,
                title: r.get(2)?,
                artist: r.get(3)?,
                album: r.get(4)?,
                duration: r.get(5)?,
                liked: r.get::<_, i64>(6)? != 0,
            })
        })?
        .collect::<Result<Vec<_>>>()?;
    Ok(rows)
}

/// Toggle a track's liked flag and return the new value.
pub fn set_liked(conn: &Connection, track_id: i64, liked: bool) -> Result<()> {
    conn.execute(
        "UPDATE tracks SET liked = ?1 WHERE id = ?2",
        (liked as i64, track_id),
    )?;
    Ok(())
}

/// All liked tracks, newest first.
pub fn liked_tracks(conn: &Connection) -> Result<Vec<Track>> {
    let mut stmt = conn.prepare(
        "SELECT id, path, title, artist, album, duration, liked
         FROM tracks WHERE liked = 1 ORDER BY id DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Track {
                id: r.get(0)?,
                path: r.get(1)?,
                title: r.get(2)?,
                artist: r.get(3)?,
                album: r.get(4)?,
                duration: r.get(5)?,
                liked: r.get::<_, i64>(6)? != 0,
            })
        })?
        .collect::<Result<Vec<_>>>()?;
    Ok(rows)
}
