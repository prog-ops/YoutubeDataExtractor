# YouTube Data Extractor 🎬📊

Aplikasi desktop berperforma tinggi yang ditulis dalam **Rust** untuk mengekstrak dan menampilkan data backup dari **PipePipe** dan **NewPipe** (termasuk file `PipePipeData-*.zip`) ke dalam tabel interaktif yang diklasifikasikan secara rapi per kategori.

---

## ✨ Fitur Utama

1. **Ekstraksi Otomatis**:
   - Mendeteksi otomatis file ZIP backup NewPipe/PipePipe di folder kerja atau folder instalasi.
   - Ekstraksi aman dengan proteksi *path-traversal* ke folder `<nama_zip>_extracted`.
   - Mengambil `newpipe.db` (database SQLite Room) dan membaca seluruh tabel data.

2. **Klasifikasi Data per Kategori**:
   - 📊 **Ringkasan (Dashboard)**:
     - Kartu metrik: Total video ditonton, total pemutaran, estimasi durasi tonton, jumlah pencarian unik, langganan channel, playlist, dan total stream di database.
     - Grafik batang: Aktivitas tonton per tahun (2018–2026).
     - Grafik interaktif: Jam favorit menonton (distribusi per jam 00:00–23:00).
     - Daftar channel teratas yang paling sering ditonton (klik nama channel untuk langsung menuju riwayat tonton channel tersebut).
     - Daftar kata kunci pencarian teratas (klik kata kunci untuk langsung memfilter tabel pencarian).
   - ▶ **Riwayat Tonton** (43.244+ baris):
     - Waktu tonton lokal, judul, nama channel, durasi video, jumlah kali diputar, posisi pemutaran terakhir (dengan persentase), tipe stream, dan nama layanan.
     - Filter sub-kategori: **Tahun**.
   - 🔍 **Riwayat Pencarian** (6.223+ baris):
     - Waktu pencarian, kata kunci, frekuensi pencarian kata kunci yang sama, dan nama layanan.
     - Filter sub-kategori: **Tahun**.
     - Tautan pencarian langsung ke YouTube.
   - ⭐ **Langganan** (200 channel):
     - Nama channel, jumlah subscriber, grup feed, status notifikasi, jumlah video yang pernah diputar dari channel tersebut, tanggal feed terakhir diperbarui, layanan, dan deskripsi channel.
     - Filter sub-kategori: **Grup Feed** (mis. *Sains/Infographic*, *FE*, *Save semua*, dsb.).
   - 📰 **Feed Langganan** (1.574+ baris):
     - Cache video terbaru dari channel yang di-subscribe.
     - Filter sub-kategori: **Channel**.
   - 📂 **Playlist Lokal** (5.008+ baris, 102 playlist):
     - Daftar video di dalam playlist buatan sendiri.
     - Filter sub-kategori: **Nama Playlist**.
   - 🔖 **Playlist Tersimpan / Remote** (80 playlist):
     - Playlist publik yang di-bookmark dari channel lain.
     - Filter sub-kategori: **Pembuat**.
   - 🎬 **Semua Video di Database** (46.689+ baris):
     - Semua video yang tercatat di database dari gabungan riwayat, playlist, dan feed.
     - Filter sub-kategori: **Tipe Stream** (Video, Live Stream, Rekaman Live).

3. **Kemampuan Tabel Interaktif**:
   - **Render Virtual**: Menampilkan hingga puluhan ribu baris dengan sangat mulus (*60 FPS*) karena hanya baris yang terlihat di layar yang dirender.
   - **Pencarian Cepat Multi-Kata**: Mendukung pencarian kata kunci di seluruh kolom sekaligus (mis. `kompas live 2024`).
   - **Filter Sub-Kategori**: Dropdown pintar dengan jumlah data per kategori.
   - **Pengurutan Kolom**: Klik header kolom untuk mengurutkan (menaik/menurun). Angka diurutkan secara numerik, bukan abjad.
   - **Tautan & Menu Klik Kanan**:
     - Klik kiri teks judul/channel untuk membuka halaman YouTube langsung di browser default.
     - Klik kanan pada sel mana pun untuk menyalin teks atau menyalin URL.
   - **Ekspor CSV**: Tombol **💾 Ekspor CSV** untuk mengekspor data yang sedang terfilter ke file `.csv` (dilengkapi BOM UTF-8 agar terbaca sempurna di Microsoft Excel).
   - **Tema Terang & Gelap**: Toggle tema instan di bilah atas.
   - **Drag & Drop**: Seret dan lepas file `.zip` lain langsung ke jendela aplikasi.

4. **Mode CLI Verifikasi Cepat (`--check`)**:
   - Menjalankan ekstraksi dan analisis data langsung di terminal tanpa membuka GUI.

---

## 🚀 Cara Menjalankan

### 1. Menjalankan Mode GUI Desktop
Jalankan aplikasi langsung dengan Cargo:
```bash
cargo run --release
```
Atau jika ingin menentukan file ZIP tertentu secara eksplisit:
```bash
cargo run --release -- "PipePipeData-20260309_004207.zip"
```

Aplikasi juga bisa dijalankan langsung dari binary release:
```powershell
.\target\release\youtube_data_extractor.exe
```

### 2. Menjalankan Mode Analisis Cepat Terminal (`--check`)
```bash
cargo run -- --check PipePipeData-20260309_004207.zip
```
Output contoh:
```text
ZIP      : .\PipePipeData-20260309_004207.zip
Ekstrak  : .\PipePipeData-20260309_004207_extracted
Waktu    : 2.26s

Riwayat Tonton         43.244 baris  | 9 sub-kategori (Tahun)
Riwayat Pencarian       6.223 baris  | 9 sub-kategori (Tahun)
Langganan                 200 baris  | 5 sub-kategori (Grup Feed)
Feed Langganan          1.574 baris  | 120 sub-kategori (Channel)
Playlist Lokal          5.008 baris  | 102 sub-kategori (Playlist)
Playlist Tersimpan         80 baris  | 73 sub-kategori (Pembuat)
Semua Video            46.689 baris  | 3 sub-kategori (Tipe)

Ringkasan: 42.356 video ditonton, 83.284 putar, estimasi 1464 hari 18 jam 49 menit
Top channel: [("KOMPASTV", 3707), ("Rapid Trailer", 2402), ("MimbarTube", 2286), ("CoD Narco Live", 2117), ("tvOneNews", 1930)]
```

---

## 🛠 Arsitektur Kode

```text
src/
├── main.rs          # Entry point aplikasi, penanganan CLI argumen & eframe native window
├── extract.rs       # Dekompresi ZIP aman (anti-path-traversal) & auto-discovery file backup
├── loader.rs        # Parser SQLite `newpipe.db` ke model data kategori & penghitung statistik
├── model.rs         # Data model generik (Cell, Column, Table), pengurutan, format angka & durasi
├── table_view.rs    # Komponen virtualized table egui (search, filter, sort, context menu)
├── export.rs        # Modul ekspor CSV terfilter dengan UTF-8 BOM
└── app.rs           # State management GUI, sidebar navigasi, stat cards, visual charts & tema
```

---

## 📦 Dependensi Utama

- **[eframe / egui 0.37](https://github.com/emilk/egui)**: Framework GUI langsung (immediate-mode) yang ringan, cepat, dan modern.
- **[egui_extras](https://docs.rs/egui_extras)**: Widget tabel berkinerja tinggi dengan virtual scrolling.
- **[rusqlite](https://docs.rs/rusqlite)** (fitur `bundled`): Driver SQLite terintegrasi langsung tanpa perlu instalasi DLL eksternal.
- **[zip](https://docs.rs/zip)**: Pembaca dan pengekstrak arsip ZIP.
- **[chrono](https://docs.rs/chrono)**: Penanganan tanggal dan konversi waktu lokal.
- **[rfd](https://docs.rs/rfd)**: Native file picker dialog untuk memilih file ZIP dan menyimpan file CSV.
