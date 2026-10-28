//! Generic data model for tables displayed in the UI.
//!
//! Each data category (watch history, subscriptions, playlists, etc.) is transformed
//! into a [`Table`] containing columns, rows, and per-row sub-categories. This allows
//! a single reusable table component to handle all categories cleanly.

use std::cmp::Ordering;
use std::collections::HashMap;

use chrono::{DateTime, Datelike, Local, Timelike, Utc};

/// Sorting key for a table cell. Numbers are compared numerically,
/// and strings are compared lexicographically (case-insensitive).
#[derive(Clone, Debug)]
pub enum SortKey {
    Num(i64),
    Text(String),
}

impl SortKey {
    pub fn compare(&self, other: &Self) -> Ordering {
        match (self, other) {
            (SortKey::Num(a), SortKey::Num(b)) => a.cmp(b),
            (SortKey::Text(a), SortKey::Text(b)) => a.cmp(b),
            (SortKey::Num(_), SortKey::Text(_)) => Ordering::Less,
            (SortKey::Text(_), SortKey::Num(_)) => Ordering::Greater,
        }
    }
}

/// A single table cell: display text, sorting key, and an optional link URL.
#[derive(Clone, Debug)]
pub struct Cell {
    pub text: String,
    pub key: SortKey,
    pub url: Option<String>,
}

const EMPTY: &str = "–";

impl Cell {
    pub fn text(s: impl Into<String>) -> Self {
        let text = s.into();
        let key = SortKey::Text(text.to_lowercase());
        Self { text, key, url: None }
    }

    /// Clickable link text that opens a URL in the default browser.
    pub fn link(s: impl Into<String>, url: Option<String>) -> Self {
        let mut c = Self::text(s);
        c.url = url.filter(|u| !u.trim().is_empty());
        c
    }

    fn empty_num() -> Self {
        Self { text: EMPTY.into(), key: SortKey::Num(i64::MIN), url: None }
    }

    /// Number formatted with standard thousands comma separators (e.g., "1,234,567").
    pub fn int(v: Option<i64>) -> Self {
        match v {
            Some(n) if n >= 0 => Self { text: fmt_thousands(n), key: SortKey::Num(n), url: None },
            _ => Self::empty_num(),
        }
    }

    /// Local date and time from millisecond epoch timestamp ("YYYY-MM-DD HH:MM").
    pub fn datetime_ms(v: Option<i64>) -> Self {
        match v.and_then(local_dt) {
            Some(dt) => Self {
                text: dt.format("%Y-%m-%d %H:%M").to_string(),
                key: SortKey::Num(v.unwrap_or_default()),
                url: None,
            },
            None => Self::empty_num(),
        }
    }

    /// Date only (UTC) from millisecond epoch timestamp ("YYYY-MM-DD") — used for upload dates.
    pub fn date_ms(v: Option<i64>) -> Self {
        match v.and_then(DateTime::<Utc>::from_timestamp_millis) {
            Some(dt) if v.unwrap_or_default() > 0 => Self {
                text: dt.format("%Y-%m-%d").to_string(),
                key: SortKey::Num(v.unwrap_or_default()),
                url: None,
            },
            _ => Self::empty_num(),
        }
    }

    /// Duration in seconds, displayed as "H:MM:SS" or "M:SS".
    pub fn duration_secs(v: Option<i64>) -> Self {
        match v {
            Some(s) if s > 0 => Self { text: fmt_duration(s), key: SortKey::Num(s), url: None },
            _ => Self::empty_num(),
        }
    }

    /// Last playback position with percentage relative to video duration.
    pub fn progress(progress_ms: Option<i64>, duration_secs: i64) -> Self {
        match progress_ms {
            Some(ms) if ms > 0 => {
                let secs = ms / 1000;
                let text = if duration_secs > 0 {
                    let pct = ((secs as f64 / duration_secs as f64) * 100.0).min(100.0);
                    format!("{} ({pct:.0}%)", fmt_duration(secs))
                } else {
                    fmt_duration(secs)
                };
                Self { text, key: SortKey::Num(ms), url: None }
            }
            _ => Self::empty_num(),
        }
    }
}

/// Column definition.
#[derive(Clone, Debug)]
pub struct Column {
    pub name: &'static str,
    pub width: f32,
    /// Right-align numeric columns.
    pub numeric: bool,
}

impl Column {
    pub fn text(name: &'static str, width: f32) -> Self {
        Self { name, width, numeric: false }
    }
    pub fn num(name: &'static str, width: f32) -> Self {
        Self { name, width, numeric: true }
    }
}

/// Sorting order for sub-categories in the filter dropdown.
#[derive(Clone, Copy, Debug)]
pub enum CatOrder {
    NameAsc,
    NameDesc,
    CountDesc,
    FirstSeen,
}

/// A data category ready to be rendered in the table view.
pub struct Table {
    pub id: &'static str,
    pub icon: &'static str,
    pub title: &'static str,
    pub description: String,
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Cell>>,
    /// Sub-category label (e.g., "Year", "Playlist", "Feed Group").
    pub category_label: Option<&'static str>,
    pub category_order: CatOrder,
    /// Sub-categories associated with each row (a row can belong to multiple).
    pub row_categories: Vec<Vec<String>>,
    /// Unique sub-categories with their row counts.
    pub categories: Vec<(String, usize)>,
    /// Concatenated lowercase text for fast multi-keyword search.
    pub haystack: Vec<String>,
}

impl Table {
    pub fn new(
        id: &'static str,
        icon: &'static str,
        title: &'static str,
        description: impl Into<String>,
        columns: Vec<Column>,
    ) -> Self {
        Self {
            id,
            icon,
            title,
            description: description.into(),
            columns,
            rows: Vec::new(),
            category_label: None,
            category_order: CatOrder::NameAsc,
            row_categories: Vec::new(),
            categories: Vec::new(),
            haystack: Vec::new(),
        }
    }

    pub fn with_categories(mut self, label: &'static str, order: CatOrder) -> Self {
        self.category_label = Some(label);
        self.category_order = order;
        self
    }

    pub fn push(&mut self, row: Vec<Cell>, cats: Vec<String>) {
        debug_assert_eq!(row.len(), self.columns.len(), "cell count != column count ({})", self.id);
        self.rows.push(row);
        self.row_categories.push(cats);
    }

    /// Compute unique category counts and generate search index. Call once after loading rows.
    pub fn finish(mut self) -> Self {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        let mut order: Vec<&str> = Vec::new();
        for cats in &self.row_categories {
            for c in cats {
                let e = counts.entry(c.as_str()).or_insert_with(|| {
                    order.push(c.as_str());
                    0
                });
                *e += 1;
            }
        }
        let mut cats: Vec<(String, usize)> =
            order.iter().map(|c| (c.to_string(), counts[c])).collect();
        match self.category_order {
            CatOrder::NameAsc => cats.sort_by_key(|a| a.0.to_lowercase()),
            CatOrder::NameDesc => cats.sort_by_key(|a| std::cmp::Reverse(a.0.to_lowercase())),
            CatOrder::CountDesc => cats.sort_by_key(|a| std::cmp::Reverse(a.1)),
            CatOrder::FirstSeen => {}
        }
        self.categories = cats;

        self.haystack = self
            .rows
            .iter()
            .zip(&self.row_categories)
            .map(|(row, cats)| {
                let mut s = String::new();
                for c in row {
                    s.push_str(&c.text.to_lowercase());
                    s.push('\u{1}');
                }
                for c in cats {
                    s.push_str(&c.to_lowercase());
                    s.push('\u{1}');
                }
                s
            })
            .collect();
        self
    }
}

// ---------------------------------------------------------------------------
// Formatting utilities
// ---------------------------------------------------------------------------

pub fn local_dt(ms: i64) -> Option<DateTime<Local>> {
    DateTime::<Utc>::from_timestamp_millis(ms).map(|d| d.with_timezone(&Local))
}

pub fn year_of(ms: i64) -> Option<i32> {
    local_dt(ms).map(|d| d.year())
}

pub fn hour_of(ms: i64) -> Option<u32> {
    local_dt(ms).map(|d| d.hour())
}

pub fn fmt_date(ms: i64) -> String {
    local_dt(ms).map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| EMPTY.into())
}

/// 1234567 -> "1,234,567"
pub fn fmt_thousands(n: i64) -> String {
    let neg = n < 0;
    let digits = n.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    if neg { format!("-{out}") } else { out }
}

/// 3725 -> "1:02:05", 65 -> "1:05"
pub fn fmt_duration(secs: i64) -> String {
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// Human-readable long duration: "12 days 4 hrs 30 mins".
pub fn fmt_long_duration(secs: i64) -> String {
    let d = secs / 86_400;
    let h = (secs % 86_400) / 3600;
    let m = (secs % 3600) / 60;
    match (d, h) {
        (0, 0) => format!("{m} mins"),
        (0, 1) => format!("1 hr {m} mins"),
        (0, _) => format!("{h} hrs {m} mins"),
        (1, 1) => format!("1 day 1 hr {m} mins"),
        (1, _) => format!("1 day {h} hrs {m} mins"),
        (_, 1) => format!("{d} days 1 hr {m} mins"),
        _ => format!("{d} days {h} hrs {m} mins"),
    }
}

pub fn stream_type_label(t: &str) -> &'static str {
    match t {
        "VIDEO_STREAM" => "Video",
        "LIVE_STREAM" => "Live Stream",
        "POST_LIVE_STREAM" => "Past Live Stream",
        "AUDIO_STREAM" => "Audio",
        "AUDIO_LIVE_STREAM" => "Audio Live",
        "POST_LIVE_AUDIO_STREAM" => "Past Audio Live",
        _ => "Other",
    }
}

/// NewPipe/PipePipe service identifier to human-readable label.
pub fn service_label(id: i64) -> String {
    match id {
        0 => "YouTube".into(),
        1 => "SoundCloud".into(),
        2 => "media.ccc.de".into(),
        3 => "PeerTube".into(),
        4 => "Bandcamp".into(),
        5 => "BiliBili".into(),
        6 => "NicoNico".into(),
        n => format!("Service #{n}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands() {
        assert_eq!(fmt_thousands(0), "0");
        assert_eq!(fmt_thousands(999), "999");
        assert_eq!(fmt_thousands(1000), "1,000");
        assert_eq!(fmt_thousands(1234567), "1,234,567");
        assert_eq!(fmt_thousands(-45000), "-45,000");
    }

    #[test]
    fn durations() {
        assert_eq!(fmt_duration(65), "1:05");
        assert_eq!(fmt_duration(3725), "1:02:05");
        assert_eq!(fmt_long_duration(90_061), "1 day 1 hr 1 mins");
    }
}
