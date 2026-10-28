//! Ekspor tabel (baris yang sedang terlihat) ke CSV.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::Result;

use crate::model::Table;

fn quote(s: &str) -> String {
    if s.contains([',', ';', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Tulis baris `indices` dari `table` ke CSV (UTF-8 dengan BOM agar terbaca benar di Excel).
/// Kolom yang berisi tautan mendapat kolom tambahan "URL <nama kolom>".
pub fn export_csv(path: &Path, table: &Table, indices: &[usize]) -> Result<()> {
    let mut w = BufWriter::new(File::create(path)?);
    w.write_all(b"\xEF\xBB\xBF")?;

    let link_cols: Vec<bool> = (0..table.columns.len())
        .map(|c| table.rows.iter().any(|r| r[c].url.is_some()))
        .collect();

    let mut header: Vec<String> = Vec::new();
    for (c, col) in table.columns.iter().enumerate() {
        header.push(quote(col.name));
        if link_cols[c] {
            header.push(quote(&format!("URL {}", col.name)));
        }
    }
    writeln!(w, "{}", header.join(","))?;

    for &i in indices {
        let mut fields: Vec<String> = Vec::new();
        for (c, cell) in table.rows[i].iter().enumerate() {
            fields.push(quote(&cell.text));
            if link_cols[c] {
                fields.push(quote(cell.url.as_deref().unwrap_or("")));
            }
        }
        writeln!(w, "{}", fields.join(","))?;
    }
    w.flush()?;
    Ok(())
}
