#![windows_subsystem = "windows"]

mod audio;
mod config;
mod cursor;
mod engine;
mod ui;
mod updater;
mod voice;

use std::time::{Duration, Instant};

use windows::core::PCWSTR;
use windows::Win32::Foundation::ERROR_ALREADY_EXISTS;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Controls::{
    InitCommonControlsEx, ICC_BAR_CLASSES, ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_NOREPEAT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SetForegroundWindow, SetProcessDPIAware, ShowWindow, SW_RESTORE,
};

use config::{BufferStorageMode, HotkeyBinding, ReplayConfig, VideoCodecType};
use engine::ReplayEngine;

fn wide_null(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = s.encode_utf16().collect();
    v.push(0);
    v
}

fn run_self_test() -> i32 {
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
    let mut log_lines = Vec::<String>::new();
    let mut log = |line: String| {
        println!("{}", line);
        log_lines.push(line);
    };

    log("=== NVLiteReplayRustVoiceRecog Self-Test ===".to_string());

    // 1. Pure-Rust MFCC + CMVN + Sakoe-Chiba DTW DSP self-test
    let (voice_ok, voice_summary) = voice::verify_dsp_pipeline();
    log(voice_summary);

    // 1b. DXGI GPU shader + GDI cursor compositor self-test
    let (cursor_ok, cursor_summary) = cursor::verify_cursor_compositor();
    log(cursor_summary);

    // 2. Capture Engine + Hardware Video/NVENC + MP4 Save test
    let mut cfg = ReplayConfig::load();
    cfg.replay_duration_seconds = 10;
    cfg.segment_duration_seconds = 1;
    cfg.fps = 60;
    cfg.play_sound_on_save = false;

    let temp_dir = std::env::temp_dir().join("NVLiteReplayRustVoiceRecog_SelfTest");
    let _ = std::fs::create_dir_all(&temp_dir);
    cfg.output_folder = temp_dir.to_string_lossy().to_string();

    let engine = ReplayEngine::new(cfg.clone());
    let gpu = engine.gpu_name.lock().unwrap().clone();
    let caps = *engine.codec_caps.lock().unwrap();
    let displays = engine.displays.lock().unwrap().clone();
    let mic_devs = engine.mic_devices.lock().unwrap().clone();
    log(format!("GPU Detected: {}", gpu));
    log(format!(
        "Hardware Codec Caps: H.264=true, HEVC={}, AV1={}",
        caps.hevc_hw, caps.av1_hw
    ));
    log(format!("Monitors Detected: {}", displays.len()));
    for (i, d) in displays.iter().enumerate() {
        log(format!("  [{}] {}", i, d.display_label()));
    }
    log(format!("Audio Inputs Detected: {}", mic_devs.len()));
    for (i, m) in mic_devs.iter().enumerate() {
        log(format!(
            "  [{}] {}{}",
            i,
            m.name,
            if m.is_default { " [Default]" } else { "" }
        ));
    }

    unsafe {
        let h_ok_t = RegisterHotKey(
            None,
            9101,
            HOT_KEY_MODIFIERS(cfg.hotkey_toggle.modifiers | MOD_NOREPEAT.0),
            cfg.hotkey_toggle.vk,
        )
        .is_ok();
        let h_ok_s = RegisterHotKey(
            None,
            9102,
            HOT_KEY_MODIFIERS(cfg.hotkey_save.modifiers | MOD_NOREPEAT.0),
            cfg.hotkey_save.vk,
        )
        .is_ok();
        let custom_hk = HotkeyBinding::parse("Ctrl+Alt+K").unwrap();
        let h_ok_c = RegisterHotKey(
            None,
            9103,
            HOT_KEY_MODIFIERS(custom_hk.modifiers | MOD_NOREPEAT.0),
            custom_hk.vk,
        )
        .is_ok();
        let _ = UnregisterHotKey(None, 9101);
        let _ = UnregisterHotKey(None, 9102);
        let _ = UnregisterHotKey(None, 9103);
        log(format!(
            "Hotkey test ({} / {} / custom {}): toggle={}, save={}, custom={}",
            cfg.hotkey_toggle.display(),
            cfg.hotkey_save.display(),
            custom_hk.display(),
            h_ok_t,
            h_ok_s,
            h_ok_c
        ));
    }

    log("Starting capture engine for 3.8s...".to_string());
    engine.start();
    std::thread::sleep(Duration::from_millis(3800));

    let cap_mode = engine.capture_mode_status.lock().unwrap().clone();
    let pipe_status = engine.video_pipeline_status.lock().unwrap().clone();
    let cur_fps = engine.current_fps();
    let buf_sec = engine.buffered_seconds();
    let buf_bytes = engine.buffer_size_bytes();
    log(format!(
        "Capture Status: mode='{}', pipeline='{}', fps={:.1}, buffered_sec={:.1}s, buffer_kb={:.1} KB",
        cap_mode,
        pipe_status,
        cur_fps,
        buf_sec,
        buf_bytes as f64 / 1024.0
    ));

    let save_t0 = Instant::now();
    engine.save_replay_async();

    let mut saved_path = String::new();
    for _ in 0..80 {
        std::thread::sleep(Duration::from_millis(100));
        let last = engine.last_saved_file.lock().unwrap().clone();
        if !last.is_empty() && std::path::Path::new(&last).exists() {
            if let Ok(meta) = std::fs::metadata(&last) {
                if meta.len() > 20_000 {
                    saved_path = last;
                    break;
                }
            }
        }
    }
    let save_ms = save_t0.elapsed().as_millis();
    let status_msg = engine.status_message.lock().unwrap().clone();
    log(format!("Engine status after save: {}", status_msg));

    engine.stop();

    let mut exit_code = if voice_ok && cursor_ok { 0 } else { 1 };
    if !voice_ok {
        log("FAIL: Voice MFCC+DTW self-test failed!".to_string());
    }
    if !cursor_ok {
        log("FAIL: Cursor compositor self-test failed!".to_string());
    }
    if !saved_path.is_empty() {
        let len = std::fs::metadata(&saved_path).map(|m| m.len()).unwrap_or(0);
        let mp4_info = engine::inspect_mp4_video_info(std::path::Path::new(&saved_path));
        if let Some((ref cname, mw, mh, yuv_m, frames)) = mp4_info {
            log(format!(
                "PASS: Saved MP4 '{}' ({} bytes, {:.1} KB, codec={}, {}x{}, yuv_matrix={}, frames={}) in {} ms",
                saved_path,
                len,
                len as f64 / 1024.0,
                cname,
                mw,
                mh,
                yuv_m,
                frames,
                save_ms
            ));
            if frames == 0 {
                log("FAIL: Saved MP4 has 0 readable video frames!".to_string());
                exit_code = 1;
            }
        } else {
            log(format!(
                "PASS: Saved MP4 '{}' ({} bytes, {:.1} KB) in {} ms",
                saved_path,
                len,
                len as f64 / 1024.0,
                save_ms
            ));
        }
        let _ = std::fs::remove_file(&saved_path);
    } else {
        log("FAIL: Replay MP4 was not created or too small!".to_string());
        exit_code = 1;
    }

    let _ = std::fs::remove_dir_all(&temp_dir);

    // 3. Disk Temp Buffer + HEVC / H.265 Multi-Segment Merge + Async Stop Regression Test
    let mut cfg_hevc = ReplayConfig::load();
    cfg_hevc.storage_mode = BufferStorageMode::DiskTempBuffer;
    cfg_hevc.codec = VideoCodecType::HardwareHEVC;
    cfg_hevc.fps = 60;
    cfg_hevc.replay_duration_seconds = 60;
    cfg_hevc.segment_duration_seconds = 1;
    cfg_hevc.bitrate_mbps = 12;
    cfg_hevc.use_native_resolution = true;
    cfg_hevc.play_sound_on_save = false;
    let ssd_dir = temp_dir.join("CustomDiskTemp");
    let _ = std::fs::remove_dir_all(&temp_dir);
    cfg_hevc.output_folder = temp_dir.to_string_lossy().to_string();
    cfg_hevc.temp_folder = ssd_dir.to_string_lossy().to_string();

    let hevc_engine = ReplayEngine::new(cfg_hevc);
    hevc_engine.start();
    std::thread::sleep(Duration::from_millis(2600));
    let h_buf_sec = hevc_engine.buffered_seconds();
    let h_buf_bytes = hevc_engine.buffer_size_bytes();
    let h_pipe = hevc_engine.video_pipeline_status.lock().unwrap().clone();

    hevc_engine.save_replay_async();
    let mut hevc_saved_info: Option<(String, u32, u32, u32, u32)> = None;
    for _ in 0..60 {
        std::thread::sleep(Duration::from_millis(100));
        let last = hevc_engine.last_saved_file.lock().unwrap().clone();
        if !last.is_empty() && std::path::Path::new(&last).exists() {
            hevc_saved_info = engine::inspect_mp4_video_info(std::path::Path::new(&last));
            break;
        }
    }
    if let Some((ref cname, mw, mh, yuv_m, frames)) = hevc_saved_info {
        log(format!(
            "HEVC Multi-Segment Merge Test: pipeline='{}', codec={}, {}x{}, yuv_matrix={}, frames={} -> {}",
            h_pipe,
            cname,
            mw,
            mh,
            yuv_m,
            frames,
            if frames > 30 { "PASS" } else { "FAIL" }
        ));
        if frames <= 30 {
            exit_code = 1;
        }
    } else {
        log("FAIL: HEVC multi-segment MP4 save failed!".to_string());
        exit_code = 1;
    }

    let t_stop = Instant::now();
    hevc_engine.stop_async();
    let async_dispatch_us = t_stop.elapsed().as_micros();
    let mut settled = false;
    while t_stop.elapsed() < Duration::from_secs(6) {
        if !hevc_engine.is_stopping() {
            settled = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    let settle_ms = t_stop.elapsed().as_millis();
    if !settled {
        hevc_engine.stop();
        exit_code = 1;
    }

    let ssd_exists = ssd_dir.exists();
    let pid_prefix = format!("seg_{}_", std::process::id());
    let leftovers: usize = std::fs::read_dir(&ssd_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.starts_with(&pid_prefix))
                .unwrap_or(false)
        })
        .count();
    let _ = std::fs::remove_dir_all(&temp_dir);

    let hevc_pass =
        settled && async_dispatch_us < 50_000 && h_buf_bytes > 0 && ssd_exists && leftovers == 0;
    if !hevc_pass {
        exit_code = 1;
    }
    log(format!(
        "DiskTemp+HEVC Async Stop Test: buf={:.1}s ({} bytes), ui_dispatch={} us, settled={} in {} ms, leftovers={} -> {}",
        h_buf_sec,
        h_buf_bytes,
        async_dispatch_us,
        settled,
        settle_ms,
        leftovers,
        if hevc_pass { "PASS" } else { "FAIL" }
    ));

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let _ = std::fs::write(dir.join("self_test_last.log"), log_lines.join("\r\n"));
        }
    }
    exit_code
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    unsafe {
        let _ = SetProcessDPIAware();
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_STANDARD_CLASSES | ICC_BAR_CLASSES,
        };
        let _ = InitCommonControlsEx(&icc);
    }

    if args.iter().any(|a| a == "--verify-self-test") {
        let code = run_self_test();
        unsafe { CoUninitialize(); }
        std::process::exit(code);
    }

    let update_notice = updater::handle_update_startup_args(&args);
    let start_minimized = args.iter().any(|a| a.eq_ignore_ascii_case("--minimized"));

    unsafe {
        let mutex_name = wide_null("Global\\NVLiteReplayRustVoiceRecog_SingleInstance_Mutex");
        let _mutex = CreateMutexW(None, false, PCWSTR(mutex_name.as_ptr()));
        if windows::Win32::Foundation::GetLastError() == ERROR_ALREADY_EXISTS {
            if !start_minimized {
                let cls = wide_null("NVLiteReplayRustVoiceRecogMainWnd");
                if let Ok(existing) = FindWindowW(PCWSTR(cls.as_ptr()), PCWSTR::null()) {
                    if !existing.0.is_null() {
                        let _ = ShowWindow(existing, SW_RESTORE);
                        let _ = SetForegroundWindow(existing);
                    }
                }
            }
            CoUninitialize();
            return;
        }
    }

    let cfg = ReplayConfig::load();
    let engine = ReplayEngine::new(cfg);
    if let Some(msg) = update_notice {
        engine.raise_status(msg);
    }
    ui::run(engine, start_minimized);

    unsafe {
        CoUninitialize();
    }
}
