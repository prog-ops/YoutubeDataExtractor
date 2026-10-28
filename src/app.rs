//! Aplikasi GUI utama (egui / eframe).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use egui::{
    Align, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Frame, Layout, Margin,
    RichText, Sense, Stroke, TextFormat, TextStyle, Ui, pos2, text::LayoutJob, vec2,
};

use crate::loader::{self, Dataset, Summary};
use crate::model::{fmt_date, fmt_long_duration, fmt_thousands};
use crate::table_view::{self, TableAction, TableState};

// Palet warna
const RED: Color32 = Color32::from_rgb(255, 82, 82);
const AMBER: Color32 = Color32::from_rgb(246, 173, 55);
const TEAL: Color32 = Color32::from_rgb(56, 201, 214);
const VIOLET: Color32 = Color32::from_rgb(160, 140, 255);
const GREEN: Color32 = Color32::from_rgb(92, 214, 140);
const PINK: Color32 = Color32::from_rgb(255, 121, 198);
const BLUE: Color32 = Color32::from_rgb(98, 160, 255);

enum State {
    Empty,
    Loading { path: PathBuf, rx: Receiver<Result<Dataset, String>> },
    Ready(Box<Dataset>),
    Error { path: Option<PathBuf>, msg: String },
}

/// Navigasi: 0 = Ringkasan, n = tabel ke-(n-1).
pub struct YtApp {
    state: State,
    nav: usize,
    table_states: Vec<TableState>,
    toast: Option<(String, bool, f64)>,
}

impl YtApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial: Option<PathBuf>) -> Self {
        setup_fonts(&cc.egui_ctx);
        setup_style(&cc.egui_ctx);
        let mut app = Self { state: State::Empty, nav: 0, table_states: Vec::new(), toast: None };
        if let Some(p) = initial {
            app.start_load(p, &cc.egui_ctx);
        }
        app
    }

    fn start_load(&mut self, path: PathBuf, ctx: &egui::Context) {
        let (tx, rx) = mpsc::channel();
        let ctx2 = ctx.clone();
        let p = path.clone();
        // Ekstraksi + baca DB di thread terpisah agar UI tetap responsif.
        std::thread::spawn(move || {
            let res = loader::load(&p).map_err(|e| format!("{e:#}"));
            let _ = tx.send(res);
            ctx2.request_repaint();
        });
        self.state = State::Loading { path, rx };
    }

    fn open_dialog(&mut self, ctx: &egui::Context) {
        if let Some(p) = rfd::FileDialog::new()
            .set_title("Pilih file backup NewPipe / PipePipe")
            .add_filter("Backup ZIP", &["zip"])
            .pick_file()
        {
            self.start_load(p, ctx);
        }
    }

    fn poll_loading(&mut self) {
        let res = match &self.state {
            State::Loading { rx, .. } => rx.try_recv(),
            _ => return,
        };
        match res {
            Ok(Ok(ds)) => {
                self.table_states = ds.tables.iter().map(TableState::new).collect();
                self.nav = 0;
                self.state = State::Ready(Box::new(ds));
            }
            Ok(Err(msg)) => {
                let path = match &self.state {
                    State::Loading { path, .. } => Some(path.clone()),
                    _ => None,
                };
                self.state = State::Error { path, msg };
            }
            Err(TryRecvError::Disconnected) => {
                self.state = State::Error { path: None, msg: "Proses pembacaan berhenti tiba-tiba.".into() };
            }
            Err(TryRecvError::Empty) => {}
        }
    }

    fn set_toast(&mut self, ctx: &egui::Context, msg: impl Into<String>, ok: bool) {
        let now = ctx.input(|i| i.time);
        self.toast = Some((msg.into(), ok, now + 6.0));
    }

    /// Lompat ke tabel tertentu dengan kata kunci pencarian terisi.
    fn jump_to(&mut self, table_id: &str, query: String) {
        let State::Ready(ds) = &self.state else { return };
        if let Some(idx) = ds.tables.iter().position(|t| t.id == table_id) {
            let st = &mut self.table_states[idx];
            st.query = query;
            st.category = None;
            st.refresh(&ds.tables[idx]);
            self.nav = idx + 1;
        }
    }

    // ------------------------------------------------------------------
    // Panel-panel
    // ------------------------------------------------------------------

    fn top_bar(&mut self, ui: &mut Ui) {
        let mut open = false;
        let mut reload = None;
        ui.horizontal(|ui| {
            ui.label(RichText::new("▶").size(24.0).color(RED));
            ui.label(RichText::new("YouTube Data Extractor").size(20.0).strong());
            ui.label(RichText::new("·  penampil backup NewPipe / PipePipe").weak());

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                egui::widgets::global_theme_preference_buttons(ui);
                ui.separator();
                if ui.button("📂  Buka ZIP…").on_hover_text("Ctrl+O").clicked() {
                    open = true;
                }
                if let State::Ready(ds) = &self.state {
                    if ui.button("⟳").on_hover_text("Muat ulang file ini").clicked() {
                        reload = Some(ds.zip_path.clone());
                    }
                    let name = ds.zip_path.file_name().unwrap_or_default().to_string_lossy();
                    ui.label(RichText::new(format!("🗄  {name}")).weak());
                }
            });
        });
        if open || ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            self.open_dialog(ui.ctx());
        }
        if let Some(p) = reload {
            self.start_load(p, ui.ctx());
        }
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        let now = ui.input(|i| i.time);
        ui.horizontal(|ui| {
            match &self.state {
                State::Ready(ds) => {
                    ui.label(RichText::new("📁 Diekstrak ke:").weak());
                    ui.label(RichText::new(ds.extract_dir.display().to_string()).monospace().small());
                    if ui.small_button("Buka folder").clicked() {
                        open_in_explorer(&ds.extract_dir);
                    }
                }
                State::Loading { .. } => {
                    ui.label(RichText::new("Sedang memproses…").weak());
                }
                _ => {
                    ui.label(RichText::new("Belum ada file yang dibuka").weak());
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some((msg, ok, until)) = &self.toast
                    && now < *until
                {
                    ui.label(RichText::new(msg).color(if *ok { GREEN } else { RED }).strong());
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
                }
            });
        });
        if self.toast.as_ref().is_some_and(|t| now >= t.2) {
            self.toast = None;
        }
    }

    fn nav_panel(&mut self, ui: &mut Ui) {
        let State::Ready(ds) = &self.state else { return };
        ui.add_space(10.0);
        ui.label(RichText::new("KATEGORI DATA").small().strong().color(ui.visuals().weak_text_color()));
        ui.add_space(4.0);

        let mut new_nav = None;
        ui.with_layout(Layout::top_down_justified(Align::LEFT), |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            if nav_item(ui, self.nav == 0, "📊", "Ringkasan", "Statistik & grafik").clicked() {
                new_nav = Some(0);
            }
            ui.add_space(4.0);
            for (i, t) in ds.tables.iter().enumerate() {
                let sub = match t.category_label {
                    Some(l) => format!(
                        "{} baris · {} {}",
                        fmt_thousands(t.rows.len() as i64),
                        t.categories.len(),
                        l.to_lowercase()
                    ),
                    None => format!("{} baris", fmt_thousands(t.rows.len() as i64)),
                };
                if nav_item(ui, self.nav == i + 1, t.icon, t.title, &sub).clicked() {
                    new_nav = Some(i + 1);
                }
            }
        });
        if let Some(n) = new_nav {
            self.nav = n;
        }

        ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
            ui.add_space(8.0);
            for (name, size) in ds.files.iter().rev() {
                ui.label(
                    RichText::new(format!("📄 {name}  ({})", fmt_bytes(*size)))
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
            }
            ui.label(RichText::new("ISI FILE ZIP").small().strong().color(ui.visuals().weak_text_color()));
            ui.separator();
        });
    }

    fn central(&mut self, ui: &mut Ui) {
        match &self.state {
            State::Empty => {
                if welcome(ui, None) {
                    self.open_dialog(ui.ctx());
                }
            }
            State::Error { path, msg } => {
                let retry = path.clone();
                let msg = msg.clone();
                let mut open = false;
                let mut again = false;
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.25);
                    ui.label(RichText::new("⚠").size(48.0).color(RED));
                    ui.label(RichText::new("Gagal membaca backup").size(22.0).strong());
                    ui.add_space(6.0);
                    ui.label(RichText::new(&msg).color(RED));
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        ui.add_space((ui.available_width() - 260.0).max(0.0) / 2.0);
                        open = ui.button("📂  Pilih file lain…").clicked();
                        if retry.is_some() {
                            again = ui.button("⟳  Coba lagi").clicked();
                        }
                    });
                });
                if open {
                    self.open_dialog(ui.ctx());
                } else if again && let Some(p) = retry {
                    self.start_load(p, ui.ctx());
                }
            }
            State::Loading { path, .. } => {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.35);
                    ui.add(egui::Spinner::new().size(42.0).color(RED));
                    ui.add_space(12.0);
                    ui.label(RichText::new("Mengekstrak ZIP & membaca database…").size(18.0).strong());
                    ui.label(RichText::new(path.display().to_string()).weak());
                });
            }
            State::Ready(_) => self.ready_view(ui),
        }
    }

    fn ready_view(&mut self, ui: &mut Ui) {
        let State::Ready(ds) = &self.state else { return };

        if self.nav == 0 {
            if let Some((id, q)) = summary_view(ui, ds) {
                self.jump_to(id, q);
            }
            return;
        }

        let idx = self.nav - 1;
        let table = &ds.tables[idx];
        let state = &mut self.table_states[idx];

        ui.horizontal(|ui| {
            ui.label(RichText::new(table.icon).size(24.0).color(RED));
            ui.heading(RichText::new(table.title).strong());
        });
        ui.label(RichText::new(&table.description).weak());
        ui.add_space(8.0);

        let action = table_view::show(ui, table, state, RED);

        if let TableAction::ExportCsv = action {
            let default_name = format!("{}.csv", table.title.to_lowercase().replace(' ', "_"));
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Simpan sebagai CSV")
                .set_file_name(&default_name)
                .add_filter("CSV", &["csv"])
                .save_file()
            {
                let n = state.visible.len();
                let res = crate::export::export_csv(&path, table, &state.visible);
                let ctx = ui.ctx().clone();
                match res {
                    Ok(()) => self.set_toast(&ctx, format!("✔ {} baris diekspor ke {}", fmt_thousands(n as i64), path.display()), true),
                    Err(e) => self.set_toast(&ctx, format!("✖ Gagal ekspor: {e:#}"), false),
                }
            }
        }
    }
}

impl eframe::App for YtApp {
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_loading();
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        // Drag & drop file ZIP ke jendela.
        let dropped = ui.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .find(|p| !p.as_os_str().is_empty())
        });
        if let Some(p) = dropped {
            self.start_load(p, ui.ctx());
        }
        let hovering_file = ui.input(|i| !i.raw.hovered_files.is_empty());

        let bar_fill = ui.visuals().extreme_bg_color;
        egui::Panel::top("top_bar")
            .frame(Frame::new().fill(bar_fill).inner_margin(Margin::symmetric(16, 10)))
            .show(ui, |ui| self.top_bar(ui));

        egui::Panel::bottom("status_bar")
            .frame(Frame::new().fill(bar_fill).inner_margin(Margin::symmetric(16, 5)))
            .show(ui, |ui| self.status_bar(ui));

        if matches!(self.state, State::Ready(_)) {
            egui::Panel::left("nav")
                .exact_size(250.0)
                .resizable(false)
                .frame(Frame::new().fill(ui.visuals().panel_fill).inner_margin(Margin::symmetric(10, 4)))
                .show(ui, |ui| self.nav_panel(ui));
        }

        egui::CentralPanel::default_margins().show(ui, |ui| self.central(ui));

        if hovering_file {
            let rect = ui.ctx().content_rect();
            let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("drop_overlay")));
            painter.rect_filled(rect, 0.0, Color32::from_black_alpha(190));
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "📥  Lepaskan file ZIP di sini",
                FontId::proportional(28.0),
                Color32::WHITE,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Widget kecil
// ---------------------------------------------------------------------------

fn nav_item(ui: &mut Ui, selected: bool, icon: &str, title: &str, sub: &str) -> egui::Response {
    let strong = if selected { Color32::WHITE } else { ui.visuals().strong_text_color() };
    let weak = if selected { Color32::from_white_alpha(190) } else { ui.visuals().weak_text_color() };
    let mut job = LayoutJob::default();
    job.append(
        &format!("{icon}   {title}\n"),
        0.0,
        TextFormat { font_id: FontId::proportional(15.0), color: strong, ..Default::default() },
    );
    job.append(
        sub,
        26.0,
        TextFormat { font_id: FontId::proportional(11.5), color: weak, ..Default::default() },
    );
    ui.selectable_label(selected, job)
}

fn welcome(ui: &mut Ui, _hint: Option<&str>) -> bool {
    let mut clicked = false;
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.22);
        ui.label(RichText::new("▶").size(72.0).color(RED));
        ui.label(RichText::new("YouTube Data Extractor").size(30.0).strong());
        ui.add_space(4.0);
        ui.label(
            RichText::new("Ekstrak dan jelajahi data backup NewPipe / PipePipe dalam tabel yang rapi.")
                .size(15.0)
                .weak(),
        );
        ui.add_space(22.0);
        clicked = ui
            .add(egui::Button::new(RichText::new("📂   Pilih file ZIP backup…").size(17.0)).min_size(vec2(260.0, 44.0)))
            .clicked();
        ui.add_space(10.0);
        ui.label(RichText::new("atau seret & lepas file .zip ke jendela ini").weak());
    });
    clicked
}

fn card_frame(ui: &Ui, accent: Color32) -> Frame {
    Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .stroke(Stroke::new(1.0, accent.gamma_multiply(0.45)))
        .corner_radius(10)
        .inner_margin(Margin::symmetric(16, 12))
}

fn stat_card(ui: &mut Ui, icon: &str, label: &str, value: &str, sub: &str, color: Color32) {
    card_frame(ui, color).show(ui, |ui| {
        ui.set_width(228.0);
        ui.set_min_height(74.0);
        ui.vertical(|ui| {
            ui.label(RichText::new(format!("{icon}  {label}")).size(13.0).color(ui.visuals().weak_text_color()));
            ui.label(RichText::new(value).size(24.0).strong().color(color));
            if !sub.is_empty() {
                ui.label(RichText::new(sub).small().color(ui.visuals().weak_text_color()));
            }
        });
    });
}

fn section<R>(ui: &mut Ui, title: &str, color: Color32, add: impl FnOnce(&mut Ui) -> R) -> R {
    card_frame(ui, color)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).size(16.0).strong());
            ui.add_space(6.0);
            add(ui)
        })
        .inner
}

/// Daftar batang horizontal; mengembalikan label yang diklik.
fn bar_list(ui: &mut Ui, items: &[(String, i64)], color: Color32) -> Option<String> {
    let max = items.iter().map(|x| x.1).max().unwrap_or(1).max(1) as f32;
    let mut clicked = None;
    let label_w = (ui.available_width() * 0.42).clamp(120.0, 300.0);
    for (name, v) in items {
        ui.horizontal(|ui| {
            let r = ui
                .allocate_ui_with_layout(vec2(label_w, 20.0), Layout::left_to_right(Align::Center), |ui| {
                    ui.set_width(label_w);
                    ui.add(egui::Label::new(name).truncate().sense(Sense::click()))
                })
                .inner
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Klik untuk melihat detail di tabel");
            if r.clicked() {
                clicked = Some(name.clone());
            }
            let w = (ui.available_width() - 70.0).max(20.0);
            let (rect, resp) = ui.allocate_exact_size(vec2(w, 14.0), Sense::click());
            let p = ui.painter();
            p.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
            let mut fill = rect;
            fill.set_width((rect.width() * (*v as f32 / max)).max(2.0));
            p.rect_filled(fill, 4.0, if resp.hovered() { color } else { color.gamma_multiply(0.8) });
            if resp.clicked() {
                clicked = Some(name.clone());
            }
            ui.label(RichText::new(fmt_thousands(*v)).strong());
        });
    }
    clicked
}

/// Grafik kolom vertikal sederhana.
fn column_chart(ui: &mut Ui, items: &[(String, i64)], color: Color32, height: f32, always_values: bool) {
    let max = items.iter().map(|x| x.1).max().unwrap_or(0).max(1) as f32;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height + 36.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let n = items.len().max(1) as f32;
    let slot = rect.width() / n;
    let bar_w = (slot * 0.68).min(56.0);
    let base_y = rect.bottom() - 18.0;
    let hover = resp.hover_pos();
    let weak = ui.visuals().weak_text_color();
    let strong = ui.visuals().strong_text_color();

    painter.line_segment(
        [pos2(rect.left(), base_y), pos2(rect.right(), base_y)],
        Stroke::new(1.0, weak.gamma_multiply(0.4)),
    );
    for (i, (label, v)) in items.iter().enumerate() {
        let cx = rect.left() + slot * (i as f32 + 0.5);
        let h = (*v as f32 / max) * (height - 4.0);
        let bar = egui::Rect::from_min_max(pos2(cx - bar_w / 2.0, base_y - h), pos2(cx + bar_w / 2.0, base_y));
        let hovered = hover.is_some_and(|p| (p.x - cx).abs() < slot / 2.0);
        painter.rect_filled(bar, 4.0, if hovered { color } else { color.gamma_multiply(0.7) });
        painter.text(pos2(cx, base_y + 4.0), Align2::CENTER_TOP, label, FontId::proportional(11.5), weak);
        if always_values || hovered {
            painter.text(
                pos2(cx, bar.top() - 3.0),
                Align2::CENTER_BOTTOM,
                fmt_thousands(*v),
                FontId::proportional(11.0),
                strong,
            );
        }
    }
}

/// Halaman ringkasan. Mengembalikan (id tabel, kata kunci) bila pengguna mengklik item.
fn summary_view(ui: &mut Ui, ds: &Dataset) -> Option<(&'static str, String)> {
    let s: &Summary = &ds.summary;
    let mut jump = None;

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("📊").size(24.0).color(RED));
            ui.heading(RichText::new("Ringkasan").strong());
        });
        let range = match (s.first_watch, s.last_watch) {
            (Some(a), Some(b)) => format!("Riwayat dari {} sampai {}", fmt_date(a), fmt_date(b)),
            _ => "Tidak ada riwayat tonton".into(),
        };
        ui.label(RichText::new(range).weak());
        ui.add_space(10.0);

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(12.0, 12.0);
            stat_card(ui, "▶", "Video Ditonton", &fmt_thousands(s.watched_videos as i64),
                &format!("{} kali diputar", fmt_thousands(s.total_plays)), RED);
            stat_card(ui, "⏱", "Total Durasi Video Diputar", &fmt_long_duration(s.est_watch_secs),
                "durasi × jumlah putar (batas atas)", AMBER);
            stat_card(ui, "🔍", "Pencarian", &fmt_thousands(s.searches as i64),
                &format!("{} kata kunci unik", fmt_thousands(s.unique_searches as i64)), TEAL);
            stat_card(ui, "⭐", "Langganan", &fmt_thousands(s.subscriptions as i64), "channel", VIOLET);
            stat_card(ui, "📂", "Playlist Lokal", &fmt_thousands(s.playlists as i64),
                &format!("{} video di dalamnya", fmt_thousands(s.playlist_items as i64)), GREEN);
            stat_card(ui, "🔖", "Playlist Tersimpan", &fmt_thousands(s.remote_playlists as i64), "dari channel lain", PINK);
            stat_card(ui, "🎬", "Video di Database", &fmt_thousands(s.known_streams as i64), "semua stream yang dikenal", BLUE);
        });
        ui.add_space(14.0);

        section(ui, "📅  Aktivitas tonton per tahun (jumlah putar)", RED, |ui| {
            column_chart(ui, &s.plays_per_year, RED, 150.0, true);
        });
        ui.add_space(12.0);

        ui.columns(2, |cols| {
            section(&mut cols[0], "📺  Channel paling sering ditonton", AMBER, |ui| {
                if let Some(c) = bar_list(ui, &s.top_channels, AMBER) {
                    jump = Some(("history", c));
                }
            });
            section(&mut cols[1], "🔍  Kata kunci paling sering dicari", TEAL, |ui| {
                if let Some(q) = bar_list(ui, &s.top_searches, TEAL) {
                    jump = Some(("search", q));
                }
            });
        });
        ui.add_space(12.0);

        section(ui, "🕒  Jam favorit menonton (arahkan kursor untuk melihat angka)", VIOLET, |ui| {
            column_chart(ui, &s.plays_per_hour, VIOLET, 120.0, false);
        });
        ui.add_space(12.0);
    });
    jump
}

fn fmt_bytes(b: u64) -> String {
    match b {
        b if b >= 1 << 20 => format!("{:.1} MB", b as f64 / (1u64 << 20) as f64),
        b if b >= 1 << 10 => format!("{:.1} KB", b as f64 / 1024.0),
        b => format!("{b} B"),
    }
}

fn open_in_explorer(path: &Path) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(path).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
}

// ---------------------------------------------------------------------------
// Tampilan & font
// ---------------------------------------------------------------------------

/// Pakai Segoe UI (Windows) sebagai font utama, plus font fallback untuk
/// simbol, emoji, dan aksara CJK agar judul video berbahasa asing tetap terbaca.
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    let fdir = Path::new(&windir).join("Fonts");

    let mut add = |name: &str, file: &str, primary: bool| {
        if let Ok(bytes) = std::fs::read(fdir.join(file)) {
            fonts.font_data.insert(name.to_owned(), Arc::new(FontData::from_owned(bytes)));
            let prop = fonts.families.entry(FontFamily::Proportional).or_default();
            if primary { prop.insert(0, name.to_owned()) } else { prop.push(name.to_owned()) }
            fonts.families.entry(FontFamily::Monospace).or_default().push(name.to_owned());
        }
    };
    add("segoe_ui", "segoeui.ttf", true);
    add("segoe_symbol", "seguisym.ttf", false);
    add("segoe_emoji", "seguiemj.ttf", false);
    add("ms_yahei", "msyh.ttc", false);
    add("malgun", "malgun.ttf", false);
    add("nirmala", "Nirmala.ttf", false);
    ctx.set_fonts(fonts);
}

fn setup_style(ctx: &egui::Context) {
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.style_mut_of(theme, |style| {
            use TextStyle::*;
            style.text_styles.insert(Heading, FontId::proportional(24.0));
            style.text_styles.insert(Body, FontId::proportional(14.5));
            style.text_styles.insert(Button, FontId::proportional(14.5));
            style.text_styles.insert(Small, FontId::proportional(11.5));
            style.text_styles.insert(Monospace, FontId::monospace(13.0));
            style.spacing.item_spacing = vec2(8.0, 6.0);
            style.spacing.button_padding = vec2(10.0, 5.0);
            style.spacing.interact_size.y = 26.0;

            let v = &mut style.visuals;
            v.selection.bg_fill = Color32::from_rgb(196, 48, 58);
            v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
            if theme == egui::Theme::Dark {
                v.panel_fill = Color32::from_rgb(20, 21, 27);
                v.window_fill = Color32::from_rgb(26, 28, 35);
                v.extreme_bg_color = Color32::from_rgb(13, 14, 18);
                v.faint_bg_color = Color32::from_rgb(28, 30, 38);
                v.hyperlink_color = Color32::from_rgb(130, 190, 255);
            } else {
                v.hyperlink_color = Color32::from_rgb(20, 100, 200);
            }
        });
    }
    ctx.set_theme(egui::ThemePreference::Dark);
}
