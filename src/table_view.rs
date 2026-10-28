//! Komponen tabel generik: pencarian, filter sub-kategori, sortir kolom,
//! dan render baris secara virtual (hanya baris yang terlihat yang digambar),
//! sehingga tetap lancar untuk puluhan ribu baris.

use egui::{Align, Color32, CursorIcon, Label, Layout, RichText, Sense, TextEdit, Ui};
use egui_extras::{Column as TColumn, TableBuilder};

use crate::model::{fmt_thousands, Cell, Table};

/// Status interaksi untuk satu tabel.
pub struct TableState {
    pub query: String,
    pub category: Option<String>,
    /// (indeks kolom, menaik?)
    pub sort: Option<(usize, bool)>,
    /// Indeks baris yang lolos filter, dalam urutan tampil.
    pub visible: Vec<usize>,
}

/// Aksi yang diminta tabel ke aplikasi.
pub enum TableAction {
    None,
    ExportCsv,
}

impl TableState {
    pub fn new(table: &Table) -> Self {
        Self { query: String::new(), category: None, sort: None, visible: (0..table.rows.len()).collect() }
    }

    pub fn refresh(&mut self, table: &Table) {
        let q = self.query.to_lowercase();
        let terms: Vec<&str> = q.split_whitespace().collect();
        let cat = self.category.as_deref();

        self.visible = (0..table.rows.len())
            .filter(|&i| cat.is_none_or(|c| table.row_categories[i].iter().any(|x| x == c)))
            .filter(|&i| terms.iter().all(|t| table.haystack[i].contains(t)))
            .collect();

        if let Some((col, asc)) = self.sort {
            self.visible.sort_by(|&a, &b| {
                let o = table.rows[a][col].key.compare(&table.rows[b][col].key);
                if asc { o } else { o.reverse() }
            });
        }
    }
}

pub fn show(ui: &mut Ui, table: &Table, state: &mut TableState, accent: Color32) -> TableAction {
    let mut dirty = false;
    let mut action = TableAction::None;

    // ---------------- Toolbar ----------------
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let resp = ui.add(
            TextEdit::singleline(&mut state.query)
                .hint_text("🔍  Cari di semua kolom… (pisahkan kata dengan spasi)")
                .desired_width(340.0),
        );
        if resp.changed() {
            dirty = true;
        }
        if !state.query.is_empty() && ui.button("✖").on_hover_text("Hapus pencarian").clicked() {
            state.query.clear();
            dirty = true;
        }

        if let Some(label) = table.category_label {
            ui.separator();
            ui.label(RichText::new(format!("{label}:")).strong());
            let selected = match &state.category {
                Some(c) => c.clone(),
                None => format!("Semua ({})", table.categories.len()),
            };
            egui::ComboBox::from_id_salt((table.id, "category"))
                .selected_text(selected)
                .width(240.0)
                .height(420.0)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(
                            state.category.is_none(),
                            format!("Semua  ·  {}", fmt_thousands(table.rows.len() as i64)),
                        )
                        .clicked()
                    {
                        state.category = None;
                        dirty = true;
                    }
                    ui.separator();
                    for (name, count) in &table.categories {
                        let sel = state.category.as_deref() == Some(name.as_str());
                        if ui
                            .selectable_label(sel, format!("{name}  ·  {}", fmt_thousands(*count as i64)))
                            .clicked()
                        {
                            state.category = Some(name.clone());
                            dirty = true;
                        }
                    }
                });
        }

        if state.sort.is_some() && ui.button("↺ Reset urutan").clicked() {
            state.sort = None;
            dirty = true;
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("💾 Ekspor CSV").on_hover_text("Simpan baris yang sedang tampil ke file CSV").clicked() {
                action = TableAction::ExportCsv;
            }
            ui.label(
                RichText::new(format!(
                    "{} dari {} baris",
                    fmt_thousands(state.visible.len() as i64),
                    fmt_thousands(table.rows.len() as i64)
                ))
                .color(accent)
                .strong(),
            );
        });
    });
    ui.add_space(6.0);

    // ---------------- Tabel ----------------
    let mut clicked_col: Option<usize> = None;
    let row_h = 24.0;
    let num_w = (fmt_thousands(table.rows.len() as i64).len() as f32 * 8.0 + 16.0).max(44.0);

    egui::ScrollArea::horizontal().id_salt((table.id, "hscroll")).show(ui, |ui| {
        let mut builder = TableBuilder::new(ui)
            .id_salt(table.id)
            .striped(true)
            .resizable(true)
            .cell_layout(Layout::left_to_right(Align::Center))
            .min_scrolled_height(0.0)
            .max_scroll_height(f32::INFINITY)
            .column(TColumn::exact(num_w));
        for c in &table.columns {
            builder = builder.column(TColumn::initial(c.width).at_least(40.0).clip(true));
        }

        builder
            .header(28.0, |mut header| {
                header.col(|ui| {
                    ui.label(RichText::new("#").weak());
                });
                for (ci, c) in table.columns.iter().enumerate() {
                    header.col(|ui| {
                        let arrow = match state.sort {
                            Some((s, true)) if s == ci => " ⏶",
                            Some((s, false)) if s == ci => " ⏷",
                            _ => "",
                        };
                        let mut text = RichText::new(format!("{}{arrow}", c.name)).strong();
                        if !arrow.is_empty() {
                            text = text.color(accent);
                        }
                        let resp = ui
                            .add(egui::Button::new(text).frame(false))
                            .on_hover_text("Klik untuk mengurutkan")
                            .on_hover_cursor(CursorIcon::PointingHand);
                        if resp.clicked() {
                            clicked_col = Some(ci);
                        }
                    });
                }
            })
            .body(|body| {
                body.rows(row_h, state.visible.len(), |mut row| {
                    let pos = row.index();
                    let ri = state.visible[pos];
                    row.col(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(fmt_thousands(pos as i64 + 1)).weak().small());
                        });
                    });
                    for (ci, cell) in table.rows[ri].iter().enumerate() {
                        row.col(|ui| draw_cell(ui, cell, table.columns[ci].numeric));
                    }
                });
            });
    });

    // Klik header: naik -> turun -> naik ...
    if let Some(ci) = clicked_col {
        state.sort = match state.sort {
            Some((s, asc)) if s == ci => Some((ci, !asc)),
            _ => Some((ci, !table.columns[ci].numeric)), // angka default turun (terbesar dulu)
        };
        dirty = true;
    }
    if dirty {
        state.refresh(table);
    }
    action
}

fn draw_cell(ui: &mut Ui, cell: &Cell, numeric: bool) {
    if let Some(url) = &cell.url {
        let resp = ui
            .add(
                Label::new(RichText::new(&cell.text).color(ui.visuals().hyperlink_color))
                    .truncate()
                    .sense(Sense::click()),
            )
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(format!("{}\n\n🔗 {url}\n(klik untuk membuka, klik kanan untuk menyalin)", cell.text));
        if resp.clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(url));
        }
        resp.context_menu(|ui| {
            if ui.button("🔗 Buka di browser").clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                ui.close();
            }
            if ui.button("📋 Salin URL").clicked() {
                ui.ctx().copy_text(url.clone());
                ui.close();
            }
            if ui.button("📋 Salin teks").clicked() {
                ui.ctx().copy_text(cell.text.clone());
                ui.close();
            }
        });
    } else if numeric {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(&cell.text);
        });
    } else {
        let resp = ui.add(Label::new(&cell.text).truncate().sense(Sense::click()));
        resp.context_menu(|ui| {
            if ui.button("📋 Salin teks").clicked() {
                ui.ctx().copy_text(cell.text.clone());
                ui.close();
            }
        });
    }
}
