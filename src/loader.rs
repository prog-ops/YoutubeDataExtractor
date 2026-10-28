//! Membaca `newpipe.db` (format NewPipe / PipePipe) dan mengubahnya menjadi
//! tabel-tabel per kategori + ringkasan statistik.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags, Row};

use crate::extract;
use crate::model::*;

/// Ringkasan statistik untuk halaman "Ringkasan".
#[derive(Default)]
pub struct Summary {
    pub watched_videos: usize,
    pub total_plays: i64,
    pub est_watch_secs: i64,
    pub first_watch: Option<i64>,
    pub last_watch: Option<i64>,
    pub searches: usize,
    pub unique_searches: usize,
    pub subscriptions: usize,
    pub playlists: usize,
    pub playlist_items: usize,
    pub remote_playlists: usize,
    pub known_streams: usize,
    pub top_channels: Vec<(String, i64)>,
    pub top_searches: Vec<(String, i64)>,
    pub plays_per_year: Vec<(String, i64)>,
    pub plays_per_hour: Vec<(String, i64)>,
}

/// Semua data hasil pembacaan backup.
pub struct Dataset {
    pub zip_path: PathBuf,
    pub extract_dir: PathBuf,
    pub files: Vec<(String, u64)>,
    pub summary: Summary,
    pub tables: Vec<Table>,
}

/// Ekstrak ZIP lalu baca database-nya.
pub fn load(zip_path: &Path) -> Result<Dataset> {
    let zip_path = std::path::absolute(zip_path).unwrap_or_else(|_| zip_path.to_path_buf());
    let zip_path = zip_path.as_path();
    let ex = extract::extract_zip(zip_path)?;
    let db = ex
        .db_path
        .clone()
        .context("File `newpipe.db` tidak ditemukan di dalam ZIP. Pastikan ini backup NewPipe/PipePipe.")?;

    let conn = Connection::open_with_flags(
        &db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("Gagal membuka database {}", db.display()))?;

    let mut summary = Summary::default();
    let mut tables = Vec::new();

    tables.push(load_watch_history(&conn, &mut summary)?);
    tables.push(load_search_history(&conn, &mut summary)?);
    tables.push(load_subscriptions(&conn, &mut summary)?);
    if has_table(&conn, "feed") {
        tables.push(load_feed(&conn)?);
    }
    tables.push(load_local_playlists(&conn, &mut summary)?);
    if has_table(&conn, "remote_playlists") {
        tables.push(load_remote_playlists(&conn, &mut summary)?);
    }
    tables.push(load_streams(&conn, &mut summary)?);

    Ok(Dataset {
        zip_path: zip_path.to_path_buf(),
        extract_dir: ex.dir,
        files: ex.files,
        summary,
        tables,
    })
}

// ---------------------------------------------------------------------------
// Helper SQL yang toleran terhadap tipe data (SQLite bertipe dinamis)
// ---------------------------------------------------------------------------

fn has_table(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
        [name],
        |_| Ok(()),
    )
    .is_ok()
}

fn int(row: &Row, i: usize) -> Option<i64> {
    match row.get_ref(i).ok()? {
        ValueRef::Integer(n) => Some(n),
        ValueRef::Real(f) => Some(f as i64),
        ValueRef::Text(t) => std::str::from_utf8(t).ok()?.trim().parse().ok(),
        _ => None,
    }
}

fn text(row: &Row, i: usize) -> String {
    match row.get_ref(i) {
        Ok(ValueRef::Text(t)) => String::from_utf8_lossy(t).into_owned(),
        Ok(ValueRef::Integer(n)) => n.to_string(),
        Ok(ValueRef::Real(f)) => f.to_string(),
        _ => String::new(),
    }
}

fn opt_text(row: &Row, i: usize) -> Option<String> {
    Some(text(row, i)).filter(|s| !s.is_empty())
}

fn for_each_row(conn: &Connection, sql: &str, mut f: impl FnMut(&Row)) -> Result<()> {
    let mut stmt = conn.prepare(sql).with_context(|| format!("SQL gagal: {sql}"))?;
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        f(r);
    }
    Ok(())
}

fn year_cat(ms: Option<i64>) -> Vec<String> {
    vec![ms.and_then(year_of).map(|y| y.to_string()).unwrap_or_else(|| "Tidak diketahui".into())]
}

fn top_n(map: HashMap<String, i64>, n: usize) -> Vec<(String, i64)> {
    let mut v: Vec<_> = map.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v.truncate(n);
    v
}

// ---------------------------------------------------------------------------
// Kategori: Riwayat Tonton
// ---------------------------------------------------------------------------

fn load_watch_history(conn: &Connection, sum: &mut Summary) -> Result<Table> {
    let mut t = Table::new(
        "history",
        "▶",
        "Riwayat Tonton",
        "Setiap video yang pernah diputar, lengkap dengan waktu, jumlah putar, dan posisi terakhir.",
        vec![
            Column::text("Waktu Tonton", 125.0),
            Column::text("Judul", 380.0),
            Column::text("Channel", 170.0),
            Column::num("Durasi", 70.0),
            Column::num("Diputar", 60.0),
            Column::num("Posisi Terakhir", 115.0),
            Column::text("Tipe", 85.0),
            Column::text("Layanan", 85.0),
        ],
    )
    .with_categories("Tahun", CatOrder::NameDesc);

    let mut per_channel: HashMap<String, i64> = HashMap::new();
    let mut per_year: HashMap<String, i64> = HashMap::new();
    let mut per_hour = [0i64; 24];
    let mut videos: HashSet<i64> = HashSet::new();

    for_each_row(
        conn,
        "SELECT h.access_date, s.title, s.uploader, s.duration, h.repeat_count,
                st.progress_time, s.stream_type, s.service_id, s.url, s.uploader_url, h.stream_id
         FROM stream_history h
         JOIN streams s ON s.uid = h.stream_id
         LEFT JOIN stream_state st ON st.stream_id = h.stream_id
         ORDER BY h.access_date DESC",
        |r| {
            let date = int(r, 0);
            let uploader = text(r, 2);
            let duration = int(r, 3).unwrap_or(0);
            let plays = int(r, 4).unwrap_or(1).max(1);

            // Statistik
            videos.insert(int(r, 10).unwrap_or_default());
            sum.total_plays += plays;
            sum.est_watch_secs += duration.max(0) * plays;
            if let Some(d) = date {
                sum.first_watch = Some(sum.first_watch.map_or(d, |f| f.min(d)));
                sum.last_watch = Some(sum.last_watch.map_or(d, |l| l.max(d)));
                if let Some(h) = hour_of(d) {
                    per_hour[h as usize] += plays;
                }
            }
            if !uploader.is_empty() {
                *per_channel.entry(uploader.clone()).or_default() += plays;
            }
            let cats = year_cat(date);
            *per_year.entry(cats[0].clone()).or_default() += plays;

            t.push(
                vec![
                    Cell::datetime_ms(date),
                    Cell::link(text(r, 1), opt_text(r, 8)),
                    Cell::link(uploader, opt_text(r, 9)),
                    Cell::duration_secs(Some(duration)),
                    Cell::int(Some(plays)),
                    Cell::progress(int(r, 5), duration),
                    Cell::text(stream_type_label(&text(r, 6))),
                    Cell::text(service_label(int(r, 7).unwrap_or(0))),
                ],
                cats,
            );
        },
    )?;

    sum.watched_videos = videos.len();
    sum.top_channels = top_n(per_channel, 15);
    let mut years: Vec<_> = per_year.into_iter().collect();
    years.sort();
    sum.plays_per_year = years;
    sum.plays_per_hour = per_hour
        .iter()
        .enumerate()
        .map(|(h, n)| (format!("{h:02}"), *n))
        .collect();

    Ok(t.finish())
}

// ---------------------------------------------------------------------------
// Kategori: Riwayat Pencarian
// ---------------------------------------------------------------------------

fn load_search_history(conn: &Connection, sum: &mut Summary) -> Result<Table> {
    // Kumpulkan dulu agar bisa menghitung berapa kali kata kunci yang sama dicari.
    let mut raw: Vec<(Option<i64>, String, i64)> = Vec::new();
    for_each_row(
        conn,
        "SELECT creation_date, search, service_id FROM search_history ORDER BY creation_date DESC",
        |r| raw.push((int(r, 0), text(r, 1), int(r, 2).unwrap_or(0))),
    )?;

    let mut freq: HashMap<String, i64> = HashMap::new();
    let mut display: HashMap<String, String> = HashMap::new();
    for (_, q, _) in &raw {
        let k = q.trim().to_lowercase();
        *freq.entry(k.clone()).or_default() += 1;
        display.entry(k).or_insert_with(|| q.trim().to_string());
    }

    let mut t = Table::new(
        "search",
        "🔍",
        "Riwayat Pencarian",
        "Semua kata kunci yang pernah dicari, beserta frekuensi pencarian kata kunci yang sama.",
        vec![
            Column::text("Waktu", 125.0),
            Column::text("Kata Kunci", 480.0),
            Column::num("Total Dicari", 90.0),
            Column::text("Layanan", 90.0),
        ],
    )
    .with_categories("Tahun", CatOrder::NameDesc);

    for (date, q, svc) in &raw {
        let n = freq.get(&q.trim().to_lowercase()).copied().unwrap_or(1);
        let url = format!(
            "https://www.youtube.com/results?search_query={}",
            url_encode(q.trim())
        );
        let url = (*svc == 0).then_some(url);
        t.push(
            vec![
                Cell::datetime_ms(*date),
                Cell::link(q.clone(), url),
                Cell::int(Some(n)),
                Cell::text(service_label(*svc)),
            ],
            year_cat(*date),
        );
    }

    sum.searches = raw.len();
    sum.unique_searches = freq.len();
    let named: HashMap<String, i64> = freq
        .into_iter()
        .map(|(k, v)| (display.remove(&k).unwrap_or(k), v))
        .collect();
    sum.top_searches = top_n(named, 15);

    Ok(t.finish())
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Kategori: Langganan (dikelompokkan berdasarkan Grup Feed)
// ---------------------------------------------------------------------------

fn load_subscriptions(conn: &Connection, sum: &mut Summary) -> Result<Table> {
    // Jumlah putar per channel (berdasarkan URL channel, fallback nama).
    let mut plays_by_url: HashMap<String, i64> = HashMap::new();
    let mut plays_by_name: HashMap<String, i64> = HashMap::new();
    for_each_row(
        conn,
        "SELECT s.uploader_url, s.uploader, SUM(h.repeat_count)
         FROM stream_history h JOIN streams s ON s.uid = h.stream_id
         GROUP BY s.uploader_url, s.uploader",
        |r| {
            let n = int(r, 2).unwrap_or(0);
            if let Some(u) = opt_text(r, 0) {
                *plays_by_url.entry(u).or_default() += n;
            }
            *plays_by_name.entry(text(r, 1)).or_default() += n;
        },
    )?;

    // Grup feed per langganan.
    let mut groups: HashMap<i64, Vec<String>> = HashMap::new();
    if has_table(conn, "feed_group") && has_table(conn, "feed_group_subscription_join") {
        for_each_row(
            conn,
            "SELECT j.subscription_id, g.name FROM feed_group_subscription_join j
             JOIN feed_group g ON g.uid = j.group_id ORDER BY g.sort_order",
            |r| {
                groups
                    .entry(int(r, 0).unwrap_or_default())
                    .or_default()
                    .push(text(r, 1).trim().to_string())
            },
        )?;
    }

    let mut last_updated: HashMap<i64, i64> = HashMap::new();
    if has_table(conn, "feed_last_updated") {
        for_each_row(conn, "SELECT subscription_id, last_updated FROM feed_last_updated", |r| {
            if let (Some(id), Some(ts)) = (int(r, 0), int(r, 1)) {
                last_updated.insert(id, ts);
            }
        })?;
    }

    let mut t = Table::new(
        "subs",
        "⭐",
        "Langganan",
        "Channel yang di-subscribe. Gunakan filter Grup Feed untuk melihat channel per grup.",
        vec![
            Column::text("Channel", 230.0),
            Column::num("Subscriber", 100.0),
            Column::text("Grup Feed", 170.0),
            Column::text("Notifikasi", 80.0),
            Column::num("Video Diputar", 100.0),
            Column::text("Feed Diperbarui", 125.0),
            Column::text("Layanan", 80.0),
            Column::text("Deskripsi", 420.0),
        ],
    )
    .with_categories("Grup Feed", CatOrder::NameAsc);

    for_each_row(
        conn,
        "SELECT uid, name, subscriber_count, notification_mode, url, description, service_id
         FROM subscriptions ORDER BY TRIM(name) COLLATE NOCASE",
        |r| {
            let uid = int(r, 0).unwrap_or_default();
            let name = text(r, 1).trim().to_string();
            let url = opt_text(r, 4);
            let plays = url
                .as_ref()
                .and_then(|u| plays_by_url.get(u).copied())
                .or_else(|| plays_by_name.get(&name).copied())
                .or_else(|| plays_by_name.get(&text(r, 1)).copied())
                .unwrap_or(0);
            let g = groups.get(&uid).cloned().unwrap_or_default();
            let desc = text(r, 5).replace(['\n', '\r'], " ");
            let cats = if g.is_empty() { vec!["(Tanpa grup)".to_string()] } else { g.clone() };

            t.push(
                vec![
                    Cell::link(name, url),
                    Cell::int(int(r, 2)),
                    Cell::text(if g.is_empty() { "–".to_string() } else { g.join(", ") }),
                    Cell::text(if int(r, 3).unwrap_or(0) != 0 { "Aktif" } else { "Mati" }),
                    Cell::int(Some(plays)),
                    Cell::datetime_ms(last_updated.get(&uid).copied()),
                    Cell::text(service_label(int(r, 6).unwrap_or(0))),
                    Cell::text(desc),
                ],
                cats,
            );
        },
    )?;

    sum.subscriptions = t.rows.len();
    Ok(t.finish())
}

// ---------------------------------------------------------------------------
// Kategori: Feed langganan
// ---------------------------------------------------------------------------

fn load_feed(conn: &Connection) -> Result<Table> {
    let mut t = Table::new(
        "feed",
        "📰",
        "Feed Langganan",
        "Video terbaru dari channel langganan yang tersimpan di cache feed.",
        vec![
            Column::text("Tanggal Upload", 110.0),
            Column::text("Judul", 400.0),
            Column::text("Channel", 190.0),
            Column::num("Durasi", 70.0),
            Column::num("Penayangan", 100.0),
            Column::text("Tipe", 85.0),
        ],
    )
    .with_categories("Channel", CatOrder::NameAsc);

    for_each_row(
        conn,
        "SELECT s.upload_date, s.title, sub.name, s.duration, s.view_count, s.stream_type, s.url, sub.url
         FROM feed f
         JOIN streams s ON s.uid = f.stream_id
         JOIN subscriptions sub ON sub.uid = f.subscription_id
         ORDER BY s.upload_date DESC",
        |r| {
            let channel = text(r, 2).trim().to_string();
            t.push(
                vec![
                    Cell::date_ms(int(r, 0)),
                    Cell::link(text(r, 1), opt_text(r, 6)),
                    Cell::link(channel.clone(), opt_text(r, 7)),
                    Cell::duration_secs(int(r, 3)),
                    Cell::int(int(r, 4)),
                    Cell::text(stream_type_label(&text(r, 5))),
                ],
                vec![channel],
            );
        },
    )?;
    Ok(t.finish())
}

// ---------------------------------------------------------------------------
// Kategori: Playlist Lokal (dikelompokkan per playlist)
// ---------------------------------------------------------------------------

fn load_local_playlists(conn: &Connection, sum: &mut Summary) -> Result<Table> {
    let mut t = Table::new(
        "playlists",
        "📂",
        "Playlist Lokal",
        "Isi playlist buatan sendiri. Pilih playlist di filter untuk melihat isinya saja.",
        vec![
            Column::text("Playlist", 210.0),
            Column::num("No.", 50.0),
            Column::text("Judul", 400.0),
            Column::text("Channel", 180.0),
            Column::num("Durasi", 70.0),
            Column::text("Tipe", 85.0),
        ],
    )
    .with_categories("Playlist", CatOrder::FirstSeen);

    for_each_row(
        conn,
        "SELECT p.name, j.join_index, s.title, s.uploader, s.duration, s.stream_type, s.url, s.uploader_url
         FROM playlist_stream_join j
         JOIN playlists p ON p.uid = j.playlist_id
         JOIN streams s ON s.uid = j.stream_id
         ORDER BY p.display_index, p.uid, j.join_index",
        |r| {
            let pl = text(r, 0);
            t.push(
                vec![
                    Cell::text(pl.clone()),
                    Cell::int(int(r, 1).map(|i| i + 1)),
                    Cell::link(text(r, 2), opt_text(r, 6)),
                    Cell::link(text(r, 3), opt_text(r, 7)),
                    Cell::duration_secs(int(r, 4)),
                    Cell::text(stream_type_label(&text(r, 5))),
                ],
                vec![pl],
            );
        },
    )?;

    sum.playlists = conn
        .query_row("SELECT COUNT(*) FROM playlists", [], |r| r.get::<_, i64>(0))
        .unwrap_or(0) as usize;
    sum.playlist_items = t.rows.len();
    Ok(t.finish())
}

// ---------------------------------------------------------------------------
// Kategori: Playlist Tersimpan (remote)
// ---------------------------------------------------------------------------

fn load_remote_playlists(conn: &Connection, sum: &mut Summary) -> Result<Table> {
    let mut t = Table::new(
        "remote",
        "🔖",
        "Playlist Tersimpan",
        "Playlist milik channel lain yang di-bookmark.",
        vec![
            Column::text("Nama Playlist", 380.0),
            Column::text("Pembuat", 200.0),
            Column::num("Jumlah Video", 100.0),
            Column::text("Layanan", 90.0),
        ],
    )
    .with_categories("Pembuat", CatOrder::NameAsc);

    for_each_row(
        conn,
        "SELECT name, uploader, stream_count, service_id, url FROM remote_playlists ORDER BY display_index",
        |r| {
            let up = text(r, 1).trim().to_string();
            t.push(
                vec![
                    Cell::link(text(r, 0), opt_text(r, 4)),
                    Cell::text(up.clone()),
                    Cell::int(int(r, 2)),
                    Cell::text(service_label(int(r, 3).unwrap_or(0))),
                ],
                vec![if up.is_empty() { "(Tidak diketahui)".into() } else { up }],
            );
        },
    )?;
    sum.remote_playlists = t.rows.len();
    Ok(t.finish())
}

// ---------------------------------------------------------------------------
// Kategori: Semua Video (dikelompokkan per tipe stream)
// ---------------------------------------------------------------------------

fn load_streams(conn: &Connection, sum: &mut Summary) -> Result<Table> {
    let mut t = Table::new(
        "streams",
        "🎬",
        "Semua Video",
        "Seluruh video/stream yang dikenal database (dari riwayat, playlist, dan feed).",
        vec![
            Column::text("Judul", 380.0),
            Column::text("Channel", 180.0),
            Column::text("Tipe", 95.0),
            Column::num("Durasi", 70.0),
            Column::num("Penayangan", 105.0),
            Column::text("Tanggal Upload", 110.0),
            Column::num("Total Diputar", 95.0),
            Column::text("Terakhir Ditonton", 125.0),
            Column::text("Layanan", 90.0),
        ],
    )
    .with_categories("Tipe", CatOrder::CountDesc);

    for_each_row(
        conn,
        "SELECT s.title, s.uploader, s.stream_type, s.duration, s.view_count, s.upload_date,
                COALESCE(h.plays, 0), h.last, s.service_id, s.url, s.uploader_url
         FROM streams s
         LEFT JOIN (SELECT stream_id, SUM(repeat_count) AS plays, MAX(access_date) AS last
                    FROM stream_history GROUP BY stream_id) h ON h.stream_id = s.uid
         ORDER BY h.last DESC, s.uid DESC",
        |r| {
            let ty = stream_type_label(&text(r, 2));
            t.push(
                vec![
                    Cell::link(text(r, 0), opt_text(r, 9)),
                    Cell::link(text(r, 1), opt_text(r, 10)),
                    Cell::text(ty),
                    Cell::duration_secs(int(r, 3)),
                    Cell::int(int(r, 4)),
                    Cell::date_ms(int(r, 5)),
                    Cell::int(int(r, 6)),
                    Cell::datetime_ms(int(r, 7)),
                    Cell::text(service_label(int(r, 8).unwrap_or(0))),
                ],
                vec![ty.to_string()],
            );
        },
    )?;
    sum.known_streams = t.rows.len();
    Ok(t.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode() {
        assert_eq!(url_encode("rust lang"), "rust+lang");
        assert_eq!(url_encode("a&b"), "a%26b");
    }
}
