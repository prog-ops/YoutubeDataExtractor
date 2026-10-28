# YouTube Data Extractor 🎬📊

A high-performance desktop application written in **Rust** to extract and explore backup archives from **PipePipe** and **NewPipe** (such as `PipePipeData-*.zip`) inside categorized, interactive tables with instant search, sub-category filtering, and analytics.

---

## ✨ Key Features

1. **Automatic Extraction**:
   - Automatically detects NewPipe/PipePipe backup ZIP files in the current folder or executable directory.
   - Safe zip decompression with path-traversal protection into `<zip_name>_extracted`.
   - Reads SQLite database `newpipe.db` (Android Room schema) without requiring external SQLite installations.

2. **Data Classification by Categories**:
   - 📊 **Overview (Dashboard)**:
     - Metric cards: Total videos watched, total play count, cumulative video duration played, unique search queries, subscribed channels, local playlists, and total indexed streams.
     - Bar chart: Annual watching activity (play count distribution across years).
     - Interactive chart: 24-hour daily watch pattern (hover over bars to inspect counts).
     - Quick links: Click top watched channels or search queries to jump directly to the filtered table view.
   - ▶ **Watch History**:
     - Local timestamp, video title, channel name, duration, play count, last playback position (with progress percentage), stream type, and service name.
     - Sub-category filter: **Year**.
   - 🔍 **Search History**:
     - Search timestamp, query, repeat search frequency, and service.
     - Direct search link to open query results on YouTube.
     - Sub-category filter: **Year**.
   - ⭐ **Subscriptions**:
     - Channel name, subscriber count, feed group, notification status, total videos played from channel, last feed update timestamp, service, and channel description.
     - Sub-category filter: **Feed Group** (e.g., custom folders / groups).
   - 📰 **Subscription Feed**:
     - Cached stream entries from subscribed channels.
     - Sub-category filter: **Channel**.
   - 📂 **Local Playlists**:
     - Track-ordered video items within user-created local playlists.
     - Sub-category filter: **Playlist Name**.
   - 🔖 **Saved Playlists**:
     - Bookmarked public playlists created by other creators.
     - Sub-category filter: **Creator**.
   - 🎬 **All Videos**:
     - Master catalog of all streams indexed across history, playlists, and feed caches.
     - Sub-category filter: **Stream Type** (Video, Live Stream, Past Live Stream).

3. **Interactive Table Capabilities**:
   - **Virtualized Rendering**: Handles 40,000+ rows smoothly at 60 FPS using immediate-mode virtual scrolling (`egui_extras::TableBuilder`).
   - **Multi-Keyword Search**: Real-time filtering across all columns simultaneously (space-separated query terms).
   - **Smart Sub-Category Filters**: Instant dropdown selection with live row counters per category.
   - **Numeric & Lexicographical Sorting**: Click headers to cycle ascending/descending order. Numeric columns sort by real values, not text.
   - **Hyperlinks & Context Menu**:
     - Left-click titles or channels to open directly in the default browser.
     - Right-click any cell to copy text or copy URL.
   - **Excel-Compatible CSV Export**: Click **💾 Export CSV** to export the currently filtered rows with automatic UTF-8 BOM.
   - **Dark / Light / System Theme**: Instant theme switching from the top toolbar.
   - **Drag & Drop**: Drag and drop any `.zip` file into the application window to load it immediately.

4. **Headless Verification Mode (`--check`)**:
   - Run data analysis and verification directly from the command line without opening the GUI.

---

## 🚀 Getting Started

### 1. Launching the GUI Application
Run using Cargo:
```bash
cargo run --release
```
Or specify a backup ZIP file explicitly:
```bash
cargo run --release -- "PipePipeData-sample.zip"
```

Or run the compiled release executable directly:
```powershell
.\target\release\youtube_data_extractor.exe
```

### 2. Headless CLI Verification Mode (`--check`)
```bash
cargo run -- --check backup.zip
```
Example terminal output:
```text
ZIP archive: .\backup.zip
Extracted  : .\backup_extracted
Elapsed    : 1.85s

Watch History            42,500 rows  | 8 sub-categories (Year)
Search History            5,800 rows  | 8 sub-categories (Year)
Subscriptions               150 rows  | 4 sub-categories (Feed Group)
Subscription Feed         1,200 rows  | 95 sub-categories (Channel)
Local Playlists           3,400 rows  | 45 sub-categories (Playlist)
Saved Playlists              60 rows  | 50 sub-categories (Creator)
All Videos               45,000 rows  | 3 sub-categories (Type)

Overview: 41,200 videos watched, 75,000 plays, estimated 1,200 days 14 hrs 30 mins
Top channels: [("Tech Channel", 2450), ("Science Daily", 1820), ("Documentary Hub", 1540)]
```

---

## 🛠 Project Architecture

```text
src/
├── main.rs          # Application entry point, CLI arguments, and native window setup
├── extract.rs       # Safe ZIP decompression (anti path-traversal) & backup discovery
├── loader.rs        # SQLite `newpipe.db` parser & analytics metric aggregator
├── model.rs         # Generic table model (Cell, Column, Table), sorting keys, and formatters
├── table_view.rs    # Virtualized table component (search, category filter, sort, context menu)
├── export.rs        # Filtered CSV export module with UTF-8 BOM
└── app.rs           # GUI state management, sidebar navigation, stat cards, and charts
```

---

## 📦 Core Dependencies

- **[eframe / egui 0.37](https://github.com/emilk/egui)**: Fast, lightweight immediate-mode GUI framework.
- **[egui_extras](https://docs.rs/egui_extras)**: High-performance virtualized table layout.
- **[rusqlite](https://docs.rs/rusqlite)** (`bundled` feature): Embedded SQLite driver without requiring external DLLs.
- **[zip](https://docs.rs/zip)**: ZIP archive decompression and inspection.
- **[chrono](https://docs.rs/chrono)**: Date-time conversions and local timezone formatting.
- **[rfd](https://docs.rs/rfd)**: Native file picker dialogs for selecting files and saving CSVs.

---

## 📄 License

This project is licensed under the [MIT License](LICENSE).
