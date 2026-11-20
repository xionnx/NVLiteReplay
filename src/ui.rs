use std::ffi::c_void;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use windows::core::{Interface, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_IGNORE, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_POINT_2F, D2D_RECT_F,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1DCRenderTarget, ID2D1Factory, ID2D1RenderTarget,
    ID2D1SolidColorBrush, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_DRAW_TEXT_OPTIONS_CLIP,
    D2D1_ELLIPSE, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_SOFTWARE,
    D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT, D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_BOLD,
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_WORD_WRAPPING_NO_WRAP,
};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateBitmap, CreateCompatibleDC, CreateDIBSection, DeleteDC,
    DeleteObject, EndPaint, GetDC, GetDeviceCaps, GetMonitorInfoW, InvalidateRect,
    MonitorFromWindow, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ, LOGPIXELSX, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    PAINTSTRUCT, SRCCOPY,
};
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, RegisterHotKey, ReleaseCapture, SetCapture, TrackMouseEvent,
    UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_NOREPEAT, TME_LEAVE, TRACKMOUSEEVENT, VK_CAPITAL,
    VK_CONTROL, VK_ESCAPE, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_NUMLOCK,
    VK_PACKET, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SCROLL, VK_SHIFT, VK_SNAPSHOT,
};
use windows::Win32::UI::Shell::{
    FileOpenDialog, IFileOpenDialog, ShellExecuteW, Shell_NotifyIconW, FOS_PICKFOLDERS,
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIIF_NOSOUND, NIM_ADD, NIM_DELETE,
    NIM_MODIFY, NOTIFYICONDATAW, SIGDN_FILESYSPATH,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW,
    DefWindowProcW, DestroyIcon, DestroyMenu, DestroyWindow, DispatchMessageW, GetClassNameW,
    GetClientRect, GetCursorPos, GetMessageW, GetSystemMetrics, GetWindowLongPtrW, GetWindowRect,
    IsWindowVisible, KillTimer, LoadCursorW, PostMessageW, PostQuitMessage, RegisterClassExW,
    SendMessageW, SetCursor, SetForegroundWindow, SetTimer, SetWindowLongPtrW, ShowWindow,
    TrackPopupMenu, TranslateMessage, CS_HREDRAW, CS_VREDRAW, GWL_USERDATA, HICON, ICONINFO,
    IDC_ARROW, IDC_HAND, IDC_IBEAM, MF_SEPARATOR, MF_STRING, MSG, SM_CXSCREEN, SM_CYSCREEN,
    SW_HIDE, SW_RESTORE, SW_SHOW, TPM_BOTTOMALIGN, TPM_LEFTALIGN, WM_APP, WM_CHAR, WM_CLOSE,
    WM_COMMAND, WM_CREATE, WM_DESTROY, WM_ERASEBKGND, WM_HOTKEY, WM_KEYDOWN, WM_KEYUP,
    WM_KILLFOCUS, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NULL,
    WM_PAINT, WM_RBUTTONUP, WM_SETCURSOR, WM_SIZE, WM_SYSCHAR, WM_SYSKEYDOWN, WM_SYSKEYUP,
    WM_TIMER, WNDCLASSEXW, WS_CAPTION, WS_CLIPCHILDREN, WS_EX_APPWINDOW, WS_MINIMIZEBOX,
    WS_OVERLAPPED, WS_SYSMENU,
};

use crate::config::{
    BufferStorageMode, HotkeyBinding, HotkeyPresetType, VideoCodecType, HK_MOD_ALT, HK_MOD_CONTROL,
    HK_MOD_SHIFT, HK_MOD_WIN,
};
use crate::engine::{ReplayEngine, WM_APP_SAVED, WM_APP_STATUS, WM_APP_UPDATE, WM_APP_VOICE_TRIGGER};
use crate::updater::{UpdateState, APP_VERSION};

const WM_MOUSELEAVE: u32 = 0x02A3;
const WM_PRINTCLIENT: u32 = 0x0318;
const WM_APP_TRAY: u32 = WM_APP + 1;
const TIMER_UPDATE_ID: usize = 1;

const HOTKEY_TOGGLE_ID: i32 = 9001;
const HOTKEY_SAVE_ID: i32 = 9002;
const HOTKEY_MUTE_ID: i32 = 9003;

const ID_TRAY_OPEN: usize = 2001;
const ID_TRAY_TOGGLE: usize = 2002;
const ID_TRAY_SAVE: usize = 2003;
const ID_TRAY_FOLDER: usize = 2004;
const ID_TRAY_EXIT: usize = 2005;
const ID_TRAY_UPDATE: usize = 2006;

const LOGICAL_W: f32 = 680.0;
const LOGICAL_H: f32 = 700.0;

static UI_PTR: OnceLock<usize> = OnceLock::new();

#[derive(Clone, Copy)]
struct RectF {
    l: f32,
    t: f32,
    r: f32,
    b: f32,
}

impl RectF {
    const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { l: x, t: y, r: x + w, b: y + h }
    }
    fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.l && x <= self.r && y >= self.t && y <= self.b
    }
    fn to_d2d(&self) -> D2D_RECT_F {
        D2D_RECT_F {
            left: self.l,
            top: self.t,
            right: self.r,
            bottom: self.b,
        }
    }
    fn to_rounded(&self, rx: f32, ry: f32) -> D2D1_ROUNDED_RECT {
        D2D1_ROUNDED_RECT {
            rect: self.to_d2d(),
            radiusX: rx,
            radiusY: ry,
        }
    }
}

const fn c_rgba(r: u8, g: u8, b: u8, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: (r as f32) / 255.0,
        g: (g as f32) / 255.0,
        b: (b as f32) / 255.0,
        a,
    }
}

const fn c_rgb(r: u8, g: u8, b: u8) -> D2D1_COLOR_F {
    c_rgba(r, g, b, 1.0)
}

// Minimalist Monochrome Matte Dark Palette (inspired by Linear / Geist dark UI)
const COL_BG: D2D1_COLOR_F = c_rgb(9, 9, 11);           // #09090B pitch obsidian canvas
const COL_SURFACE: D2D1_COLOR_F = c_rgb(17, 17, 20);    // #111114 subtle flat card
const COL_SURFACE_ALT: D2D1_COLOR_F = c_rgb(13, 13, 16); // #0D0D10 recessed input/chip
const COL_SURFACE_HOV: D2D1_COLOR_F = c_rgb(24, 24, 29); // #18181D hover fill
const COL_SURFACE_ACT: D2D1_COLOR_F = c_rgb(29, 29, 35); // #1D1D23 active tab/chip
const COL_DIVIDER: D2D1_COLOR_F = c_rgb(30, 30, 35);    // #1E1E23 crisp 1px hairline
const COL_BORDER: D2D1_COLOR_F = c_rgb(34, 34, 40);     // #222228 card/input border
const COL_BORDER_HI: D2D1_COLOR_F = c_rgb(56, 56, 66);  // #383842 focused/selected border
const COL_TRACK: D2D1_COLOR_F = c_rgb(28, 28, 34);      // #1C1C22 progress/slider track

const COL_EMERALD: D2D1_COLOR_F = c_rgb(52, 211, 153);      // #34D399 status dot / active bar / check
const COL_EMERALD_HOVER: D2D1_COLOR_F = c_rgb(110, 231, 183);
const COL_BLUE: D2D1_COLOR_F = c_rgb(56, 189, 248);         // #38BDF8 subtle sky accent bar
const COL_BLUE_HOVER: D2D1_COLOR_F = c_rgb(125, 211, 252);
const COL_RED: D2D1_COLOR_F = c_rgb(248, 113, 113);         // #F87171 danger / muted
const COL_RED_HOVER: D2D1_COLOR_F = c_rgb(252, 165, 165);
const COL_AMBER: D2D1_COLOR_F = c_rgb(251, 191, 36);        // #FBBF24 capture / warning

const COL_INK: D2D1_COLOR_F = c_rgb(9, 9, 11);
const COL_TEXT: D2D1_COLOR_F = c_rgb(244, 244, 245);        // #F4F4F5 primary crisp white
const COL_TEXT_SUB: D2D1_COLOR_F = c_rgb(161, 161, 170);    // #A1A1AA secondary cool zinc
const COL_TEXT_MUTED: D2D1_COLOR_F = c_rgb(113, 113, 122);  // #71717A muted uppercase labels
const COL_TEXT_DIM: D2D1_COLOR_F = c_rgb(76, 76, 86);       // #4C4C56 disabled text

#[derive(Clone, Copy, PartialEq, Eq)]
enum DropdownKind {
    Monitor,
    Resolution,
    Bitrate,
    MicInput,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DragSlider {
    None,
    Desktop,
    Mic,
    VoiceSensitivity,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HotkeySlot {
    Toggle,
    Save,
    Mute,
}

impl HotkeySlot {
    const ALL: [HotkeySlot; 3] = [HotkeySlot::Toggle, HotkeySlot::Save, HotkeySlot::Mute];

    fn title(self) -> &'static str {
        match self {
            HotkeySlot::Toggle => "TOGGLE REPLAY",
            HotkeySlot::Save => "SIMPAN KLIP",
            HotkeySlot::Mute => "MUTE / UNMUTE MIC",
        }
    }
}

#[derive(Clone, Copy)]
struct ResolutionOption {
    w: u32,
    h: u32,
    native: bool,
    label: &'static str,
}

const RESOLUTIONS: &[ResolutionOption] = &[
    ResolutionOption { w: 0, h: 0, native: true, label: "Native (Auto-Match Monitor / Game)" },
    ResolutionOption { w: 1920, h: 1080, native: false, label: "1920 x 1080 (Full HD 1080p)" },
    ResolutionOption { w: 1280, h: 720, native: false, label: "1280 x 720 (HD 720p)" },
    ResolutionOption { w: 2560, h: 1440, native: false, label: "2560 x 1440 (QHD 1440p)" },
    ResolutionOption { w: 1280, h: 1024, native: false, label: "1280 x 1024 (4:3 Stretched)" },
    ResolutionOption { w: 1280, h: 960, native: false, label: "1280 x 960 (4:3 Stretched)" },
];

const DURATIONS: &[(u32, &str)] = &[
    (15, "15 dtk"),
    (30, "30 dtk"),
    (60, "60 dtk"),
    (120, "2 mnt"),
    (180, "3 mnt"),
    (300, "5 mnt"),
];

const FPS_OPTIONS: &[(u32, &str)] = &[
    (30, "30 FPS"),
    (60, "60 FPS"),
    (120, "120 FPS"),
];

const BITRATE_OPTIONS: &[(u32, &str)] = &[
    (4, "4 Mbps — Hemat Ruang"),
    (8, "8 Mbps — Seimbang"),
    (12, "12 Mbps — Ideal Streaming/Klip"),
    (20, "20 Mbps — Kualitas Tinggi"),
    (35, "35 Mbps — Ultra Tajam"),
    (50, "50 Mbps — Master 1440p/120fps"),
];

const PHRASE_PRESETS: &[&str] = &[
    "Roman, clip that",
    "Chat, clip that",
    "Simpan klip!",
];

// Sensitivitas DTW dikustomisasi via slider + tombol -/+ (30..=95%)
const SENS_MIN_PERCENT: u32 = 30;
const SENS_MAX_PERCENT: u32 = 95;

const COOLDOWN_OPTIONS: &[(u32, &str)] = &[
    (5, "5 dtk"),
    (8, "8 dtk"),
    (15, "15 dtk"),
];

// Fixed layout geometry (DIPs) — Minimalist Monochrome Grid
const R_TAB_0: RectF = RectF::new(148.0, 10.0, 56.0, 26.0);
const R_TAB_1: RectF = RectF::new(208.0, 10.0, 56.0, 26.0);
const R_TAB_2: RectF = RectF::new(268.0, 10.0, 62.0, 26.0);
const R_TAB_3: RectF = RectF::new(334.0, 10.0, 58.0, 26.0);

const R_BTN_TOGGLE: RectF = RectF::new(20.0, 150.0, 188.0, 32.0);
const R_BTN_SAVE: RectF = RectF::new(216.0, 150.0, 188.0, 32.0);
const R_BTN_FOLDER: RectF = RectF::new(444.0, 150.0, 216.0, 32.0);

// Tab 0 geometry (Video)
const R_CMB_MONITOR: RectF = RectF::new(20.0, 232.0, 640.0, 34.0);
const R_STO_RAM: RectF = RectF::new(20.0, 302.0, 314.0, 60.0);
const R_STO_DISK: RectF = RectF::new(346.0, 302.0, 314.0, 60.0);
const R_CODEC_H264: RectF = RectF::new(346.0, 398.0, 100.0, 32.0);
const R_CODEC_HEVC: RectF = RectF::new(453.0, 398.0, 100.0, 32.0);
const R_CODEC_AV1: RectF = RectF::new(560.0, 398.0, 100.0, 32.0);
const R_CMB_RES: RectF = RectF::new(20.0, 534.0, 314.0, 34.0);
const R_CMB_BITRATE: RectF = RectF::new(346.0, 534.0, 314.0, 34.0);

// Tab 1 geometry (Audio)
const R_TOG_DESKTOP: RectF = RectF::new(600.0, 231.0, 42.0, 22.0);
const R_SLD_DESKTOP: RectF = RectF::new(36.0, 280.0, 540.0, 26.0);
const R_TOG_MIC: RectF = RectF::new(600.0, 355.0, 42.0, 22.0);
const R_MUTE_PILL: RectF = RectF::new(510.0, 354.0, 80.0, 24.0);
const R_SLD_MIC: RectF = RectF::new(36.0, 404.0, 540.0, 26.0);
const R_CMB_MIC: RectF = RectF::new(36.0, 474.0, 608.0, 34.0);

// Tab 2 geometry (Sistem & Hotkey)
fn hk_slot_rect(idx: usize) -> RectF {
    let x = match idx {
        0 => 20.0,
        1 => 237.0,
        _ => 454.0,
    };
    RectF::new(x, 232.0, 206.0, 56.0)
}

fn hk_clear_rect(idx: usize) -> RectF {
    let r = hk_slot_rect(idx);
    RectF::new(r.r - 24.0, r.t + 6.0, 18.0, 18.0)
}

const R_HK_PRESET_0: RectF = RectF::new(112.0, 297.0, 146.0, 26.0);
const R_HK_PRESET_1: RectF = RectF::new(264.0, 297.0, 94.0, 26.0);
const R_HK_PRESET_2: RectF = RectF::new(364.0, 297.0, 112.0, 26.0);
const R_BTN_BROWSE: RectF = RectF::new(544.0, 370.0, 116.0, 32.0);
const R_BTN_TEMP_BROWSE: RectF = RectF::new(466.0, 434.0, 116.0, 32.0);
const R_BTN_TEMP_RESET: RectF = RectF::new(588.0, 434.0, 72.0, 32.0);
const R_TOG_TRAY: RectF = RectF::new(618.0, 506.0, 42.0, 22.0);
const R_TOG_BEEP: RectF = RectF::new(618.0, 540.0, 42.0, 22.0);
const R_TOG_FOCUS: RectF = RectF::new(618.0, 574.0, 42.0, 22.0);
const R_TOG_AUTO_UPDATE: RectF = RectF::new(618.0, 608.0, 42.0, 22.0);

// Global Footer GitHub Update Bar (y=646..700)
const R_BTN_GITHUB_PAGE: RectF = RectF::new(426.0, 658.0, 94.0, 30.0);
const R_BTN_UPDATE_ACTION: RectF = RectF::new(528.0, 658.0, 132.0, 30.0);

// Tab 3 geometry (Voice Clip)
const R_TOG_VOICE: RectF = RectF::new(600.0, 229.0, 42.0, 22.0);
const R_VP_0: RectF = RectF::new(318.0, 336.0, 108.0, 22.0);
const R_VP_1: RectF = RectF::new(432.0, 336.0, 102.0, 22.0);
const R_VP_2: RectF = RectF::new(540.0, 336.0, 102.0, 22.0);
const R_TXT_PHRASE: RectF = RectF::new(36.0, 368.0, 488.0, 36.0);
const R_BTN_SAVE_PHRASE: RectF = RectF::new(532.0, 368.0, 110.0, 36.0);
const R_BTN_ENROLL: RectF = RectF::new(320.0, 474.0, 234.0, 34.0);
const R_BTN_RESET_VOICE: RectF = RectF::new(562.0, 474.0, 80.0, 34.0);

fn phrase_preset_rect(idx: usize) -> RectF {
    match idx {
        0 => R_VP_0,
        1 => R_VP_1,
        _ => R_VP_2,
    }
}

const R_BTN_SENS_DEC: RectF = RectF::new(36.0, 580.0, 26.0, 26.0);
const R_SLD_VOICE_SENS: RectF = RectF::new(70.0, 580.0, 266.0, 26.0);
const R_BTN_SENS_INC: RectF = RectF::new(344.0, 580.0, 26.0, 26.0);

fn cd_pill_rect(idx: usize) -> RectF {
    match idx {
        0 => RectF::new(402.0, 580.0, 76.0, 26.0),
        1 => RectF::new(484.0, 580.0, 76.0, 26.0),
        _ => RectF::new(566.0, 580.0, 76.0, 26.0),
    }
}

struct D2DResources {
    factory: ID2D1Factory,
    #[allow(dead_code)]
    dwrite: IDWriteFactory,
    dc_rt: Option<ID2D1DCRenderTarget>,
    rt: Option<ID2D1RenderTarget>,
    brush: Option<ID2D1SolidColorBrush>,
    mem_dc: HDC,
    mem_bmp: HBITMAP,
    old_obj: HGDIOBJ,
    buf_w: i32,
    buf_h: i32,
    fmt_brand: IDWriteTextFormat,
    fmt_title: IDWriteTextFormat,
    fmt_bold: IDWriteTextFormat,
    fmt_reg: IDWriteTextFormat,
    fmt_small: IDWriteTextFormat,
    fmt_metric: IDWriteTextFormat,
    fmt_center_bold: IDWriteTextFormat,
    fmt_center_reg: IDWriteTextFormat,
    fmt_center_small: IDWriteTextFormat,
    fmt_right_reg: IDWriteTextFormat,
    fmt_right_small: IDWriteTextFormat,
}

struct UiState {
    engine: Arc<ReplayEngine>,
    hwnd: HWND,
    dpi_scale: f32,
    app_icon: HICON,
    d2d: D2DResources,

    active_tab: usize,
    hover_x: f32,
    hover_y: f32,
    mouse_inside: bool,
    mouse_down: bool,
    dragging_slider: DragSlider,
    open_dropdown: Option<DropdownKind>,

    phrase_draft: String,
    editing_phrase: bool,
    capturing_hotkey: Option<HotkeySlot>,
    caret_tick: u32,
    fast_timer_active: bool,

    hotkey_warning: String,
    tray_added: bool,
    last_process_ram_mb: f64,
}

fn wide_null(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = s.encode_utf16().collect();
    v.push(0);
    v
}

unsafe fn make_text_fmt(
    dwrite: &IDWriteFactory,
    size: f32,
    weight: windows::Win32::Graphics::DirectWrite::DWRITE_FONT_WEIGHT,
    halign: windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_ALIGNMENT,
) -> IDWriteTextFormat {
    let font_name = wide_null("Segoe UI");
    let locale = wide_null("en-US");
    let fmt = dwrite
        .CreateTextFormat(
            PCWSTR(font_name.as_ptr()),
            None,
            weight,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            PCWSTR(locale.as_ptr()),
        )
        .expect("CreateTextFormat failed");
    let _ = fmt.SetTextAlignment(halign);
    let _ = fmt.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
    let _ = fmt.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP);
    fmt
}

unsafe fn init_d2d_resources() -> D2DResources {
    let factory: ID2D1Factory =
        D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).expect("D2D1CreateFactory");
    let dwrite: IDWriteFactory =
        DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).expect("DWriteCreateFactory");

    let fmt_brand = make_text_fmt(&dwrite, 13.5, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_LEADING);
    let fmt_title = make_text_fmt(&dwrite, 13.5, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_LEADING);
    let fmt_bold = make_text_fmt(&dwrite, 12.5, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_LEADING);
    let fmt_reg = make_text_fmt(&dwrite, 12.5, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_ALIGNMENT_LEADING);
    let fmt_small = make_text_fmt(&dwrite, 10.5, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_TEXT_ALIGNMENT_LEADING);
    let fmt_metric = make_text_fmt(&dwrite, 23.0, DWRITE_FONT_WEIGHT_BOLD, DWRITE_TEXT_ALIGNMENT_LEADING);
    let fmt_center_bold = make_text_fmt(&dwrite, 12.5, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER);
    let fmt_center_reg = make_text_fmt(&dwrite, 12.0, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_ALIGNMENT_CENTER);
    let fmt_center_small = make_text_fmt(&dwrite, 10.5, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_TEXT_ALIGNMENT_CENTER);
    let fmt_right_reg = make_text_fmt(&dwrite, 12.0, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_TRAILING);
    let fmt_right_small = make_text_fmt(&dwrite, 10.5, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_ALIGNMENT_TRAILING);

    D2DResources {
        factory,
        dwrite,
        dc_rt: None,
        rt: None,
        brush: None,
        mem_dc: HDC::default(),
        mem_bmp: HBITMAP::default(),
        old_obj: HGDIOBJ::default(),
        buf_w: 0,
        buf_h: 0,
        fmt_brand,
        fmt_title,
        fmt_bold,
        fmt_reg,
        fmt_small,
        fmt_metric,
        fmt_center_bold,
        fmt_center_reg,
        fmt_center_small,
        fmt_right_reg,
        fmt_right_small,
    }
}

unsafe fn release_backbuffer(d2d: &mut D2DResources) {
    if !d2d.mem_dc.0.is_null() {
        if !d2d.old_obj.0.is_null() {
            let _ = SelectObject(d2d.mem_dc, d2d.old_obj);
            d2d.old_obj = HGDIOBJ::default();
        }
        if !d2d.mem_bmp.0.is_null() {
            let _ = DeleteObject(d2d.mem_bmp);
            d2d.mem_bmp = HBITMAP::default();
        }
        let _ = DeleteDC(d2d.mem_dc);
        d2d.mem_dc = HDC::default();
    }
    d2d.buf_w = 0;
    d2d.buf_h = 0;
}

unsafe fn ensure_render_target(state: &mut UiState, w: i32, h: i32) -> bool {
    if w <= 0 || h <= 0 {
        return false;
    }
    if state.d2d.rt.is_none() {
        let rt_props = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            },
            dpiX: 96.0 * state.dpi_scale,
            dpiY: 96.0 * state.dpi_scale,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        if let Ok(dc_rt) = state.d2d.factory.CreateDCRenderTarget(&rt_props) {
            if let Ok(rt) = dc_rt.cast::<ID2D1RenderTarget>() {
                rt.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
                rt.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE);
                if let Ok(brush) = rt.CreateSolidColorBrush(&COL_TEXT, None) {
                    state.d2d.brush = Some(brush);
                    state.d2d.dc_rt = Some(dc_rt);
                    state.d2d.rt = Some(rt);
                }
            }
        }
    }

    if state.d2d.rt.is_none() {
        return false;
    }

    if state.d2d.mem_dc.0.is_null() || state.d2d.buf_w != w || state.d2d.buf_h != h {
        release_backbuffer(&mut state.d2d);
        let screen_dc = GetDC(None);
        let mem_dc = CreateCompatibleDC(screen_dc);
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let mem_bmp = CreateDIBSection(screen_dc, &bmi, DIB_RGB_COLORS, &mut bits, None, 0).unwrap_or_default();
        let _ = ReleaseDC(None, screen_dc);
        if mem_dc.0.is_null() || mem_bmp.0.is_null() {
            if !mem_bmp.0.is_null() {
                let _ = DeleteObject(mem_bmp);
            }
            if !mem_dc.0.is_null() {
                let _ = DeleteDC(mem_dc);
            }
            return false;
        }
        let old_obj = SelectObject(mem_dc, HGDIOBJ(mem_bmp.0));
        state.d2d.mem_dc = mem_dc;
        state.d2d.mem_bmp = mem_bmp;
        state.d2d.old_obj = old_obj;
        state.d2d.buf_w = w;
        state.d2d.buf_h = h;
    }

    if let Some(ref dc_rt) = state.d2d.dc_rt {
        let sub = RECT { left: 0, top: 0, right: w, bottom: h };
        return dc_rt.BindDC(state.d2d.mem_dc, &sub).is_ok();
    }
    false
}

unsafe fn apply_dark_window_frame(hwnd: HWND) {
    let dark: i32 = 1;
    let corner_round: i32 = DWMWCP_ROUND.0;
    let caption_bgr: u32 = 0x000B_0909; // #09090B matte pitch obsidian
    let border_bgr: u32 = 0x0023_1E1E;  // #1E1E23 crisp hairline frame
    let text_bgr: u32 = 0x00AA_A1A1;    // #A1A1AA quiet monochrome title

    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_USE_IMMERSIVE_DARK_MODE,
        &dark as *const i32 as *const c_void,
        4,
    );
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_WINDOW_CORNER_PREFERENCE,
        &corner_round as *const i32 as *const c_void,
        4,
    );
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_CAPTION_COLOR,
        &caption_bgr as *const u32 as *const c_void,
        4,
    );
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_BORDER_COLOR,
        &border_bgr as *const u32 as *const c_void,
        4,
    );
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_TEXT_COLOR,
        &text_bgr as *const u32 as *const c_void,
        4,
    );
}

unsafe fn create_app_icon() -> HICON {
    let size = 32i32;
    let mut bmi = BITMAPINFO::default();
    bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = size;
    bmi.bmiHeader.biHeight = -size;
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB.0;

    let mut bits_ptr: *mut c_void = std::ptr::null_mut();
    let hdc = CreateCompatibleDC(None);
    let hbm_color = CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, &mut bits_ptr, None, 0).unwrap_or_default();
    if !bits_ptr.is_null() {
        let pixels = std::slice::from_raw_parts_mut(bits_ptr as *mut u32, (size * size) as usize);
        let cx = 15.5f32;
        let cy = 15.5f32;
        for y in 0..size {
            for x in 0..size {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                let idx = (y * size + x) as usize;
                if dist <= 14.5 {
                    if dist >= 10.5 {
                        pixels[idx] = 0xFF10B981;
                    } else if dist <= 5.2 {
                        pixels[idx] = 0xFF34D399;
                    } else {
                        pixels[idx] = 0xFF0B0D13;
                    }
                } else {
                    pixels[idx] = 0x00000000;
                }
            }
        }
    }
    let hbm_mask = CreateBitmap(size, size, 1, 1, None);
    let ii = ICONINFO {
        fIcon: true.into(),
        hbmColor: hbm_color,
        hbmMask: hbm_mask,
        ..Default::default()
    };
    let icon = CreateIconIndirect(&ii).unwrap_or_default();
    let _ = DeleteObject(hbm_color);
    let _ = DeleteObject(hbm_mask);
    let _ = DeleteDC(hdc);
    icon
}

unsafe fn query_process_ram_mb() -> f64 {
    #[repr(C)]
    #[derive(Default)]
    struct ProcessMemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }
    if let Ok(hmod) = LoadLibraryW(PCWSTR(wide_null("psapi.dll").as_ptr())) {
        if let Some(proc_addr) = GetProcAddress(hmod, windows::core::s!("GetProcessMemoryInfo")) {
            type GetMemFn = unsafe extern "system" fn(*mut c_void, *mut ProcessMemoryCounters, u32) -> i32;
            let func: GetMemFn = std::mem::transmute(proc_addr);
            let proc_handle = windows::Win32::System::Threading::GetCurrentProcess();
            let mut pmc = ProcessMemoryCounters {
                cb: std::mem::size_of::<ProcessMemoryCounters>() as u32,
                ..Default::default()
            };
            if func(proc_handle.0, &mut pmc, pmc.cb) != 0 {
                return pmc.working_set_size as f64 / (1024.0 * 1024.0);
            }
        }
    }
    0.0
}

unsafe fn is_other_app_fullscreen(self_hwnd: HWND) -> bool {
    let fg = windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
    if fg.0.is_null() || fg == self_hwnd {
        return false;
    }
    let mut cls = [0u16; 128];
    let len = GetClassNameW(fg, &mut cls);
    if len > 0 {
        let cname = String::from_utf16_lossy(&cls[..len as usize]);
        if cname == "Progman" || cname == "WorkerW" || cname == "Shell_TrayWnd" {
            return false;
        }
    }
    let mut rc = RECT::default();
    if GetWindowRect(fg, &mut rc).is_err() {
        return false;
    }
    let mon = MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST);
    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !GetMonitorInfoW(mon, &mut mi).as_bool() {
        return false;
    }
    let m = mi.rcMonitor;
    rc.left <= m.left + 2 && rc.top <= m.top + 2 && rc.right >= m.right - 2 && rc.bottom >= m.bottom - 2
}

unsafe fn copy_str_to_wide_buf(s: &str, buf: &mut [u16]) {
    buf.fill(0);
    let max_chars = buf.len().saturating_sub(1);
    for (i, ch) in s.encode_utf16().take(max_chars).enumerate() {
        buf[i] = ch;
    }
}

unsafe fn ensure_tray_icon(state: &mut UiState) {
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: state.hwnd,
        uID: 1,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_APP_TRAY,
        hIcon: state.app_icon,
        ..Default::default()
    };
    copy_str_to_wide_buf("NVLiteReplay — Instant Replay", &mut nid.szTip);
    let ok = Shell_NotifyIconW(if state.tray_added { NIM_MODIFY } else { NIM_ADD }, &nid).as_bool();
    if ok {
        state.tray_added = true;
    }
}

unsafe fn remove_tray_icon(state: &mut UiState) {
    if state.tray_added {
        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: state.hwnd,
            uID: 1,
            ..Default::default()
        };
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        state.tray_added = false;
    }
}

unsafe fn show_tray_balloon(state: &UiState, title: &str, msg: &str) {
    if !state.tray_added {
        return;
    }
    let cfg = state.engine.config.lock().unwrap().clone();
    if cfg.suppress_notifications_in_game && is_other_app_fullscreen(state.hwnd) {
        return;
    }
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: state.hwnd,
        uID: 1,
        uFlags: NIF_INFO | NIF_TIP,
        dwInfoFlags: NIIF_INFO | NIIF_NOSOUND,
        ..Default::default()
    };
    copy_str_to_wide_buf("NVLiteReplay — Instant Replay", &mut nid.szTip);
    copy_str_to_wide_buf(title, &mut nid.szInfoTitle);
    copy_str_to_wide_buf(msg, &mut nid.szInfo);
    let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
}

unsafe fn unregister_hotkeys(hwnd: HWND) {
    let _ = UnregisterHotKey(hwnd, HOTKEY_TOGGLE_ID);
    let _ = UnregisterHotKey(hwnd, HOTKEY_SAVE_ID);
    let _ = UnregisterHotKey(hwnd, HOTKEY_MUTE_ID);
}

unsafe fn register_hotkeys(state: &mut UiState) {
    unregister_hotkeys(state.hwnd);

    let (tog, save, mute) = {
        let cfg = state.engine.config.lock().unwrap();
        (cfg.hotkey_toggle, cfg.hotkey_save, cfg.hotkey_mute)
    };

    let mut failed = Vec::new();
    for (id, binding) in [
        (HOTKEY_TOGGLE_ID, tog),
        (HOTKEY_SAVE_ID, save),
        (HOTKEY_MUTE_ID, mute),
    ] {
        if !binding.is_enabled() {
            continue;
        }
        let mods = HOT_KEY_MODIFIERS(binding.modifiers | MOD_NOREPEAT.0);
        if RegisterHotKey(state.hwnd, id, mods, binding.vk).is_err() {
            failed.push(binding.display());
        }
    }

    if !failed.is_empty() {
        state.hotkey_warning = format!(
            "Shortcut {} dipakai app lain — klik slot di Tab Sistem untuk ganti!",
            failed.join(", ")
        );
    } else {
        state.hotkey_warning.clear();
    }
}

unsafe fn current_async_hotkey_mods() -> u32 {
    let is_down = |vk: u16| -> bool { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 };
    let mut mods = 0u32;
    if is_down(VK_CONTROL.0) {
        mods |= HK_MOD_CONTROL;
    }
    if is_down(VK_SHIFT.0) {
        mods |= HK_MOD_SHIFT;
    }
    if is_down(VK_MENU.0) {
        mods |= HK_MOD_ALT;
    }
    if is_down(VK_LWIN.0) || is_down(VK_RWIN.0) {
        mods |= HK_MOD_WIN;
    }
    mods
}

fn format_modifier_preview(mods: u32) -> String {
    let mut parts = Vec::new();
    if mods & HK_MOD_CONTROL != 0 {
        parts.push("Ctrl");
    }
    if mods & HK_MOD_SHIFT != 0 {
        parts.push("Shift");
    }
    if mods & HK_MOD_ALT != 0 {
        parts.push("Alt");
    }
    if mods & HK_MOD_WIN != 0 {
        parts.push("Win");
    }
    if parts.is_empty() {
        "Tekan tombol...".to_string()
    } else {
        format!("{} + ...", parts.join("+"))
    }
}

fn is_modifier_or_toggle_vk(vk: u32) -> bool {
    vk == VK_SHIFT.0 as u32
        || vk == VK_CONTROL.0 as u32
        || vk == VK_MENU.0 as u32
        || vk == VK_LWIN.0 as u32
        || vk == VK_RWIN.0 as u32
        || vk == VK_LSHIFT.0 as u32
        || vk == VK_RSHIFT.0 as u32
        || vk == VK_LCONTROL.0 as u32
        || vk == VK_RCONTROL.0 as u32
        || vk == VK_LMENU.0 as u32
        || vk == VK_RMENU.0 as u32
        || vk == VK_CAPITAL.0 as u32
        || vk == VK_NUMLOCK.0 as u32
        || vk == VK_SCROLL.0 as u32
        || vk == VK_PACKET.0 as u32
}

unsafe fn begin_hotkey_capture(state: &mut UiState, slot: HotkeySlot) {
    state.capturing_hotkey = Some(slot);
    state.hotkey_warning.clear();
    // Unregister active global hotkeys sementara agar kombinasi yang sudah terpasang
    // tetap diterima via WM_KEYDOWN alih-alih dimakan WM_HOTKEY.
    unregister_hotkeys(state.hwnd);
    let _ = InvalidateRect(state.hwnd, None, false);
}

unsafe fn cancel_hotkey_capture(state: &mut UiState) {
    if state.capturing_hotkey.take().is_some() {
        register_hotkeys(state);
        let _ = InvalidateRect(state.hwnd, None, false);
    }
}

unsafe fn apply_captured_hotkey(state: &mut UiState, vk: u32, mods: u32) {
    let Some(slot) = state.capturing_hotkey else {
        return;
    };
    let candidate = HotkeyBinding::new(mods, vk);
    let duplicate = {
        let mut cfg = state.engine.config.lock().unwrap();
        let conflict = match slot {
            HotkeySlot::Toggle => candidate == cfg.hotkey_save || candidate == cfg.hotkey_mute,
            HotkeySlot::Save => candidate == cfg.hotkey_toggle || candidate == cfg.hotkey_mute,
            HotkeySlot::Mute => candidate == cfg.hotkey_toggle || candidate == cfg.hotkey_save,
        };
        if !conflict {
            match slot {
                HotkeySlot::Toggle => cfg.hotkey_toggle = candidate,
                HotkeySlot::Save => cfg.hotkey_save = candidate,
                HotkeySlot::Mute => cfg.hotkey_mute = candidate,
            }
            cfg.save();
        }
        conflict
    };

    state.capturing_hotkey = None;
    register_hotkeys(state);
    if duplicate {
        state.hotkey_warning = format!(
            "Shortcut {} sudah dipakai aksi lain — pilih kombinasi berbeda.",
            candidate.display()
        );
    }
    let _ = InvalidateRect(state.hwnd, None, false);
}

fn sens_tier_info(percent: u32) -> (&'static str, D2D1_COLOR_F) {
    if percent <= 50 {
        ("Sensitif", COL_AMBER)
    } else if percent <= 72 {
        ("Seimbang", COL_EMERALD_HOVER)
    } else {
        ("Ketat", COL_BLUE_HOVER)
    }
}

unsafe fn open_folder_in_explorer(folder: &str) {
    let _ = std::fs::create_dir_all(folder);
    let op = wide_null("open");
    let path = wide_null(folder);
    let _ = ShellExecuteW(None, PCWSTR(op.as_ptr()), PCWSTR(path.as_ptr()), PCWSTR::null(), PCWSTR::null(), SW_SHOW);
}

unsafe fn open_url_in_browser(url: &str) {
    if !url.starts_with("https://") {
        return;
    }
    let op = wide_null("open");
    let target = wide_null(url);
    let _ = ShellExecuteW(None, PCWSTR(op.as_ptr()), PCWSTR(target.as_ptr()), PCWSTR::null(), PCWSTR::null(), SW_SHOW);
}

fn compact_path_label(path: &str, max_chars: usize) -> String {
    let total = path.chars().count();
    if total <= max_chars || max_chars < 10 {
        return path.to_string();
    }
    let keep_head = 3usize;
    let keep_tail = max_chars.saturating_sub(keep_head + 3);
    let head: String = path.chars().take(keep_head).collect();
    let tail: String = path.chars().skip(total - keep_tail).collect();
    format!("{}...{}", head, tail)
}

unsafe fn pick_folder(hwnd: HWND, title_str: &str) -> Option<String> {
    let dlg: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_ALL).ok()?;
    if let Ok(opts) = dlg.GetOptions() {
        let _ = dlg.SetOptions(opts | FOS_PICKFOLDERS);
    }
    let title = wide_null(title_str);
    let _ = dlg.SetTitle(PCWSTR(title.as_ptr()));
    if dlg.Show(hwnd).is_ok() {
        if let Ok(item) = dlg.GetResult() {
            if let Ok(pwstr) = item.GetDisplayName(SIGDN_FILESYSPATH) {
                let len = (0usize..).find(|&i| *pwstr.0.add(i) == 0).unwrap_or(0);
                let path = String::from_utf16_lossy(std::slice::from_raw_parts(pwstr.0, len));
                CoTaskMemFree(Some(pwstr.0 as *const c_void));
                if !path.is_empty() {
                    return Some(path);
                }
            }
        }
    }
    None
}

unsafe fn show_tray_menu(state: &UiState) {
    let hmenu = CreatePopupMenu().unwrap_or_default();
    if hmenu.0.is_null() {
        return;
    }
    let running = state.engine.is_running();
    let stopping = state.engine.is_stopping();
    let w_open = wide_null("Buka NVLiteReplay");
    let w_tog = wide_null(if stopping {
        "Menghentikan Replay..."
    } else if running {
        "Matikan Instant Replay"
    } else {
        "Nyalakan Instant Replay"
    });
    let w_save = wide_null("Simpan Replay Sekarang");
    let w_dir = wide_null("Buka Folder Video");
    let update_label = match state.engine.updater.snapshot() {
        UpdateState::Available { latest_tag, .. } => format!("Update Tersedia ({})!", latest_tag),
        UpdateState::Checking | UpdateState::Downloading { .. } => "Memeriksa Update...".to_string(),
        _ => format!("Periksa Update (v{})", APP_VERSION),
    };
    let w_upd = wide_null(&update_label);
    let w_exit = wide_null("Keluar Aplikasi (Exit)");

    let _ = AppendMenuW(hmenu, MF_STRING, ID_TRAY_OPEN, PCWSTR(w_open.as_ptr()));
    let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
    let _ = AppendMenuW(hmenu, MF_STRING, ID_TRAY_TOGGLE, PCWSTR(w_tog.as_ptr()));
    if running {
        let _ = AppendMenuW(hmenu, MF_STRING, ID_TRAY_SAVE, PCWSTR(w_save.as_ptr()));
    }
    let _ = AppendMenuW(hmenu, MF_STRING, ID_TRAY_FOLDER, PCWSTR(w_dir.as_ptr()));
    let _ = AppendMenuW(hmenu, MF_STRING, ID_TRAY_UPDATE, PCWSTR(w_upd.as_ptr()));
    let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
    let _ = AppendMenuW(hmenu, MF_STRING, ID_TRAY_EXIT, PCWSTR(w_exit.as_ptr()));

    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);
    let _ = SetForegroundWindow(state.hwnd);
    let _ = TrackPopupMenu(hmenu, TPM_LEFTALIGN | TPM_BOTTOMALIGN, pt.x, pt.y, 0, state.hwnd, None);
    let _ = PostMessageW(state.hwnd, WM_NULL, WPARAM(0), LPARAM(0));
    let _ = DestroyMenu(hmenu);
}

// ---------- Direct2D Vector Drawing Helpers ----------

unsafe fn fill_rounded(
    rt: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    rect: RectF,
    rad: f32,
    col: D2D1_COLOR_F,
) {
    brush.SetColor(&col);
    rt.FillRoundedRectangle(&rect.to_rounded(rad, rad), brush);
}

unsafe fn stroke_rounded(
    rt: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    rect: RectF,
    rad: f32,
    col: D2D1_COLOR_F,
    width: f32,
) {
    brush.SetColor(&col);
    rt.DrawRoundedRectangle(&rect.to_rounded(rad, rad), brush, width, None);
}

unsafe fn draw_hline(
    rt: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    x1: f32,
    x2: f32,
    y: f32,
    col: D2D1_COLOR_F,
) {
    brush.SetColor(&col);
    rt.DrawLine(
        D2D_POINT_2F { x: x1, y: y + 0.5 },
        D2D_POINT_2F { x: x2, y: y + 0.5 },
        brush,
        1.0,
        None,
    );
}

unsafe fn draw_vline(
    rt: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    x: f32,
    y1: f32,
    y2: f32,
    col: D2D1_COLOR_F,
) {
    brush.SetColor(&col);
    rt.DrawLine(
        D2D_POINT_2F { x: x + 0.5, y: y1 },
        D2D_POINT_2F { x: x + 0.5, y: y2 },
        brush,
        1.0,
        None,
    );
}

unsafe fn draw_card(
    rt: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    rect: RectF,
    rad: f32,
    bg: D2D1_COLOR_F,
    border: D2D1_COLOR_F,
) {
    fill_rounded(rt, brush, rect, rad, bg);
    stroke_rounded(rt, brush, rect, rad, border, 1.0);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BtnStyle {
    Primary,
    Secondary,
    Danger,
    Amber,
}

unsafe fn draw_button(
    rt: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    rect: RectF,
    rad: f32,
    style: BtnStyle,
    hovered: bool,
    disabled: bool,
) {
    if disabled {
        draw_card(rt, brush, rect, rad, COL_SURFACE_ALT, c_rgb(24, 24, 29));
        return;
    }
    match style {
        BtnStyle::Primary => {
            let bg = if hovered { c_rgb(255, 255, 255) } else { COL_TEXT };
            fill_rounded(rt, brush, rect, rad, bg);
        }
        BtnStyle::Secondary => {
            let bg = if hovered { COL_SURFACE_HOV } else { COL_SURFACE };
            let bd = if hovered { COL_BORDER_HI } else { COL_BORDER };
            draw_card(rt, brush, rect, rad, bg, bd);
        }
        BtnStyle::Danger => {
            let bg = if hovered { c_rgb(46, 20, 24) } else { c_rgb(34, 16, 19) };
            let bd = if hovered { c_rgb(130, 44, 52) } else { c_rgb(92, 34, 40) };
            draw_card(rt, brush, rect, rad, bg, bd);
        }
        BtnStyle::Amber => {
            let bg = if hovered { c_rgb(48, 36, 16) } else { c_rgb(36, 27, 13) };
            let bd = if hovered { c_rgb(154, 108, 32) } else { c_rgb(112, 78, 24) };
            draw_card(rt, brush, rect, rad, bg, bd);
        }
    }
}

unsafe fn draw_chip(
    rt: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    rect: RectF,
    rad: f32,
    active: bool,
    hovered: bool,
    disabled: bool,
) {
    if disabled && !active {
        draw_card(rt, brush, rect, rad, c_rgb(11, 11, 14), c_rgb(22, 22, 27));
    } else if active {
        draw_card(rt, brush, rect, rad, COL_SURFACE_ACT, COL_BORDER_HI);
    } else {
        let bg = if hovered { COL_SURFACE_HOV } else { COL_SURFACE_ALT };
        let bd = if hovered { c_rgb(44, 44, 52) } else { COL_BORDER };
        draw_card(rt, brush, rect, rad, bg, bd);
    }
}

unsafe fn draw_progress_bar(
    rt: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    rect: RectF,
    ratio: f32,
    fill_col: D2D1_COLOR_F,
) {
    let rad = (rect.b - rect.t) * 0.5;
    fill_rounded(rt, brush, rect, rad, COL_TRACK);
    let fill_w = (rect.r - rect.l) * ratio.clamp(0.0, 1.0);
    if fill_w > 1.5 {
        let filled = RectF::new(rect.l, rect.t, fill_w, rect.b - rect.t);
        fill_rounded(rt, brush, filled, rad, fill_col);
    }
}

unsafe fn draw_text(
    rt: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    fmt: &IDWriteTextFormat,
    rect: RectF,
    text: &str,
    col: D2D1_COLOR_F,
) {
    let wide: Vec<u16> = text.encode_utf16().collect();
    brush.SetColor(&col);
    rt.DrawText(
        &wide,
        fmt,
        &rect.to_d2d(),
        brush,
        D2D1_DRAW_TEXT_OPTIONS_CLIP,
        Default::default(),
    );
}

unsafe fn draw_toggle_switch(
    rt: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    rect: RectF,
    checked: bool,
    hovered: bool,
    disabled: bool,
    accent: D2D1_COLOR_F,
) {
    let rad = (rect.b - rect.t) * 0.5;
    if disabled {
        draw_card(rt, brush, rect, rad, c_rgb(18, 18, 22), c_rgb(28, 28, 34));
    } else if checked {
        fill_rounded(rt, brush, rect, rad, accent);
    } else {
        let bg = if hovered { c_rgb(36, 36, 44) } else { c_rgb(26, 26, 32) };
        let bd = if hovered { COL_BORDER_HI } else { c_rgb(44, 44, 52) };
        draw_card(rt, brush, rect, rad, bg, bd);
    }

    let knob_r = rad - 3.0;
    let knob_cx = if checked { rect.r - rad } else { rect.l + rad };
    let knob_cy = rect.t + rad;
    let knob_col = if disabled {
        COL_TEXT_DIM
    } else if checked {
        COL_INK
    } else {
        COL_TEXT_SUB
    };
    brush.SetColor(&knob_col);
    rt.FillEllipse(
        &D2D1_ELLIPSE {
            point: D2D_POINT_2F { x: knob_cx, y: knob_cy },
            radiusX: knob_r,
            radiusY: knob_r,
        },
        brush,
    );
}

unsafe fn draw_slider(
    rt: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    rect: RectF,
    percent: u32,
    hovered: bool,
    dragging: bool,
    accent: D2D1_COLOR_F,
) {
    draw_slider_range(rt, brush, rect, percent, 0, 150, hovered, dragging, accent);
}

#[allow(clippy::too_many_arguments)]
unsafe fn draw_slider_range(
    rt: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    rect: RectF,
    value: u32,
    min_val: u32,
    max_val: u32,
    hovered: bool,
    dragging: bool,
    accent: D2D1_COLOR_F,
) {
    let cy = (rect.t + rect.b) * 0.5;
    let track_h = 3.5;
    let span = max_val.saturating_sub(min_val).max(1) as f32;
    let ratio = ((value.saturating_sub(min_val)) as f32 / span).clamp(0.0, 1.0);
    let track = RectF::new(rect.l, cy - track_h * 0.5, rect.r - rect.l, track_h);
    fill_rounded(rt, brush, track, 1.75, COL_TRACK);

    let fill_w = (rect.r - rect.l) * ratio;
    if fill_w > 1.5 {
        let filled = RectF::new(rect.l, cy - track_h * 0.5, fill_w, track_h);
        fill_rounded(rt, brush, filled, 1.75, accent);
    }

    let kx = rect.l + fill_w;
    let kr = if hovered || dragging { 6.2 } else { 5.4 };
    brush.SetColor(&COL_BG);
    rt.FillEllipse(
        &D2D1_ELLIPSE {
            point: D2D_POINT_2F { x: kx, y: cy },
            radiusX: kr + 1.4,
            radiusY: kr + 1.4,
        },
        brush,
    );
    brush.SetColor(&COL_TEXT);
    rt.FillEllipse(
        &D2D1_ELLIPSE {
            point: D2D_POINT_2F { x: kx, y: cy },
            radiusX: kr,
            radiusY: kr,
        },
        brush,
    );
}

fn duration_pill_rect(idx: usize) -> RectF {
    let w = 101.6;
    let gap = 6.08;
    let x = 20.0 + (idx as f32) * (w + gap);
    RectF::new(x, 466.0, w, 32.0)
}

fn fps_pill_rect(idx: usize) -> RectF {
    let w = 100.6;
    let gap = 6.1;
    let x = 20.0 + (idx as f32) * (w + gap);
    RectF::new(x, 398.0, w, 32.0)
}

fn truncate_tail(text: &str, max_chars: usize) -> String {
    let total = text.chars().count();
    if total <= max_chars || max_chars < 6 {
        return text.to_string();
    }
    let keep: String = text.chars().take(max_chars - 3).collect();
    format!("{}...", keep)
}

fn resolved_mic_display(
    cfg: &crate::config::ReplayConfig,
    mic_devices: &[crate::audio::AudioInputDevice],
    runtime_active_name: &str,
    running: bool,
) -> (String, String, bool) {
    let def_dev = mic_devices
        .iter()
        .find(|d| d.is_default)
        .or_else(|| mic_devices.first());
    match cfg.effective_mic_device_id() {
        None => {
            let active = if running && !runtime_active_name.is_empty() {
                runtime_active_name.to_string()
            } else if let Some(d) = def_dev {
                d.name.clone()
            } else {
                "Tidak ada mic aktif".to_string()
            };
            let avail = def_dev.is_some() || (running && !runtime_active_name.is_empty());
            let label = if avail {
                format!("Default Windows — {}", active)
            } else {
                "Default Windows (Tidak ada mic aktif)".to_string()
            };
            (label, active, avail)
        }
        Some(id) => {
            if let Some(d) = mic_devices.iter().find(|d| d.id == id) {
                let label = if d.is_default {
                    format!("{} [Default]", d.name)
                } else {
                    d.name.clone()
                };
                (label, d.name.clone(), true)
            } else if let Some(def) = def_dev {
                (
                    format!("Terputus (Fallback: {})", def.name),
                    def.name.clone(),
                    true,
                )
            } else {
                (
                    "Perangkat mic tidak tersedia".to_string(),
                    "Tidak tersedia".to_string(),
                    false,
                )
            }
        }
    }
}

fn dropdown_menu_rect(kind: DropdownKind, count: usize) -> RectF {
    let row_h = 30.0;
    let h = (count as f32) * row_h + 8.0;
    match kind {
        DropdownKind::Monitor => RectF::new(R_CMB_MONITOR.l, R_CMB_MONITOR.b + 4.0, R_CMB_MONITOR.r - R_CMB_MONITOR.l, h),
        DropdownKind::Resolution => {
            RectF::new(R_CMB_RES.l, R_CMB_RES.t - h - 4.0, R_CMB_RES.r - R_CMB_RES.l, h)
        }
        DropdownKind::Bitrate => {
            RectF::new(R_CMB_BITRATE.l, R_CMB_BITRATE.t - h - 4.0, R_CMB_BITRATE.r - R_CMB_BITRATE.l, h)
        }
        DropdownKind::MicInput => {
            let top = (R_CMB_MIC.t - h - 4.0).max(16.0);
            RectF::new(R_CMB_MIC.l, top, R_CMB_MIC.r - R_CMB_MIC.l, h)
        }
    }
}

unsafe fn paint_ui_to_hdc(state: &mut UiState, target_hdc: HDC) {
    let mut rc = RECT::default();
    let _ = GetClientRect(state.hwnd, &mut rc);
    let w = (rc.right - rc.left).max(1);
    let h = (rc.bottom - rc.top).max(1);
    if !ensure_render_target(state, w, h) {
        return;
    }

    let running = state.engine.is_running();
    let stopping = state.engine.is_stopping();
    let cfg = state.engine.config.lock().unwrap().clone();
    let displays = state.engine.displays.lock().unwrap().clone();
    let mic_devices = state.engine.mic_devices.lock().unwrap().clone();
    let active_mic_name = state.engine.active_mic_name.lock().unwrap().clone();
    let gpu_name = state.engine.gpu_name.lock().unwrap().clone();
    let cap_mode = state.engine.capture_mode_status.lock().unwrap().clone();
    let pipe_status = state.engine.video_pipeline_status.lock().unwrap().clone();
    let detail_msg = state.engine.status_message.lock().unwrap().clone();
    let voice_snap = state.engine.voice.ui_snapshot();
    let buf_sec = state.engine.buffered_seconds();
    let cur_fps = state.engine.current_fps();
    let buf_mb = state.engine.buffer_size_bytes() as f64 / (1024.0 * 1024.0);
    let ram_mb = state.last_process_ram_mb;

    let d2d = &state.d2d;
    let Some(ref rt) = d2d.rt else { return; };
    let Some(ref brush) = d2d.brush else { return; };

    let hx = state.hover_x;
    let hy = state.hover_y;
    let drop_open = state.open_dropdown;

    rt.BeginDraw();
    rt.Clear(Some(&COL_BG));

    // ---- 1. Top Minimalist Navigation Bar (y=0..46) ----
    brush.SetColor(&if running { COL_EMERALD } else { COL_TEXT_SUB });
    rt.DrawEllipse(
        &D2D1_ELLIPSE {
            point: D2D_POINT_2F { x: 28.0, y: 23.0 },
            radiusX: 6.0,
            radiusY: 6.0,
        },
        brush,
        1.4,
        None,
    );
    brush.SetColor(&if running { COL_EMERALD } else { COL_TEXT });
    rt.FillEllipse(
        &D2D1_ELLIPSE {
            point: D2D_POINT_2F { x: 28.0, y: 23.0 },
            radiusX: 2.3,
            radiusY: 2.3,
        },
        brush,
    );
    draw_text(rt, brush, &d2d.fmt_brand, RectF::new(41.0, 10.0, 100.0, 26.0), "NVLiteReplay", COL_TEXT);

    let tabs = [
        (0, R_TAB_0, "Video"),
        (1, R_TAB_1, "Audio"),
        (2, R_TAB_2, "Sistem"),
        (3, R_TAB_3, "Voice"),
    ];
    for (idx, tr, label) in tabs {
        let active = state.active_tab == idx;
        let hov = drop_open.is_none() && tr.contains(hx, hy);
        if active {
            draw_card(rt, brush, tr, 6.0, COL_SURFACE_ACT, COL_BORDER_HI);
        } else if hov {
            fill_rounded(rt, brush, tr, 6.0, COL_SURFACE);
        }
        draw_text(
            rt,
            brush,
            if active { &d2d.fmt_center_bold } else { &d2d.fmt_center_reg },
            tr,
            label,
            if active {
                COL_TEXT
            } else if hov {
                COL_TEXT_SUB
            } else {
                COL_TEXT_MUTED
            },
        );
    }
    if cfg.voice_trigger_enabled && voice_snap.template_count > 0 {
        brush.SetColor(&if running { COL_EMERALD } else { COL_BLUE });
        rt.FillEllipse(
            &D2D1_ELLIPSE {
                point: D2D_POINT_2F { x: R_TAB_3.r - 7.0, y: 16.0 },
                radiusX: 2.5,
                radiusY: 2.5,
            },
            brush,
        );
    }

    // Top-right live status dot + label (mirrors reference "● Discord linked")
    let (top_st_text, top_st_dot, top_st_fg) = if stopping {
        ("Menutup buffer...".to_string(), COL_AMBER, COL_AMBER)
    } else if running {
        (format!("Aktif · {}", cap_mode), COL_EMERALD, COL_TEXT_SUB)
    } else {
        ("Standby · Siap".to_string(), COL_EMERALD, COL_TEXT_SUB)
    };
    let est_w = (top_st_text.chars().count() as f32 * 5.05).clamp(56.0, 210.0);
    let dot_x = (660.0 - est_w - 8.0).max(408.0);
    brush.SetColor(&top_st_dot);
    rt.FillEllipse(
        &D2D1_ELLIPSE {
            point: D2D_POINT_2F { x: dot_x, y: 23.0 },
            radiusX: 3.2,
            radiusY: 3.2,
        },
        brush,
    );
    draw_text(
        rt,
        brush,
        &d2d.fmt_right_small,
        RectF::new(416.0, 11.0, 244.0, 24.0),
        &top_st_text,
        top_st_fg,
    );
    draw_hline(rt, brush, 0.0, LOGICAL_W, 46.0, COL_DIVIDER);

    // ---- 2. Minimalist 2-Column Telemetry & Control Grid (y=46..196) ----
    draw_vline(rt, brush, 424.0, 46.0, 196.0, COL_DIVIDER);
    draw_hline(rt, brush, 0.0, LOGICAL_W, 196.0, COL_DIVIDER);

    // Left column (x=20..404): Hero Buffer Replay + Primary Actions
    draw_text(rt, brush, &d2d.fmt_small, RectF::new(20.0, 58.0, 130.0, 16.0), "BUFFER REPLAY", COL_TEXT_MUTED);
    draw_text(
        rt,
        brush,
        &d2d.fmt_right_small,
        RectF::new(150.0, 58.0, 254.0, 16.0),
        &truncate_tail(&gpu_name, 38),
        COL_TEXT_MUTED,
    );

    let buf_sec_u = buf_sec.clamp(0.0, cfg.replay_duration_seconds as f64).round() as u32;
    let hero_val = format!("{} / {} dtk", buf_sec_u, cfg.replay_duration_seconds);
    draw_text(rt, brush, &d2d.fmt_metric, RectF::new(20.0, 77.0, 230.0, 30.0), &hero_val, COL_TEXT);

    let fps_disp = if cur_fps <= 0.04 { 0.0 } else { cur_fps };
    draw_text(
        rt,
        brush,
        &d2d.fmt_right_reg,
        RectF::new(254.0, 84.0, 150.0, 20.0),
        &format!("{:.1} / {} FPS", fps_disp, cfg.fps),
        if running { COL_EMERALD } else { COL_TEXT_MUTED },
    );

    if !running && !state.hotkey_warning.is_empty() {
        draw_text(
            rt,
            brush,
            &d2d.fmt_small,
            RectF::new(20.0, 110.0, 384.0, 16.0),
            &state.hotkey_warning,
            COL_AMBER,
        );
    } else {
        draw_text(
            rt,
            brush,
            &d2d.fmt_small,
            RectF::new(20.0, 110.0, 384.0, 16.0),
            &truncate_tail(&detail_msg, 64),
            COL_TEXT_MUTED,
        );
    }

    let ratio = (buf_sec / (cfg.replay_duration_seconds.max(1) as f64)).clamp(0.0, 1.0) as f32;
    draw_progress_bar(
        rt,
        brush,
        RectF::new(20.0, 134.0, 384.0, 3.5),
        ratio,
        if running { COL_EMERALD } else { COL_BLUE },
    );

    let (k_tog, k_save) = (
        if cfg.hotkey_toggle.is_enabled() {
            cfg.hotkey_toggle.display()
        } else {
            "Off".to_string()
        },
        if cfg.hotkey_save.is_enabled() {
            cfg.hotkey_save.display()
        } else {
            "Off".to_string()
        },
    );

    let tog_hov = !stopping && drop_open.is_none() && R_BTN_TOGGLE.contains(hx, hy);
    let tog_style = if stopping {
        BtnStyle::Amber
    } else if running {
        BtnStyle::Danger
    } else {
        BtnStyle::Primary
    };
    draw_button(rt, brush, R_BTN_TOGGLE, 6.0, tog_style, tog_hov, false);
    draw_text(
        rt,
        brush,
        &d2d.fmt_center_bold,
        R_BTN_TOGGLE,
        &if stopping {
            "Menghentikan...".to_string()
        } else if running {
            format!("■  Matikan ({})", k_tog)
        } else {
            format!("▸  Nyalakan ({})", k_tog)
        },
        if stopping {
            COL_AMBER
        } else if running {
            COL_RED_HOVER
        } else {
            COL_INK
        },
    );

    let save_hov = drop_open.is_none() && running && R_BTN_SAVE.contains(hx, hy);
    draw_button(
        rt,
        brush,
        R_BTN_SAVE,
        6.0,
        if running { BtnStyle::Primary } else { BtnStyle::Secondary },
        save_hov,
        !running,
    );
    draw_text(
        rt,
        brush,
        &d2d.fmt_center_bold,
        R_BTN_SAVE,
        &format!("Simpan Klip ({})", k_save),
        if running { COL_INK } else { COL_TEXT_DIM },
    );

    // Right column (x=444..660): Compact Resource Breakdown + Folder Button
    draw_text(rt, brush, &d2d.fmt_small, RectF::new(444.0, 58.0, 216.0, 16.0), "RINGKASAN", COL_TEXT_MUTED);

    draw_text(rt, brush, &d2d.fmt_small, RectF::new(444.0, 77.0, 96.0, 16.0), "Memori", COL_TEXT_SUB);
    draw_text(
        rt,
        brush,
        &d2d.fmt_right_reg,
        RectF::new(540.0, 77.0, 120.0, 16.0),
        &format!("{:.1} MB", ram_mb),
        COL_TEXT,
    );
    let ram_ratio = (ram_mb as f32 / 150.0).clamp(0.06, 1.0);
    draw_progress_bar(rt, brush, RectF::new(444.0, 95.0, 216.0, 2.5), ram_ratio, COL_EMERALD);

    let mode_tag = if cfg.storage_mode == BufferStorageMode::RamRingBuffer { "RAM" } else { "SSD" };
    draw_text(rt, brush, &d2d.fmt_small, RectF::new(444.0, 102.0, 96.0, 16.0), "Buffer", COL_TEXT_SUB);
    draw_text(
        rt,
        brush,
        &d2d.fmt_right_reg,
        RectF::new(540.0, 102.0, 120.0, 16.0),
        &format!("{} · {:.1} MB", mode_tag, buf_mb),
        COL_TEXT,
    );
    draw_progress_bar(rt, brush, RectF::new(444.0, 120.0, 216.0, 2.5), ratio.max(0.04), COL_BLUE);

    let enc_vendor = displays
        .get(cfg.monitor_index)
        .or_else(|| displays.first())
        .map(|d| d.vendor_tag.as_str())
        .unwrap_or("GPU");
    let codec_short = match cfg.codec {
        VideoCodecType::HardwareAV1 => "AV1",
        VideoCodecType::HardwareHEVC => "HEVC",
        VideoCodecType::HardwareH264 => "H.264",
    };
    draw_text(rt, brush, &d2d.fmt_small, RectF::new(444.0, 127.0, 96.0, 16.0), "Encoder", COL_TEXT_SUB);
    draw_text(
        rt,
        brush,
        &d2d.fmt_right_small,
        RectF::new(530.0, 127.0, 130.0, 16.0),
        &format!("{} · {}", codec_short, enc_vendor),
        COL_TEXT_SUB,
    );

    let dir_hov = drop_open.is_none() && R_BTN_FOLDER.contains(hx, hy);
    draw_button(rt, brush, R_BTN_FOLDER, 6.0, BtnStyle::Secondary, dir_hov, false);
    draw_text(rt, brush, &d2d.fmt_center_reg, R_BTN_FOLDER, "Buka Folder Video", COL_TEXT);

    // ---- 3. Active Tab Content (y=196..656, flat pitch obsidian canvas) ----
    let (_mic_combo_lbl, mic_active_short, _mic_avail) =
        resolved_mic_display(&cfg, &mic_devices, &active_mic_name, running);
    let codec_caps = *state.engine.codec_caps.lock().unwrap();
    match state.active_tab {
        0 => paint_tab_video(rt, brush, d2d, &cfg, &displays, codec_caps, &pipe_status, running, hx, hy, drop_open),
        1 => paint_tab_audio(
            rt,
            brush,
            d2d,
            &cfg,
            &mic_devices,
            &active_mic_name,
            running,
            hx,
            hy,
            state.dragging_slider,
            drop_open,
        ),
        2 => paint_tab_system(rt, brush, d2d, &cfg, state.capturing_hotkey, running, hx, hy),
        _ => paint_tab_voice(
            rt,
            brush,
            d2d,
            &cfg,
            &mic_active_short,
            &voice_snap,
            &state.phrase_draft,
            state.editing_phrase,
            state.caret_tick,
            running,
            hx,
            hy,
            state.dragging_slider,
        ),
    }

    // ---- 3b. Global Bottom Footer (Version + GitHub Releases Check/Install) ----
    let up_state = state.engine.updater.snapshot();
    paint_update_footer(rt, brush, d2d, &up_state, hx, hy, drop_open);

    // ---- 4. Floating Custom Dropdown Overlay (if open) ----
    if let Some(kind) = drop_open {
        paint_dropdown_overlay(rt, brush, d2d, kind, &cfg, &displays, &mic_devices, hx, hy);
    }

    if rt.EndDraw(None, None).is_err() {
        state.d2d.brush = None;
        state.d2d.rt = None;
        state.d2d.dc_rt = None;
    } else if !state.d2d.mem_dc.0.is_null() {
        let _ = BitBlt(target_hdc, 0, 0, state.d2d.buf_w, state.d2d.buf_h, state.d2d.mem_dc, 0, 0, SRCCOPY);
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn paint_tab_video(
    rt: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    d2d: &D2DResources,
    cfg: &crate::config::ReplayConfig,
    displays: &[crate::engine::DisplayTarget],
    codec_caps: crate::engine::HardwareCodecCaps,
    pipe_status: &str,
    running: bool,
    hx: f32,
    hy: f32,
    drop_open: Option<DropdownKind>,
) {
    let dim_alpha = if running { COL_TEXT_DIM } else { COL_TEXT };

    draw_text(rt, brush, &d2d.fmt_small, RectF::new(20.0, 212.0, 360.0, 16.0), "MONITOR TARGET", COL_TEXT_MUTED);
    if running {
        draw_text(rt, brush, &d2d.fmt_right_small, RectF::new(400.0, 212.0, 260.0, 16.0), "Terkunci saat merekam", COL_AMBER);
    }
    let mon_label = displays
        .get(cfg.monitor_index)
        .or_else(|| displays.first())
        .map(|d| d.display_label())
        .unwrap_or_else(|| "Layar Utama".to_string());
    let mon_hov = !running && drop_open.is_none() && R_CMB_MONITOR.contains(hx, hy);
    let mon_open = drop_open == Some(DropdownKind::Monitor);
    draw_card(
        rt,
        brush,
        R_CMB_MONITOR,
        6.0,
        if mon_hov || mon_open { COL_SURFACE_HOV } else { COL_SURFACE },
        if mon_hov || mon_open { COL_BORDER_HI } else { COL_BORDER },
    );
    draw_text(rt, brush, &d2d.fmt_reg, RectF::new(32.0, 232.0, 594.0, 34.0), &mon_label, dim_alpha);
    draw_text(rt, brush, &d2d.fmt_center_small, RectF::new(632.0, 232.0, 24.0, 34.0), "▾", COL_TEXT_MUTED);

    draw_text(rt, brush, &d2d.fmt_small, RectF::new(20.0, 282.0, 300.0, 16.0), "MODE PENYIMPANAN BUFFER", COL_TEXT_MUTED);

    let ram_sel = cfg.storage_mode == BufferStorageMode::RamRingBuffer;
    let ram_hov = !running && drop_open.is_none() && R_STO_RAM.contains(hx, hy);
    draw_card(
        rt,
        brush,
        R_STO_RAM,
        8.0,
        if ram_sel { COL_SURFACE_ACT } else if ram_hov { COL_SURFACE_HOV } else { COL_SURFACE },
        if ram_sel { COL_BORDER_HI } else { COL_BORDER },
    );
    draw_text(rt, brush, &d2d.fmt_bold, RectF::new(34.0, 312.0, 260.0, 18.0), "RAM Ring Buffer", if ram_sel { COL_TEXT } else { dim_alpha });
    draw_text(rt, brush, &d2d.fmt_small, RectF::new(34.0, 333.0, 284.0, 16.0), "0% tulis disk saat standby · Instan merge", COL_TEXT_MUTED);
    if ram_sel {
        draw_text(rt, brush, &d2d.fmt_right_reg, RectF::new(294.0, 311.0, 28.0, 18.0), "✓", COL_EMERALD);
    }

    let disk_sel = cfg.storage_mode == BufferStorageMode::DiskTempBuffer;
    let disk_hov = !running && drop_open.is_none() && R_STO_DISK.contains(hx, hy);
    draw_card(
        rt,
        brush,
        R_STO_DISK,
        8.0,
        if disk_sel { COL_SURFACE_ACT } else if disk_hov { COL_SURFACE_HOV } else { COL_SURFACE },
        if disk_sel { COL_BORDER_HI } else { COL_BORDER },
    );
    draw_text(rt, brush, &d2d.fmt_bold, RectF::new(360.0, 312.0, 260.0, 18.0), "Disk Temp Buffer", if disk_sel { COL_TEXT } else { dim_alpha });
    draw_text(rt, brush, &d2d.fmt_small, RectF::new(360.0, 333.0, 284.0, 16.0), "Ultra-low RAM · Segmen disimpan di folder temp", COL_TEXT_MUTED);
    if disk_sel {
        draw_text(rt, brush, &d2d.fmt_right_reg, RectF::new(620.0, 311.0, 28.0, 18.0), "✓", COL_EMERALD);
    }

    draw_text(rt, brush, &d2d.fmt_small, RectF::new(20.0, 378.0, 200.0, 16.0), "FRAMERATE", COL_TEXT_MUTED);
    for (i, &(fps_val, label)) in FPS_OPTIONS.iter().enumerate() {
        let r = fps_pill_rect(i);
        let active = cfg.fps == fps_val;
        let hov = !running && drop_open.is_none() && r.contains(hx, hy);
        draw_chip(rt, brush, r, 6.0, active, hov, running);
        draw_text(rt, brush, if active { &d2d.fmt_center_bold } else { &d2d.fmt_center_reg }, r, label, if active { COL_TEXT } else if running { COL_TEXT_DIM } else { COL_TEXT_SUB });
    }

    let enc_vendor = displays
        .get(cfg.monitor_index)
        .or_else(|| displays.first())
        .map(|d| d.vendor_tag.as_str())
        .unwrap_or("GPU");
    let enc_hdr = format!("ENCODER HARDWARE ({})", enc_vendor);
    draw_text(rt, brush, &d2d.fmt_small, RectF::new(346.0, 378.0, 314.0, 16.0), &enc_hdr, COL_TEXT_MUTED);

    let h264_act = cfg.codec == VideoCodecType::HardwareH264;
    let h264_hov = !running && drop_open.is_none() && R_CODEC_H264.contains(hx, hy);
    draw_chip(rt, brush, R_CODEC_H264, 6.0, h264_act, h264_hov, running);
    draw_text(rt, brush, if h264_act { &d2d.fmt_center_bold } else { &d2d.fmt_center_reg }, R_CODEC_H264, "H.264", if h264_act { COL_TEXT } else if running { COL_TEXT_DIM } else { COL_TEXT_SUB });

    let hevc_avail = codec_caps.hevc_hw;
    let hevc_act = cfg.codec == VideoCodecType::HardwareHEVC;
    let hevc_hov = !running && hevc_avail && drop_open.is_none() && R_CODEC_HEVC.contains(hx, hy);
    draw_chip(rt, brush, R_CODEC_HEVC, 6.0, hevc_act, hevc_hov, running || !hevc_avail);
    draw_text(
        rt,
        brush,
        if hevc_act { &d2d.fmt_center_bold } else { &d2d.fmt_center_reg },
        R_CODEC_HEVC,
        "HEVC",
        if hevc_act { COL_TEXT } else if !hevc_avail || running { COL_TEXT_DIM } else { COL_TEXT_SUB },
    );

    let av1_avail = codec_caps.av1_hw;
    let av1_act = cfg.codec == VideoCodecType::HardwareAV1;
    let av1_hov = !running && av1_avail && drop_open.is_none() && R_CODEC_AV1.contains(hx, hy);
    draw_chip(rt, brush, R_CODEC_AV1, 6.0, av1_act, av1_hov, running || !av1_avail);
    draw_text(
        rt,
        brush,
        if av1_act { &d2d.fmt_center_bold } else { &d2d.fmt_center_reg },
        R_CODEC_AV1,
        if av1_avail { "AV1" } else { "AV1 (N/A)" },
        if av1_act { COL_TEXT } else if !av1_avail || running { COL_TEXT_DIM } else { COL_TEXT_SUB },
    );

    draw_text(rt, brush, &d2d.fmt_small, RectF::new(20.0, 446.0, 300.0, 16.0), "DURASI REPLAY", COL_TEXT_MUTED);
    for (i, &(dur_s, label)) in DURATIONS.iter().enumerate() {
        let r = duration_pill_rect(i);
        let active = cfg.replay_duration_seconds == dur_s;
        let hov = !running && drop_open.is_none() && r.contains(hx, hy);
        draw_chip(rt, brush, r, 6.0, active, hov, running);
        draw_text(rt, brush, if active { &d2d.fmt_center_bold } else { &d2d.fmt_center_reg }, r, label, if active { COL_TEXT } else if running { COL_TEXT_DIM } else { COL_TEXT_SUB });
    }

    draw_text(rt, brush, &d2d.fmt_small, RectF::new(20.0, 514.0, 260.0, 16.0), "RESOLUSI VIDEO", COL_TEXT_MUTED);
    let res_label = if cfg.use_native_resolution {
        RESOLUTIONS[0].label
    } else {
        RESOLUTIONS
            .iter()
            .find(|o| !o.native && o.w == cfg.width && o.h == cfg.height)
            .map(|o| o.label)
            .unwrap_or("1920 x 1080 (Full HD 1080p)")
    };
    let res_hov = !running && drop_open.is_none() && R_CMB_RES.contains(hx, hy);
    let res_open = drop_open == Some(DropdownKind::Resolution);
    draw_card(
        rt,
        brush,
        R_CMB_RES,
        6.0,
        if res_hov || res_open { COL_SURFACE_HOV } else { COL_SURFACE },
        if res_hov || res_open { COL_BORDER_HI } else { COL_BORDER },
    );
    draw_text(rt, brush, &d2d.fmt_reg, RectF::new(32.0, 534.0, 268.0, 34.0), res_label, dim_alpha);
    draw_text(rt, brush, &d2d.fmt_center_small, RectF::new(306.0, 534.0, 24.0, 34.0), "▾", COL_TEXT_MUTED);

    draw_text(rt, brush, &d2d.fmt_small, RectF::new(346.0, 514.0, 260.0, 16.0), "BITRATE VIDEO", COL_TEXT_MUTED);
    let br_label = BITRATE_OPTIONS
        .iter()
        .find(|&&(b, _)| b == cfg.bitrate_mbps)
        .map(|&(_, l)| l)
        .unwrap_or("12 Mbps — Ideal Streaming/Klip");
    let br_hov = !running && drop_open.is_none() && R_CMB_BITRATE.contains(hx, hy);
    let br_open = drop_open == Some(DropdownKind::Bitrate);
    draw_card(
        rt,
        brush,
        R_CMB_BITRATE,
        6.0,
        if br_hov || br_open { COL_SURFACE_HOV } else { COL_SURFACE },
        if br_hov || br_open { COL_BORDER_HI } else { COL_BORDER },
    );
    draw_text(rt, brush, &d2d.fmt_reg, RectF::new(358.0, 534.0, 268.0, 34.0), br_label, dim_alpha);
    draw_text(rt, brush, &d2d.fmt_center_small, RectF::new(632.0, 534.0, 24.0, 34.0), "▾", COL_TEXT_MUTED);

    // Subtle bottom pipeline spec line (like reference catalog rows)
    draw_hline(rt, brush, 20.0, 660.0, 588.0, COL_DIVIDER);
    draw_text(rt, brush, &d2d.fmt_small, RectF::new(20.0, 602.0, 170.0, 18.0), "PIPELINE WARNA & VBR", COL_TEXT_MUTED);
    let pipe_desc = if running && pipe_status != "Standby" {
        pipe_status.to_string()
    } else {
        "NV12 GPU BT.709 · Peak-VBR 1.5x · GOP 2 dtk".to_string()
    };
    draw_text(rt, brush, &d2d.fmt_right_small, RectF::new(196.0, 602.0, 464.0, 18.0), &pipe_desc, COL_TEXT_SUB);
}

#[allow(clippy::too_many_arguments)]
unsafe fn paint_tab_audio(
    rt: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    d2d: &D2DResources,
    cfg: &crate::config::ReplayConfig,
    mic_devices: &[crate::audio::AudioInputDevice],
    active_mic_name: &str,
    running: bool,
    hx: f32,
    hy: f32,
    dragging: DragSlider,
    drop_open: Option<DropdownKind>,
) {
    let dim_alpha = if running { COL_TEXT_DIM } else { COL_TEXT };

    // Card 1: Audio Desktop
    let c1 = RectF::new(20.0, 214.0, 640.0, 110.0);
    draw_card(rt, brush, c1, 8.0, COL_SURFACE, COL_BORDER);
    draw_text(rt, brush, &d2d.fmt_title, RectF::new(36.0, 226.0, 380.0, 20.0), "Audio Desktop", COL_TEXT);
    draw_text(rt, brush, &d2d.fmt_small, RectF::new(36.0, 247.0, 440.0, 16.0), "WASAPI Loopback 48 kHz · Game, Discord, Musik", COL_TEXT_MUTED);
    draw_toggle_switch(rt, brush, R_TOG_DESKTOP, cfg.record_desktop_audio, !running && R_TOG_DESKTOP.contains(hx, hy), running, COL_EMERALD);

    draw_slider(
        rt,
        brush,
        R_SLD_DESKTOP,
        cfg.desktop_volume_percent,
        R_SLD_DESKTOP.contains(hx, hy),
        dragging == DragSlider::Desktop,
        COL_EMERALD,
    );
    draw_text(
        rt,
        brush,
        &d2d.fmt_right_reg,
        RectF::new(584.0, 280.0, 58.0, 26.0),
        &format!("{}%", cfg.desktop_volume_percent),
        COL_TEXT,
    );

    // Card 2: Audio Mikrofon + Device Selector
    let c2 = RectF::new(20.0, 338.0, 640.0, 186.0);
    draw_card(rt, brush, c2, 8.0, COL_SURFACE, COL_BORDER);
    draw_text(rt, brush, &d2d.fmt_title, RectF::new(36.0, 350.0, 300.0, 20.0), "Audio Mikrofon", COL_TEXT);
    draw_text(rt, brush, &d2d.fmt_small, RectF::new(36.0, 371.0, 420.0, 16.0), "Tercampur rapi dengan soft-limiter otomatis", COL_TEXT_MUTED);

    let mute_hov = R_MUTE_PILL.contains(hx, hy);
    draw_card(
        rt,
        brush,
        R_MUTE_PILL,
        6.0,
        if cfg.mic_muted {
            c_rgb(36, 16, 19)
        } else if mute_hov {
            COL_SURFACE_HOV
        } else {
            COL_SURFACE_ALT
        },
        if cfg.mic_muted {
            c_rgb(96, 36, 42)
        } else if mute_hov {
            COL_BORDER_HI
        } else {
            COL_BORDER
        },
    );
    brush.SetColor(&if cfg.mic_muted { COL_RED } else { COL_EMERALD });
    rt.FillEllipse(
        &D2D1_ELLIPSE {
            point: D2D_POINT_2F { x: R_MUTE_PILL.l + 14.0, y: (R_MUTE_PILL.t + R_MUTE_PILL.b) * 0.5 },
            radiusX: 3.0,
            radiusY: 3.0,
        },
        brush,
    );
    draw_text(
        rt,
        brush,
        &d2d.fmt_center_small,
        RectF::new(R_MUTE_PILL.l + 14.0, R_MUTE_PILL.t, R_MUTE_PILL.r - R_MUTE_PILL.l - 16.0, R_MUTE_PILL.b - R_MUTE_PILL.t),
        if cfg.mic_muted { "MUTED" } else { "LIVE" },
        if cfg.mic_muted { COL_RED_HOVER } else { COL_TEXT_SUB },
    );

    draw_toggle_switch(rt, brush, R_TOG_MIC, cfg.record_microphone, !running && R_TOG_MIC.contains(hx, hy), running, COL_EMERALD);
    draw_slider(
        rt,
        brush,
        R_SLD_MIC,
        cfg.mic_volume_percent,
        R_SLD_MIC.contains(hx, hy),
        dragging == DragSlider::Mic,
        if cfg.mic_muted { COL_RED } else { COL_BLUE },
    );
    draw_text(
        rt,
        brush,
        &d2d.fmt_right_reg,
        RectF::new(584.0, 404.0, 58.0, 26.0),
        &format!("{}%", cfg.mic_volume_percent),
        if cfg.mic_muted { COL_RED_HOVER } else { COL_TEXT },
    );

    draw_hline(rt, brush, 36.0, 644.0, 442.0, COL_DIVIDER);

    let (mic_label, _, mic_avail) = resolved_mic_display(cfg, mic_devices, active_mic_name, running);
    draw_text(rt, brush, &d2d.fmt_small, RectF::new(36.0, 452.0, 240.0, 16.0), "INPUT MIKROFON", COL_TEXT_MUTED);
    let (mic_hint, mic_hint_col) = if running {
        ("Terkunci saat merekam", COL_AMBER)
    } else if !mic_avail {
        ("Tidak ada mic aktif terdeteksi", COL_RED_HOVER)
    } else if cfg.effective_mic_device_id().is_some() {
        ("Perangkat Pilihan Aktif", COL_EMERALD)
    } else {
        ("Mengikuti Default Windows", COL_TEXT_MUTED)
    };
    draw_text(rt, brush, &d2d.fmt_right_small, RectF::new(300.0, 452.0, 344.0, 16.0), mic_hint, mic_hint_col);

    let cmb_hov = !running && drop_open.is_none() && R_CMB_MIC.contains(hx, hy);
    let cmb_open = drop_open == Some(DropdownKind::MicInput);
    draw_card(
        rt,
        brush,
        R_CMB_MIC,
        6.0,
        if cmb_hov || cmb_open { COL_SURFACE_HOV } else { COL_SURFACE_ALT },
        if cmb_hov || cmb_open { COL_BORDER_HI } else { COL_BORDER },
    );
    let disp_lbl = truncate_tail(&mic_label, 74);
    draw_text(
        rt,
        brush,
        &d2d.fmt_reg,
        RectF::new(48.0, 474.0, 560.0, 34.0),
        &disp_lbl,
        if !mic_avail { COL_RED_HOVER } else { dim_alpha },
    );
    draw_text(rt, brush, &d2d.fmt_center_small, RectF::new(616.0, 474.0, 24.0, 34.0), "▾", COL_TEXT_MUTED);

    // Bottom minimalist notes (mirrors reference catalog checklist)
    draw_text(rt, brush, &d2d.fmt_small, RectF::new(20.0, 544.0, 300.0, 16.0), "SINKRONISASI AUDIO", COL_TEXT_MUTED);
    draw_text(rt, brush, &d2d.fmt_bold, RectF::new(20.0, 568.0, 18.0, 20.0), "✓", COL_EMERALD);
    draw_text(
        rt,
        brush,
        &d2d.fmt_small,
        RectF::new(38.0, 569.0, 620.0, 18.0),
        "Slider volume dapat digeser langsung (live) meskipun Instant Replay sedang berjalan.",
        COL_TEXT_SUB,
    );
    draw_hline(rt, brush, 20.0, 660.0, 594.0, COL_DIVIDER);
    draw_text(rt, brush, &d2d.fmt_bold, RectF::new(20.0, 602.0, 18.0, 20.0), "✓", COL_EMERALD);
    draw_text(
        rt,
        brush,
        &d2d.fmt_small,
        RectF::new(38.0, 603.0, 620.0, 18.0),
        "Input mikrofon di atas dipakai serentak untuk rekaman Instant Replay dan Voice Clip.",
        COL_TEXT_SUB,
    );
}

#[allow(clippy::too_many_arguments)]
unsafe fn paint_tab_system(
    rt: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    d2d: &D2DResources,
    cfg: &crate::config::ReplayConfig,
    capturing: Option<HotkeySlot>,
    running: bool,
    hx: f32,
    hy: f32,
) {
    draw_text(rt, brush, &d2d.fmt_small, RectF::new(20.0, 212.0, 340.0, 16.0), "GLOBAL SHORTCUT — KLIK SLOT UNTUK REKAM", COL_TEXT_MUTED);
    let (mode_txt, mode_col) = if capturing.is_some() {
        ("Tekan kombinasi tombol (Esc batal)", COL_AMBER)
    } else if cfg.matching_hotkey_preset().is_none() {
        ("Mode Custom Aktif", COL_EMERALD)
    } else {
        ("Preset Standar", COL_TEXT_MUTED)
    };
    draw_text(rt, brush, &d2d.fmt_right_small, RectF::new(380.0, 212.0, 280.0, 16.0), mode_txt, mode_col);

    let live_mods = if capturing.is_some() {
        current_async_hotkey_mods()
    } else {
        0
    };
    let bindings = [cfg.hotkey_toggle, cfg.hotkey_save, cfg.hotkey_mute];
    for (idx, slot) in HotkeySlot::ALL.iter().copied().enumerate() {
        let r = hk_slot_rect(idx);
        let binding = bindings[idx];
        let is_cap = capturing == Some(slot);
        let clear_r = hk_clear_rect(idx);
        let clear_hov = binding.is_enabled() && !is_cap && clear_r.contains(hx, hy);
        let hov = r.contains(hx, hy) && !clear_hov;

        let (bg, bd) = if is_cap {
            (c_rgb(32, 24, 12), c_rgb(138, 96, 26))
        } else if hov {
            (COL_SURFACE_HOV, COL_BORDER_HI)
        } else {
            (COL_SURFACE, COL_BORDER)
        };
        draw_card(rt, brush, r, 8.0, bg, bd);

        draw_text(
            rt,
            brush,
            &d2d.fmt_small,
            RectF::new(r.l + 12.0, r.t + 8.0, 158.0, 16.0),
            slot.title(),
            if is_cap { COL_AMBER } else { COL_TEXT_MUTED },
        );

        if binding.is_enabled() && !is_cap {
            if clear_hov {
                draw_card(rt, brush, clear_r, 4.0, c_rgb(44, 18, 22), c_rgb(104, 38, 44));
            }
            draw_text(
                rt,
                brush,
                &d2d.fmt_center_small,
                clear_r,
                "×",
                if clear_hov { COL_RED_HOVER } else { COL_TEXT_MUTED },
            );
        }

        let val_r = RectF::new(r.l + 12.0, r.t + 28.0, r.r - r.l - 24.0, 22.0);
        let (val_str, val_col) = if is_cap {
            (format_modifier_preview(live_mods), COL_AMBER)
        } else if binding.is_enabled() {
            (
                binding.display(),
                if idx == 0 {
                    COL_EMERALD
                } else if idx == 1 {
                    COL_BLUE
                } else {
                    COL_TEXT
                },
            )
        } else {
            ("Tidak Aktif".to_string(), COL_TEXT_DIM)
        };
        draw_text(rt, brush, &d2d.fmt_bold, val_r, &val_str, val_col);
    }

    draw_text(rt, brush, &d2d.fmt_small, RectF::new(20.0, 301.0, 86.0, 18.0), "Preset Cepat:", COL_TEXT_MUTED);
    let active_preset = cfg.matching_hotkey_preset();
    let presets = [
        (R_HK_PRESET_0, HotkeyPresetType::CtrlShiftF9F10, "Ctrl+Shift+F9/F10/M"),
        (R_HK_PRESET_1, HotkeyPresetType::FunctionKeysF8F7, "F8 / F7 / F6"),
        (R_HK_PRESET_2, HotkeyPresetType::AltF9F10, "Alt+F9 / F10 / M"),
    ];
    for (pr, p_kind, label) in presets {
        let active = active_preset == Some(p_kind);
        let hov = pr.contains(hx, hy);
        draw_chip(rt, brush, pr, 6.0, active, hov, false);
        draw_text(
            rt,
            brush,
            if active { &d2d.fmt_center_bold } else { &d2d.fmt_center_small },
            pr,
            label,
            if active { COL_TEXT } else { COL_TEXT_SUB },
        );
    }
    draw_text(rt, brush, &d2d.fmt_right_small, RectF::new(484.0, 301.0, 176.0, 18.0), "Ctrl / Shift / Alt / Win", COL_TEXT_MUTED);

    draw_hline(rt, brush, 20.0, 660.0, 336.0, COL_DIVIDER);

    draw_text(rt, brush, &d2d.fmt_small, RectF::new(20.0, 350.0, 300.0, 16.0), "FOLDER PENYIMPANAN VIDEO MP4", COL_TEXT_MUTED);
    let path_box = RectF::new(20.0, 370.0, 516.0, 32.0);
    draw_card(rt, brush, path_box, 6.0, COL_SURFACE, COL_BORDER);
    let out_disp = compact_path_label(&cfg.output_folder, 66);
    draw_text(rt, brush, &d2d.fmt_reg, RectF::new(32.0, 370.0, 492.0, 32.0), &out_disp, COL_TEXT_SUB);

    let br_hov = R_BTN_BROWSE.contains(hx, hy);
    draw_button(rt, brush, R_BTN_BROWSE, 6.0, BtnStyle::Secondary, br_hov, false);
    draw_text(rt, brush, &d2d.fmt_center_bold, R_BTN_BROWSE, "Pilih Folder", COL_TEXT);

    let default_temp = crate::config::ReplayConfig::default_temp_folder();
    let is_custom_temp = cfg.temp_folder.trim() != default_temp;
    draw_text(rt, brush, &d2d.fmt_small, RectF::new(20.0, 414.0, 360.0, 16.0), "FOLDER TEMP BUFFER (MODE DISK)", COL_TEXT_MUTED);
    let (temp_hint, temp_hint_col) = if running {
        ("Terkunci saat Replay aktif", COL_AMBER)
    } else if is_custom_temp {
        ("Lokasi Custom Aktif", COL_EMERALD)
    } else {
        ("Default Windows %TEMP%", COL_TEXT_MUTED)
    };
    draw_text(rt, brush, &d2d.fmt_right_small, RectF::new(400.0, 414.0, 260.0, 16.0), temp_hint, temp_hint_col);

    let temp_box = RectF::new(20.0, 434.0, 438.0, 32.0);
    draw_card(
        rt,
        brush,
        temp_box,
        6.0,
        COL_SURFACE,
        if is_custom_temp { COL_BORDER_HI } else { COL_BORDER },
    );
    let temp_disp = compact_path_label(&cfg.temp_folder, 54);
    draw_text(
        rt,
        brush,
        &d2d.fmt_reg,
        RectF::new(32.0, 434.0, 414.0, 32.0),
        &temp_disp,
        if running { COL_TEXT_DIM } else { COL_TEXT_SUB },
    );

    let tbr_hov = !running && R_BTN_TEMP_BROWSE.contains(hx, hy);
    draw_button(rt, brush, R_BTN_TEMP_BROWSE, 6.0, BtnStyle::Secondary, tbr_hov, running);
    draw_text(
        rt,
        brush,
        &d2d.fmt_center_bold,
        R_BTN_TEMP_BROWSE,
        "Pilih Folder",
        if running { COL_TEXT_DIM } else { COL_TEXT },
    );

    let trst_active = !running && is_custom_temp;
    let trst_hov = trst_active && R_BTN_TEMP_RESET.contains(hx, hy);
    draw_button(rt, brush, R_BTN_TEMP_RESET, 6.0, BtnStyle::Secondary, trst_hov, !trst_active);
    draw_text(
        rt,
        brush,
        &d2d.fmt_center_reg,
        R_BTN_TEMP_RESET,
        "Default",
        if trst_active { COL_TEXT_SUB } else { COL_TEXT_DIM },
    );

    draw_hline(rt, brush, 20.0, 660.0, 476.0, COL_DIVIDER);
    draw_text(rt, brush, &d2d.fmt_small, RectF::new(20.0, 484.0, 300.0, 16.0), "PERILAKU APLIKASI", COL_TEXT_MUTED);

    let rows = [
        (R_TOG_TRAY, 502.0, "Minimize ke System Tray saat jendela ditutup (X)", cfg.minimize_to_tray_on_close),
        (R_TOG_BEEP, 536.0, "Bunyi Beep konfirmasi saat klip replay tersimpan", cfg.play_sound_on_save),
        (R_TOG_FOCUS, 570.0, "Mode Fokus Game — Tahan notifikasi popup saat Fullscreen", cfg.suppress_notifications_in_game),
        (R_TOG_AUTO_UPDATE, 604.0, "Periksa pembaruan GitHub Releases otomatis saat dibuka", cfg.auto_check_updates),
    ];
    for (idx, (tog_r, y, title, val)) in rows.into_iter().enumerate() {
        draw_text(rt, brush, &d2d.fmt_reg, RectF::new(20.0, y, 580.0, 30.0), title, COL_TEXT_SUB);
        draw_toggle_switch(rt, brush, tog_r, val, tog_r.contains(hx, hy), false, COL_EMERALD);
        if idx < 3 {
            draw_hline(rt, brush, 20.0, 660.0, y + 32.0, c_rgb(24, 24, 29));
        }
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn paint_update_footer(
    rt: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    d2d: &D2DResources,
    up_state: &UpdateState,
    hx: f32,
    hy: f32,
    drop_open: Option<DropdownKind>,
) {
    let footer_bg = RectF::new(0.0, 646.0, LOGICAL_W, LOGICAL_H - 646.0);
    brush.SetColor(&c_rgb(12, 12, 15));
    rt.FillRectangle(&footer_bg.to_d2d(), brush);
    draw_hline(rt, brush, 0.0, LOGICAL_W, 646.0, COL_DIVIDER);

    let (dot_col, title_txt, title_col, sub_txt, act_label, act_style, act_disabled, dl_ratio) =
        match up_state {
            UpdateState::Idle => (
                COL_TEXT_DIM,
                format!("NVLiteReplay v{} · GitHub Releases", APP_VERSION),
                COL_TEXT_SUB,
                "Repo: xionnx/NVLiteReplay · Portable Windows x64".to_string(),
                "Periksa Update".to_string(),
                BtnStyle::Secondary,
                false,
                None,
            ),
            UpdateState::Checking => (
                COL_BLUE,
                format!("NVLiteReplay v{} · Memeriksa GitHub...", APP_VERSION),
                COL_TEXT,
                "Menghubungi GitHub Releases API (xionnx/NVLiteReplay)...".to_string(),
                "Memeriksa...".to_string(),
                BtnStyle::Secondary,
                true,
                None,
            ),
            UpdateState::UpToDate { latest_tag, .. } => (
                COL_EMERALD,
                format!("NVLiteReplay v{} · Sudah versi terbaru", APP_VERSION),
                COL_TEXT,
                format!("Sinkron dengan rilis GitHub {} · xionnx/NVLiteReplay", latest_tag),
                "Periksa Update".to_string(),
                BtnStyle::Secondary,
                false,
                None,
            ),
            UpdateState::Available {
                latest_tag,
                release_name,
                notes_summary,
                exe_url,
                exe_size,
                ..
            } => {
                let sub = if !notes_summary.is_empty() {
                    truncate_tail(&format!("{} — {}", release_name, notes_summary), 62)
                } else if exe_url.is_some() && *exe_size > 0 {
                    format!(
                        "{} siap dipasang otomatis ({:.0} KB)",
                        release_name,
                        *exe_size as f64 / 1024.0
                    )
                } else {
                    "Klik Unduh Rilis untuk membuka halaman GitHub Releases".to_string()
                };
                let act = if exe_url.is_some() {
                    format!("Pasang {}", latest_tag)
                } else {
                    format!("Unduh {} ↗", latest_tag)
                };
                (
                    COL_EMERALD,
                    format!("Update {} tersedia! (Terpasang: v{})", latest_tag, APP_VERSION),
                    COL_EMERALD_HOVER,
                    sub,
                    act,
                    BtnStyle::Primary,
                    false,
                    None,
                )
            }
            UpdateState::Downloading {
                latest_tag,
                downloaded_bytes,
                total_bytes,
            } => {
                let (pct_str, ratio) = if *total_bytes > 0 {
                    let r = (*downloaded_bytes as f32 / *total_bytes as f32).clamp(0.0, 1.0);
                    (format!("{:.0}%", r * 100.0), Some(r))
                } else {
                    (format!("{:.0} KB", *downloaded_bytes as f64 / 1024.0), Some(0.35))
                };
                let sub = if *total_bytes > 0 {
                    format!(
                        "Mengunduh binary dari GitHub ({:.0} / {:.0} KB)...",
                        *downloaded_bytes as f64 / 1024.0,
                        *total_bytes as f64 / 1024.0
                    )
                } else {
                    "Mengunduh paket pembaruan dari GitHub Releases...".to_string()
                };
                (
                    COL_BLUE,
                    format!("Mengunduh {} ({})...", latest_tag, pct_str),
                    COL_TEXT,
                    sub,
                    "Mengunduh...".to_string(),
                    BtnStyle::Secondary,
                    true,
                    ratio,
                )
            }
            UpdateState::Installing { latest_tag } | UpdateState::ReadyToRestart { latest_tag } => (
                COL_EMERALD,
                format!("Memasang {} & me-restart aplikasi...", latest_tag),
                COL_EMERALD_HOVER,
                "Menutup sesi buffer & mengganti executable secara aman...".to_string(),
                "Memasang...".to_string(),
                BtnStyle::Secondary,
                true,
                Some(1.0),
            ),
            UpdateState::Error(err) => (
                COL_AMBER,
                format!("NVLiteReplay v{} · Pembaruan terganggu", APP_VERSION),
                COL_AMBER,
                truncate_tail(err, 64),
                "Coba Lagi".to_string(),
                BtnStyle::Secondary,
                false,
                None,
            ),
        };

    if let Some(ratio) = dl_ratio {
        draw_progress_bar(rt, brush, RectF::new(0.0, 646.0, LOGICAL_W, 2.5), ratio, COL_EMERALD);
    }

    brush.SetColor(&dot_col);
    rt.FillEllipse(
        &D2D1_ELLIPSE {
            point: D2D_POINT_2F { x: 24.0, y: 664.0 },
            radiusX: 3.0,
            radiusY: 3.0,
        },
        brush,
    );
    draw_text(rt, brush, &d2d.fmt_bold, RectF::new(34.0, 655.0, 384.0, 18.0), &title_txt, title_col);
    draw_text(rt, brush, &d2d.fmt_small, RectF::new(34.0, 673.0, 384.0, 16.0), &sub_txt, COL_TEXT_MUTED);

    let gh_hov = drop_open.is_none() && R_BTN_GITHUB_PAGE.contains(hx, hy);
    draw_button(rt, brush, R_BTN_GITHUB_PAGE, 6.0, BtnStyle::Secondary, gh_hov, false);
    let gh_label = if matches!(up_state, UpdateState::Available { .. }) {
        "Catatan ↗"
    } else {
        "GitHub ↗"
    };
    draw_text(rt, brush, &d2d.fmt_center_reg, R_BTN_GITHUB_PAGE, gh_label, COL_TEXT_SUB);

    let act_hov = !act_disabled && drop_open.is_none() && R_BTN_UPDATE_ACTION.contains(hx, hy);
    draw_button(rt, brush, R_BTN_UPDATE_ACTION, 6.0, act_style, act_hov, act_disabled);
    let act_fg = if act_disabled {
        COL_TEXT_DIM
    } else if matches!(act_style, BtnStyle::Primary) {
        COL_INK
    } else {
        COL_TEXT
    };
    draw_text(rt, brush, &d2d.fmt_center_bold, R_BTN_UPDATE_ACTION, &act_label, act_fg);
}

#[allow(clippy::too_many_arguments)]
unsafe fn paint_tab_voice(
    rt: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    d2d: &D2DResources,
    cfg: &crate::config::ReplayConfig,
    mic_active_short: &str,
    vs: &crate::voice::VoiceUiSnapshot,
    phrase_draft: &str,
    editing_phrase: bool,
    caret_tick: u32,
    running: bool,
    hx: f32,
    hy: f32,
    dragging: DragSlider,
) {
    // ---- Card 1: Status Header + Master Toggle + Live VAD Mic Meter ----
    let c1 = RectF::new(20.0, 214.0, 640.0, 98.0);
    draw_card(
        rt,
        brush,
        c1,
        8.0,
        COL_SURFACE,
        if vs.is_enrolling { c_rgb(138, 96, 26) } else { COL_BORDER },
    );

    draw_text(rt, brush, &d2d.fmt_title, RectF::new(36.0, 224.0, 360.0, 20.0), "Voice Clip Trigger", COL_TEXT);
    draw_text(
        rt,
        brush,
        &d2d.fmt_small,
        RectF::new(36.0, 245.0, 410.0, 16.0),
        &format!("Input aktif: {}", truncate_tail(mic_active_short, 50)),
        COL_TEXT_MUTED,
    );

    let (st_label, st_dot, st_fg) = if vs.is_enrolling {
        ("MEREKAM", COL_AMBER, COL_AMBER)
    } else if !cfg.voice_trigger_enabled {
        ("NONAKTIF", COL_TEXT_DIM, COL_TEXT_MUTED)
    } else if vs.template_count == 0 {
        ("PERLU CONTOH", COL_AMBER, COL_AMBER)
    } else if !running {
        ("STANDBY", COL_BLUE, COL_TEXT_SUB)
    } else if vs.is_speaking {
        ("BICARA", COL_BLUE, COL_BLUE_HOVER)
    } else {
        ("MENDENGARKAN", COL_EMERALD, COL_EMERALD)
    };
    let st_badge = RectF::new(470.0, 228.0, 118.0, 24.0);
    draw_card(rt, brush, st_badge, 6.0, COL_SURFACE_ALT, COL_BORDER);
    brush.SetColor(&st_dot);
    rt.FillEllipse(
        &D2D1_ELLIPSE {
            point: D2D_POINT_2F { x: st_badge.l + 12.0, y: (st_badge.t + st_badge.b) * 0.5 },
            radiusX: 2.8,
            radiusY: 2.8,
        },
        brush,
    );
    draw_text(
        rt,
        brush,
        &d2d.fmt_center_small,
        RectF::new(st_badge.l + 14.0, st_badge.t, st_badge.r - st_badge.l - 16.0, st_badge.b - st_badge.t),
        st_label,
        st_fg,
    );

    draw_toggle_switch(
        rt,
        brush,
        R_TOG_VOICE,
        cfg.voice_trigger_enabled,
        R_TOG_VOICE.contains(hx, hy),
        false,
        COL_EMERALD,
    );

    // Live Mic Level Meter
    draw_text(rt, brush, &d2d.fmt_small, RectF::new(36.0, 278.0, 74.0, 18.0), "Level Mic", COL_TEXT_MUTED);
    let meter_bg = RectF::new(112.0, 285.0, 456.0, 3.5);
    let lvl = vs.mic_level.clamp(0.0, 1.0);
    draw_progress_bar(rt, brush, meter_bg, lvl, if vs.is_speaking { COL_EMERALD } else { COL_BLUE });
    let meter_txt = if vs.last_confidence > 0.0 {
        format!("Skor {:.0}%", vs.last_confidence)
    } else {
        format!("{:.0}% Mic", (lvl * 100.0).round())
    };
    draw_text(
        rt,
        brush,
        &d2d.fmt_right_small,
        RectF::new(572.0, 278.0, 70.0, 18.0),
        &meter_txt,
        if vs.is_speaking { COL_EMERALD } else { COL_TEXT_SUB },
    );

    // ---- Card 2: Custom Trigger Phrase Input + 1-Tap Presets ----
    let c2 = RectF::new(20.0, 326.0, 640.0, 94.0);
    draw_card(rt, brush, c2, 8.0, COL_SURFACE, COL_BORDER);

    draw_text(rt, brush, &d2d.fmt_small, RectF::new(36.0, 338.0, 240.0, 16.0), "FRASA PEMICU CUSTOM", COL_TEXT_MUTED);
    for (i, &preset) in PHRASE_PRESETS.iter().enumerate() {
        let r = phrase_preset_rect(i);
        let active = cfg.voice_phrase.eq_ignore_ascii_case(preset);
        let hov = r.contains(hx, hy);
        draw_chip(rt, brush, r, 5.0, active, hov, false);
        draw_text(rt, brush, &d2d.fmt_center_small, r, preset, if active { COL_TEXT } else { COL_TEXT_MUTED });
    }

    let box_hov = R_TXT_PHRASE.contains(hx, hy);
    draw_card(
        rt,
        brush,
        R_TXT_PHRASE,
        6.0,
        COL_SURFACE_ALT,
        if editing_phrase { COL_BORDER_HI } else if box_hov { c_rgb(44, 44, 52) } else { COL_BORDER },
    );

    let show_caret = editing_phrase && (caret_tick % 6 < 3);
    let display_phrase = if editing_phrase {
        if show_caret {
            format!("{}|", phrase_draft)
        } else {
            phrase_draft.to_string()
        }
    } else if phrase_draft.is_empty() {
        "Ketik kata bebas (misal: Roman, clip that)...".to_string()
    } else {
        phrase_draft.to_string()
    };
    draw_text(
        rt,
        brush,
        &d2d.fmt_bold,
        RectF::new(R_TXT_PHRASE.l + 12.0, R_TXT_PHRASE.t, 360.0, 36.0),
        &display_phrase,
        if phrase_draft.is_empty() && !editing_phrase { COL_TEXT_DIM } else { COL_TEXT },
    );
    if !editing_phrase {
        draw_text(
            rt,
            brush,
            &d2d.fmt_right_small,
            RectF::new(R_TXT_PHRASE.r - 116.0, R_TXT_PHRASE.t, 104.0, 36.0),
            "Klik untuk edit",
            COL_TEXT_MUTED,
        );
    }

    let dirty = editing_phrase || phrase_draft.trim() != cfg.voice_phrase;
    let save_hov = R_BTN_SAVE_PHRASE.contains(hx, hy);
    draw_button(
        rt,
        brush,
        R_BTN_SAVE_PHRASE,
        6.0,
        if dirty { BtnStyle::Primary } else { BtnStyle::Secondary },
        save_hov,
        false,
    );
    draw_text(
        rt,
        brush,
        &d2d.fmt_center_bold,
        R_BTN_SAVE_PHRASE,
        if dirty { "Simpan" } else { "Ubah Teks" },
        if dirty { COL_INK } else { COL_TEXT_SUB },
    );

    // ---- Card 3: 3 Slot Contoh Suara + Rekam & Reset ----
    let c3 = RectF::new(20.0, 434.0, 640.0, 92.0);
    draw_card(rt, brush, c3, 8.0, COL_SURFACE, COL_BORDER);

    let kb = vs.memory_bytes as f32 / 1024.0;
    draw_text(
        rt,
        brush,
        &d2d.fmt_small,
        RectF::new(36.0, 446.0, 290.0, 16.0),
        &format!("CONTOH SUARA ({}/3 SLOT · {:.1} KB)", vs.template_count, kb),
        COL_TEXT_MUTED,
    );
    draw_text(
        rt,
        brush,
        &d2d.fmt_right_small,
        RectF::new(336.0, 446.0, 306.0, 16.0),
        &vs.last_status,
        if vs.is_enrolling { COL_AMBER } else if vs.template_count > 0 { COL_EMERALD } else { COL_TEXT_MUTED },
    );

    for i in 0..3 {
        let sr = RectF::new(36.0 + (i as f32) * 92.0, 474.0, 86.0, 34.0);
        if let Some(&dur) = vs.template_durations.get(i) {
            draw_card(rt, brush, sr, 6.0, COL_SURFACE_ACT, COL_BORDER_HI);
            draw_text(rt, brush, &d2d.fmt_center_small, sr, &format!("✓ #{} ({:.1}s)", i + 1, dur), COL_EMERALD);
        } else {
            draw_card(rt, brush, sr, 6.0, COL_SURFACE_ALT, COL_BORDER);
            draw_text(rt, brush, &d2d.fmt_center_small, sr, &format!("Slot #{}", i + 1), COL_TEXT_DIM);
        }
    }

    let enr_hov = R_BTN_ENROLL.contains(hx, hy);
    draw_button(
        rt,
        brush,
        R_BTN_ENROLL,
        6.0,
        if vs.is_enrolling { BtnStyle::Amber } else { BtnStyle::Primary },
        enr_hov,
        false,
    );
    draw_text(
        rt,
        brush,
        &d2d.fmt_center_bold,
        R_BTN_ENROLL,
        if vs.is_enrolling { "Ucapkan Frasa Sekarang..." } else { "Rekam Contoh Suara" },
        if vs.is_enrolling { COL_AMBER } else { COL_INK },
    );

    let rst_hov = R_BTN_RESET_VOICE.contains(hx, hy);
    draw_button(
        rt,
        brush,
        R_BTN_RESET_VOICE,
        6.0,
        if rst_hov { BtnStyle::Danger } else { BtnStyle::Secondary },
        rst_hov,
        false,
    );
    draw_text(
        rt,
        brush,
        &d2d.fmt_center_reg,
        R_BTN_RESET_VOICE,
        "Reset",
        if rst_hov { COL_RED_HOVER } else { COL_TEXT_SUB },
    );

    // ---- Card 4: Sensitivitas Deteksi & Cooldown ----
    let c4 = RectF::new(20.0, 540.0, 640.0, 88.0);
    draw_card(rt, brush, c4, 8.0, COL_SURFACE, COL_BORDER);
    draw_vline(rt, brush, 386.0, 554.0, 614.0, COL_DIVIDER);

    let sens = cfg.voice_sensitivity.clamp(SENS_MIN_PERCENT, SENS_MAX_PERCENT);
    let (tier_lbl, tier_col) = sens_tier_info(sens);
    draw_text(rt, brush, &d2d.fmt_small, RectF::new(36.0, 554.0, 190.0, 16.0), "SENSITIVITAS DETEKSI", COL_TEXT_MUTED);
    draw_text(
        rt,
        brush,
        &d2d.fmt_right_small,
        RectF::new(230.0, 554.0, 140.0, 16.0),
        &format!("{}% · {}", sens, tier_lbl),
        tier_col,
    );

    let dec_hov = R_BTN_SENS_DEC.contains(hx, hy);
    draw_button(rt, brush, R_BTN_SENS_DEC, 6.0, BtnStyle::Secondary, dec_hov, false);
    draw_text(rt, brush, &d2d.fmt_center_bold, R_BTN_SENS_DEC, "-", COL_TEXT);

    draw_slider_range(
        rt,
        brush,
        R_SLD_VOICE_SENS,
        sens,
        SENS_MIN_PERCENT,
        SENS_MAX_PERCENT,
        R_SLD_VOICE_SENS.contains(hx, hy),
        dragging == DragSlider::VoiceSensitivity,
        tier_col,
    );

    let inc_hov = R_BTN_SENS_INC.contains(hx, hy);
    draw_button(rt, brush, R_BTN_SENS_INC, 6.0, BtnStyle::Secondary, inc_hov, false);
    draw_text(rt, brush, &d2d.fmt_center_bold, R_BTN_SENS_INC, "+", COL_TEXT);

    draw_text(rt, brush, &d2d.fmt_small, RectF::new(402.0, 554.0, 240.0, 16.0), "COOLDOWN ANTAR KLIP", COL_TEXT_MUTED);
    let sel_cd_idx = if cfg.voice_cooldown_seconds <= 6 {
        0
    } else if cfg.voice_cooldown_seconds <= 11 {
        1
    } else {
        2
    };
    for (i, &(_val, label)) in COOLDOWN_OPTIONS.iter().enumerate() {
        let r = cd_pill_rect(i);
        let active = i == sel_cd_idx;
        let hov = r.contains(hx, hy);
        draw_chip(rt, brush, r, 6.0, active, hov, false);
        draw_text(rt, brush, if active { &d2d.fmt_center_bold } else { &d2d.fmt_center_small }, r, label, if active { COL_TEXT } else { COL_TEXT_SUB });
    }
}

unsafe fn paint_dropdown_overlay(
    rt: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    d2d: &D2DResources,
    kind: DropdownKind,
    cfg: &crate::config::ReplayConfig,
    displays: &[crate::engine::DisplayTarget],
    mic_devices: &[crate::audio::AudioInputDevice],
    hx: f32,
    hy: f32,
) {
    let items: Vec<(String, bool)> = match kind {
        DropdownKind::Monitor => displays
            .iter()
            .enumerate()
            .map(|(i, d)| (d.display_label(), i == cfg.monitor_index))
            .collect(),
        DropdownKind::Resolution => RESOLUTIONS
            .iter()
            .map(|r| {
                let sel = if r.native {
                    cfg.use_native_resolution
                } else {
                    !cfg.use_native_resolution && cfg.width == r.w && cfg.height == r.h
                };
                (r.label.to_string(), sel)
            })
            .collect(),
        DropdownKind::Bitrate => BITRATE_OPTIONS
            .iter()
            .map(|&(b, l)| (l.to_string(), cfg.bitrate_mbps == b))
            .collect(),
        DropdownKind::MicInput => {
            let def_name = mic_devices
                .iter()
                .find(|d| d.is_default)
                .or_else(|| mic_devices.first())
                .map(|d| d.name.as_str())
                .unwrap_or("Tidak ada mic aktif");
            let mut list = vec![(
                format!("Default Sistem — {}", def_name),
                cfg.effective_mic_device_id().is_none(),
            )];
            for dev in mic_devices.iter().take(12) {
                let lbl = if dev.is_default {
                    format!("{} [Default]", dev.name)
                } else {
                    dev.name.clone()
                };
                let sel = cfg.effective_mic_device_id() == Some(dev.id.as_str());
                list.push((lbl, sel));
            }
            list
        }
    };
    if items.is_empty() {
        return;
    }

    let menu = dropdown_menu_rect(kind, items.len());
    let shadow = RectF::new(menu.l - 2.0, menu.t - 1.0, menu.r - menu.l + 4.0, menu.b - menu.t + 6.0);
    fill_rounded(rt, brush, shadow, 9.0, c_rgba(0, 0, 0, 0.72));
    draw_card(rt, brush, menu, 8.0, c_rgb(16, 16, 20), COL_BORDER_HI);

    for (i, (label, sel)) in items.iter().enumerate() {
        let rr = RectF::new(menu.l + 4.0, menu.t + 4.0 + (i as f32) * 30.0, menu.r - menu.l - 8.0, 28.0);
        let hov = rr.contains(hx, hy);
        if *sel {
            fill_rounded(rt, brush, rr, 5.0, COL_SURFACE_ACT);
        } else if hov {
            fill_rounded(rt, brush, rr, 5.0, COL_SURFACE_HOV);
        }
        draw_text(
            rt,
            brush,
            if *sel { &d2d.fmt_bold } else { &d2d.fmt_reg },
            RectF::new(rr.l + 10.0, rr.t, rr.r - rr.l - 34.0, 28.0),
            label,
            if *sel { COL_TEXT } else { COL_TEXT_SUB },
        );
        if *sel {
            draw_text(rt, brush, &d2d.fmt_center_bold, RectF::new(rr.r - 26.0, rr.t, 20.0, 28.0), "✓", COL_EMERALD);
        }
    }
}

fn slider_value_from_x(slider: RectF, x: f32, min_val: u32, max_val: u32) -> u32 {
    let ratio = ((x - slider.l) / (slider.r - slider.l)).clamp(0.0, 1.0);
    min_val + (ratio * max_val.saturating_sub(min_val) as f32).round() as u32
}

fn slider_percent_from_x(slider: RectF, x: f32) -> u32 {
    slider_value_from_x(slider, x, 0, 150)
}

enum HoverCursorKind {
    Arrow,
    Hand,
    Ibeam,
}

fn hover_cursor_kind(state: &UiState, x: f32, y: f32) -> HoverCursorKind {
    let running = state.engine.is_running() || state.engine.is_stopping();
    if let Some(kind) = state.open_dropdown {
        let cnt = match kind {
            DropdownKind::Monitor => state.engine.displays.lock().unwrap().len(),
            DropdownKind::Resolution => RESOLUTIONS.len(),
            DropdownKind::Bitrate => BITRATE_OPTIONS.len(),
            DropdownKind::MicInput => state.engine.mic_devices.lock().unwrap().len().min(12) + 1,
        };
        return if dropdown_menu_rect(kind, cnt).contains(x, y) {
            HoverCursorKind::Hand
        } else {
            HoverCursorKind::Arrow
        };
    }

    if R_BTN_TOGGLE.contains(x, y) || (running && R_BTN_SAVE.contains(x, y)) || R_BTN_FOLDER.contains(x, y) {
        return HoverCursorKind::Hand;
    }
    if R_TAB_0.contains(x, y) || R_TAB_1.contains(x, y) || R_TAB_2.contains(x, y) || R_TAB_3.contains(x, y) {
        return HoverCursorKind::Hand;
    }
    if R_BTN_GITHUB_PAGE.contains(x, y)
        || (R_BTN_UPDATE_ACTION.contains(x, y) && !state.engine.updater.snapshot().is_busy())
    {
        return HoverCursorKind::Hand;
    }
    match state.active_tab {
        0 => {
            let caps = *state.engine.codec_caps.lock().unwrap();
            if !running
                && (R_CMB_MONITOR.contains(x, y)
                    || R_STO_RAM.contains(x, y)
                    || R_STO_DISK.contains(x, y)
                    || R_CODEC_H264.contains(x, y)
                    || (caps.hevc_hw && R_CODEC_HEVC.contains(x, y))
                    || (caps.av1_hw && R_CODEC_AV1.contains(x, y))
                    || R_CMB_RES.contains(x, y)
                    || R_CMB_BITRATE.contains(x, y)
                    || (0..FPS_OPTIONS.len()).any(|i| fps_pill_rect(i).contains(x, y))
                    || (0..DURATIONS.len()).any(|i| duration_pill_rect(i).contains(x, y)))
            {
                HoverCursorKind::Hand
            } else {
                HoverCursorKind::Arrow
            }
        }
        1 => {
            if R_SLD_DESKTOP.contains(x, y)
                || R_SLD_MIC.contains(x, y)
                || R_MUTE_PILL.contains(x, y)
                || (!running
                    && (R_TOG_DESKTOP.contains(x, y)
                        || R_TOG_MIC.contains(x, y)
                        || R_CMB_MIC.contains(x, y)))
            {
                HoverCursorKind::Hand
            } else {
                HoverCursorKind::Arrow
            }
        }
        2 => {
            let is_custom_temp = {
                let cfg = state.engine.config.lock().unwrap();
                cfg.temp_folder.trim() != crate::config::ReplayConfig::default_temp_folder()
            };
            if (0..HotkeySlot::ALL.len()).any(|i| hk_slot_rect(i).contains(x, y))
                || R_HK_PRESET_0.contains(x, y)
                || R_HK_PRESET_1.contains(x, y)
                || R_HK_PRESET_2.contains(x, y)
                || R_BTN_BROWSE.contains(x, y)
                || (!running && R_BTN_TEMP_BROWSE.contains(x, y))
                || (!running && is_custom_temp && R_BTN_TEMP_RESET.contains(x, y))
                || R_TOG_TRAY.contains(x, y)
                || R_TOG_BEEP.contains(x, y)
                || R_TOG_FOCUS.contains(x, y)
                || R_TOG_AUTO_UPDATE.contains(x, y)
            {
                HoverCursorKind::Hand
            } else {
                HoverCursorKind::Arrow
            }
        }
        _ => {
            if R_TXT_PHRASE.contains(x, y) {
                HoverCursorKind::Ibeam
            } else if R_TOG_VOICE.contains(x, y)
                || R_BTN_SAVE_PHRASE.contains(x, y)
                || R_BTN_ENROLL.contains(x, y)
                || R_BTN_RESET_VOICE.contains(x, y)
                || R_BTN_SENS_DEC.contains(x, y)
                || R_SLD_VOICE_SENS.contains(x, y)
                || R_BTN_SENS_INC.contains(x, y)
                || (0..PHRASE_PRESETS.len()).any(|i| phrase_preset_rect(i).contains(x, y))
                || (0..COOLDOWN_OPTIONS.len()).any(|i| cd_pill_rect(i).contains(x, y))
            {
                HoverCursorKind::Hand
            } else {
                HoverCursorKind::Arrow
            }
        }
    }
}

fn commit_phrase(state: &mut UiState, new_phrase: &str) {
    let trimmed = new_phrase.trim();
    let final_phrase = if trimmed.is_empty() {
        "Roman, clip that".to_string()
    } else {
        trimmed.to_string()
    };
    state.phrase_draft = final_phrase.clone();
    {
        let mut cfg = state.engine.config.lock().unwrap();
        cfg.voice_phrase = final_phrase.clone();
        cfg.save();
    }
    state.engine.voice.set_phrase_label(&final_phrase);
    state.editing_phrase = false;
}

unsafe fn sync_timer_rate(state: &mut UiState) {
    let visible = IsWindowVisible(state.hwnd).as_bool();
    let voice_cfg_on = state.engine.config.lock().unwrap().voice_trigger_enabled;
    let enrolling = state.engine.voice.is_enrolling();
    let want_fast = visible && state.active_tab == 3 && (enrolling || voice_cfg_on);
    if want_fast != state.fast_timer_active {
        state.fast_timer_active = want_fast;
        let _ = SetTimer(state.hwnd, TIMER_UPDATE_ID, if want_fast { 100 } else { 250 }, None);
    }
}

unsafe fn toggle_replay(state: &mut UiState) {
    if state.engine.is_stopping() {
        return;
    }
    if state.engine.is_running() {
        state.engine.stop_async();
    } else {
        state.engine.start();
    }
    let _ = InvalidateRect(state.hwnd, None, false);
}

unsafe fn handle_click(state: &mut UiState, x: f32, y: f32) {
    let can_save = state.engine.is_running();
    let running = can_save || state.engine.is_stopping();

    // Commit phrase draft if user clicks outside textbox & Save button while typing
    if state.editing_phrase
        && !R_TXT_PHRASE.contains(x, y)
        && !R_BTN_SAVE_PHRASE.contains(x, y)
    {
        let draft = state.phrase_draft.clone();
        commit_phrase(state, &draft);
    }

    // 1. Open dropdown menu
    if let Some(kind) = state.open_dropdown {
        let count = match kind {
            DropdownKind::Monitor => state.engine.displays.lock().unwrap().len(),
            DropdownKind::Resolution => RESOLUTIONS.len(),
            DropdownKind::Bitrate => BITRATE_OPTIONS.len(),
            DropdownKind::MicInput => state.engine.mic_devices.lock().unwrap().len().min(12) + 1,
        };
        let menu = dropdown_menu_rect(kind, count);
        if menu.contains(x, y) {
            let idx = ((y - menu.t - 4.0) / 30.0).floor() as isize;
            if idx >= 0 && (idx as usize) < count {
                let ui = idx as usize;
                let mics_snap = if kind == DropdownKind::MicInput {
                    Some(state.engine.mic_devices.lock().unwrap().clone())
                } else {
                    None
                };
                let mut cfg = state.engine.config.lock().unwrap();
                match kind {
                    DropdownKind::Monitor => cfg.monitor_index = ui,
                    DropdownKind::Resolution => {
                        let opt = RESOLUTIONS[ui];
                        cfg.use_native_resolution = opt.native;
                        if !opt.native {
                            cfg.width = opt.w;
                            cfg.height = opt.h;
                        }
                    }
                    DropdownKind::Bitrate => cfg.bitrate_mbps = BITRATE_OPTIONS[ui].0,
                    DropdownKind::MicInput => {
                        if ui == 0 {
                            cfg.mic_device_id.clear();
                        } else if let Some(dev) = mics_snap.as_ref().and_then(|v| v.get(ui - 1)) {
                            cfg.mic_device_id = dev.id.clone();
                        }
                    }
                }
                cfg.save();
            }
        }
        state.open_dropdown = None;
        let _ = InvalidateRect(state.hwnd, None, false);
        return;
    }

    // 2. Hero action buttons
    if R_BTN_TOGGLE.contains(x, y) {
        toggle_replay(state);
        return;
    }
    if can_save && R_BTN_SAVE.contains(x, y) {
        state.engine.save_replay_async();
        let _ = InvalidateRect(state.hwnd, None, false);
        return;
    }
    if R_BTN_FOLDER.contains(x, y) {
        let dir = state.engine.config.lock().unwrap().output_folder.clone();
        open_folder_in_explorer(&dir);
        return;
    }

    // 2b. Global GitHub Releases update bar buttons
    if R_BTN_GITHUB_PAGE.contains(x, y) {
        cancel_hotkey_capture(state);
        let up = state.engine.updater.snapshot();
        open_url_in_browser(up.release_page_url());
        return;
    }
    if R_BTN_UPDATE_ACTION.contains(x, y) {
        cancel_hotkey_capture(state);
        let up = state.engine.updater.snapshot();
        match up {
            UpdateState::Available { exe_url: Some(_), .. } => {
                state.engine.updater.install_available_update_async(Arc::clone(&state.engine));
            }
            UpdateState::Available { html_url, .. } => {
                open_url_in_browser(&html_url);
            }
            UpdateState::Idle | UpdateState::UpToDate { .. } | UpdateState::Error(_) => {
                state.engine.updater.check_for_updates_async(true);
            }
            _ => {}
        }
        let _ = InvalidateRect(state.hwnd, None, false);
        return;
    }

    // 3. Tabs
    for (idx, rect) in [(0, R_TAB_0), (1, R_TAB_1), (2, R_TAB_2), (3, R_TAB_3)] {
        if rect.contains(x, y) {
            if state.active_tab != idx {
                cancel_hotkey_capture(state);
            }
            state.active_tab = idx;
            sync_timer_rate(state);
            let _ = InvalidateRect(state.hwnd, None, false);
            return;
        }
    }

    // 4. Tab Content
    match state.active_tab {
        0 => {
            if running {
                return;
            }
            if R_CMB_MONITOR.contains(x, y) {
                state.open_dropdown = Some(DropdownKind::Monitor);
            } else if R_CMB_RES.contains(x, y) {
                state.open_dropdown = Some(DropdownKind::Resolution);
            } else if R_CMB_BITRATE.contains(x, y) {
                state.open_dropdown = Some(DropdownKind::Bitrate);
            } else if R_STO_RAM.contains(x, y) {
                let mut cfg = state.engine.config.lock().unwrap();
                cfg.storage_mode = BufferStorageMode::RamRingBuffer;
                cfg.save();
            } else if R_STO_DISK.contains(x, y) {
                let mut cfg = state.engine.config.lock().unwrap();
                cfg.storage_mode = BufferStorageMode::DiskTempBuffer;
                cfg.save();
            } else if R_CODEC_H264.contains(x, y) {
                let mut cfg = state.engine.config.lock().unwrap();
                cfg.codec = VideoCodecType::HardwareH264;
                cfg.save();
            } else if R_CODEC_HEVC.contains(x, y) {
                if state.engine.codec_caps.lock().unwrap().hevc_hw {
                    let mut cfg = state.engine.config.lock().unwrap();
                    cfg.codec = VideoCodecType::HardwareHEVC;
                    cfg.save();
                }
            } else if R_CODEC_AV1.contains(x, y) {
                if state.engine.codec_caps.lock().unwrap().av1_hw {
                    let mut cfg = state.engine.config.lock().unwrap();
                    cfg.codec = VideoCodecType::HardwareAV1;
                    cfg.save();
                }
            } else {
                for (i, &(fps_v, _)) in FPS_OPTIONS.iter().enumerate() {
                    if fps_pill_rect(i).contains(x, y) {
                        let mut cfg = state.engine.config.lock().unwrap();
                        cfg.fps = fps_v;
                        cfg.save();
                        break;
                    }
                }
                for (i, &(dur_v, _)) in DURATIONS.iter().enumerate() {
                    if duration_pill_rect(i).contains(x, y) {
                        let mut cfg = state.engine.config.lock().unwrap();
                        cfg.replay_duration_seconds = dur_v;
                        cfg.save();
                        break;
                    }
                }
            }
            let _ = InvalidateRect(state.hwnd, None, false);
        }
        1 => {
            if !running && R_TOG_DESKTOP.contains(x, y) {
                let mut cfg = state.engine.config.lock().unwrap();
                cfg.record_desktop_audio = !cfg.record_desktop_audio;
                cfg.save();
            } else if !running && R_TOG_MIC.contains(x, y) {
                let mut cfg = state.engine.config.lock().unwrap();
                cfg.record_microphone = !cfg.record_microphone;
                cfg.save();
            } else if !running && R_CMB_MIC.contains(x, y) {
                state.engine.refresh_audio_inputs();
                state.open_dropdown = Some(DropdownKind::MicInput);
            } else if R_MUTE_PILL.contains(x, y) {
                let mut cfg = state.engine.config.lock().unwrap();
                cfg.mic_muted = !cfg.mic_muted;
                cfg.save();
            } else if R_SLD_DESKTOP.contains(x, y) {
                state.dragging_slider = DragSlider::Desktop;
                SetCapture(state.hwnd);
                let mut cfg = state.engine.config.lock().unwrap();
                cfg.desktop_volume_percent = slider_percent_from_x(R_SLD_DESKTOP, x);
            } else if R_SLD_MIC.contains(x, y) {
                state.dragging_slider = DragSlider::Mic;
                SetCapture(state.hwnd);
                let mut cfg = state.engine.config.lock().unwrap();
                cfg.mic_volume_percent = slider_percent_from_x(R_SLD_MIC, x);
            }
            let _ = InvalidateRect(state.hwnd, None, false);
        }
        2 => {
            // Check [✕] clear buttons before slot cards because [✕] sits inside each slot bounds.
            for (idx, slot) in HotkeySlot::ALL.iter().copied().enumerate() {
                if hk_slot_rect(idx).contains(x, y) {
                    let enabled = {
                        let cfg = state.engine.config.lock().unwrap();
                        match slot {
                            HotkeySlot::Toggle => cfg.hotkey_toggle.is_enabled(),
                            HotkeySlot::Save => cfg.hotkey_save.is_enabled(),
                            HotkeySlot::Mute => cfg.hotkey_mute.is_enabled(),
                        }
                    };
                    if enabled && state.capturing_hotkey != Some(slot) && hk_clear_rect(idx).contains(x, y) {
                        {
                            let mut cfg = state.engine.config.lock().unwrap();
                            match slot {
                                HotkeySlot::Toggle => cfg.hotkey_toggle = HotkeyBinding::disabled(),
                                HotkeySlot::Save => cfg.hotkey_save = HotkeyBinding::disabled(),
                                HotkeySlot::Mute => cfg.hotkey_mute = HotkeyBinding::disabled(),
                            }
                            cfg.save();
                        }
                        state.capturing_hotkey = None;
                        register_hotkeys(state);
                        let _ = InvalidateRect(state.hwnd, None, false);
                        return;
                    }
                    if state.capturing_hotkey == Some(slot) {
                        cancel_hotkey_capture(state);
                    } else {
                        begin_hotkey_capture(state, slot);
                    }
                    return;
                }
            }

            let preset_hit = if R_HK_PRESET_0.contains(x, y) {
                Some(HotkeyPresetType::CtrlShiftF9F10)
            } else if R_HK_PRESET_1.contains(x, y) {
                Some(HotkeyPresetType::FunctionKeysF8F7)
            } else if R_HK_PRESET_2.contains(x, y) {
                Some(HotkeyPresetType::AltF9F10)
            } else {
                None
            };
            if let Some(preset) = preset_hit {
                state.capturing_hotkey = None;
                {
                    let mut cfg = state.engine.config.lock().unwrap();
                    cfg.apply_hotkey_preset(preset);
                    cfg.save();
                }
                register_hotkeys(state);
                let _ = InvalidateRect(state.hwnd, None, false);
                return;
            }

            cancel_hotkey_capture(state);
            if R_BTN_BROWSE.contains(x, y) {
                if let Some(chosen) = pick_folder(state.hwnd, "Pilih Folder Penyimpanan Replay") {
                    let mut cfg = state.engine.config.lock().unwrap();
                    cfg.output_folder = chosen;
                    cfg.save();
                }
            } else if !running && R_BTN_TEMP_BROWSE.contains(x, y) {
                if let Some(chosen) =
                    pick_folder(state.hwnd, "Pilih Folder Temp Buffer Replay (Disarankan SSD)")
                {
                    let mut cfg = state.engine.config.lock().unwrap();
                    cfg.temp_folder = chosen;
                    cfg.save();
                }
            } else if !running && R_BTN_TEMP_RESET.contains(x, y) {
                let def = crate::config::ReplayConfig::default_temp_folder();
                let mut cfg = state.engine.config.lock().unwrap();
                if cfg.temp_folder != def {
                    cfg.temp_folder = def;
                    cfg.save();
                }
            } else if R_TOG_TRAY.contains(x, y) {
                let mut cfg = state.engine.config.lock().unwrap();
                cfg.minimize_to_tray_on_close = !cfg.minimize_to_tray_on_close;
                cfg.save();
            } else if R_TOG_BEEP.contains(x, y) {
                let mut cfg = state.engine.config.lock().unwrap();
                cfg.play_sound_on_save = !cfg.play_sound_on_save;
                cfg.save();
            } else if R_TOG_FOCUS.contains(x, y) {
                let mut cfg = state.engine.config.lock().unwrap();
                cfg.suppress_notifications_in_game = !cfg.suppress_notifications_in_game;
                cfg.save();
            } else if R_TOG_AUTO_UPDATE.contains(x, y) {
                let mut cfg = state.engine.config.lock().unwrap();
                cfg.auto_check_updates = !cfg.auto_check_updates;
                cfg.save();
            }
            let _ = InvalidateRect(state.hwnd, None, false);
        }
        _ => {
            if R_TOG_VOICE.contains(x, y) {
                {
                    let mut cfg = state.engine.config.lock().unwrap();
                    cfg.voice_trigger_enabled = !cfg.voice_trigger_enabled;
                    cfg.save();
                }
                sync_timer_rate(state);
            } else if R_TXT_PHRASE.contains(x, y) {
                state.editing_phrase = true;
                state.caret_tick = 0;
            } else if R_BTN_SAVE_PHRASE.contains(x, y) {
                let dirty = state.editing_phrase
                    || state.phrase_draft.trim() != state.engine.config.lock().unwrap().voice_phrase;
                if dirty {
                    let draft = state.phrase_draft.clone();
                    commit_phrase(state, &draft);
                } else {
                    state.editing_phrase = true;
                    state.caret_tick = 0;
                }
            } else if R_BTN_ENROLL.contains(x, y) {
                let draft = state.phrase_draft.clone();
                commit_phrase(state, &draft);
                let (phrase, mic_id) = {
                    let cfg = state.engine.config.lock().unwrap();
                    (
                        cfg.voice_phrase.clone(),
                        cfg.effective_mic_device_id().map(str::to_string),
                    )
                };
                state
                    .engine
                    .voice
                    .begin_enrollment(running, &phrase, mic_id.as_deref());
                sync_timer_rate(state);
            } else if R_BTN_RESET_VOICE.contains(x, y) {
                state.engine.voice.clear_templates();
            } else if R_BTN_SENS_DEC.contains(x, y) {
                let mut cfg = state.engine.config.lock().unwrap();
                cfg.voice_sensitivity = cfg
                    .voice_sensitivity
                    .saturating_sub(1)
                    .clamp(SENS_MIN_PERCENT, SENS_MAX_PERCENT);
                cfg.save();
            } else if R_BTN_SENS_INC.contains(x, y) {
                let mut cfg = state.engine.config.lock().unwrap();
                cfg.voice_sensitivity = (cfg.voice_sensitivity + 1).clamp(SENS_MIN_PERCENT, SENS_MAX_PERCENT);
                cfg.save();
            } else if R_SLD_VOICE_SENS.contains(x, y) {
                state.dragging_slider = DragSlider::VoiceSensitivity;
                SetCapture(state.hwnd);
                let mut cfg = state.engine.config.lock().unwrap();
                cfg.voice_sensitivity =
                    slider_value_from_x(R_SLD_VOICE_SENS, x, SENS_MIN_PERCENT, SENS_MAX_PERCENT);
            } else {
                for (i, &preset) in PHRASE_PRESETS.iter().enumerate() {
                    if phrase_preset_rect(i).contains(x, y) {
                        commit_phrase(state, preset);
                        break;
                    }
                }
                for (i, &(cd_val, _)) in COOLDOWN_OPTIONS.iter().enumerate() {
                    if cd_pill_rect(i).contains(x, y) {
                        let mut cfg = state.engine.config.lock().unwrap();
                        cfg.voice_cooldown_seconds = cd_val;
                        cfg.save();
                        break;
                    }
                }
            }
            let _ = InvalidateRect(state.hwnd, None, false);
        }
    }
}

fn mouse_xy(lparam: LPARAM, dpi_scale: f32) -> (f32, f32) {
    let raw_x = (lparam.0 & 0xFFFF) as i16 as f32;
    let raw_y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
    (raw_x / dpi_scale, raw_y / dpi_scale)
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let ptr = GetWindowLongPtrW(hwnd, GWL_USERDATA) as *mut UiState;

    match msg {
        WM_CREATE => {
            if let Some(&engine_ptr) = UI_PTR.get() {
                let engine = (*(engine_ptr as *const Arc<ReplayEngine>)).clone();
                engine.set_ui_hwnd(hwnd);

                let hdc = GetDC(None);
                let dpi_x = GetDeviceCaps(hdc, LOGPIXELSX);
                let _ = ReleaseDC(None, hdc);
                let dpi_scale = if dpi_x > 0 { dpi_x as f32 / 96.0 } else { 1.0 };

                let initial_phrase = engine.config.lock().unwrap().voice_phrase.clone();
                let app_icon = create_app_icon();
                let d2d = init_d2d_resources();
                let mut state = Box::new(UiState {
                    engine,
                    hwnd,
                    dpi_scale,
                    app_icon,
                    d2d,
                    active_tab: 0,
                    hover_x: -100.0,
                    hover_y: -100.0,
                    mouse_inside: false,
                    mouse_down: false,
                    dragging_slider: DragSlider::None,
                    open_dropdown: None,
                    phrase_draft: initial_phrase,
                    editing_phrase: false,
                    capturing_hotkey: None,
                    caret_tick: 0,
                    fast_timer_active: false,
                    hotkey_warning: String::new(),
                    tray_added: false,
                    last_process_ram_mb: query_process_ram_mb(),
                });

                apply_dark_window_frame(hwnd);
                SendMessageW(hwnd, 0x0080, WPARAM(1), LPARAM(state.app_icon.0 as isize));
                SendMessageW(hwnd, 0x0080, WPARAM(0), LPARAM(state.app_icon.0 as isize));

                register_hotkeys(&mut state);
                ensure_tray_icon(&mut state);
                let auto_upd = state.engine.config.lock().unwrap().auto_check_updates;
                if auto_upd {
                    state.engine.updater.check_for_updates_async(false);
                }
                let _ = SetTimer(hwnd, TIMER_UPDATE_ID, 250, None);
                SetWindowLongPtrW(hwnd, GWL_USERDATA, Box::into_raw(state) as isize);
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_SIZE => LRESULT(0),
        WM_PAINT => {
            if !ptr.is_null() {
                let st = &mut *ptr;
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                paint_ui_to_hdc(st, hdc);
                let _ = EndPaint(hwnd, &ps);
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_PRINTCLIENT => {
            if !ptr.is_null() && wparam.0 != 0 {
                let st = &mut *ptr;
                paint_ui_to_hdc(st, HDC(wparam.0 as *mut c_void));
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_SETCURSOR => {
            if !ptr.is_null() && (lparam.0 & 0xFFFF) as u32 == 1 {
                let st = &*ptr;
                let cid = match hover_cursor_kind(st, st.hover_x, st.hover_y) {
                    HoverCursorKind::Hand => IDC_HAND,
                    HoverCursorKind::Ibeam => IDC_IBEAM,
                    HoverCursorKind::Arrow => IDC_ARROW,
                };
                if let Ok(cur) = LoadCursorW(None, cid) {
                    SetCursor(cur);
                    return LRESULT(1);
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_MOUSEMOVE => {
            if !ptr.is_null() {
                let st = &mut *ptr;
                if !st.mouse_inside {
                    st.mouse_inside = true;
                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    let _ = TrackMouseEvent(&mut tme);
                }
                let (x, y) = mouse_xy(lparam, st.dpi_scale);
                st.hover_x = x;
                st.hover_y = y;

                match st.dragging_slider {
                    DragSlider::Desktop => {
                        let mut cfg = st.engine.config.lock().unwrap();
                        cfg.desktop_volume_percent = slider_percent_from_x(R_SLD_DESKTOP, x);
                    }
                    DragSlider::Mic => {
                        let mut cfg = st.engine.config.lock().unwrap();
                        cfg.mic_volume_percent = slider_percent_from_x(R_SLD_MIC, x);
                    }
                    DragSlider::VoiceSensitivity => {
                        let mut cfg = st.engine.config.lock().unwrap();
                        cfg.voice_sensitivity = slider_value_from_x(
                            R_SLD_VOICE_SENS,
                            x,
                            SENS_MIN_PERCENT,
                            SENS_MAX_PERCENT,
                        );
                    }
                    DragSlider::None => {}
                }
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            if !ptr.is_null() {
                let st = &mut *ptr;
                st.mouse_inside = false;
                st.hover_x = -100.0;
                st.hover_y = -100.0;
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            if !ptr.is_null() {
                let st = &mut *ptr;
                st.mouse_down = true;
                let (x, y) = mouse_xy(lparam, st.dpi_scale);
                st.hover_x = x;
                st.hover_y = y;
                handle_click(st, x, y);
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            if !ptr.is_null() {
                let st = &mut *ptr;
                st.mouse_down = false;
                if st.dragging_slider != DragSlider::None {
                    st.dragging_slider = DragSlider::None;
                    let _ = ReleaseCapture();
                    st.engine.config.lock().unwrap().save();
                    let _ = InvalidateRect(hwnd, None, false);
                }
            }
            LRESULT(0)
        }
        WM_CHAR => {
            if !ptr.is_null() {
                let st = &mut *ptr;
                if st.capturing_hotkey.is_some() {
                    return LRESULT(0);
                }
                if st.active_tab == 3 && st.editing_phrase {
                    if let Some(ch) = char::from_u32(wparam.0 as u32) {
                        match ch {
                            '\x08' => {
                                st.phrase_draft.pop();
                            }
                            '\r' | '\n' => {
                                let draft = st.phrase_draft.clone();
                                commit_phrase(st, &draft);
                            }
                            '\x1b' => {
                                st.phrase_draft = st.engine.config.lock().unwrap().voice_phrase.clone();
                                st.editing_phrase = false;
                            }
                            c if !c.is_control() && st.phrase_draft.chars().count() < 42 => {
                                st.phrase_draft.push(c);
                            }
                            _ => {}
                        }
                        let _ = InvalidateRect(hwnd, None, false);
                        return LRESULT(0);
                    }
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            if !ptr.is_null() {
                let st = &mut *ptr;
                if st.capturing_hotkey.is_some() {
                    let vk = wparam.0 as u32;
                    if vk == VK_ESCAPE.0 as u32 {
                        cancel_hotkey_capture(st);
                    } else if is_modifier_or_toggle_vk(vk) {
                        let _ = InvalidateRect(hwnd, None, false);
                    } else {
                        let mods = current_async_hotkey_mods();
                        apply_captured_hotkey(st, vk, mods);
                    }
                    return LRESULT(0);
                }
                if msg == WM_SYSKEYDOWN {
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }
                if wparam.0 == 0x1B {
                    if st.open_dropdown.is_some() {
                        st.open_dropdown = None;
                        let _ = InvalidateRect(hwnd, None, false);
                    } else if st.editing_phrase {
                        st.phrase_draft = st.engine.config.lock().unwrap().voice_phrase.clone();
                        st.editing_phrase = false;
                        let _ = InvalidateRect(hwnd, None, false);
                    }
                }
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_KEYUP | WM_SYSKEYUP => {
            if !ptr.is_null() {
                let st = &mut *ptr;
                if st.capturing_hotkey.is_some() {
                    let vk = wparam.0 as u32;
                    if vk == VK_SNAPSHOT.0 as u32 {
                        let mods = current_async_hotkey_mods();
                        apply_captured_hotkey(st, vk, mods);
                    } else if is_modifier_or_toggle_vk(vk) {
                        let _ = InvalidateRect(hwnd, None, false);
                    }
                    return LRESULT(0);
                }
            }
            if msg == WM_SYSKEYUP {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            } else {
                LRESULT(0)
            }
        }
        WM_SYSCHAR => {
            if !ptr.is_null() && (*ptr).capturing_hotkey.is_some() {
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_KILLFOCUS => {
            if !ptr.is_null() {
                let st = &mut *ptr;
                cancel_hotkey_capture(st);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_COMMAND => {
            if !ptr.is_null() {
                let st = &mut *ptr;
                let id = (wparam.0 & 0xFFFF) as usize;
                match id {
                    ID_TRAY_OPEN => {
                        let _ = ShowWindow(hwnd, SW_RESTORE);
                        let _ = SetForegroundWindow(hwnd);
                        sync_timer_rate(st);
                    }
                    ID_TRAY_TOGGLE => toggle_replay(st),
                    ID_TRAY_SAVE => st.engine.save_replay_async(),
                    ID_TRAY_FOLDER => {
                        let dir = st.engine.config.lock().unwrap().output_folder.clone();
                        open_folder_in_explorer(&dir);
                    }
                    ID_TRAY_UPDATE => {
                        let _ = ShowWindow(hwnd, SW_RESTORE);
                        let _ = SetForegroundWindow(hwnd);
                        sync_timer_rate(st);
                        let snap = st.engine.updater.snapshot();
                        if !matches!(snap, UpdateState::Available { .. }) && !snap.is_busy() {
                            st.engine.updater.check_for_updates_async(true);
                        }
                        let _ = InvalidateRect(hwnd, None, false);
                    }
                    ID_TRAY_EXIT => {
                        let _ = ShowWindow(hwnd, SW_HIDE);
                        remove_tray_icon(st);
                        let _ = DestroyWindow(hwnd);
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        WM_HOTKEY => {
            if !ptr.is_null() {
                let st = &mut *ptr;
                match wparam.0 as i32 {
                    HOTKEY_TOGGLE_ID => {
                        toggle_replay(st);
                        let active = st.engine.is_running();
                        show_tray_balloon(
                            st,
                            "NVLiteReplay",
                            if active { "Instant Replay NYALA!" } else { "Instant Replay MATI." },
                        );
                    }
                    HOTKEY_SAVE_ID => st.engine.save_replay_async(),
                    HOTKEY_MUTE_ID => {
                        let muted = {
                            let mut cfg = st.engine.config.lock().unwrap();
                            cfg.mic_muted = !cfg.mic_muted;
                            cfg.save();
                            cfg.mic_muted
                        };
                        show_tray_balloon(st, "Mikrofon", if muted { "Mic DIMUTE" } else { "Mic AKTIF" });
                        let _ = InvalidateRect(hwnd, None, false);
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        WM_APP_STATUS => {
            let _ = InvalidateRect(hwnd, None, false);
            LRESULT(0)
        }
        WM_APP_UPDATE => {
            if !ptr.is_null() {
                let st = &mut *ptr;
                if wparam.0 == 1 {
                    cancel_hotkey_capture(st);
                    let _ = ShowWindow(hwnd, SW_HIDE);
                    remove_tray_icon(st);
                    let _ = DestroyWindow(hwnd);
                    return LRESULT(0);
                }
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_APP_VOICE_TRIGGER => {
            if !ptr.is_null() {
                let st = &*ptr;
                let snap = st.engine.voice.ui_snapshot();
                show_tray_balloon(
                    st,
                    "Voice Clip Terdeteksi!",
                    &format!("Frasa \"{}\" ({:.0}%) — Menyimpan klip...", snap.phrase_label, snap.last_confidence),
                );
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_APP_SAVED => {
            if !ptr.is_null() {
                let st = &*ptr;
                let file = st.engine.last_saved_file.lock().unwrap().clone();
                if !file.is_empty() {
                    let name = Path::new(&file).file_name().and_then(|s| s.to_str()).unwrap_or("Replay.mp4");
                    show_tray_balloon(st, "Instant Replay Tersimpan!", &format!("File: {}", name));
                }
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_APP_TRAY => {
            if !ptr.is_null() {
                let st = &mut *ptr;
                let ev = lparam.0 as u32;
                if ev == WM_LBUTTONDBLCLK {
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                    let _ = SetForegroundWindow(hwnd);
                    sync_timer_rate(st);
                } else if ev == WM_RBUTTONUP {
                    show_tray_menu(st);
                }
            }
            LRESULT(0)
        }
        WM_TIMER => {
            if !ptr.is_null() && wparam.0 == TIMER_UPDATE_ID {
                let st = &mut *ptr;
                st.caret_tick = st.caret_tick.wrapping_add(1);
                if !st.fast_timer_active || (st.caret_tick % 4 == 0) {
                    st.last_process_ram_mb = query_process_ram_mb();
                }
                sync_timer_rate(st);
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            if !ptr.is_null() {
                let st = &mut *ptr;
                cancel_hotkey_capture(st);
                let min_tray = st.engine.config.lock().unwrap().minimize_to_tray_on_close;
                if min_tray {
                    release_backbuffer(&mut st.d2d);
                    let _ = ShowWindow(hwnd, SW_HIDE);
                    sync_timer_rate(st);
                    show_tray_balloon(st, "NVLiteReplay", "Berjalan hemat resource di System Tray.");
                    return LRESULT(0);
                }
                let _ = ShowWindow(hwnd, SW_HIDE);
                remove_tray_icon(st);
            }
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            let _ = KillTimer(hwnd, TIMER_UPDATE_ID);
            unregister_hotkeys(hwnd);
            if !ptr.is_null() {
                let mut boxed = Box::from_raw(ptr);
                boxed.engine.stop();
                remove_tray_icon(&mut boxed);
                release_backbuffer(&mut boxed.d2d);
                let _ = DestroyIcon(boxed.app_icon);
                SetWindowLongPtrW(hwnd, GWL_USERDATA, 0);
            }
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

pub fn run(engine: Arc<ReplayEngine>) {
    unsafe {
        let hinst: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
        let class_name = wide_null("NVLiteReplayRustVoiceRecogMainWnd");
        let title = wide_null("NVLiteReplay — Instant Replay");

        let boxed_arc = Box::new(engine);
        let raw_arc = Box::into_raw(boxed_arc) as usize;
        let _ = UI_PTR.set(raw_arc);

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinst,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        RegisterClassExW(&wc);

        let hdc = GetDC(None);
        let dpi_x = GetDeviceCaps(hdc, LOGPIXELSX);
        let _ = ReleaseDC(None, hdc);
        let dpi_scale = if dpi_x > 0 { dpi_x as f32 / 96.0 } else { 1.0 };

        let client_w = (LOGICAL_W * dpi_scale).round() as i32;
        let client_h = (LOGICAL_H * dpi_scale).round() as i32;
        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN;
        let mut adj = RECT {
            left: 0,
            top: 0,
            right: client_w,
            bottom: client_h,
        };
        let _ = AdjustWindowRectEx(&mut adj, style, false, WS_EX_APPWINDOW);
        let win_w = adj.right - adj.left;
        let win_h = adj.bottom - adj.top;
        let scr_w = GetSystemMetrics(SM_CXSCREEN);
        let scr_h = GetSystemMetrics(SM_CYSCREEN);
        let pos_x = ((scr_w - win_w) / 2).max(20);
        let pos_y = ((scr_h - win_h) / 2).max(20);

        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            style,
            pos_x,
            pos_y,
            win_w,
            win_h,
            None,
            None,
            hinst,
            None,
        )
        .unwrap_or(HWND::default());

        let _ = ShowWindow(hwnd, SW_SHOW);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
