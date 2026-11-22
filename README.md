<div align="center">

# NVLiteReplay

**Lightweight Instant Replay & Offline Voice Clip Trigger for Windows**

![Platform](https://img.shields.io/badge/Platform-Windows%2010%20%2F%2011%20x64-09090B?style=flat-square&labelColor=18181B)
![GPU](https://img.shields.io/badge/GPU-NVIDIA%20%C2%B7%20AMD%20%C2%B7%20Intel-09090B?style=flat-square&labelColor=18181B)
![Portable](https://img.shields.io/badge/Binary-Single%20EXE%20(~573%20KB)-09090B?style=flat-square&labelColor=18181B)
![License](https://img.shields.io/badge/License-MIT-09090B?style=flat-square&labelColor=18181B)

Aplikasi Instant Replay ultra-ringan dalam satu file portable tanpa service background, tanpa telemetry, dan tanpa install driver tambahan. Mendukung **NVIDIA NVENC**, **AMD AMF**, maupun **Intel QSV**.

<br>

<img src="assets/ui-video.png" alt="NVLiteReplay Minimalist Dark UI" width="520">

</div>

---

## Tampilan Antarmuka

| Audio & Input Mic | Sistem & Hotkey | Voice Clip Trigger |
| :---: | :---: | :---: |
| <img src="assets/ui-audio.png" alt="Tab Audio" width="270"> | <img src="assets/ui-system.png" alt="Tab Sistem" width="270"> | <img src="assets/ui-voice.png" alt="Tab Voice Clip" width="270"> |

---

## Ringkasan

**NVLiteReplay** menangkap layar langsung di memori GPU (*zero-copy*), melakukan konversi warna **NV12 BT.709** di hardware, lalu menyimpan rekaman klip terakhir Anda dalam hitungan milidetik (*stream-copy / remux* tanpa *re-encode*). Dilengkapi pemicu suara offline (**Voice Clip Trigger**) sehingga Anda bisa menyimpan klip cukup dengan mengucapkan frasa pilihan Anda sendiri.

| Komponen | Implementasi |
| :--- | :--- |
| **Screen Capture** | `DXGI Desktop Duplication` + komposit kursor GPU (*fallback* GDI otomatis) |
| **Color & Scaling** | `ID3D11VideoProcessor` hardware `BGRA -> NV12` standar **BT.709** (Studio 16–235) |
| **Hardware Encoder** | Media Foundation Hardware MFT + tuning `ICodecAPI` (**Peak-VBR terkendali**, **GOP 2 dtk**) |
| **Codec Didukung** | **H.264 High Profile**, **HEVC (H.265)**, dan **AV1** *(auto-detect kemampuan GPU)* |
| **Rolling Buffer** | **RAM Ring Buffer** (0% tulis SSD saat standby) atau **Disk Temp Buffer** (custom folder) |
| **Audio Mixing** | **WASAPI Loopback 48 kHz** (Desktop) + Mikrofon (pilihan input custom & *soft-limiter*) |
| **Voice Trigger** | **MFCC + CMVN + Sakoe-Chiba DTW** offline penuh (~10–35 KB RAM, 0 file model eksternal) |
| **Antarmuka** | Direct2D + DirectWrite minimalis matte gelap (`~32 MB` RAM saat standby) |

---

## Fitur Utama

### 1. Pipeline Video Lintas GPU & Hemat Resource
- **Universal Hardware Encoding**: Otomatis mendeteksi GPU **NVIDIA (NVENC)**, **AMD (AMF)**, atau **Intel (QSV)** serta kemampuan **HEVC** dan **AV1** masing-masing GPU saat startup, lengkap dengan *auto-fallback* aman.
- **Akurasi Warna BT.709**: Tag metadata warna BT.709 ditulis eksplisit agar hasil klip di Discord, YouTube, dan media player tidak pucat atau meleset.
- **Game Stretched 4:3 & Alt+Tab**: Menangani perubahan resolusi (`CopySubresourceRegion`) serta pemulihan otomatis saat *exclusive fullscreen* Alt+Tab (`DXGI_ERROR_ACCESS_LOST`).

### 2. Dua Mode Penyimpanan Buffer
- **RAM Ring Buffer**: Seluruh segmen berputar di memori (`IStream`), **0% menulis ke SSD/NVMe** selama menunggu momen klip, dan penggabungan instan saat disimpan.
- **Disk Temp Buffer**: Menjaga pemakaian RAM serendah mungkin dengan menyimpan segmen sementara di folder temp pilihan Anda (bisa diarahkan ke SSD/drive terpisah di tab **Sistem**).

### 3. Audio Mixer Live & Pemilih Input Mikrofon
- Mengambil audio sistem (Game/Discord/Musik) via **WASAPI Loopback 48 kHz** dengan *silent keep-alive* agar bebas suara kresek saat hening.
- **Pilih Perangkat Input Mikrofon**: Pilih mikrofon spesifik langsung di tab **Audio** (dipakai serentak untuk rekaman klip dan Voice Trigger), lengkap dengan tombol **Mute/Live** dan slider volume yang bisa digeser *real-time* saat Replay aktif.

### 4. Custom Voice Clip Trigger (100% Offline)
- Ucapkan frasa bebas pilihan Anda (misalnya `"Roman, clip that"`, `"Chat, clip that"`, atau frasa custom apa pun) untuk langsung menyimpan klip tanpa menekan tombol.
- Mendukung **3 slot rekaman contoh suara** yang tersimpan lokal ke file `.voice`, slider **Sensitivitas Deteksi (`30%–95%`)**, serta pengaturan **Cooldown (`5 / 8 / 15 dtk`)**.

### 5. Hotkey Custom & Mode Fokus Game
- Atur bebas **3 slot global shortcut** (*Toggle Replay*, *Simpan Klip*, *Mute/Unmute Mic*) dengan kombinasi `Ctrl / Shift / Alt / Win` atau gunakan preset cepat (`Ctrl+Shift+F9/F10/M`, `F8/F7/F6`, `Alt+F9/F10/M`).
- Dilengkapi **Minimize to System Tray** dan **Mode Fokus Game** (menahan notifikasi popup saat bermain *fullscreen*).

---

## Cara Pakai Singkat

1. **Jalankan `NVLiteReplayRustVoiceRecog.exe`** (atau unduh dari halaman **Releases**).
2. Di tab **Video**, pilih monitor target, mode buffer (**RAM** / **Disk Temp**), framerate (`30/60/120 FPS`), codec (**H.264 / HEVC / AV1**), durasi (`15 dtk–5 mnt`), serta resolusi/bitrate.
3. Di tab **Audio**, aktifkan audio desktop/mikrofon dan pilih perangkat mikrofon yang ingin dipakai.
4. *(Opsional)* Di tab **Voice**, ketik frasa pemicu, rekam **2–3 contoh suara**, lalu aktifkan switch **Voice Clip Trigger**.
5. Tekan tombol **Nyalakan** (default `Ctrl+Shift+F9`), mainkan game Anda, dan tekan **Simpan Klip** (`Ctrl+Shift+F10`) atau ucapkan frasa pemicu Anda untuk menyimpan file `.mp4`.

---

## Build dari Source & Self-Test

Membutuhkan **Rust toolchain** (`x86_64-pc-windows-gnu`) dan **MinGW-w64**:

```powershell
# Build portable release binary (NVLiteReplayRustVoiceRecog.exe)
.\build.ps1

# Jalankan unit test
cargo test

# Jalankan verifikasi end-to-end (Voice DSP, DXGI capture, NVENC/HEVC, DiskTemp async stop)
.\NVLiteReplayRustVoiceRecog.exe --verify-self-test
```

---

## Struktur Proyek

```text
src/
├── main.rs      # Entry point, single-instance guard, & --verify-self-test runner
├── config.rs    # Konfigurasi INI, hotkey parser, temp folder, & enum codec
├── engine.rs    # DXGI capture, D3D11 NV12 BT.709 converter, encoder MFT, & MP4 remuxer
├── cursor.rs    # Komposit kursor hardware D3D11 shader (+ GDI fallback)
├── audio.rs     # WASAPI Loopback & enumerasi/capture mikrofon + tap 16 kHz
├── voice.rs     # Pipeline Voice Recognition offline (MFCC + CMVN + Sakoe-Chiba DTW)
└── ui.rs        # Antarmuka Direct2D + DirectWrite, system tray, & global hotkeys
```

---

## Lisensi

Dirilis di bawah lisensi [MIT](LICENSE).
