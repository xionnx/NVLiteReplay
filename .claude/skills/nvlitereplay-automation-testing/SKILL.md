---
name: nvlitereplay-automation-testing
description: Automation testing for NVLiteReplay (native Win32 Rust app, Direct2D UI, DXGI capture, Media Foundation encoders, WASAPI, offline voice trigger). Use when writing or running unit tests (cargo test), the --verify-self-test runner, or black-box E2E checks (sandboxed exe + INI, global hotkeys, ffprobe/SHA256 on saved clips), when turning docs/TEST_CASES.md into automated checks, or when debugging flaky capture/save tests. Not for Playwright/browser testing — this app has no DOM.
---

# Automation Testing NVLiteReplay

Diadaptasi dari skill `logilink-automation-testing` dan `logilink-playwright-expert`. Bedanya: NVLiteReplay adalah **aplikasi native Win32** (Rust, UI Direct2D yang digambar manual). Tidak ada DOM, browser, atau server, jadi **Playwright tidak dipakai**. Prinsipnya tetap sama, tetapi alatnya diganti:

| Konsep di skill LogiLink | Padanan di NVLiteReplay |
|---|---|
| Playwright / browser | `cargo test`, `--verify-self-test`, harness PowerShell (exe + hotkey global) |
| `.feature` / Gherkin | `docs/TEST_CASES.md` (ID `TC-xxx-nn`) |
| `ALLOWED_HOSTS` guard | **Sandbox folder**: exe disalin ke folder uji, INI/`.voice`/output milik user tidak disentuh |
| DB read-only | File output/temp di sandbox saja; tidak pernah menghapus di luar sandbox |
| Page Object / locator | Tidak ada locator (UI Direct2D tidak punya elemen UIA). Kontrol lewat **INI + hotkey global**, verifikasi lewat **file hasil** |
| Trace / screenshot | Status file, `ffprobe` JSON, hash, log harness per TC |

Test case acuan: `docs/TEST_CASES.md`.

---

## 0. Aturan keamanan (wajib)

1. **Jangan menyentuh data user.** Exe membaca `NVLiteReplayRustVoiceRecog.ini` dan `.voice` **dari folder exe**. E2E wajib memakai salinan exe di folder sandbox (mis. `%TEMP%\nvlr-e2e-<runId>\`) dengan INI sendiri. Jangan pernah menjalankan E2E dari folder repo/rilis user, jangan menimpa INI/`.voice` user.
2. **Folder output & temp hanya di sandbox.** `OutputFolder` dan `TempFolder` di INI sandbox wajib menunjuk ke dalam folder sandbox. Penghapusan file hanya boleh di dalam folder sandbox (cek prefix path sebelum `Remove-Item`).
3. **Single instance global.** Aplikasi memakai mutex `Global\NVLiteReplayRustVoiceRecog_SingleInstance_Mutex`. Kalau user sedang menjalankan NVLiteReplay, instance uji akan langsung keluar dan hanya memfokuskan jendela user. Harness **wajib berhenti** (bukan membunuh proses user) bila `Get-Process NVLiteReplayRustVoiceRecog` sudah ada sebelum uji dimulai. Minta user menutupnya.
4. **Hotkey global berefek ke seluruh sistem.** Jangan jalankan E2E saat user sedang bermain/bekerja; hotkey sintetis juga diterima aplikasi lain.
5. **Perangkat sungguhan.** Capture, encoder, dan audio memakai GPU/monitor/mikrofon asli. Uji yang butuh hardware tertentu (AV1, multi-monitor, mic kedua) di-skip dengan status **N/A**, bukan Pass.
6. **Hasil wajib jujur.** Test yang tidak dijalankan (manual, hardware tidak ada, di-skip) dilaporkan **tidak dijalankan**, bukan lulus.
7. Jangan commit/push tanpa izin eksplisit user; commit **tanpa** `Co-Authored-By`.

---

## 1. Lapisan test

| Lapis | Perintah | Butuh | Cakupan |
|---|---|---|---|
| 1. Unit | `cargo test` | Toolchain saja (beberapa test memakai GPU) | Parser hotkey/INI, Annex-B converter, DSP voice, repro stop DiskTemp+HEVC |
| 2. Self-test | `NVLiteReplayRustVoiceRecog.exe --verify-self-test` | GPU + monitor | Voice DSP, DXGI capture, NVENC/HEVC, async stop DiskTemp. Output ke folder temp, membaca INI di folder exe |
| 3. E2E black-box | Harness PowerShell (bagian 4) | GPU + monitor, tidak ada instance lain | Alur user: nyala → simpan → matikan → tutup, validasi MP4 |
| 4. Manual | `docs/TEST_CASES.md` | Tester | UI visual, voice, game fullscreen, Alt+Tab, UAC, ganti perangkat audio |

Urutan eksekusi: 1 → 2 → 3 → 4. Berhenti dan laporkan bila lapis 1 gagal.

---

## 2. Toolchain (sekali per mesin)

| Tool | Cek | Catatan |
|---|---|---|
| Rust `stable-x86_64-pc-windows-gnu` | `rustc -V` | `winget install Rustlang.Rustup` lalu `rustup default stable-x86_64-pc-windows-gnu` |
| MinGW-w64 (WinLibs) | `gcc --version` | `winget install BrechtSanders.WinLibs.POSIX.UCRT`. `build.ps1` menambahkannya ke PATH otomatis |
| ffprobe / ffmpeg | `ffprobe -version` | Validasi codec, durasi, stream, ekstrak frame |
| PowerShell 5.1+ | `$PSVersionTable` | Harness E2E |

Di Git Bash, PATH perlu ditambah manual:
```bash
export PATH="$HOME/.cargo/bin:$PATH"
for d in "$LOCALAPPDATA"/Microsoft/WinGet/Packages/BrechtSanders.WinLibs*/mingw64/bin; do export PATH="$d:$PATH"; done
```

Build: `.\build.ps1` → `NVLiteReplayRustVoiceRecog.exe` di root repo (di-ignore git).

---

## 3. Unit test (`cargo test`)

- Test ada di modul `#[cfg(test)]` masing-masing file (`engine.rs`, `config.rs`, `voice.rs`, ...).
- Logic murni (parser INI/hotkey, konversi hvcC/avcC → Annex-B, MFCC/DTW, urutan segmen) **wajib** punya unit test saat diubah. Satu test kecil yang gagal bila logic rusak; tidak perlu framework tambahan.
- Test yang menyentuh GPU (mis. `repro_disk_hevc_stop`) memakai folder temp sendiri dan memastikan tidak ada `seg_<pid>_*.mp4` tersisa. Pola ini dipakai untuk test baru yang menulis file.
- Saat ini 8 test. Jumlah berubah → perbarui `TC-UNIT-01` di `docs/TEST_CASES.md`.

---

## 4. E2E black-box (harness PowerShell)

Implementasi siap pakai: **`tests/e2e.ps1`** (14 TC otomatis, hasil ke `test-results/e2e-<runId>.json`, exit code = jumlah gagal).
```powershell
.\build.ps1
powershell -ExecutionPolicy Bypass -File tests\e2e.ps1                       # semua (~6 mnt)
powershell -ExecutionPolicy Bypass -File tests\e2e.ps1 -Only TC-REC-02,TC-REG-01
```
Potongan di bawah menjelaskan polanya; ubah `tests/e2e.ps1`, bukan menyalin ulang.

### 4.1 Sandbox
```powershell
$ErrorActionPreference = 'Stop'
$runId   = Get-Date -Format 'yyyyMMdd-HHmmss'
$sandbox = Join-Path $env:TEMP "nvlr-e2e-$runId"
$exeName = 'NVLiteReplayRustVoiceRecog'

if (Get-Process $exeName -ErrorAction SilentlyContinue) {
    throw "NVLiteReplay sedang berjalan (milik user). Tutup dulu — uji dihentikan."
}
New-Item -ItemType Directory -Path "$sandbox\out", "$sandbox\tmp" | Out-Null
Copy-Item "$PSScriptRoot\..\$exeName.exe" $sandbox
```

### 4.2 INI uji
Kunci INI (lihat `ReplayConfig::save` di `src/config.rs`). Enum ditulis sebagai angka: `StorageMode` 0=RAM, 1=Disk Temp; `Codec` 0=H.264, 1=HEVC, 2=AV1. `ReplayDurationSeconds` valid 10–600; `Fps` hanya 30/60/120. Durasi segmen **tetap 5 dtk** (tidak ada di INI).

```powershell
function Write-TestIni([hashtable]$o = @{}) {
    $cfg = [ordered]@{
        ReplayDurationSeconds = 15; Fps = 60; BitrateMbps = 12
        StorageMode = 0; Codec = 0; UseNativeResolution = 'true'
        HotkeyToggle = 'Ctrl+Shift+F9'; HotkeySave = 'Ctrl+Shift+F10'; HotkeyMute = 'Ctrl+Shift+M'
        RecordDesktopAudio = 'true'; RecordMicrophone = 'false'; VoiceTriggerEnabled = 'false'
        OutputFolder = "$sandbox\out"; TempFolder = "$sandbox\tmp"
        MinimizeToTrayOnClose = 'false'; PlaySoundOnSave = 'false'; SuppressNotificationsInGame = 'true'
    }
    foreach ($k in $o.Keys) { $cfg[$k] = $o[$k] }
    ($cfg.GetEnumerator() | ForEach-Object { "$($_.Key)=$($_.Value)" }) -join "`r`n" |
        Set-Content "$sandbox\$exeName.ini" -Encoding ASCII   # BUKAN UTF8: PS 5.1 menulis BOM
}
```
`MinimizeToTrayOnClose=false` wajib supaya `CloseMainWindow()` benar-benar menutup aplikasi.

### 4.3 Hotkey global
UI Direct2D tidak punya elemen yang bisa diklik lewat UI Automation. Semua aksi lewat hotkey (`RegisterHotKey` menerima input sintetis):
```powershell
Add-Type -Namespace W -Name K -MemberDefinition @'
[DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, System.UIntPtr extra);
'@
function Send-Hotkey([byte[]]$mods, [byte]$key) {
    foreach ($m in $mods) { [W.K]::keybd_event($m, 0, 0, [UIntPtr]::Zero) }
    [W.K]::keybd_event($key, 0, 0, [UIntPtr]::Zero); [W.K]::keybd_event($key, 0, 2, [UIntPtr]::Zero)
    foreach ($m in $mods) { [W.K]::keybd_event($m, 0, 2, [UIntPtr]::Zero) }
}
$CtrlShift = [byte[]](0x11, 0x10)
function Toggle-Replay { Send-Hotkey $CtrlShift 0x78 }   # F9
function Save-Clip     { Send-Hotkey $CtrlShift 0x79 }   # F10
```

### 4.4 Menunggu: kondisi, bukan `Start-Sleep` buta
- **Mengisi buffer** memang butuh waktu nyata. `Start-Sleep -Seconds (durasi + 6)` di sini adalah **kondisi uji**, bukan penantian sembarang.
- **Selesai menyimpan**: poll file baru di `out\` sampai ukurannya stabil, dengan batas waktu.
```powershell
function Wait-NewClip([string[]]$before, [int]$timeoutSec = 30) {
    $deadline = (Get-Date).AddSeconds($timeoutSec)
    while ((Get-Date) -lt $deadline) {
        $new = Get-ChildItem "$sandbox\out\*.mp4" | Where-Object { $before -notcontains $_.FullName } | Select-Object -First 1
        if ($new) {
            $s1 = $new.Length; Start-Sleep -Milliseconds 300; $new.Refresh()
            if ($new.Length -gt 0 -and $new.Length -eq $s1) { return $new }
        }
        Start-Sleep -Milliseconds 200
    }
    throw "Klip tidak muncul dalam $timeoutSec dtk"
}
```

### 4.5 Validasi klip
```powershell
function Get-ClipInfo([string]$path) {
    $j = ffprobe -v error -show_streams -show_format -of json $path | ConvertFrom-Json
    $v = $j.streams | Where-Object codec_type -eq 'video'
    [pscustomobject]@{
        Codec = $v.codec_name; Width = $v.width; Height = $v.height
        Fps = $v.avg_frame_rate; Duration = [double]$j.format.duration
        HasAudio = [bool]($j.streams | Where-Object codec_type -eq 'audio')
        ColorSpace = $v.color_space
        Sha256 = (Get-FileHash $path -Algorithm SHA256).Hash
    }
}
```
Assert minimal tiap klip: bisa diparse, codec sesuai `Codec`, durasi ≈ `ReplayDurationSeconds` (toleransi ±1 segmen = 5 dtk), audio ada bila `RecordDesktopAudio=true`, `color_space` = `bt709`.

### 4.6 Penutupan & kebersihan
```powershell
$p = Get-Process $exeName -ErrorAction SilentlyContinue
if ($p) { [void]$p.CloseMainWindow(); if (-not $p.WaitForExit(40000)) { throw 'Aplikasi tidak keluar' } }
# Disk Temp: setelah keluar, tidak boleh ada segmen tersisa
if (Get-ChildItem "$sandbox\tmp" -Filter 'seg_*.mp4' -Recurse) { throw 'Segmen temp tidak dibersihkan' }
```
Hapus sandbox hanya bila semua TC lulus. Bila gagal, **simpan** sebagai evidence.

### 4.7 Hasil per TC
Tulis `test-results/e2e-<runId>.json`: `[{ id, status: Pass|Fail|Blocked|N/A|Not Run, actual, evidence }]`. `evidence` = path klip/ffprobe JSON/log di sandbox. Ringkasan ke user memakai tabel yang sama dengan `docs/TEST_CASES.md`.

Hasil lalu diisi ke Excel QA (format sama dengan `app_testing`: sheet Petunjuk / Summary / Test Cases, kolom A-R):
```bash
python scripts/update-xlsx.py docs/nvlitereplay-test-cases.xlsx "<tester>" <YYYY-MM-DD>
```
Script hanya menulis kolom N-R (Actual, Status, Tester, Tanggal, Catatan); formula Summary dan dropdown tidak disentuh. Hasil lapis 1-2 (`cargo test`, self-test) ditulis manual ke `test-results/*.json` dengan format yang sama. TC baru di `docs/TEST_CASES.md` wajib ditambahkan juga sebagai baris di sheet Test Cases (rentang formula Summary diperluas).

---

## 5. Pemetaan `docs/TEST_CASES.md` → level otomasi

| ID | Level | Cara |
|---|---|---|
| TC-UNIT-01 | Otomatis | `cargo test` |
| TC-STR-04 | Otomatis | `--verify-self-test`, exit code 0 |
| TC-STR-01 | Otomatis | Sandbox tanpa INI → proses hidup ≥ 3 dtk, jendela `NVLiteReplayRustVoiceRecogMainWnd` ada |
| TC-STR-02 | Otomatis | Jalankan exe kedua → keluar sendiri, proses pertama tetap hidup |
| TC-REC-01..03, 08 | Otomatis | Hotkey toggle/simpan, validasi klip (4.5) |
| TC-REC-04, 05, 06, 07 | Otomatis | Variasi waktu simpan; 05/06/07 → assert **tidak ada** file baru |
| TC-REC-09, 10 | Otomatis | `StorageMode=1`, cek `tmp\seg_*.mp4` ada saat aktif dan hilang setelah keluar |
| TC-REC-11..14 | Otomatis | Variasi `Codec`/`Fps`/`ReplayDurationSeconds`; AV1/HEVC → N/A bila GPU tidak mendukung |
| TC-REG-01 | Otomatis | 3 klip jeda 10 dtk → SHA256 berbeda semua. Urutan isi: semi-otomatis (bagian 6) |
| TC-REG-02, 03 | Semi | Simpan berulang jeda 5–6 dtk (Disk + HEVC); urutan isi dicek lewat frame (bagian 6) |
| TC-REG-06 | Otomatis | 2× `Save-Clip` cepat (RAM, 15 dtk) → bila nama sama detik, file kedua `_2.mp4`, keduanya valid |
| TC-REG-07, 08 | Otomatis | Disk, durasi besar → `Save-Clip` lalu langsung toggle/`CloseMainWindow` → klip utuh (ffprobe OK, durasi penuh) |
| TC-REG-09 | Otomatis | Disk, hapus `tmp\seg_*.mp4` (hanya di sandbox) lalu simpan → tidak ada MP4 baru/0 byte |
| TC-REG-12 | Otomatis | Tulis `.voice` korup di sandbox → proses tetap hidup ≥ 5 dtk |
| TC-REG-04, 05, 10, 11, 13 | Manual | Dialog folder, cabut flashdisk/mic |
| TC-AUD-*, TC-VOC-*, TC-SYS-02..09, TC-CAP-* | Manual (sebagian semi) | Butuh telinga/mata, game, perangkat fisik. TC-CAP-06 bisa semi: harness simpan klip tiap 15 mnt + catat `WorkingSet64` & GDI handle |

---

## 6. Verifikasi urutan isi klip (anti-duplikat)

Hash berbeda belum membuktikan urutan benar. Untuk TC-REG-01..03:
1. Tampilkan penanda waktu besar di monitor target (mis. jendela jam/stopwatch).
2. Setelah klip tersimpan, ekstrak 1 frame per detik:
   `ffmpeg -v error -i clip.mp4 -vf fps=1 "$sandbox\frames\%03d.png"`
3. Bandingkan penanda waktu antar frame: harus **naik terus** tanpa lompat mundur atau celah ≥ 5 dtk. Pemeriksaan bisa dilakukan tester, atau Claude dengan membaca PNG-nya.
4. Frame terakhir ≈ waktu hotkey simpan ditekan.

---

## 7. Best practice

**Struktur**
- Satu fungsi harness = satu TC, diberi ID `TC-xxx-nn` yang sama dengan dokumen.
- Tiap TC mandiri: INI ditulis ulang, aplikasi dijalankan dan ditutup di TC itu sendiri. Jangan berbagi state antar TC.
- Variasi parameter (codec × mode × fps) memakai tabel data, bukan copy-paste fungsi.

**Menunggu & flaky**
- Tidak ada `Start-Sleep` untuk "semoga sudah selesai". Poll kondisi dengan batas waktu (4.4).
- Jangan menambah retry untuk menutupi flaky. Cari penyebabnya: timing rotasi segmen 5 dtk, finalize background, beban GPU.
- Ulangi TC yang dicurigai flaky 10× dan laporkan rasio gagal.

**Evidence & analisa**
- Simpan klip, JSON ffprobe, dan log harness per TC di sandbox (folder dipertahankan bila gagal).
- Tentukan penyebab kegagalan:
  - **Bug aplikasi**: laporkan dengan evidence dan langkah reproduksi.
  - **Harness salah** (path, timeout, hotkey bentrok): perbaiki harness.
  - **Lingkungan** (GPU tidak mendukung codec, instance user berjalan, monitor mati): `Blocked`/`N/A`.
- Known issues di akhir `docs/TEST_CASES.md` (#6, #7, #8, #13): catat hasilnya, jangan langsung `Fail`.

---

## 8. Troubleshooting

| Gejala | Penyebab umum | Solusi |
|---|---|---|
| Exe uji langsung keluar | Instance lain (milik user) masih berjalan, mutex global | Minta user menutup aplikasinya; jangan bunuh proses user |
| Hotkey tidak bereaksi | Kombinasi sudah didaftarkan aplikasi lain | Pakai hotkey lain di INI sandbox (mis. `Ctrl+Alt+F11`) |
| `CloseMainWindow()` tidak menutup | `MinimizeToTrayOnClose=true` | Set `false` di INI sandbox |
| Klip tidak muncul | Replay belum aktif / buffer kosong / encoder gagal | Cek urutan toggle → tunggu ≥ 6 dtk → simpan; lihat status di UI |
| Durasi klip lebih pendek | Buffer belum penuh | Tunggu `ReplayDurationSeconds + 6` dtk sebelum simpan |
| Codec tidak sesuai INI | GPU tidak mendukung → fallback otomatis | Tandai `N/A` untuk codec itu |
| `seg_*.mp4` tersisa di tmp | Bug cleanup atau proses dibunuh paksa | Hanya `Fail` bila aplikasi keluar normal |
| `cargo`/`gcc` tidak ditemukan | PATH belum memuat toolchain | Lihat bagian 2; buka terminal baru setelah install rustup |
| Link error saat `cargo test` | Target MSVC dipakai tanpa VS Build Tools | `rustup default stable-x86_64-pc-windows-gnu` |
| Jendela utama "tidak muncul" padahal proses hidup | `FindWindowW(cls, $null)`: PowerShell mengubah `$null` jadi `""` | Pakai `[NullString]::Value` |
| Semua TC berikutnya "keluar saat startup (exit 0)" | Proses uji sebelumnya masih hidup dan memegang mutex | Set `$script:app` segera setelah `Start-Process`; tutup di `finally` |
| Setting INI pertama diabaikan | INI ditulis dengan BOM (`-Encoding UTF8` di PS 5.1) | `-Encoding ASCII` |
| `-Only A,B` tidak menjalankan apa pun | `-File` mengirim `"A,B"` sebagai satu string | Split koma di dalam script (sudah di `tests/e2e.ps1`) |
| fps `19290000/321499` dianggap salah | `avg_frame_rate` dihitung dari timestamp nyata | Bandingkan angka dengan toleransi ±0.5, bukan string `60/1` |
| AV1: buffer tetap `0 / N dtk`, segmen 0 byte | Encoder AV1 terdaftar di Windows tapi tidak berfungsi di GPU (mis. Intel UHD) | Bug aplikasi (tidak ada fallback/error). Laporkan `Fail`, bukan `N/A` |
