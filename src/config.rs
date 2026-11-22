use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(i32)]
pub enum BufferStorageMode {
    RamRingBuffer = 0,
    DiskTempBuffer = 1,
}

impl BufferStorageMode {
    pub fn from_i32(v: i32) -> Self {
        if v == 1 {
            BufferStorageMode::DiskTempBuffer
        } else {
            BufferStorageMode::RamRingBuffer
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(i32)]
pub enum VideoCodecType {
    HardwareH264 = 0,
    HardwareHEVC = 1,
    HardwareAV1 = 2,
}

impl VideoCodecType {
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => VideoCodecType::HardwareHEVC,
            2 => VideoCodecType::HardwareAV1,
            _ => VideoCodecType::HardwareH264,
        }
    }
}

pub const HK_MOD_ALT: u32 = 0x0001;
pub const HK_MOD_CONTROL: u32 = 0x0002;
pub const HK_MOD_SHIFT: u32 = 0x0004;
pub const HK_MOD_WIN: u32 = 0x0008;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HotkeyBinding {
    pub modifiers: u32,
    pub vk: u32,
}

impl HotkeyBinding {
    pub const fn new(modifiers: u32, vk: u32) -> Self {
        Self { modifiers, vk }
    }

    pub const fn disabled() -> Self {
        Self { modifiers: 0, vk: 0 }
    }

    pub fn is_enabled(self) -> bool {
        self.vk != 0
    }

    pub fn display(self) -> String {
        if self.vk == 0 {
            return "Tidak Aktif".to_string();
        }
        let mut parts: Vec<String> = Vec::with_capacity(5);
        if (self.modifiers & HK_MOD_CONTROL) != 0 {
            parts.push("Ctrl".to_string());
        }
        if (self.modifiers & HK_MOD_SHIFT) != 0 {
            parts.push("Shift".to_string());
        }
        if (self.modifiers & HK_MOD_ALT) != 0 {
            parts.push("Alt".to_string());
        }
        if (self.modifiers & HK_MOD_WIN) != 0 {
            parts.push("Win".to_string());
        }
        parts.push(vk_to_key_name(self.vk));
        parts.join("+")
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let s = raw.trim();
        if s.is_empty() {
            return None;
        }
        if s.eq_ignore_ascii_case("none")
            || s.eq_ignore_ascii_case("off")
            || s.eq_ignore_ascii_case("tidak aktif")
        {
            return Some(Self::disabled());
        }

        let mut mods = 0u32;
        let mut key_token: Option<&str> = None;
        for part in s.split('+').map(str::trim).filter(|p| !p.is_empty()) {
            if part.eq_ignore_ascii_case("ctrl") || part.eq_ignore_ascii_case("control") {
                mods |= HK_MOD_CONTROL;
            } else if part.eq_ignore_ascii_case("shift") {
                mods |= HK_MOD_SHIFT;
            } else if part.eq_ignore_ascii_case("alt") || part.eq_ignore_ascii_case("menu") {
                mods |= HK_MOD_ALT;
            } else if part.eq_ignore_ascii_case("win") || part.eq_ignore_ascii_case("super") {
                mods |= HK_MOD_WIN;
            } else {
                key_token = Some(part);
            }
        }
        let vk = key_token.and_then(key_name_to_vk)?;
        Some(Self { modifiers: mods, vk })
    }
}

pub fn vk_to_key_name(vk: u32) -> String {
    match vk {
        0 => "None".to_string(),
        0x41..=0x5A => ((vk as u8) as char).to_string(),
        0x30..=0x39 => ((vk as u8) as char).to_string(),
        0x70..=0x87 => format!("F{}", vk - 0x70 + 1),
        0x60..=0x69 => format!("Num{}", vk - 0x60),
        0x6A => "NumMul".to_string(),
        0x6B => "NumAdd".to_string(),
        0x6D => "NumSub".to_string(),
        0x6E => "NumDec".to_string(),
        0x6F => "NumDiv".to_string(),
        0x08 => "Backspace".to_string(),
        0x09 => "Tab".to_string(),
        0x0D => "Enter".to_string(),
        0x13 => "Pause".to_string(),
        0x14 => "CapsLock".to_string(),
        0x20 => "Space".to_string(),
        0x21 => "PgUp".to_string(),
        0x22 => "PgDn".to_string(),
        0x23 => "End".to_string(),
        0x24 => "Home".to_string(),
        0x25 => "Left".to_string(),
        0x26 => "Up".to_string(),
        0x27 => "Right".to_string(),
        0x28 => "Down".to_string(),
        0x2C => "PrtScn".to_string(),
        0x2D => "Insert".to_string(),
        0x2E => "Delete".to_string(),
        0x90 => "NumLock".to_string(),
        0x91 => "ScrollLock".to_string(),
        0xBA => "Semicolon".to_string(),
        0xBB => "Equals".to_string(),
        0xBC => "Comma".to_string(),
        0xBD => "Minus".to_string(),
        0xBE => "Period".to_string(),
        0xBF => "Slash".to_string(),
        0xC0 => "Grave".to_string(),
        0xDB => "LBracket".to_string(),
        0xDC => "Backslash".to_string(),
        0xDD => "RBracket".to_string(),
        0xDE => "Quote".to_string(),
        _ => format!("VK_0x{:02X}", vk),
    }
}

pub fn key_name_to_vk(name: &str) -> Option<u32> {
    let t = name.trim();
    if t.len() == 1 {
        let ch = t.chars().next()?.to_ascii_uppercase();
        if ch.is_ascii_uppercase() || ch.is_ascii_digit() {
            return Some(ch as u32);
        }
    }
    if let Some(rest) = t.strip_prefix('F').or_else(|| t.strip_prefix('f')) {
        if let Ok(n) = rest.parse::<u32>() {
            if (1..=24).contains(&n) {
                return Some(0x70 + n - 1);
            }
        }
    }
    if let Some(rest) = t.strip_prefix("Num").or_else(|| t.strip_prefix("num")) {
        if rest.len() == 1 {
            if let Some(d) = rest.chars().next().and_then(|c| c.to_digit(10)) {
                return Some(0x60 + d);
            }
        }
    }
    if let Some(hex) = t.strip_prefix("VK_0x").or_else(|| t.strip_prefix("vk_0x")) {
        if let Ok(v) = u32::from_str_radix(hex, 16) {
            if (1..=254).contains(&v) {
                return Some(v);
            }
        }
    }
    match t.to_ascii_lowercase().as_str() {
        "nummul" => Some(0x6A),
        "numadd" => Some(0x6B),
        "numsub" => Some(0x6D),
        "numdec" => Some(0x6E),
        "numdiv" => Some(0x6F),
        "backspace" => Some(0x08),
        "tab" => Some(0x09),
        "enter" | "return" => Some(0x0D),
        "pause" => Some(0x13),
        "capslock" => Some(0x14),
        "space" => Some(0x20),
        "pgup" | "pageup" => Some(0x21),
        "pgdn" | "pagedown" => Some(0x22),
        "end" => Some(0x23),
        "home" => Some(0x24),
        "left" => Some(0x25),
        "up" => Some(0x26),
        "right" => Some(0x27),
        "down" => Some(0x28),
        "prtscn" | "printscreen" => Some(0x2C),
        "insert" | "ins" => Some(0x2D),
        "delete" | "del" => Some(0x2E),
        "numlock" => Some(0x90),
        "scrolllock" => Some(0x91),
        "semicolon" => Some(0xBA),
        "equals" => Some(0xBB),
        "comma" => Some(0xBC),
        "minus" => Some(0xBD),
        "period" => Some(0xBE),
        "slash" => Some(0xBF),
        "grave" => Some(0xC0),
        "lbracket" => Some(0xDB),
        "backslash" => Some(0xDC),
        "rbracket" => Some(0xDD),
        "quote" => Some(0xDE),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(i32)]
pub enum HotkeyPresetType {
    CtrlShiftF9F10 = 0,
    FunctionKeysF8F7 = 1,
    AltF9F10 = 2,
}

impl HotkeyPresetType {
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => HotkeyPresetType::FunctionKeysF8F7,
            2 => HotkeyPresetType::AltF9F10,
            _ => HotkeyPresetType::CtrlShiftF9F10,
        }
    }

    pub fn bindings(self) -> (HotkeyBinding, HotkeyBinding, HotkeyBinding) {
        match self {
            HotkeyPresetType::CtrlShiftF9F10 => (
                HotkeyBinding::new(HK_MOD_CONTROL | HK_MOD_SHIFT, 0x78), // Ctrl+Shift+F9
                HotkeyBinding::new(HK_MOD_CONTROL | HK_MOD_SHIFT, 0x79), // Ctrl+Shift+F10
                HotkeyBinding::new(HK_MOD_CONTROL | HK_MOD_SHIFT, b'M' as u32),
            ),
            HotkeyPresetType::FunctionKeysF8F7 => (
                HotkeyBinding::new(0, 0x77), // F8
                HotkeyBinding::new(0, 0x76), // F7
                HotkeyBinding::new(0, 0x75), // F6
            ),
            HotkeyPresetType::AltF9F10 => (
                HotkeyBinding::new(HK_MOD_ALT, 0x78), // Alt+F9
                HotkeyBinding::new(HK_MOD_ALT, 0x79), // Alt+F10
                HotkeyBinding::new(HK_MOD_ALT, b'M' as u32),
            ),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ReplayConfig {
    pub monitor_index: usize,
    pub use_native_resolution: bool,
    pub replay_duration_seconds: u32,
    pub segment_duration_seconds: u32,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate_mbps: u32,
    pub storage_mode: BufferStorageMode,
    pub codec: VideoCodecType,
    pub hotkey_preset: HotkeyPresetType,
    pub hotkey_toggle: HotkeyBinding,
    pub hotkey_save: HotkeyBinding,
    pub hotkey_mute: HotkeyBinding,

    pub record_desktop_audio: bool,
    pub record_microphone: bool,
    pub desktop_volume_percent: u32,
    pub mic_volume_percent: u32,
    pub mic_muted: bool,
    pub mic_device_id: String,

    pub voice_trigger_enabled: bool,
    pub voice_phrase: String,
    pub voice_sensitivity: u32,
    pub voice_cooldown_seconds: u32,

    pub output_folder: String,
    pub temp_folder: String,
    pub clip_name_template: String,
    pub game_name_map: String,
    pub minimize_to_tray_on_close: bool,
    pub play_sound_on_save: bool,
    pub suppress_notifications_in_game: bool,
    pub auto_check_updates: bool,
}

pub const DEFAULT_CLIP_NAME_TEMPLATE: &str = "{game}/Replay_{date}_{time}";

pub const DEFAULT_GAME_NAME_MAP: &str =
    "msedge.exe=Browser;chrome.exe=Browser;firefox.exe=Browser;brave.exe=Browser;\
     opera.exe=Browser;vivaldi.exe=Browser;explorer.exe=Desktop;\
     valorant-win64-shipping.exe=Valorant;cs2.exe=Counter-Strike 2;dota2.exe=Dota 2;\
     r5apex.exe=Apex Legends;overwatch.exe=Overwatch 2;\
     fortniteclient-win64-shipping.exe=Fortnite;genshinimpact.exe=Genshin Impact;\
     robloxplayerbeta.exe=Roblox";

impl Default for ReplayConfig {
    fn default() -> Self {
        let user_profile = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Public".to_string());
        let output_folder = Path::new(&user_profile)
            .join("Videos")
            .join("NVLiteReplay")
            .to_string_lossy()
            .into_owned();
        let temp_folder = ReplayConfig::default_temp_folder();

        let (hotkey_toggle, hotkey_save, hotkey_mute) = HotkeyPresetType::CtrlShiftF9F10.bindings();
        Self {
            monitor_index: 0,
            use_native_resolution: true,
            replay_duration_seconds: 60,
            segment_duration_seconds: 5,
            width: 1920,
            height: 1080,
            fps: 60,
            bitrate_mbps: 12,
            storage_mode: BufferStorageMode::RamRingBuffer,
            codec: VideoCodecType::HardwareH264,
            hotkey_preset: HotkeyPresetType::CtrlShiftF9F10,
            hotkey_toggle,
            hotkey_save,
            hotkey_mute,
            record_desktop_audio: true,
            record_microphone: true,
            desktop_volume_percent: 100,
            mic_volume_percent: 100,
            mic_muted: false,
            mic_device_id: String::new(),
            voice_trigger_enabled: false,
            voice_phrase: "Roman, clip that".to_string(),
            voice_sensitivity: 65,
            voice_cooldown_seconds: 8,
            output_folder,
            temp_folder,
            clip_name_template: DEFAULT_CLIP_NAME_TEMPLATE.to_string(),
            game_name_map: DEFAULT_GAME_NAME_MAP.to_string(),
            minimize_to_tray_on_close: true,
            play_sound_on_save: true,
            suppress_notifications_in_game: true,
            auto_check_updates: true,
        }
    }
}

impl ReplayConfig {
    pub fn default_temp_folder() -> String {
        std::env::temp_dir()
            .join("NVLiteReplayRust")
            .to_string_lossy()
            .into_owned()
    }

    pub fn effective_temp_dir(&self) -> PathBuf {
        let trimmed = self.temp_folder.trim();
        if trimmed.is_empty() {
            std::env::temp_dir().join("NVLiteReplayRust")
        } else {
            PathBuf::from(trimmed)
        }
    }

    pub fn effective_mic_device_id(&self) -> Option<&str> {
        let trimmed = self.mic_device_id.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }

    pub fn apply_hotkey_preset(&mut self, preset: HotkeyPresetType) {
        self.hotkey_preset = preset;
        let (t, s, m) = preset.bindings();
        self.hotkey_toggle = t;
        self.hotkey_save = s;
        self.hotkey_mute = m;
    }

    pub fn matching_hotkey_preset(&self) -> Option<HotkeyPresetType> {
        for p in [
            HotkeyPresetType::CtrlShiftF9F10,
            HotkeyPresetType::FunctionKeysF8F7,
            HotkeyPresetType::AltF9F10,
        ] {
            let (t, s, m) = p.bindings();
            if self.hotkey_toggle == t && self.hotkey_save == s && self.hotkey_mute == m {
                return Some(p);
            }
        }
        None
    }

    pub fn config_file_path() -> PathBuf {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                return dir.join("NVLiteReplayRustVoiceRecog.ini");
            }
        }
        PathBuf::from("NVLiteReplayRustVoiceRecog.ini")
    }

    pub fn load() -> Self {
        let mut cfg = Self::default();
        let path = Self::config_file_path();
        if let Ok(content) = fs::read_to_string(path) {
            cfg.apply_ini_str(&content);
        }
        cfg
    }

    /// Parses INI key=value lines onto `self`, keeping explicit custom hotkeys
    /// overriding legacy `HotkeyPreset` regardless of key order.
    pub fn apply_ini_str(&mut self, content: &str) {
        let mut explicit_toggle = None;
        let mut explicit_save = None;
        let mut explicit_mute = None;

        for raw in content.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let key = k.trim();
            let val = v.trim();
            match key {
                "MonitorIndex" => {
                    if let Ok(n) = val.parse::<usize>() {
                        self.monitor_index = n;
                    }
                }
                "UseNativeResolution" => {
                    if let Ok(b) = parse_bool(val) {
                        self.use_native_resolution = b;
                    }
                }
                "ReplayDurationSeconds" => {
                    if let Ok(n) = val.parse::<u32>() {
                        if (10..=600).contains(&n) {
                            self.replay_duration_seconds = n;
                        }
                    }
                }
                "Width" => {
                    if let Ok(n) = val.parse::<u32>() {
                        if n >= 640 {
                            self.width = n;
                        }
                    }
                }
                "Height" => {
                    if let Ok(n) = val.parse::<u32>() {
                        if n >= 360 {
                            self.height = n;
                        }
                    }
                }
                "Fps" => {
                    if let Ok(n) = val.parse::<u32>() {
                        if n == 30 || n == 60 || n == 120 {
                            self.fps = n;
                        }
                    }
                }
                "BitrateMbps" => {
                    if let Ok(n) = val.parse::<u32>() {
                        if (2..=50).contains(&n) {
                            self.bitrate_mbps = n;
                        }
                    }
                }
                "StorageMode" => {
                    if let Ok(n) = val.parse::<i32>() {
                        self.storage_mode = BufferStorageMode::from_i32(n);
                    }
                }
                "Codec" => {
                    if let Ok(n) = val.parse::<i32>() {
                        self.codec = VideoCodecType::from_i32(n);
                    }
                }
                "HotkeyPreset" => {
                    if let Ok(n) = val.parse::<i32>() {
                        self.apply_hotkey_preset(HotkeyPresetType::from_i32(n));
                    }
                }
                "HotkeyToggle" => {
                    if let Some(hk) = HotkeyBinding::parse(val) {
                        explicit_toggle = Some(hk);
                    }
                }
                "HotkeySave" => {
                    if let Some(hk) = HotkeyBinding::parse(val) {
                        explicit_save = Some(hk);
                    }
                }
                "HotkeyMute" => {
                    if let Some(hk) = HotkeyBinding::parse(val) {
                        explicit_mute = Some(hk);
                    }
                }
                "RecordDesktopAudio" => {
                    if let Ok(b) = parse_bool(val) {
                        self.record_desktop_audio = b;
                    }
                }
                "RecordMicrophone" => {
                    if let Ok(b) = parse_bool(val) {
                        self.record_microphone = b;
                    }
                }
                "DesktopVolumePercent" => {
                    if let Ok(n) = val.parse::<u32>() {
                        if n <= 200 {
                            self.desktop_volume_percent = n;
                        }
                    }
                }
                "MicVolumePercent" => {
                    if let Ok(n) = val.parse::<u32>() {
                        if n <= 200 {
                            self.mic_volume_percent = n;
                        }
                    }
                }
                "MicMuted" => {
                    if let Ok(b) = parse_bool(val) {
                        self.mic_muted = b;
                    }
                }
                "MicDeviceId" => {
                    self.mic_device_id = val.trim().to_string();
                }
                "VoiceTriggerEnabled" => {
                    if let Ok(b) = parse_bool(val) {
                        self.voice_trigger_enabled = b;
                    }
                }
                "VoicePhrase" => {
                    if !val.is_empty() {
                        self.voice_phrase = val.to_string();
                    }
                }
                "VoiceSensitivity" => {
                    if let Ok(n) = val.parse::<u32>() {
                        self.voice_sensitivity = n.clamp(30, 95);
                    }
                }
                "VoiceCooldownSeconds" => {
                    if let Ok(n) = val.parse::<u32>() {
                        if (2..=30).contains(&n) {
                            self.voice_cooldown_seconds = n;
                        }
                    }
                }
                "OutputFolder" => {
                    if !val.is_empty() {
                        self.output_folder = val.to_string();
                    }
                }
                "TempFolder" => {
                    self.temp_folder = if val.is_empty() {
                        Self::default_temp_folder()
                    } else {
                        val.to_string()
                    };
                }
                "ClipNameTemplate" => {
                    self.clip_name_template = if val.is_empty() {
                        DEFAULT_CLIP_NAME_TEMPLATE.to_string()
                    } else {
                        val.to_string()
                    };
                }
                "GameNameMap" => {
                    if !val.is_empty() {
                        self.game_name_map = val.to_string();
                    }
                }
                "MinimizeToTrayOnClose" => {
                    if let Ok(b) = parse_bool(val) {
                        self.minimize_to_tray_on_close = b;
                    }
                }
                "PlaySoundOnSave" => {
                    if let Ok(b) = parse_bool(val) {
                        self.play_sound_on_save = b;
                    }
                }
                "SuppressNotificationsInGame" => {
                    if let Ok(b) = parse_bool(val) {
                        self.suppress_notifications_in_game = b;
                    }
                }
                "AutoCheckUpdates" => {
                    if let Ok(b) = parse_bool(val) {
                        self.auto_check_updates = b;
                    }
                }
                _ => {}
            }
        }

        if let Some(hk) = explicit_toggle {
            self.hotkey_toggle = hk;
        }
        if let Some(hk) = explicit_save {
            self.hotkey_save = hk;
        }
        if let Some(hk) = explicit_mute {
            self.hotkey_mute = hk;
        }
    }

    pub fn save(&self) {
        let content = format!(
            "# NVLiteReplayRustVoiceRecog Configuration\n\
             MonitorIndex={}\n\
             UseNativeResolution={}\n\
             ReplayDurationSeconds={}\n\
             Width={}\n\
             Height={}\n\
             Fps={}\n\
             BitrateMbps={}\n\
             StorageMode={}\n\
             Codec={}\n\
             HotkeyPreset={}\n\
             HotkeyToggle={}\n\
             HotkeySave={}\n\
             HotkeyMute={}\n\
             RecordDesktopAudio={}\n\
             RecordMicrophone={}\n\
             DesktopVolumePercent={}\n\
             MicVolumePercent={}\n\
             MicMuted={}\n\
             MicDeviceId={}\n\
             VoiceTriggerEnabled={}\n\
             VoicePhrase={}\n\
             VoiceSensitivity={}\n\
             VoiceCooldownSeconds={}\n\
             OutputFolder={}\n\
             TempFolder={}\n\
             ClipNameTemplate={}\n\
             GameNameMap={}\n\
             MinimizeToTrayOnClose={}\n\
             PlaySoundOnSave={}\n\
             SuppressNotificationsInGame={}\n\
             AutoCheckUpdates={}\n",
            self.monitor_index,
            self.use_native_resolution,
            self.replay_duration_seconds,
            self.width,
            self.height,
            self.fps,
            self.bitrate_mbps,
            self.storage_mode as i32,
            self.codec as i32,
            self.hotkey_preset as i32,
            self.hotkey_toggle.display(),
            self.hotkey_save.display(),
            self.hotkey_mute.display(),
            self.record_desktop_audio,
            self.record_microphone,
            self.desktop_volume_percent,
            self.mic_volume_percent,
            self.mic_muted,
            self.mic_device_id,
            self.voice_trigger_enabled,
            self.voice_phrase,
            self.voice_sensitivity,
            self.voice_cooldown_seconds,
            self.output_folder,
            self.temp_folder,
            self.clip_name_template,
            self.game_name_map,
            self.minimize_to_tray_on_close,
            self.play_sound_on_save,
            self.suppress_notifications_in_game,
            self.auto_check_updates
        );
        let _ = fs::write(Self::config_file_path(), content);
    }
}

/// Looks up a friendly game/app name for `exe_name_or_path` (e.g. `msedge.exe` -> `Browser`)
/// in `custom_map`, falling back to `DEFAULT_GAME_NAME_MAP`.
/// Supports `;`, `,`, or newline-separated rules with `=`, `->`, or `:` key/value separators.
pub fn lookup_custom_game_name(custom_map: &str, exe_name_or_path: &str) -> Option<String> {
    let file_name = Path::new(exe_name_or_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(exe_name_or_path)
        .trim();
    if file_name.is_empty() {
        return None;
    }
    let stem = file_name
        .strip_suffix(".exe")
        .or_else(|| file_name.strip_suffix(".EXE"))
        .unwrap_or(file_name);

    for source in [custom_map, DEFAULT_GAME_NAME_MAP] {
        for entry in source.split([';', ',', '\n', '\r']) {
            let trimmed = entry.trim();
            if trimmed.is_empty() {
                continue;
            }
            let pair = trimmed
                .split_once("->")
                .or_else(|| trimmed.split_once('='))
                .or_else(|| trimmed.split_once(':'));
            let Some((raw_k, raw_v)) = pair else {
                continue;
            };
            let k = raw_k.trim();
            let v = raw_v.trim();
            if k.is_empty() || v.is_empty() {
                continue;
            }
            let k_stem = k
                .strip_suffix(".exe")
                .or_else(|| k.strip_suffix(".EXE"))
                .unwrap_or(k);
            if k.eq_ignore_ascii_case(file_name) || k_stem.eq_ignore_ascii_case(stem) {
                let clean = sanitize_path_component(v, "");
                if !clean.is_empty() {
                    return Some(clean);
                }
            }
        }
    }
    None
}

/// Cleans an executable filename or stem into a readable game/app folder name
/// (e.g. `VALORANT-Win64-Shipping.exe` -> `Valorant`, `my_cool_game_x64.exe` -> `my cool game`).
pub fn clean_exe_stem_name(exe_name_or_path: &str) -> String {
    let file_name = Path::new(exe_name_or_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(exe_name_or_path)
        .trim();
    let mut s = file_name
        .strip_suffix(".exe")
        .or_else(|| file_name.strip_suffix(".EXE"))
        .unwrap_or(file_name)
        .to_string();

    for suffix in [
        "-Win64-Shipping",
        "-WinGDK-Shipping",
        "-Win32-Shipping",
        "_Win64_Shipping",
        "-Win64",
        "_x64",
        "-x64",
        "_dx12",
        "_dx11",
    ] {
        if s.len() > suffix.len() && s[s.len() - suffix.len()..].eq_ignore_ascii_case(suffix) {
            s.truncate(s.len() - suffix.len());
        }
    }

    let s = s.replace('_', " ");
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return "Desktop".to_string();
    }

    // If a single word is ALL-UPPERCASE (>4 chars, e.g. "VALORANT") or all-lowercase ("valorant"),
    // convert to Title Case ("Valorant").
    let is_all_upper = trimmed.len() > 4
        && trimmed.chars().all(|c| !c.is_ascii_lowercase())
        && trimmed.chars().any(|c| c.is_ascii_uppercase());
    let is_all_lower = trimmed.chars().all(|c| !c.is_ascii_uppercase())
        && trimmed.chars().any(|c| c.is_ascii_lowercase());

    let formatted = if is_all_upper || is_all_lower {
        trimmed
            .split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    Some(first) => {
                        first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                    }
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        trimmed.to_string()
    };

    sanitize_path_component(&formatted, "Desktop")
}

/// Sanitizes a single Windows path component (folder or filename stem), stripping
/// illegal characters (`< > : " / \ | ? *`, control chars, `®`, `™`) and reserved names.
pub fn sanitize_path_component(raw: &str, fallback: &str) -> String {
    let mut cleaned = String::with_capacity(raw.len());
    let mut prev_space = false;
    for ch in raw.chars() {
        if ch == '®' || ch == '™' || ch.is_control() {
            continue;
        }
        if matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
            if !prev_space && !cleaned.is_empty() {
                cleaned.push(' ');
                prev_space = true;
            }
            continue;
        }
        if ch.is_whitespace() {
            if !prev_space && !cleaned.is_empty() {
                cleaned.push(' ');
                prev_space = true;
            }
        } else {
            cleaned.push(ch);
            prev_space = false;
        }
    }

    let trimmed = cleaned.trim_matches(|c: char| c.is_whitespace() || c == '.');
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        return fallback.to_string();
    }

    let upper = trimmed.to_ascii_uppercase();
    let is_reserved = matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );
    if is_reserved {
        return fallback.to_string();
    }

    trimmed.chars().take(64).collect::<String>().trim().to_string()
}

/// Expands `{game}`, `{date}`, and `{time}` placeholders inside `template` into a sanitized
/// relative path (without `.mp4` suffix), supporting `/` or `\` subdirectory separators.
pub fn format_clip_relative_path(
    template: &str,
    game_name: &str,
    date_str: &str,
    time_str: &str,
) -> PathBuf {
    let tpl = if template.trim().is_empty() {
        DEFAULT_CLIP_NAME_TEMPLATE
    } else {
        template.trim()
    };
    let safe_game = sanitize_path_component(game_name, "Desktop");
    let safe_date = sanitize_path_component(date_str, "2026-01-01");
    let safe_time = sanitize_path_component(time_str, "00-00-00");

    let expanded = tpl
        .replace("{game}", &safe_game)
        .replace("{GAME}", &safe_game)
        .replace("{date}", &safe_date)
        .replace("{DATE}", &safe_date)
        .replace("{time}", &safe_time)
        .replace("{TIME}", &safe_time);

    let mut parts: Vec<String> = expanded
        .split(['/', '\\'])
        .map(|seg| sanitize_path_component(seg, ""))
        .filter(|seg| !seg.is_empty())
        .collect();

    if parts.is_empty() {
        parts.push(format!("Replay_{}_{}", safe_date, safe_time));
    }

    if let Some(last) = parts.last_mut() {
        if last.len() > 4 && last[last.len() - 4..].eq_ignore_ascii_case(".mp4") {
            last.truncate(last.len() - 4);
        }
        let cleaned_last = sanitize_path_component(last, "");
        if cleaned_last.is_empty() {
            *last = format!("Replay_{}_{}", safe_date, safe_time);
        } else {
            *last = cleaned_last;
        }
    }

    let mut rel = PathBuf::new();
    for p in parts {
        rel.push(p);
    }
    rel
}

fn parse_bool(s: &str) -> Result<bool, ()> {
    if s.eq_ignore_ascii_case("true") || s == "1" {
        Ok(true)
    } else if s.eq_ignore_ascii_case("false") || s == "0" {
        Ok(false)
    } else {
        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkey_binding_roundtrip_and_disabled() {
        let cases = [
            "Ctrl+Shift+F9",
            "Alt+Win+K",
            "F8",
            "Ctrl+Space",
            "Shift+PgUp",
            "Num0",
            "PrtScn",
            "Ctrl+NumAdd",
        ];
        for raw in cases {
            let parsed = HotkeyBinding::parse(raw).expect(raw);
            assert!(parsed.is_enabled(), "{raw} should be enabled");
            assert_eq!(parsed.display(), raw);
        }

        for off in ["Tidak Aktif", "None", "none", "OFF"] {
            let d = HotkeyBinding::parse(off).expect(off);
            assert!(!d.is_enabled());
            assert_eq!(d, HotkeyBinding::disabled());
            assert_eq!(d.display(), "Tidak Aktif");
        }

        let built = HotkeyBinding::new(HK_MOD_CONTROL | HK_MOD_ALT, b'K' as u32);
        assert_eq!(built.modifiers, HK_MOD_CONTROL | HK_MOD_ALT);
        assert_eq!(built.vk, b'K' as u32);
        assert_eq!(built.display(), "Ctrl+Alt+K");
    }

    #[test]
    fn hotkey_presets_and_matching() {
        let mut cfg = ReplayConfig::default();
        assert_eq!(
            cfg.matching_hotkey_preset(),
            Some(HotkeyPresetType::CtrlShiftF9F10)
        );

        cfg.apply_hotkey_preset(HotkeyPresetType::FunctionKeysF8F7);
        assert_eq!(
            cfg.matching_hotkey_preset(),
            Some(HotkeyPresetType::FunctionKeysF8F7)
        );
        assert_eq!(cfg.hotkey_toggle.display(), "F8");
        assert_eq!(cfg.hotkey_save.display(), "F7");
        assert_eq!(cfg.hotkey_mute.display(), "F6");

        cfg.apply_hotkey_preset(HotkeyPresetType::AltF9F10);
        assert_eq!(
            cfg.matching_hotkey_preset(),
            Some(HotkeyPresetType::AltF9F10)
        );

        cfg.hotkey_save = HotkeyBinding::parse("Ctrl+Win+S").unwrap();
        assert_eq!(cfg.matching_hotkey_preset(), None);
    }

    #[test]
    fn ini_custom_hotkey_override_and_sensitivity_clamp() {
        let mut cfg = ReplayConfig::default();
        cfg.apply_ini_str(
            "HotkeyToggle=Ctrl+Win+R\n\
             HotkeyPreset=1\n\
             HotkeyMute=Tidak Aktif\n\
             VoiceSensitivity=12\n",
        );
        // Custom toggle & mute override HotkeyPreset=1 (F8/F7/F6); save takes preset F7.
        assert_eq!(cfg.hotkey_toggle.display(), "Ctrl+Win+R");
        assert_eq!(cfg.hotkey_save.display(), "F7");
        assert!(!cfg.hotkey_mute.is_enabled());
        assert_eq!(cfg.voice_sensitivity, 30);

        cfg.apply_ini_str("VoiceSensitivity=999\n");
        assert_eq!(cfg.voice_sensitivity, 95);

        cfg.apply_ini_str("VoiceSensitivity=42\n");
        assert_eq!(cfg.voice_sensitivity, 42);
    }

    #[test]
    fn temp_folder_ini_and_default_fallback() {
        let mut cfg = ReplayConfig::default();
        assert_eq!(cfg.temp_folder, ReplayConfig::default_temp_folder());
        assert_eq!(
            cfg.effective_temp_dir(),
            std::env::temp_dir().join("NVLiteReplayRust")
        );

        cfg.apply_ini_str("TempFolder=D:\\CustomReplayTmp\n");
        assert_eq!(cfg.temp_folder, "D:\\CustomReplayTmp");
        assert_eq!(cfg.effective_temp_dir(), PathBuf::from("D:\\CustomReplayTmp"));

        cfg.apply_ini_str("TempFolder=\n");
        assert_eq!(cfg.temp_folder, ReplayConfig::default_temp_folder());

        cfg.temp_folder = "   ".to_string();
        assert_eq!(
            cfg.effective_temp_dir(),
            std::env::temp_dir().join("NVLiteReplayRust")
        );
    }

    #[test]
    fn mic_device_id_ini_roundtrip() {
        let mut cfg = ReplayConfig::default();
        assert_eq!(cfg.effective_mic_device_id(), None);

        cfg.apply_ini_str("MicDeviceId={0.0.1.00000000}.{abc-123}\n");
        assert_eq!(
            cfg.effective_mic_device_id(),
            Some("{0.0.1.00000000}.{abc-123}")
        );

        cfg.apply_ini_str("MicDeviceId=\n");
        assert_eq!(cfg.effective_mic_device_id(), None);
    }

    #[test]
    fn video_codec_type_ini_roundtrip() {
        let mut cfg = ReplayConfig::default();
        assert_eq!(cfg.codec, VideoCodecType::HardwareH264);

        cfg.apply_ini_str("Codec=1\n");
        assert_eq!(cfg.codec, VideoCodecType::HardwareHEVC);

        cfg.apply_ini_str("Codec=2\n");
        assert_eq!(cfg.codec, VideoCodecType::HardwareAV1);

        cfg.apply_ini_str("Codec=99\n");
        assert_eq!(cfg.codec, VideoCodecType::HardwareH264);
    }

    #[test]
    fn auto_check_updates_ini_roundtrip() {
        let mut cfg = ReplayConfig::default();
        assert!(cfg.auto_check_updates);

        cfg.apply_ini_str("AutoCheckUpdates=false\n");
        assert!(!cfg.auto_check_updates);

        cfg.apply_ini_str("AutoCheckUpdates=1\n");
        assert!(cfg.auto_check_updates);
    }

    #[test]
    fn clip_name_template_and_game_name_map_helpers() {
        assert_eq!(
            lookup_custom_game_name(DEFAULT_GAME_NAME_MAP, "msedge.exe"),
            Some("Browser".to_string())
        );
        assert_eq!(
            lookup_custom_game_name(
                DEFAULT_GAME_NAME_MAP,
                r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
            ),
            Some("Browser".to_string())
        );
        assert_eq!(
            lookup_custom_game_name(
                "mygame.exe -> My Custom Game; msedge.exe -> Edge Browser",
                "mygame.exe"
            ),
            Some("My Custom Game".to_string())
        );
        assert_eq!(
            lookup_custom_game_name(
                "mygame.exe -> My Custom Game; msedge.exe -> Edge Browser",
                "msedge.exe"
            ),
            Some("Edge Browser".to_string())
        );
        assert_eq!(
            clean_exe_stem_name(r"D:\Riot Games\VALORANT\live\ShooterGame\Binaries\Win64\VALORANT-Win64-Shipping.exe"),
            "Valorant"
        );
        assert_eq!(clean_exe_stem_name("Apex_x64.exe"), "Apex");

        let rel = format_clip_relative_path(
            "{game}/Replay_{date}_{time}",
            "Valorant",
            "2026-11-21",
            "14-05-00",
        );
        assert_eq!(
            rel,
            PathBuf::from("Valorant").join("Replay_2026-11-21_14-05-00")
        );

        let flat = format_clip_relative_path(
            "Replay_{date}_{time}",
            "Valorant",
            "2026-11-21",
            "14-05-00",
        );
        assert_eq!(flat, PathBuf::from("Replay_2026-11-21_14-05-00"));

        let mut cfg = ReplayConfig::default();
        assert_eq!(cfg.clip_name_template, DEFAULT_CLIP_NAME_TEMPLATE);
        cfg.apply_ini_str(
            "ClipNameTemplate={game}/{game}_{date}_{time}\n\
             GameNameMap=msedge.exe->Browser;custom.exe=CustomApp\n",
        );
        assert_eq!(cfg.clip_name_template, "{game}/{game}_{date}_{time}");
        assert_eq!(
            lookup_custom_game_name(&cfg.game_name_map, "custom.exe"),
            Some("CustomApp".to_string())
        );
    }
}
