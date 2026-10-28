// Sembunyikan jendela konsol pada build release di Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! YouTube Data Extractor
//!
//! Mengekstrak file ZIP backup NewPipe / PipePipe lalu menampilkan isinya
//! (riwayat tonton, pencarian, langganan, playlist, dll.) dalam tabel yang
//! dikelompokkan per kategori.
//!
//! Pemakaian:
//!   youtube_data_extractor [path/ke/backup.zip]
//! Tanpa argumen, aplikasi mencari file ZIP PipePipe/NewPipe di folder kerja.

mod app;
mod export;
mod extract;
mod loader;
mod model;
mod table_view;

use std::path::PathBuf;

fn main() -> eframe::Result {
    let mut args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let check = args.iter().position(|a| a == "--check").map(|i| args.remove(i)).is_some();

    let initial = args
        .into_iter()
        .next()
        .map(PathBuf::from)
        .or_else(extract::find_default_zip);

    if check {
        run_check(initial);
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("YouTube Data Extractor")
            .with_inner_size([1440.0, 880.0])
            .with_min_inner_size([960.0, 600.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "YouTube Data Extractor",
        options,
        Box::new(move |cc| Ok(Box::new(app::YtApp::new(cc, initial)))),
    )
}

/// Mode verifikasi tanpa GUI: `cargo run -- --check [file.zip]`.
/// (Pada build release Windows, output konsol tidak terlihat karena subsystem GUI.)
fn run_check(zip: Option<PathBuf>) {
    let Some(zip) = zip else {
        eprintln!("Tidak ada file ZIP. Pemakaian: youtube_data_extractor --check <file.zip>");
        std::process::exit(2);
    };
    let t0 = std::time::Instant::now();
    match loader::load(&zip) {
        Ok(ds) => {
            println!("ZIP      : {}", ds.zip_path.display());
            println!("Ekstrak  : {}", ds.extract_dir.display());
            println!("Waktu    : {:.2?}\n", t0.elapsed());
            for t in &ds.tables {
                println!(
                    "{:<20} {:>8} baris  | {} sub-kategori ({})",
                    t.title,
                    model::fmt_thousands(t.rows.len() as i64),
                    t.categories.len(),
                    t.category_label.unwrap_or("-")
                );
                if let Some(row) = t.rows.first() {
                    let sample: Vec<String> = t
                        .columns
                        .iter()
                        .zip(row)
                        .map(|(c, v)| format!("{}={}", c.name, v.text.chars().take(40).collect::<String>()))
                        .collect();
                    println!("    contoh: {}", sample.join(" | "));
                }
            }
            let s = &ds.summary;
            println!(
                "\nRingkasan: {} video ditonton, {} putar, estimasi {}",
                model::fmt_thousands(s.watched_videos as i64),
                model::fmt_thousands(s.total_plays),
                model::fmt_long_duration(s.est_watch_secs)
            );
            println!("Top channel: {:?}", s.top_channels.iter().take(5).collect::<Vec<_>>());
        }
        Err(e) => {
            eprintln!("Gagal: {e:#}");
            std::process::exit(1);
        }
    }
}
