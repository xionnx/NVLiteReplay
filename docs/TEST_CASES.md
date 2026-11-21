# NVLiteReplay — Test Case

| Item | Keterangan |
| :--- | :--- |
| Aplikasi | NVLiteReplayRustVoiceRecog.exe v1.0.0 |
| Branch | `fix/replay-segment-ordering` |
| Jenis Uji | Functional, Regression, Negative, Edge Case |
| Platform | Windows 10 / 11 x64 |

**Prioritas:** P1 = kritikal (klip hilang/rusak, crash) · P2 = fitur utama · P3 = minor/kosmetik
**Status:** `Pass` / `Fail` / `Blocked` / `N/A` — isi kolom *Aktual* bila `Fail`.

## Lingkungan Uji

| Item | Nilai |
| :--- | :--- |
| GPU | (isi: NVIDIA / AMD / Intel + model + versi driver) |
| Monitor | (isi: jumlah, resolusi, refresh rate) |
| Audio | (isi: output default, mikrofon) |
| Default config | Durasi 60 dtk, segmen 5 dtk, 60 FPS, 12 Mbps, RAM Buffer, H.264, hotkey `Ctrl+Shift+F9/F10/M` |
| File runtime | `NVLiteReplayRustVoiceRecog.ini` & `.voice` di folder exe; output default `%USERPROFILE%\Videos\NVLiteReplay` |

**Alat bantu:** `ffprobe` (cek codec/durasi/urutan frame), `certutil -hashfile <file> SHA256` (cek duplikat), Task Manager (RAM/GDI objects).

---

## 1. Instalasi & Startup

| ID | Judul | Prio | Prasyarat | Langkah | Hasil yang Diharapkan | Status | Aktual |
| :--- | :--- | :---: | :--- | :--- | :--- | :---: | :--- |
| TC-STR-01 | Aplikasi berjalan pertama kali | P1 | Belum ada file `.ini` | 1. Jalankan exe | Jendela UI tampil, file `.ini` dibuat saat setting diubah, tidak ada crash | | |
| TC-STR-02 | Single instance | P2 | Aplikasi sudah berjalan | 1. Jalankan exe kedua kalinya | Instance kedua tidak membuka jendela baru; instance pertama tetap berjalan | | |
| TC-STR-03 | Deteksi GPU & codec | P2 | — | 1. Buka tab Video<br>2. Lihat daftar codec | Nama GPU tampil; HEVC/AV1 hanya aktif jika GPU mendukung | | |
| TC-STR-04 | Self-test | P2 | — | 1. `NVLiteReplayRustVoiceRecog.exe --verify-self-test` | Semua tahap (Voice DSP, DXGI, encoder, DiskTemp stop) lulus | | |

## 2. Rekam & Simpan Klip (Core)

| ID | Judul | Prio | Prasyarat | Langkah | Hasil yang Diharapkan | Status | Aktual |
| :--- | :--- | :---: | :--- | :--- | :--- | :---: | :--- |
| TC-REC-01 | Nyalakan Replay | P1 | Config default | 1. Klik **Nyalakan** | Status "Instant Replay Aktif (RAM Buffer)", FPS tampil ±60 | | |
| TC-REC-02 | Simpan klip via tombol | P1 | Replay aktif ≥ 70 dtk | 1. Klik **Simpan Klip** | Notifikasi "Tersimpan", file `Replay_YYYY-MM-DD_HH-MM-SS.mp4` ada di folder output, durasi ±60 dtk, bisa diputar | | |
| TC-REC-03 | Simpan klip via hotkey | P1 | Replay aktif, fokus di aplikasi lain (game) | 1. Tekan `Ctrl+Shift+F10` | Klip tersimpan seperti TC-REC-02 | | |
| TC-REC-04 | Simpan saat buffer belum penuh | P2 | Replay baru aktif 15 dtk | 1. Simpan klip | Klip tersimpan dengan durasi ±15 dtk (bukan 60) | | |
| TC-REC-05 | Simpan saat buffer kosong | P3 | Replay baru dinyalakan < 1 dtk | 1. Langsung simpan | Pesan "Buffer masih kosong, tunggu beberapa detik."; tidak ada file 0 byte | | |
| TC-REC-06 | Simpan saat Replay mati | P2 | Replay nonaktif | 1. Tekan hotkey simpan | Pesan "Instant Replay belum dinyalakan!"; tidak ada file baru | | |
| TC-REC-07 | Simpan saat sedang menyimpan | P2 | Replay aktif | 1. Tekan hotkey simpan 2× berurutan sangat cepat (< 200 ms) | Pesan "Sedang menyimpan replay..." untuk tekanan kedua; aplikasi tidak hang | | |
| TC-REC-08 | Matikan Replay | P1 | Replay aktif | 1. Klik **Matikan** | Status "Instant Replay Nonaktif", FPS 0, RAM turun | | |
| TC-REC-09 | Mode Disk Temp Buffer | P1 | Tab Video: mode **Disk Temp** | 1. Nyalakan 70 dtk<br>2. Simpan<br>3. Matikan | Klip valid; file `seg_<pid>_*.mp4` muncul di folder temp selama aktif dan **terhapus semua** setelah dimatikan | | |
| TC-REC-10 | Folder temp custom | P2 | Tab Sistem: atur folder temp ke drive lain | 1. Ulangi TC-REC-09 | Segmen ditulis ke folder custom | | |
| TC-REC-11 | Codec HEVC | P2 | GPU mendukung HEVC | 1. Pilih HEVC, nyalakan, simpan<br>2. `ffprobe` klip | Codec `hevc`, warna BT.709 | | |
| TC-REC-12 | Codec AV1 | P3 | GPU mendukung AV1 | 1. Pilih AV1, nyalakan, simpan | Codec `av1`, klip bisa diputar | | |
| TC-REC-13 | Variasi FPS | P2 | — | 1. Uji 30 / 60 / 120 FPS masing-masing | `ffprobe` menunjukkan FPS sesuai pilihan | | |
| TC-REC-14 | Variasi durasi | P2 | — | 1. Uji durasi terpendek (15 dtk) dan terpanjang (5 mnt) | Durasi klip sesuai pilihan (toleransi ±1 segmen / 5 dtk) | | |

## 3. Regresi — Perbaikan Branch Ini

| ID | Judul | Prio | Prasyarat | Langkah | Hasil yang Diharapkan | Status | Aktual |
| :--- | :--- | :---: | :--- | :--- | :--- | :---: | :--- |
| TC-REG-01 | Klip berturut-turut tidak duplikat | P1 | Replay aktif ≥ 70 dtk, tampilkan jam berdetik di layar (mis. stopwatch online) | 1. Simpan klip A<br>2. Tunggu 10 dtk, simpan klip B<br>3. Tunggu 10 dtk, simpan klip C<br>4. Hitung SHA256 ketiganya | Hash A, B, C berbeda; detik terakhir di tiap klip = waktu klip disimpan; C tidak sama dengan A | | |
| TC-REG-02 | Simpan tepat setelah rotasi segmen | P1 | Seperti TC-REG-01, mode Disk Temp + HEVC (finalize lebih lambat) | 1. Simpan klip berulang 10× dengan jeda 5–6 dtk | Di setiap klip stopwatch berjalan maju tanpa lompat mundur / tanpa celah ±5 dtk | | |
| TC-REG-03 | Klip setelah urutan tertukar | P1 | Lanjutan TC-REG-02 | 1. Tunggu 60 dtk tanpa simpan<br>2. Simpan 1 klip | Stopwatch dalam klip urut naik dari awal sampai akhir | | |
| TC-REG-04 | Tombol Browse folder output | P1 | — | 1. Tab Sistem → klik **Browse** folder output | Dialog pilih folder Windows terbuka; folder terpilih tersimpan | | |
| TC-REG-05 | Tombol Browse folder temp | P2 | — | 1. Klik **Browse** folder temp | Dialog terbuka dan berfungsi | | |
| TC-REG-06 | Dua klip dalam detik yang sama | P1 | RAM Buffer, durasi 15 dtk (simpan selesai < 500 ms), Replay aktif ≥ 20 dtk | 1. Tekan hotkey simpan<br>2. Tepat setelah notifikasi "Tersimpan", tekan lagi (masih di detik yang sama)<br>3. Ulangi sampai dua file punya timestamp detik sama | File kedua bernama `..._2.mp4`; kedua file utuh, tidak ada yang tertimpa | | |
| TC-REG-07 | Matikan Replay saat menyimpan | P1 | Mode Disk Temp, durasi 5 mnt, Replay aktif ≥ 5 mnt | 1. Simpan klip<br>2. Segera (< 1 dtk) klik **Matikan** | Klip tetap utuh dan bisa diputar penuh; segmen temp terhapus setelah simpan selesai | | |
| TC-REG-08 | Tutup aplikasi saat menyimpan | P1 | Seperti TC-REG-07, "Minimize to tray" off | 1. Simpan klip<br>2. Segera tutup aplikasi (Quit dari tray) | Proses keluar setelah simpan selesai; klip utuh | | |
| TC-REG-09 | Merge gagal tidak meninggalkan file rusak | P2 | Mode Disk Temp | 1. Nyalakan 30 dtk<br>2. Hapus manual semua `seg_*.mp4` dari folder temp<br>3. Simpan | Pesan "Gagal menggabungkan segmen replay."; tidak ada file MP4 rusak/0 byte di folder output | | |
| TC-REG-10 | Encoder gagal saat rotasi | P2 | Mode Disk Temp, folder temp di flashdisk | 1. Nyalakan Replay<br>2. Cabut flashdisk | Status berubah ke "Error: perekaman berhenti..."; Replay tercatat mati; hotkey simpan menampilkan "belum dinyalakan" | | |
| TC-REG-11 | Nyalakan ulang setelah error | P2 | Lanjutan TC-REG-10, folder temp dikembalikan | 1. Nyalakan Replay 30 dtk<br>2. Simpan | Klip hanya berisi rekaman sesi baru | | |
| TC-REG-12 | File `.voice` korup | P1 | Aplikasi tertutup | 1. Edit `.voice`, ubah baris `TPL` menjadi `TPL 1 9999999999`<br>2. Jalankan aplikasi | Aplikasi terbuka tanpa crash (template diabaikan) | | |
| TC-REG-13 | Enrollment suara tidak macet | P2 | Replay aktif, mikrofon dicabut | 1. Tab Voice → rekam contoh suara<br>2. Tunggu 6 dtk<br>3. Colok mikrofon, rekam ulang | Rekam ulang bisa dimulai (tidak terkunci di status merekam) | | |

## 4. Audio

| ID | Judul | Prio | Prasyarat | Langkah | Hasil yang Diharapkan | Status | Aktual |
| :--- | :--- | :---: | :--- | :--- | :--- | :---: | :--- |
| TC-AUD-01 | Audio desktop terekam | P1 | Desktop audio ON, putar musik | 1. Simpan klip | Klip berisi suara musik tanpa kresek | | |
| TC-AUD-02 | Mikrofon terekam | P1 | Mic ON | 1. Bicara, simpan klip | Suara mic terdengar bersama audio desktop | | |
| TC-AUD-03 | Pilih perangkat mic | P2 | ≥ 2 mikrofon | 1. Pilih mic kedua, nyalakan, bicara, simpan | Audio berasal dari mic yang dipilih | | |
| TC-AUD-04 | Mute/Live mic real-time | P2 | Replay aktif | 1. Tekan `Ctrl+Shift+M`, bicara 5 dtk<br>2. Unmute, bicara 5 dtk<br>3. Simpan | Bagian mute senyap, bagian live terdengar; notifikasi tray muncul | | |
| TC-AUD-05 | Slider volume real-time | P3 | Replay aktif | 1. Geser volume mic 100% → 20% saat bicara, simpan | Volume mic di klip mengecil setelah titik geser | | |
| TC-AUD-06 | Sinkron audio–video | P1 | Durasi 5 mnt, 60 FPS | 1. Rekam video tepuk tangan / clapper di layar selama 5 mnt<br>2. Simpan, cek awal & akhir klip | Selisih suara vs gambar ≤ 1 frame di awal dan akhir (catat bila drift — known issue) | | |
| TC-AUD-07 | Ganti perangkat output saat aktif | P2 | Replay aktif | 1. Ganti default output ke headset<br>2. Putar musik, simpan | Catat apakah audio desktop masih terekam (known issue: tidak pulih) | | |
| TC-AUD-08 | Audio senyap | P3 | Tidak ada suara diputar | 1. Simpan klip | Klip valid, track audio senyap tanpa noise | | |

## 5. Voice Clip Trigger

| ID | Judul | Prio | Prasyarat | Langkah | Hasil yang Diharapkan | Status | Aktual |
| :--- | :--- | :---: | :--- | :--- | :--- | :---: | :--- |
| TC-VOC-01 | Rekam 3 contoh suara | P2 | Frasa "Roman, clip that" | 1. Rekam slot 1–3 | Ketiga slot terisi dengan durasi; file `.voice` tersimpan | | |
| TC-VOC-02 | Trigger dengan frasa benar | P1 | TC-VOC-01, trigger ON, Replay aktif | 1. Ucapkan frasa | Notifikasi "Voice Clip Terdeteksi!", klip tersimpan | | |
| TC-VOC-03 | Frasa salah tidak memicu | P2 | Seperti TC-VOC-02 | 1. Ucapkan 10 kalimat berbeda | Tidak ada klip tersimpan (false positive = 0) | | |
| TC-VOC-04 | Cooldown | P2 | Cooldown 8 dtk | 1. Ucapkan frasa 2× dalam 3 dtk | Hanya 1 klip tersimpan | | |
| TC-VOC-05 | Sensitivitas | P3 | — | 1. Uji 30% dan 95% dengan frasa yang sama | 30% lebih mudah terpicu, 95% lebih ketat | | |
| TC-VOC-06 | Rekam contoh saat Replay mati | P2 | Replay nonaktif | 1. Rekam 1 contoh suara | Mic sementara dibuka, rekaman berhasil, mic dilepas setelahnya | | |
| TC-VOC-07 | Persistensi template | P2 | TC-VOC-01 | 1. Tutup & buka aplikasi | Template dan frasa masih ada | | |

## 6. Hotkey & Sistem

| ID | Judul | Prio | Prasyarat | Langkah | Hasil yang Diharapkan | Status | Aktual |
| :--- | :--- | :---: | :--- | :--- | :--- | :---: | :--- |
| TC-SYS-01 | Hotkey toggle | P1 | — | 1. Tekan `Ctrl+Shift+F9` dua kali | Replay nyala lalu mati; balloon tray sesuai | | |
| TC-SYS-02 | Preset hotkey | P2 | — | 1. Pilih preset `F8/F7/F6` lalu `Alt+F9/F10/M` | Hotkey baru berfungsi, hotkey lama tidak lagi | | |
| TC-SYS-03 | Hotkey custom | P2 | — | 1. Atur slot simpan ke `Ctrl+Alt+S` | Hotkey berfungsi; tersimpan setelah restart | | |
| TC-SYS-04 | Hotkey bentrok | P3 | Aplikasi lain memakai `Ctrl+Shift+F10` | 1. Jalankan aplikasi | Ada indikasi hotkey gagal didaftarkan / aplikasi tidak crash | | |
| TC-SYS-05 | Minimize to tray | P2 | Opsi ON | 1. Tutup jendela (X) | Jendela hilang, ikon tray tetap; Replay tetap berjalan | | |
| TC-SYS-06 | Menu tray | P2 | — | 1. Klik kanan ikon tray → Simpan / Keluar | Aksi berjalan sesuai menu | | |
| TC-SYS-07 | Mode fokus game | P3 | Opsi ON, game fullscreen | 1. Simpan klip saat bermain | Tidak ada popup notifikasi menutupi game | | |
| TC-SYS-08 | Suara beep saat simpan | P3 | Opsi ON / OFF | 1. Simpan klip pada kedua kondisi | Beep hanya terdengar saat ON | | |
| TC-SYS-09 | Persistensi setting | P2 | — | 1. Ubah semua setting, tutup, buka lagi | Semua setting sama seperti sebelum ditutup | | |

## 7. Capture & Ketahanan

| ID | Judul | Prio | Prasyarat | Langkah | Hasil yang Diharapkan | Status | Aktual |
| :--- | :--- | :---: | :--- | :--- | :--- | :---: | :--- |
| TC-CAP-01 | Multi-monitor | P2 | ≥ 2 monitor | 1. Pilih monitor 2, nyalakan, simpan | Klip berisi layar monitor 2 | | |
| TC-CAP-02 | Exclusive fullscreen + Alt+Tab | P1 | Game exclusive fullscreen | 1. Main 30 dtk, Alt+Tab 3×, kembali ke game<br>2. Simpan | Klip valid, capture pulih setelah Alt+Tab (bukan layar beku/hitam permanen) | | |
| TC-CAP-03 | Game stretched 4:3 | P2 | Game di resolusi 1280×960 stretched | 1. Main, simpan | Gambar tidak terpotong/rusak, sisa area hitam | | |
| TC-CAP-04 | Kursor terlihat | P3 | — | 1. Gerakkan mouse, simpan | Kursor tampil di klip pada posisi benar | | |
| TC-CAP-05 | Layar UAC / secure desktop | P3 | Replay aktif | 1. Picu prompt UAC, tutup<br>2. Simpan | Capture pulih; aplikasi tidak crash | | |
| TC-CAP-06 | Uji ketahanan 2 jam | P1 | RAM Buffer, durasi 5 mnt | 1. Biarkan aktif 2 jam sambil main<br>2. Simpan klip tiap 15 mnt | RAM stabil (tidak naik terus), GDI objects stabil, semua klip valid | | |
| TC-CAP-07 | Kapasitas RAM durasi maksimum | P2 | RAM Buffer, 5 mnt, bitrate tertinggi | 1. Aktif 6 mnt, simpan | Klip tersimpan; catat puncak RAM saat simpan (known issue: buffer disalin) | | |

## 8. Unit Test Otomatis

| ID | Judul | Prio | Langkah | Hasil yang Diharapkan | Status |
| :--- | :--- | :---: | :--- | :--- | :---: |
| TC-UNIT-01 | Unit test Rust | P1 | `cargo test` | `8 passed; 0 failed` | |

---

## Known Issues (belum diperbaiki — catat hasilnya, jangan langsung `Fail`)

| Ref | Masalah | Terkait TC |
| :--- | :--- | :--- |
| #6 | Fallback codec di tengah sesi bisa mencampur codec dalam satu klip | TC-REC-11/12 |
| #7 | Drift audio–video pada sesi panjang / FPS tidak tercapai | TC-AUD-06 |
| #8 | Audio tidak pulih setelah ganti perangkat | TC-AUD-07 |
| #13 | Simpan mode RAM menyalin seluruh buffer (lonjakan RAM) | TC-CAP-07 |
