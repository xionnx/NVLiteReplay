use std::cmp::Ordering as CmpOrdering;
use std::ffi::c_void;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, WPARAM};
use windows::Win32::Networking::WinHttp::{
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders,
    WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetOption,
    WinHttpSetTimeouts, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
    WINHTTP_FLAG_SECURE, WINHTTP_OPTION_REDIRECT_POLICY,
    WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP, WINHTTP_QUERY_CONTENT_LENGTH,
    WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_FLAG_NUMBER64, WINHTTP_QUERY_STATUS_CODE,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use crate::engine::{ReplayEngine, WM_APP_UPDATE};

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GITHUB_RELEASES_PAGE: &str = "https://github.com/xionnx/NVLiteReplay/releases";
const GITHUB_RELEASES_API: &str =
    "https://api.github.com/repos/xionnx/NVLiteReplay/releases?per_page=8";
const GITHUB_REPO_DOWNLOAD_PREFIX: &str = "https://github.com/xionnx/NVLiteReplay/";
const MAX_API_JSON_BYTES: usize = 512 * 1024;
const MAX_DOWNLOAD_EXE_BYTES: u64 = 32 * 1024 * 1024;
const MIN_VALID_EXE_BYTES: u64 = 100_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemVer {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemVer {
    /// Parses tags like `v1.0.2`, `V1.0.1`, `v.1.0.0`, or `1.2.3-beta.1`.
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw
            .trim()
            .trim_start_matches(|c: char| c == 'v' || c == 'V' || c == '.' || c.is_whitespace());
        if trimmed.is_empty() {
            return None;
        }
        let core = trimmed.split(['-', '+']).next().unwrap_or(trimmed).trim();
        let mut parts = core.split('.');
        let major = parts.next()?.parse::<u32>().ok()?;
        let minor = parts.next().unwrap_or("0").parse::<u32>().ok()?;
        let patch = parts.next().unwrap_or("0").parse::<u32>().ok()?;
        Some(Self {
            major,
            minor,
            patch,
        })
    }

    pub fn display_tag(self) -> String {
        format!("v{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> CmpOrdering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch))
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GithubReleaseInfo {
    pub tag_name: String,
    pub version: SemVer,
    pub release_name: String,
    pub html_url: String,
    pub notes_summary: String,
    pub exe_url: Option<String>,
    pub exe_name: Option<String>,
    pub exe_size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate {
        latest_tag: String,
        html_url: String,
    },
    Available {
        latest_tag: String,
        release_name: String,
        notes_summary: String,
        html_url: String,
        exe_url: Option<String>,
        exe_name: Option<String>,
        exe_size: u64,
    },
    Downloading {
        latest_tag: String,
        downloaded_bytes: u64,
        total_bytes: u64,
    },
    Installing {
        latest_tag: String,
    },
    ReadyToRestart {
        latest_tag: String,
    },
    Error(String),
}

impl UpdateState {
    pub fn is_busy(&self) -> bool {
        matches!(
            self,
            UpdateState::Checking | UpdateState::Downloading { .. } | UpdateState::Installing { .. }
        )
    }

    pub fn release_page_url(&self) -> &str {
        match self {
            UpdateState::UpToDate { html_url, .. } | UpdateState::Available { html_url, .. } => {
                if html_url.is_empty() {
                    GITHUB_RELEASES_PAGE
                } else {
                    html_url.as_str()
                }
            }
            _ => GITHUB_RELEASES_PAGE,
        }
    }
}

pub struct UpdateManager {
    state: Mutex<UpdateState>,
    busy: AtomicBool,
    ui_hwnd: Mutex<isize>,
}

impl UpdateManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(UpdateState::Idle),
            busy: AtomicBool::new(false),
            ui_hwnd: Mutex::new(0),
        })
    }

    pub fn set_ui_hwnd(&self, hwnd: HWND) {
        if let Ok(mut g) = self.ui_hwnd.lock() {
            *g = hwnd.0 as isize;
        }
    }

    pub fn snapshot(&self) -> UpdateState {
        self.state
            .lock()
            .map(|g| g.clone())
            .unwrap_or(UpdateState::Idle)
    }

    fn set_state(&self, next: UpdateState, force_exit: bool) {
        if let Ok(mut g) = self.state.lock() {
            *g = next;
        }
        let raw_hwnd = self.ui_hwnd.lock().map(|g| *g).unwrap_or(0);
        if raw_hwnd != 0 {
            unsafe {
                let _ = PostMessageW(
                    HWND(raw_hwnd as *mut _),
                    WM_APP_UPDATE,
                    WPARAM(if force_exit { 1 } else { 0 }),
                    LPARAM(0),
                );
            }
        }
    }

    /// Queries GitHub Releases API asynchronously without blocking the UI thread.
    /// Silent background startup checks (`user_initiated == false`) return to `Idle`
    /// quietly if offline or throttled rather than showing error text in the footer.
    pub fn check_for_updates_async(self: &Arc<Self>, user_initiated: bool) {
        if self.busy.swap(true, Ordering::AcqRel) {
            return;
        }
        if user_initiated {
            self.set_state(UpdateState::Checking, false);
        }

        let mgr = Arc::clone(self);
        if thread::Builder::new()
            .name("NVLiteReplay_UpdateCheck".to_string())
            .spawn(move || {
                let outcome = fetch_latest_github_release();
                match outcome {
                    Ok(Some(rel)) => {
                        let current =
                            SemVer::parse(APP_VERSION).unwrap_or(SemVer {
                                major: 1,
                                minor: 0,
                                patch: 0,
                            });
                        if rel.version > current {
                            mgr.set_state(
                                UpdateState::Available {
                                    latest_tag: rel.version.display_tag(),
                                    release_name: rel.release_name,
                                    notes_summary: rel.notes_summary,
                                    html_url: rel.html_url,
                                    exe_url: rel.exe_url,
                                    exe_name: rel.exe_name,
                                    exe_size: rel.exe_size,
                                },
                                false,
                            );
                        } else {
                            mgr.set_state(
                                UpdateState::UpToDate {
                                    latest_tag: rel.tag_name,
                                    html_url: rel.html_url,
                                },
                                false,
                            );
                        }
                    }
                    Ok(None) => {
                        if user_initiated {
                            mgr.set_state(
                                UpdateState::Error(
                                    "Belum ada rilis publik di GitHub Releases.".to_string(),
                                ),
                                false,
                            );
                        }
                    }
                    Err(err) => {
                        if user_initiated {
                            mgr.set_state(UpdateState::Error(err), false);
                        } else {
                            let cur = mgr.snapshot();
                            if matches!(cur, UpdateState::Checking) {
                                mgr.set_state(UpdateState::Idle, false);
                            }
                        }
                    }
                }
                mgr.busy.store(false, Ordering::Release);
            })
            .is_err()
        {
            self.busy.store(false, Ordering::Release);
            if user_initiated {
                self.set_state(
                    UpdateState::Error("Gagal membuat thread pemeriksa update.".to_string()),
                    false,
                );
            }
        }
    }

    /// Downloads the verified `.exe` asset for the currently discovered release, stops
    /// capture cleanly, atomically swaps the binary in-place, and triggers relaunch.
    pub fn install_available_update_async(self: &Arc<Self>, engine: Arc<ReplayEngine>) {
        let (latest_tag, exe_url, exe_size) = match self.snapshot() {
            UpdateState::Available {
                latest_tag,
                exe_url,
                exe_size,
                ..
            } => (latest_tag, exe_url, exe_size),
            _ => return,
        };

        let Some(download_url) = exe_url else {
            return;
        };

        if !is_allowed_github_url(&download_url) {
            self.set_state(
                UpdateState::Error("URL unduhan rilis tidak valid (bukan GitHub repo resmi).".to_string()),
                false,
            );
            return;
        }

        if self.busy.swap(true, Ordering::AcqRel) {
            return;
        }

        self.set_state(
            UpdateState::Downloading {
                latest_tag: latest_tag.clone(),
                downloaded_bytes: 0,
                total_bytes: exe_size,
            },
            false,
        );

        let mgr = Arc::clone(self);
        if thread::Builder::new()
            .name("NVLiteReplay_UpdateInstall".to_string())
            .spawn(move || {
                let res = perform_in_place_update(
                    &mgr,
                    &engine,
                    &latest_tag,
                    &download_url,
                    exe_size,
                );
                match res {
                    Ok(()) => {
                        mgr.set_state(
                            UpdateState::ReadyToRestart { latest_tag },
                            true,
                        );
                    }
                    Err(msg) => {
                        mgr.set_state(UpdateState::Error(msg), false);
                    }
                }
                mgr.busy.store(false, Ordering::Release);
            })
            .is_err()
        {
            self.busy.store(false, Ordering::Release);
            self.set_state(
                UpdateState::Error("Gagal memulai proses pemasangan update.".to_string()),
                false,
            );
        }
    }
}

fn perform_in_place_update(
    mgr: &Arc<UpdateManager>,
    engine: &Arc<ReplayEngine>,
    latest_tag: &str,
    download_url: &str,
    expected_size: u64,
) -> Result<(), String> {
    let current_exe = std::env::current_exe()
        .map_err(|_| "Tidak dapat membaca lokasi file .exe saat ini.".to_string())?;
    let exe_dir = current_exe
        .parent()
        .ok_or_else(|| "Folder aplikasi tidak ditemukan.".to_string())?;
    let exe_name = current_exe
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("NVLiteReplayRustVoiceRecog.exe");

    // Clean up any leftover .old / .update.tmp artifacts BEFORE creating the new download file!
    cleanup_old_binaries_in_dir(exe_dir);
    let tmp_path = exe_dir.join(format!("{}.update.tmp", exe_name));
    let _ = fs::remove_file(&tmp_path);

    let mut out_file = File::create(&tmp_path).map_err(|_| {
        "Gagal menulis ke folder aplikasi (coba pindahkan dari Program Files).".to_string()
    })?;

    let mut last_pct: i32 = -1;
    let dl_res = winhttp_download_stream(download_url, false, MAX_DOWNLOAD_EXE_BYTES, |chunk, dl, total| {
        out_file.write_all(chunk)?;
        let denom = if total > 0 { total } else { expected_size };
        let pct = if denom > 0 {
            ((dl.saturating_mul(100)) / denom).min(100) as i32
        } else {
            -1
        };
        if pct != last_pct || (denom == 0 && dl % (256 * 1024) == 0) {
            last_pct = pct;
            mgr.set_state(
                UpdateState::Downloading {
                    latest_tag: latest_tag.to_string(),
                    downloaded_bytes: dl,
                    total_bytes: denom,
                },
                false,
            );
        }
        Ok(())
    });

    if let Err(err) = dl_res {
        drop(out_file);
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }

    if out_file.flush().is_err() {
        drop(out_file);
        let _ = fs::remove_file(&tmp_path);
        return Err("Gagal menyelesaikan penulisan file update.".to_string());
    }
    drop(out_file);

    let meta_len = fs::metadata(&tmp_path).map(|m| m.len()).unwrap_or(0);
    if expected_size >= MIN_VALID_EXE_BYTES && meta_len != expected_size {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!(
            "Ukuran file unduhan tidak cocok ({} / {} byte).",
            meta_len, expected_size
        ));
    }
    if !verify_windows_x64_pe_file(&tmp_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err("File unduhan bukan binary Windows x64 yang valid.".to_string());
    }

    mgr.set_state(
        UpdateState::Installing {
            latest_tag: latest_tag.to_string(),
        },
        false,
    );

    // Wait briefly if user is saving a replay clip, then cleanly stop capture before swap.
    for _ in 0..60 {
        if !engine.is_saving() {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    if engine.is_running() || engine.is_stopping() {
        engine.stop();
    }

    let pid = unsafe { GetCurrentProcessId() };
    let backup_path = swap_downloaded_binary_in_place(&current_exe, &tmp_path, pid)?;

    if Command::new(&current_exe)
        .arg("--wait-pid")
        .arg(pid.to_string())
        .arg("--updated-from")
        .arg(APP_VERSION)
        .spawn()
        .is_err()
    {
        // Restore old binary cleanly if launching the new one failed immediately.
        let _ = fs::remove_file(&current_exe);
        let _ = fs::rename(&backup_path, &current_exe);
        let _ = fs::remove_file(&tmp_path);
        return Err("Gagal menjalankan versi baru — versi lama dipulihkan.".to_string());
    }

    Ok(())
}

/// Atomically swaps `current_exe` with the verified downloaded `tmp_path` (`*.update.tmp`),
/// retrying briefly or falling back to copy if Windows Defender holds a transient read handle.
fn swap_downloaded_binary_in_place(
    current_exe: &Path,
    tmp_path: &Path,
    pid: u32,
) -> Result<PathBuf, String> {
    let exe_dir = current_exe
        .parent()
        .ok_or_else(|| "Folder aplikasi tidak ditemukan.".to_string())?;
    let exe_name = current_exe
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("NVLiteReplayRustVoiceRecog.exe");

    let mut backup_path = exe_dir.join(format!("{}.old", exe_name));
    if backup_path.exists() && fs::remove_file(&backup_path).is_err() {
        backup_path = exe_dir.join(format!("{}.{}.old", exe_name, pid));
        let _ = fs::remove_file(&backup_path);
    }

    let mut moved_old = false;
    for attempt in 0..4 {
        if attempt > 0 {
            thread::sleep(Duration::from_millis(50));
        }
        if fs::rename(current_exe, &backup_path).is_ok() {
            moved_old = true;
            break;
        }
    }
    if !moved_old {
        let _ = fs::remove_file(tmp_path);
        return Err("File .exe sedang dikunci Windows — silakan unduh manual via GitHub.".to_string());
    }

    let mut installed_new = false;
    for attempt in 0..6 {
        if attempt > 0 {
            thread::sleep(Duration::from_millis(60));
        }
        if fs::rename(tmp_path, current_exe).is_ok() {
            installed_new = true;
            break;
        }
    }
    if !installed_new {
        // Fallback: if antivirus/SmartScreen holds a shared read lock on tmp_path blocking MoveFile,
        // copy the verified bytes to current_exe and remove tmp_path on best-effort.
        if let Ok(copied) = fs::copy(tmp_path, current_exe) {
            if copied >= MIN_VALID_EXE_BYTES || copied == fs::metadata(tmp_path).map(|m| m.len()).unwrap_or(0) {
                let _ = fs::remove_file(tmp_path);
                installed_new = true;
            }
        }
    }

    if !installed_new {
        let _ = fs::remove_file(current_exe);
        let _ = fs::rename(&backup_path, current_exe);
        let _ = fs::remove_file(tmp_path);
        return Err("Gagal mengganti file aplikasi — perubahan dibatalkan aman.".to_string());
    }

    Ok(backup_path)
}

/// Handles `--wait-pid` / `--updated-from` flags on relaunch after an in-place update
/// and cleans up leftover `.old` / `.update.tmp` files in the executable folder.
pub fn handle_update_startup_args(args: &[String]) -> Option<String> {
    let mut wait_pid: Option<u32> = None;
    let mut updated_from: Option<String> = None;

    let mut i = 1usize;
    while i < args.len() {
        if args[i] == "--wait-pid" {
            if let Some(val) = args.get(i + 1) {
                wait_pid = val.parse::<u32>().ok();
                i += 2;
                continue;
            }
        } else if args[i] == "--updated-from" {
            if let Some(val) = args.get(i + 1) {
                updated_from = Some(val.clone());
                i += 2;
                continue;
            }
        }
        i += 1;
    }

    if let Some(old_pid) = wait_pid {
        unsafe {
            if let Ok(hproc) = OpenProcess(PROCESS_SYNCHRONIZE, false, old_pid) {
                let _ = WaitForSingleObject(hproc, 8_000);
                let _ = CloseHandle(hproc);
            }
        }
        thread::sleep(Duration::from_millis(80));
    }

    if let Ok(cur) = std::env::current_exe() {
        if let Some(dir) = cur.parent() {
            cleanup_old_binaries_in_dir(dir);
        }
    }

    updated_from.map(|prev| {
        format!(
            "Berhasil diperbarui dari v{} ke v{}!",
            prev.trim_start_matches(['v', 'V', '.']),
            APP_VERSION
        )
    })
}

fn cleanup_old_binaries_in_dir(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for ent in entries.flatten() {
        let path = ent.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if (name.contains(".exe.") && name.ends_with(".old")) || name.ends_with(".update.tmp") {
            let _ = fs::remove_file(&path);
        }
    }
}

pub fn verify_windows_x64_pe_file(path: &Path) -> bool {
    let Ok(data) = fs::read(path) else {
        return false;
    };
    verify_windows_x64_pe_bytes(&data)
}

fn verify_windows_x64_pe_bytes(data: &[u8]) -> bool {
    if (data.len() as u64) < MIN_VALID_EXE_BYTES || data.len() < 128 {
        return false;
    }
    if &data[0..2] != b"MZ" {
        return false;
    }
    let e_lfanew = u32::from_le_bytes([data[0x3C], data[0x3D], data[0x3E], data[0x3F]]) as usize;
    if e_lfanew + 6 > data.len() {
        return false;
    }
    if &data[e_lfanew..e_lfanew + 4] != b"PE\0\0" {
        return false;
    }
    let machine = u16::from_le_bytes([data[e_lfanew + 4], data[e_lfanew + 5]]);
    machine == 0x8664
}

pub fn is_allowed_github_url(url: &str) -> bool {
    url.starts_with(GITHUB_REPO_DOWNLOAD_PREFIX)
        && !url.chars().any(|c| c.is_control() || c == ' ')
}

fn fetch_latest_github_release() -> Result<Option<GithubReleaseInfo>, String> {
    let mut raw = Vec::with_capacity(16 * 1024);
    winhttp_download_stream(GITHUB_RELEASES_API, true, MAX_API_JSON_BYTES as u64, |chunk, _, _| {
        if raw.len() + chunk.len() > MAX_API_JSON_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Respon GitHub terlalu besar",
            ));
        }
        raw.extend_from_slice(chunk);
        Ok(())
    })?;

    let text = String::from_utf8(raw)
        .map_err(|_| "Respon JSON GitHub bukan UTF-8 valid.".to_string())?;
    let releases = parse_github_releases_json(&text)
        .ok_or_else(|| "Gagal membaca daftar rilis GitHub.".to_string())?;

    Ok(releases.into_iter().max_by_key(|r| r.version))
}

struct WinHttpHandle(*mut c_void);

impl WinHttpHandle {
    fn new(raw: *mut c_void) -> Option<Self> {
        if raw.is_null() {
            None
        } else {
            Some(Self(raw))
        }
    }
}

impl Drop for WinHttpHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = WinHttpCloseHandle(self.0);
            }
        }
    }
}

fn wide_null(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = s.encode_utf16().collect();
    v.push(0);
    v
}

pub fn parse_https_url_parts(url: &str) -> Option<(String, u16, String)> {
    let rest = url.strip_prefix("https://")?;
    let (host_port, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };
    if host_port.is_empty() {
        return None;
    }
    let (host, port) = match host_port.rsplit_once(':') {
        Some((h, p)) if !h.is_empty() => (h.to_string(), p.parse::<u16>().ok()?),
        _ => (host_port.to_string(), 443),
    };
    Some((host, port, path.to_string()))
}

fn winhttp_download_stream<F>(
    url: &str,
    api_headers: bool,
    max_bytes: u64,
    mut on_chunk: F,
) -> Result<(), String>
where
    F: FnMut(&[u8], u64, u64) -> std::io::Result<()>,
{
    let (host, port, path) =
        parse_https_url_parts(url).ok_or_else(|| "Format URL HTTPS tidak valid.".to_string())?;

    unsafe {
        let agent = wide_null(&format!("NVLiteReplay/{}", APP_VERSION));
        let session = WinHttpHandle::new(WinHttpOpen(
            PCWSTR(agent.as_ptr()),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        ))
        .or_else(|| {
            WinHttpHandle::new(WinHttpOpen(
                PCWSTR(agent.as_ptr()),
                WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                PCWSTR::null(),
                PCWSTR::null(),
                0,
            ))
        })
        .ok_or_else(|| "Gagal menginisialisasi koneksi WinHTTP Windows.".to_string())?;

        let _ = WinHttpSetTimeouts(session.0, 8_000, 10_000, 10_000, 30_000);

        let host_w = wide_null(&host);
        let connect = WinHttpHandle::new(WinHttpConnect(session.0, PCWSTR(host_w.as_ptr()), port, 0))
            .ok_or_else(|| "Tidak dapat tersambung ke server GitHub.".to_string())?;

        let verb_w = wide_null("GET");
        let path_w = wide_null(&path);
        let request = WinHttpHandle::new(WinHttpOpenRequest(
            connect.0,
            PCWSTR(verb_w.as_ptr()),
            PCWSTR(path_w.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        ))
        .ok_or_else(|| "Gagal menyiapkan request HTTPS.".to_string())?;

        let policy: u32 = WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP;
        let _ = WinHttpSetOption(
            Some(request.0 as *const _),
            WINHTTP_OPTION_REDIRECT_POLICY,
            Some(&policy.to_le_bytes()),
        );

        let hdrs_utf16: Option<Vec<u16>> = if api_headers {
            Some(
                "Accept: application/vnd.github+json\r\nX-GitHub-Api-Version: 2022-11-28\r\n"
                    .encode_utf16()
                    .collect(),
            )
        } else {
            None
        };

        WinHttpSendRequest(request.0, hdrs_utf16.as_deref(), None, 0, 0, 0)
            .map_err(|_| "Gagal terhubung ke GitHub (cek koneksi internet).".to_string())?;

        WinHttpReceiveResponse(request.0, std::ptr::null_mut())
            .map_err(|_| "Server GitHub tidak merespon tepat waktu.".to_string())?;

        let mut status_code: u32 = 0;
        let mut status_len: u32 = std::mem::size_of::<u32>() as u32;
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some(&mut status_code as *mut u32 as *mut c_void),
            &mut status_len,
            std::ptr::null_mut(),
        )
        .map_err(|_| "Gagal membaca status HTTP dari GitHub.".to_string())?;

        if status_code == 403 || status_code == 429 {
            return Err("Batas API GitHub tercapai — coba lagi nanti atau buka GitHub.".to_string());
        }
        if status_code != 200 {
            return Err(format!("GitHub merespon HTTP {} saat memeriksa rilis.", status_code));
        }

        let mut total_bytes: u64 = 0;
        let mut cl_len: u32 = std::mem::size_of::<u64>() as u32;
        if WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_CONTENT_LENGTH | WINHTTP_QUERY_FLAG_NUMBER64,
            PCWSTR::null(),
            Some(&mut total_bytes as *mut u64 as *mut c_void),
            &mut cl_len,
            std::ptr::null_mut(),
        )
        .is_err()
        {
            total_bytes = 0;
        }
        if total_bytes > max_bytes {
            return Err("Ukuran file dari GitHub melebihi batas aman.".to_string());
        }

        let mut buf = vec![0u8; 16 * 1024];
        let mut downloaded: u64 = 0;
        loop {
            let mut bytes_read: u32 = 0;
            WinHttpReadData(
                request.0,
                buf.as_mut_ptr() as *mut c_void,
                buf.len() as u32,
                &mut bytes_read,
            )
            .map_err(|_| "Koneksi terputus saat mengunduh dari GitHub.".to_string())?;

            if bytes_read == 0 {
                break;
            }
            downloaded = downloaded.saturating_add(bytes_read as u64);
            if downloaded > max_bytes {
                return Err("Ukuran data melebihi batas aman.".to_string());
            }
            on_chunk(&buf[..bytes_read as usize], downloaded, total_bytes)
                .map_err(|e| format!("Gagal menyimpan data update: {}", e))?;
        }
    }

    Ok(())
}

// ---------- Minimal Zero-Crate Recursive-Descent JSON Parser for GitHub Releases ----------

#[derive(Debug)]
enum JsonVal {
    Null,
    Bool(bool),
    Num(u64),
    Str(String),
    Arr(Vec<JsonVal>),
    Obj(Vec<(String, JsonVal)>),
}

impl JsonVal {
    fn get(&self, key: &str) -> Option<&JsonVal> {
        let JsonVal::Obj(entries) = self else {
            return None;
        };
        entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            JsonVal::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        match self {
            JsonVal::Bool(b) => Some(*b),
            _ => None,
        }
    }

    fn as_u64(&self) -> Option<u64> {
        match self {
            JsonVal::Num(n) => Some(*n),
            _ => None,
        }
    }
}

struct JsonCursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> JsonCursor<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            bytes: src.as_bytes(),
            pos: 0,
        }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_ws();
        self.bytes.get(self.pos).copied()
    }

    fn parse_value(&mut self, depth: usize) -> Option<JsonVal> {
        if depth > 24 {
            return None;
        }
        match self.peek()? {
            b'{' => self.parse_object(depth + 1),
            b'[' => self.parse_array(depth + 1),
            b'"' => Some(JsonVal::Str(self.parse_string()?)),
            b't' => self.consume_lit(b"true", JsonVal::Bool(true)),
            b'f' => self.consume_lit(b"false", JsonVal::Bool(false)),
            b'n' => self.consume_lit(b"null", JsonVal::Null),
            b'-' | b'0'..=b'9' => Some(JsonVal::Num(self.parse_number()?)),
            _ => None,
        }
    }

    fn consume_lit(&mut self, lit: &[u8], val: JsonVal) -> Option<JsonVal> {
        if self.bytes.get(self.pos..self.pos + lit.len()) == Some(lit) {
            self.pos += lit.len();
            Some(val)
        } else {
            None
        }
    }

    fn parse_number(&mut self) -> Option<u64> {
        let start = self.pos;
        if self.bytes.get(self.pos) == Some(&b'-') {
            self.pos += 1;
        }
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        if self.bytes.get(self.pos) == Some(&b'.') {
            self.pos += 1;
            while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        if matches!(self.bytes.get(self.pos), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.bytes.get(self.pos), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        let s = std::str::from_utf8(&self.bytes[start..self.pos]).ok()?;
        Some(s.parse::<u64>().unwrap_or(0))
    }

    fn parse_string(&mut self) -> Option<String> {
        if self.bytes.get(self.pos) != Some(&b'"') {
            return None;
        }
        self.pos += 1;
        let mut out = String::new();
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            if b == b'"' {
                self.pos += 1;
                return Some(out);
            }
            if b == b'\\' {
                self.pos += 1;
                let esc = *self.bytes.get(self.pos)?;
                self.pos += 1;
                match esc {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'b' => out.push('\u{0008}'),
                    b'f' => out.push('\u{000C}'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        let hex = std::str::from_utf8(self.bytes.get(self.pos..self.pos + 4)?).ok()?;
                        let code = u16::from_str_radix(hex, 16).ok()?;
                        self.pos += 4;
                        if (0xD800..=0xDBFF).contains(&code)
                            && self.bytes.get(self.pos..self.pos + 2) == Some(b"\\u")
                        {
                            let hex2 =
                                std::str::from_utf8(self.bytes.get(self.pos + 2..self.pos + 6)?)
                                    .ok()?;
                            let low = u16::from_str_radix(hex2, 16).ok()?;
                            self.pos += 6;
                            if let Some(ch) = char::decode_utf16([code, low]).next().and_then(|r| r.ok()) {
                                out.push(ch);
                            }
                        } else if let Some(ch) = char::from_u32(code as u32) {
                            out.push(ch);
                        }
                    }
                    _ => return None,
                }
                continue;
            }
            // Fast path: advance full UTF-8 char
            let rest = std::str::from_utf8(&self.bytes[self.pos..]).ok()?;
            let ch = rest.chars().next()?;
            out.push(ch);
            self.pos += ch.len_utf8();
        }
        None
    }

    fn parse_array(&mut self, depth: usize) -> Option<JsonVal> {
        self.pos += 1;
        let mut items = Vec::new();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Some(JsonVal::Arr(items));
        }
        loop {
            items.push(self.parse_value(depth)?);
            match self.peek()? {
                b',' => self.pos += 1,
                b']' => {
                    self.pos += 1;
                    return Some(JsonVal::Arr(items));
                }
                _ => return None,
            }
        }
    }

    fn parse_object(&mut self, depth: usize) -> Option<JsonVal> {
        self.pos += 1;
        let mut pairs = Vec::new();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Some(JsonVal::Obj(pairs));
        }
        loop {
            if self.peek()? != b'"' {
                return None;
            }
            let key = self.parse_string()?;
            if self.peek()? != b':' {
                return None;
            }
            self.pos += 1;
            let val = self.parse_value(depth)?;
            pairs.push((key, val));
            match self.peek()? {
                b',' => self.pos += 1,
                b'}' => {
                    self.pos += 1;
                    return Some(JsonVal::Obj(pairs));
                }
                _ => return None,
            }
        }
    }
}

pub fn parse_github_releases_json(json_str: &str) -> Option<Vec<GithubReleaseInfo>> {
    let mut cursor = JsonCursor::new(json_str);
    let root = cursor.parse_value(0)?;

    let list: Vec<&JsonVal> = match &root {
        JsonVal::Arr(arr) => arr.iter().collect(),
        obj @ JsonVal::Obj(_) => vec![obj],
        _ => return None,
    };

    let mut parsed = Vec::new();
    for item in list {
        if item.get("draft").and_then(JsonVal::as_bool).unwrap_or(false) {
            continue;
        }
        if item
            .get("prerelease")
            .and_then(JsonVal::as_bool)
            .unwrap_or(false)
        {
            continue;
        }
        let Some(tag_name) = item.get("tag_name").and_then(JsonVal::as_str) else {
            continue;
        };
        let Some(version) = SemVer::parse(tag_name) else {
            continue;
        };

        let release_name = item
            .get("name")
            .and_then(JsonVal::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(tag_name)
            .to_string();
        let html_url = item
            .get("html_url")
            .and_then(JsonVal::as_str)
            .unwrap_or(GITHUB_RELEASES_PAGE)
            .to_string();
        let notes_summary = item
            .get("body")
            .and_then(JsonVal::as_str)
            .map(summarize_release_notes)
            .unwrap_or_default();

        let (exe_url, exe_name, exe_size) = select_exe_asset(item.get("assets"));

        parsed.push(GithubReleaseInfo {
            tag_name: tag_name.to_string(),
            version,
            release_name,
            html_url,
            notes_summary,
            exe_url,
            exe_name,
            exe_size,
        });
    }

    Some(parsed)
}

fn select_exe_asset(assets_val: Option<&JsonVal>) -> (Option<String>, Option<String>, u64) {
    let Some(JsonVal::Arr(assets)) = assets_val else {
        return (None, None, 0);
    };

    let mut best: Option<(i32, String, String, u64)> = None;
    for asset in assets {
        let Some(name) = asset.get("name").and_then(JsonVal::as_str) else {
            continue;
        };
        let Some(url) = asset
            .get("browser_download_url")
            .and_then(JsonVal::as_str)
        else {
            continue;
        };
        if !is_allowed_github_url(url) {
            continue;
        }
        let lower = name.to_ascii_lowercase();
        if !lower.ends_with(".exe") {
            continue;
        }
        let size = asset.get("size").and_then(JsonVal::as_u64).unwrap_or(0);
        let mut score = 10i32;
        if lower.contains("nvlitereplay") {
            score += 5;
        }
        if lower.contains("voicerecog") {
            score += 2;
        }
        if best.as_ref().map(|b| score > b.0).unwrap_or(true) {
            best = Some((score, url.to_string(), name.to_string(), size));
        }
    }

    match best {
        Some((_, url, name, size)) => (Some(url), Some(name), size),
        None => (None, None, 0),
    }
}

fn summarize_release_notes(body: &str) -> String {
    for raw_line in body.lines() {
        let cleaned = raw_line
            .trim()
            .trim_start_matches(['#', '-', '*', '>', ' '])
            .replace("**", "")
            .replace('`', "");
        let trimmed = cleaned.trim();
        if trimmed.is_empty() || trimmed.starts_with("NVLiteReplay v") {
            continue;
        }
        if trimmed.chars().count() > 64 {
            let cut: String = trimmed.chars().take(61).collect();
            return format!("{}...", cut.trim_end());
        }
        return trimmed.to_string();
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_parses_all_release_tag_variants() {
        assert_eq!(
            SemVer::parse("v1.0.2"),
            Some(SemVer { major: 1, minor: 0, patch: 2 })
        );
        assert_eq!(
            SemVer::parse("V1.0.1"),
            Some(SemVer { major: 1, minor: 0, patch: 1 })
        );
        assert_eq!(
            SemVer::parse("v.1.0.0"),
            Some(SemVer { major: 1, minor: 0, patch: 0 })
        );
        assert_eq!(
            SemVer::parse("2.3.1-beta.1"),
            Some(SemVer { major: 2, minor: 3, patch: 1 })
        );
        assert!(SemVer::parse("v1.0.3").unwrap() > SemVer::parse("v1.0.2").unwrap());
        assert!(SemVer::parse("v1.0.2").unwrap() > SemVer::parse("v.1.0.0").unwrap());
        assert_eq!(SemVer::parse("invalid-tag"), None);
    }

    #[test]
    fn github_releases_json_parses_real_payload_and_skips_nested_urls() {
        let sample = r####"[
          {
            "url": "https://api.github.com/repos/xionnx/NVLiteReplay/releases/1",
            "html_url": "https://github.com/xionnx/NVLiteReplay/releases/tag/v1.0.4-rc1",
            "tag_name": "v1.0.4-rc1",
            "name": "Pre-release test",
            "draft": false,
            "prerelease": true,
            "assets": []
          },
          {
            "html_url": "https://github.com/xionnx/NVLiteReplay/releases/tag/v1.0.3",
            "author": {
              "login": "xionnx",
              "html_url": "https://github.com/xionnx"
            },
            "tag_name": "v1.0.3",
            "name": "NVLiteReplay v1.0.3",
            "draft": false,
            "prerelease": false,
            "assets": [
              {
                "name": "NVLiteReplayRustVoiceRecog-Portable-v1.0.3-x64.zip",
                "uploader": { "html_url": "https://github.com/xionnx" },
                "size": 308000,
                "browser_download_url": "https://github.com/xionnx/NVLiteReplay/releases/download/v1.0.3/NVLiteReplayRustVoiceRecog-Portable-v1.0.3-x64.zip"
              },
              {
                "name": "NVLiteReplayRustVoiceRecog-v1.0.3.exe",
                "uploader": { "login": "xionnx", "html_url": "https://github.com/xionnx" },
                "size": 599040,
                "browser_download_url": "https://github.com/xionnx/NVLiteReplay/releases/download/v1.0.3/NVLiteReplayRustVoiceRecog-v1.0.3.exe"
              }
            ],
            "body": "## NVLiteReplay v1.0.3\r\n- Tambah fitur \"Check for Update\" otomatis\\n"
          },
          {
            "html_url": "https://github.com/xionnx/NVLiteReplay/releases/tag/v.1.0.0",
            "tag_name": "v.1.0.0",
            "name": "NVLiteReplay v1.0.0",
            "draft": false,
            "prerelease": false,
            "assets": []
          }
        ]"####;

        let list = parse_github_releases_json(sample).expect("JSON should parse");
        assert_eq!(list.len(), 2, "Pre-release must be filtered out");
        let latest = list.into_iter().max_by_key(|r| r.version).unwrap();
        assert_eq!(
            latest.version,
            SemVer { major: 1, minor: 0, patch: 3 }
        );
        assert_eq!(
            latest.html_url,
            "https://github.com/xionnx/NVLiteReplay/releases/tag/v1.0.3"
        );
        assert_eq!(
            latest.exe_name.as_deref(),
            Some("NVLiteReplayRustVoiceRecog-v1.0.3.exe")
        );
        assert_eq!(latest.exe_size, 599040);
        assert!(latest.notes_summary.contains("Check for Update"));
    }

    #[test]
    fn pe_x64_validator_and_url_allowlist_behave_correctly() {
        let cur_exe = std::env::current_exe().unwrap();
        assert!(
            verify_windows_x64_pe_file(&cur_exe),
            "Current test binary must pass PE x64 check"
        );

        assert!(!verify_windows_x64_pe_bytes(b"<html>404 Not Found</html>"));
        let mut fake = vec![0u8; 150_000];
        fake[0] = b'M';
        fake[1] = b'Z';
        fake[0x3C] = 0x80;
        fake[0x80..0x84].copy_from_slice(b"PE\0\0");
        // Wrong architecture (x86 0x014c instead of x64 0x8664)
        fake[0x84..0x86].copy_from_slice(&0x014Cu16.to_le_bytes());
        assert!(!verify_windows_x64_pe_bytes(&fake));

        fake[0x84..0x86].copy_from_slice(&0x8664u16.to_le_bytes());
        assert!(verify_windows_x64_pe_bytes(&fake));

        assert!(is_allowed_github_url(
            "https://github.com/xionnx/NVLiteReplay/releases/download/v1.0.2/NVLiteReplayRustVoiceRecog-v1.0.2.exe"
        ));
        assert!(!is_allowed_github_url("http://github.com/xionnx/NVLiteReplay/x.exe"));
        assert!(!is_allowed_github_url("https://evil.example.com/NVLiteReplay.exe"));

        assert_eq!(
            parse_https_url_parts("https://api.github.com/repos/xionnx/NVLiteReplay/releases?per_page=8"),
            Some((
                "api.github.com".to_string(),
                443,
                "/repos/xionnx/NVLiteReplay/releases?per_page=8".to_string()
            ))
        );
    }

    #[test]
    fn cleanup_old_binaries_removes_only_update_artifacts() {
        let dir = std::env::temp_dir().join("NVLiteReplayRust_UpdaterCleanTest");
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);

        let keep_exe = dir.join("NVLiteReplayRustVoiceRecog.exe");
        let keep_ini = dir.join("NVLiteReplayRustVoiceRecog.ini");
        let old_bin = dir.join("NVLiteReplayRustVoiceRecog.exe.old");
        let old_pid_bin = dir.join("NVLiteReplayRustVoiceRecog.exe.1234.old");
        let tmp_bin = dir.join("NVLiteReplayRustVoiceRecog.exe.update.tmp");

        fs::write(&keep_exe, b"exe").unwrap();
        fs::write(&keep_ini, b"ini").unwrap();
        fs::write(&old_bin, b"old").unwrap();
        fs::write(&old_pid_bin, b"old2").unwrap();
        fs::write(&tmp_bin, b"tmp").unwrap();

        cleanup_old_binaries_in_dir(&dir);

        assert!(keep_exe.exists());
        assert!(keep_ini.exists());
        assert!(!old_bin.exists());
        assert!(!old_pid_bin.exists());
        assert!(!tmp_bin.exists());

        let notice = handle_update_startup_args(&[
            "NVLiteReplayRustVoiceRecog.exe".to_string(),
            "--updated-from".to_string(),
            "v1.0.2".to_string(),
        ]);
        assert_eq!(
            notice.as_deref(),
            Some(format!("Berhasil diperbarui dari v1.0.2 ke v{}!", APP_VERSION).as_str())
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn in_place_binary_swap_replaces_exe_and_keeps_backup_cleanly() {
        let dir = std::env::temp_dir().join(format!(
            "NVLiteReplay_UpdateSwapTest_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let cur_exe = dir.join("NVLiteReplayRustVoiceRecog.exe");
        let stale_old = dir.join("NVLiteReplayRustVoiceRecog.exe.old");
        let tmp_new = dir.join("NVLiteReplayRustVoiceRecog.exe.update.tmp");

        fs::write(&cur_exe, b"OLD_BINARY_V1").unwrap();
        fs::write(&stale_old, b"STALE_OLD").unwrap();

        // Simulate perform_in_place_update sequence: cleanup BEFORE creating tmp_new, then swap!
        cleanup_old_binaries_in_dir(&dir);
        fs::write(&tmp_new, b"NEW_BINARY_V2").unwrap();

        let backup = swap_downloaded_binary_in_place(&cur_exe, &tmp_new, 12345)
            .expect("swap_downloaded_binary_in_place must succeed");
        assert!(cur_exe.exists(), "Replaced executable must exist");
        assert!(!tmp_new.exists(), "Temporary update file must be moved/removed");
        assert!(backup.exists(), "Backup .old file must exist until relaunch cleanup");
        assert_eq!(fs::read(&cur_exe).unwrap(), b"NEW_BINARY_V2");
        assert_eq!(fs::read(&backup).unwrap(), b"OLD_BINARY_V1");

        // Simulate relaunch cleanup
        cleanup_old_binaries_in_dir(&dir);
        assert!(!backup.exists(), "Backup .old file must be removed on relaunch");
        assert_eq!(fs::read(&cur_exe).unwrap(), b"NEW_BINARY_V2");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    #[ignore]
    fn live_winhttp_github_releases_and_exe_download_check() {
        let rel = fetch_latest_github_release()
            .expect("WinHTTP GitHub API call failed")
            .expect("Expected at least one release");
        eprintln!(
            "[live] latest tag={}, version={:?}, exe={:?} ({}B), url={}",
            rel.tag_name, rel.version, rel.exe_name, rel.exe_size, rel.html_url
        );
        if let Some(ref exe_url) = rel.exe_url {
            let mut downloaded = Vec::with_capacity(rel.exe_size as usize);
            winhttp_download_stream(exe_url, false, MAX_DOWNLOAD_EXE_BYTES, |chunk, _, _| {
                downloaded.extend_from_slice(chunk);
                Ok(())
            })
            .expect("WinHTTP EXE CDN download failed");
            eprintln!("[live] downloaded {} bytes, pe_x64={}", downloaded.len(), verify_windows_x64_pe_bytes(&downloaded));
            assert!(verify_windows_x64_pe_bytes(&downloaded));
            if rel.exe_size > 0 {
                assert_eq!(downloaded.len() as u64, rel.exe_size);
            }
        }
    }
}
