use std::fs;
use std::mem::ManuallyDrop;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use windows::core::{Interface, GUID, HSTRING, PCWSTR, PWSTR, VARIANT};
use windows::Graphics::Capture::{
    Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::{
    CloseHandle, BOOL, HANDLE, HMODULE, HWND, LPARAM, POINT, RECT, STILL_ACTIVE, WPARAM,
};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Multithread,
    ID3D11RenderTargetView, ID3D11Texture2D, ID3D11VideoContext, ID3D11VideoContext1,
    ID3D11VideoDevice, ID3D11VideoProcessor, ID3D11VideoProcessorEnumerator,
    ID3D11VideoProcessorInputView, ID3D11VideoProcessorOutputView, D3D11_BIND_RENDER_TARGET,
    D3D11_BIND_SHADER_RESOURCE, D3D11_BOX, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
    D3D11_TEX2D_VPIV, D3D11_TEX2D_VPOV, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
    D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE, D3D11_VIDEO_PROCESSOR_COLOR_SPACE,
    D3D11_VIDEO_PROCESSOR_CONTENT_DESC, D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC,
    D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0, D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC,
    D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0, D3D11_VIDEO_PROCESSOR_STREAM,
    D3D11_VIDEO_USAGE_OPTIMAL_SPEED, D3D11_VPIV_DIMENSION_TEXTURE2D,
    D3D11_VPOV_DIMENSION_TEXTURE2D,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709, DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709,
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_NV12, DXGI_RATIONAL, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter1, IDXGIDevice, IDXGIFactory1, IDXGIOutput, IDXGIOutput1,
    IDXGIOutput5, IDXGIOutputDuplication, DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT,
    DXGI_OUTDUPL_FRAME_INFO,
};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, EnumDisplayMonitors,
    EnumDisplaySettingsW, GetDC, GetMonitorInfoW, MonitorFromPoint, ReleaseDC, SelectObject,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DEVMODEW, DIB_RGB_COLORS, ENUM_CURRENT_SETTINGS,
    HBITMAP, HDC, HGDIOBJ, HMONITOR, MONITORINFOEXW, MONITOR_DEFAULTTONEAREST, SRCCOPY,
};
use windows::Win32::Media::MediaFoundation::{
    eAVEncCommonRateControlMode_CBR, eAVEncCommonRateControlMode_PeakConstrainedVBR,
    eAVEncH264VProfile_High,
    eAVEncH265VProfile_Main_420_8, ICodecAPI, IMFActivate, IMFAttributes, IMFByteStream,
    IMFDXGIDeviceManager, IMFMediaBuffer, IMFMediaType, IMFSample, IMFSinkWriter, IMFSourceReader,
    MFAudioFormat_AAC, MFAudioFormat_PCM, MFCreateAttributes, MFCreateDXGIDeviceManager,
    MFCreateDXGISurfaceBuffer, MFCreateMFByteStreamOnStream, MFCreateMediaType,
    MFCreateMemoryBuffer, MFCreateSample, MFCreateSinkWriterFromURL,
    MFCreateSourceReaderFromByteStream, MFCreateSourceReaderFromURL, MFMediaType_Audio,
    MFMediaType_Video, MFNominalRange_0_255, MFNominalRange_16_235, MFShutdown, MFStartup,
    MFTEnumEx, MFVideoFormat_ARGB32, MFVideoFormat_AV1, MFVideoFormat_H264, MFVideoFormat_HEVC,
    MFVideoFormat_NV12, MFVideoPrimaries_BT709, MFVideoTransFunc_709, MFVideoTransferMatrix_BT709,
    CODECAPI_AVEncCommonBufferSize, CODECAPI_AVEncCommonMaxBitRate,
    CODECAPI_AVEncCommonMeanBitRate, CODECAPI_AVEncCommonQualityVsSpeed,
    CODECAPI_AVEncCommonRateControlMode, CODECAPI_AVEncH264CABACEnable, CODECAPI_AVEncMPVGOPSize,
    MFT_CATEGORY_VIDEO_ENCODER, MFT_ENUM_FLAG_HARDWARE, MFT_ENUM_FLAG_SORTANDFILTER,
    MFT_REGISTER_TYPE_INFO, MF_LOW_LATENCY, MF_MT_AUDIO_AVG_BYTES_PER_SECOND,
    MF_MT_AUDIO_BITS_PER_SAMPLE, MF_MT_AUDIO_BLOCK_ALIGNMENT, MF_MT_AUDIO_NUM_CHANNELS,
    MF_MT_AUDIO_SAMPLES_PER_SECOND, MF_MT_AVG_BITRATE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE,
    MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_MPEG2_PROFILE, MF_MT_MPEG_SEQUENCE_HEADER,
    MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SUBTYPE, MF_MT_TRANSFER_FUNCTION, MF_MT_VIDEO_NOMINAL_RANGE,
    MF_MT_VIDEO_PRIMARIES, MF_MT_YUV_MATRIX, MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS,
    MF_SINK_WRITER_D3D_MANAGER, MF_SINK_WRITER_DISABLE_THROTTLING, MF_SOURCE_READERF_ENDOFSTREAM,
    MF_SOURCE_READERF_ERROR, MF_SOURCE_READER_FIRST_AUDIO_STREAM,
    MF_SOURCE_READER_FIRST_VIDEO_STREAM, MF_VERSION,
};
use windows::Win32::Media::{timeBeginPeriod, timeEndPeriod};
use windows::Win32::System::Com::{
    CoIncrementMTAUsage, CoInitializeEx, CoTaskMemFree, CoUninitialize, IStream,
    COINIT_MULTITHREADED, STATFLAG_NONAME, STATSTG, STREAM_SEEK_SET,
};
use windows::Win32::System::Diagnostics::Debug::Beep;
use windows::Win32::System::ProcessStatus::EmptyWorkingSet;
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::System::Threading::{
    CreateWaitableTimerExW, GetCurrentProcess, GetExitCodeProcess, OpenProcess,
    QueryFullProcessImageNameW, SetWaitableTimer, WaitForSingleObject,
    CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, INFINITE, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::System::WinRT::{
    RoGetActivationFactory, RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED,
};
use windows::Win32::UI::HiDpi::{
    SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Shell::SHCreateMemStream;
use windows::Win32::UI::WindowsAndMessaging::{
    GetDesktopWindow, GetForegroundWindow, GetWindowThreadProcessId, PostMessageW,
};

use crate::audio::{
    default_audio_endpoint_id, enumerate_audio_input_devices, AudioEndpointMonitor,
    AudioInputDevice, WasapiStream,
};
use crate::config::{
    clean_exe_stem_name, format_clip_relative_path, lookup_custom_game_name,
    sanitize_path_component, BufferStorageMode, DesktopAudioMode, RateControlMode, ReplayConfig,
    VideoCodecType,
};
use crate::voice::VoiceRecognizer;

pub const WM_APP_STATUS: u32 = 0x8000 + 101;
pub const WM_APP_SAVED: u32 = 0x8000 + 102;
pub const WM_APP_VOICE_TRIGGER: u32 = 0x8000 + 103;
pub const WM_APP_UPDATE: u32 = 0x8000 + 104;

#[derive(Clone, Debug)]
pub struct DisplayTarget {
    pub index: usize,
    pub adapter_index: u32,
    pub output_index: u32,
    pub device_name: String,
    pub adapter_name: String,
    pub vendor_tag: String,
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub refresh_rate_hz: u32,
    pub is_primary: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct HardwareCodecCaps {
    pub hevc_hw: bool,
    pub av1_hw: bool,
}

impl DisplayTarget {
    pub fn display_label(&self) -> String {
        let pri = if self.is_primary { " [Utama]" } else { "" };
        let hz = if self.refresh_rate_hz > 0 {
            format!(" @ {}Hz", self.refresh_rate_hz)
        } else {
            String::new()
        };
        format!(
            "Layar {}: {}x{}{}{} — {}",
            self.index + 1,
            self.width,
            self.height,
            hz,
            pri,
            self.adapter_name
        )
    }
}

static NEXT_SEG_FILE_ID: AtomicU64 = AtomicU64::new(1);

/// Reference-counted temp segment file guard.
/// Keeps evicted Disk Temp Buffer `.mp4` files alive while an async replay merge
/// still holds a cloned snapshot, and deletes them automatically once dropped.
#[derive(Debug)]
pub struct DiskTempFile {
    pub path: PathBuf,
}

impl DiskTempFile {
    pub fn new(path: PathBuf) -> Arc<Self> {
        Arc::new(Self { path })
    }
}

impl Drop for DiskTempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[derive(Clone)]
pub struct ReplaySegment {
    pub seq_id: u64,
    pub ram_data: Option<Arc<[u8]>>,
    pub disk_file: Option<Arc<DiskTempFile>>,
    pub size_bytes: u64,
    pub duration_seconds: f64,
}

impl ReplaySegment {
    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    pub fn disk_path(&self) -> Option<&Path> {
        self.disk_file.as_deref().map(|f| f.path.as_path())
    }
}

fn ensure_process_mta_pinned() {
    static MTA_PIN: std::sync::Once = std::sync::Once::new();
    MTA_PIN.call_once(|| unsafe {
        // Keep the process COM/WinRT MTA alive across capture stop/start cycles so
        // windows-rs global FactoryCache agile activation factories never dangle when
        // worker threads exit and call RoUninitialize/CoUninitialize.
        let _ = CoIncrementMTAUsage();
    });
}

struct ComMfGuard {
    ro_owned: bool,
    com_owned: bool,
}

impl ComMfGuard {
    unsafe fn new() -> Self {
        ensure_process_mta_pinned();
        // Prefer RoInitialize(RO_INIT_MULTITHREADED), which initializes both WinRT and COM MTA
        // once for the lifetime of the worker thread (avoiding mid-session RoUninitialize races).
        // RPC_E_CHANGED_MODE on an STA thread must not be paired with uninit.
        let ro_owned = RoInitialize(RO_INIT_MULTITHREADED).is_ok();
        let com_owned = if ro_owned {
            false
        } else {
            CoInitializeEx(None, COINIT_MULTITHREADED).is_ok()
        };
        let _ = MFStartup(MF_VERSION, 0);
        Self {
            ro_owned,
            com_owned,
        }
    }
}

impl Drop for ComMfGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
            if self.ro_owned {
                RoUninitialize();
            } else if self.com_owned {
                CoUninitialize();
            }
        }
    }
}

/// High-resolution kernel waitable timer (`CREATE_WAITABLE_TIMER_HIGH_RESOLUTION`, Win10 1803+)
/// for sub-millisecond frame pacing with zero busy-wait spin CPU burn.
/// Falls back cleanly to `timeBeginPeriod(1)` + `thread::sleep` on older Windows builds.
struct HighResWaitableTimer {
    handle: Option<HANDLE>,
    legacy_timer_period: bool,
}

impl HighResWaitableTimer {
    unsafe fn new() -> Self {
        // TIMER_ALL_ACCESS (0x001F0003) grants SYNCHRONIZE + TIMER_MODIFY_STATE.
        const TIMER_ACCESS: u32 = 0x001F_0003;
        if let Ok(h) = CreateWaitableTimerExW(
            None,
            PCWSTR::null(),
            CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
            TIMER_ACCESS,
        ) {
            if !h.is_invalid() {
                return Self {
                    handle: Some(h),
                    legacy_timer_period: false,
                };
            }
        }
        // Fallback for Windows builds prior to 10 1803: enable 1 ms sleep granularity.
        timeBeginPeriod(1);
        Self {
            handle: None,
            legacy_timer_period: true,
        }
    }

    #[cfg(test)]
    fn is_high_res(&self) -> bool {
        self.handle.is_some()
    }

    /// Sleeps for `remaining_ms` (positive) using the high-resolution waitable timer
    /// without any CPU spin-loop.
    unsafe fn wait_ms(&self, remaining_ms: f64) {
        if remaining_ms < 0.1 {
            return;
        }
        // Cap single wait to 50 ms so `stop()` remains immediately responsive.
        let clamped_ms = remaining_ms.min(50.0);
        if let Some(h) = self.handle {
            // Negative due time = relative wait in 100-nanosecond intervals (1 ms = 10,000 units).
            let due_100ns = -((clamped_ms * 10_000.0).round() as i64).max(1);
            if SetWaitableTimer(h, &due_100ns, 0, None, None, false).is_ok() {
                let _ = WaitForSingleObject(h, INFINITE);
                return;
            }
        }
        let sleep_ms = clamped_ms.floor() as u64;
        if sleep_ms > 0 {
            thread::sleep(Duration::from_millis(sleep_ms));
        }
    }
}

impl Drop for HighResWaitableTimer {
    fn drop(&mut self) {
        unsafe {
            if let Some(h) = self.handle.take() {
                let _ = CloseHandle(h);
            }
            if self.legacy_timer_period {
                timeEndPeriod(1);
            }
        }
    }
}

#[link(name = "avrt")]
extern "system" {
    fn AvSetMmThreadCharacteristicsW(taskname: PCWSTR, taskindex: *mut u32) -> HANDLE;
    fn AvSetMmThreadPriority(avrt_handle: HANDLE, priority: i32) -> BOOL;
    fn AvRevertMmThreadCharacteristics(avrt_handle: HANDLE) -> BOOL;
}

/// RAII Multimedia Class Scheduler Service (MMCSS) thread priority guard (`avrt.dll`).
/// Elevates real-time video capture & WASAPI audio threads via `AvSetMmThreadCharacteristicsW`
/// (`"Capture"` / `"Audio"`) so heavy CPU games cannot starve frame pacing or audio draining.
pub struct MmcssThreadGuard {
    handle: HANDLE,
}

impl MmcssThreadGuard {
    pub fn new(task_name: &str) -> Self {
        unsafe {
            let wide: Vec<u16> = task_name.encode_utf16().chain(std::iter::once(0)).collect();
            let mut task_index: u32 = 0;
            let mut handle = AvSetMmThreadCharacteristicsW(PCWSTR(wide.as_ptr()), &mut task_index);
            if handle.is_invalid() && !task_name.eq_ignore_ascii_case("Audio") {
                let fallback: Vec<u16> = "Audio\0".encode_utf16().collect();
                task_index = 0;
                handle = AvSetMmThreadCharacteristicsW(PCWSTR(fallback.as_ptr()), &mut task_index);
            }
            if !handle.is_invalid() {
                // AVRT_PRIORITY_HIGH (1): elevates capture & audio thread above normal game worker threads
                let _ = AvSetMmThreadPriority(handle, 1);
            }
            Self { handle }
        }
    }

    #[cfg(test)]
    pub fn is_active(&self) -> bool {
        !self.handle.is_invalid()
    }
}

impl Drop for MmcssThreadGuard {
    fn drop(&mut self) {
        unsafe {
            if !self.handle.is_invalid() {
                let _ = AvRevertMmThreadCharacteristics(self.handle);
                self.handle = HANDLE::default();
            }
        }
    }
}

struct ActiveSegmentWriter {
    seq_id: u64,
    sink_writer: IMFSinkWriter,
    mem_stream: Option<IStream>,
    _mf_byte_stream: Option<IMFByteStream>,
    disk_path: Option<PathBuf>,
    video_stream_index: u32,
    audio_stream_index: u32,
    has_audio: bool,
    frames_written: u32,
    audio_samples_written: i64,
    write_errors: u32,
    actual_codec: VideoCodecType,
    rc_tuned: bool,
}

enum CaptureSessionOutcome {
    Stopped,
    NeedsRestart,
    Fatal(&'static str),
}

// SAFETY: IMFSinkWriter and IMFDXGIDeviceManager are COM free-threaded/thread-safe
unsafe impl Send for ActiveSegmentWriter {}

struct SendDevMgr(Option<IMFDXGIDeviceManager>);
unsafe impl Send for SendDevMgr {}

/// Hardware fixed-function GPU BGRA -> NV12 BT.709 converter & scaler (works on NVIDIA, AMD, Intel).
struct GpuNv12Converter {
    video_ctx: ID3D11VideoContext,
    vp: ID3D11VideoProcessor,
    _vp_enum: ID3D11VideoProcessorEnumerator,
    input_view: ID3D11VideoProcessorInputView,
    output_views: Vec<ID3D11VideoProcessorOutputView>,
}

impl GpuNv12Converter {
    #[allow(clippy::too_many_arguments)]
    unsafe fn new(
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        input_bgra: &ID3D11Texture2D,
        nv12_ring: &[ID3D11Texture2D],
        in_w: u32,
        in_h: u32,
        out_w: u32,
        out_h: u32,
        fps: u32,
    ) -> Option<Self> {
        let video_dev: ID3D11VideoDevice = device.cast().ok()?;
        let video_ctx: ID3D11VideoContext = context.cast().ok()?;

        let content_desc = D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
            InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
            InputFrameRate: DXGI_RATIONAL {
                Numerator: fps.max(1),
                Denominator: 1,
            },
            InputWidth: in_w,
            InputHeight: in_h,
            OutputFrameRate: DXGI_RATIONAL {
                Numerator: fps.max(1),
                Denominator: 1,
            },
            OutputWidth: out_w,
            OutputHeight: out_h,
            Usage: D3D11_VIDEO_USAGE_OPTIMAL_SPEED,
        };
        let vp_enum = video_dev.CreateVideoProcessorEnumerator(&content_desc).ok()?;
        let vp = video_dev.CreateVideoProcessor(&vp_enum, 0).ok()?;

        video_ctx.VideoProcessorSetStreamFrameFormat(&vp, 0, D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE);
        // Disable driver movie post-processing filters (denoise/contrast) to save GPU & preserve sharp text.
        video_ctx.VideoProcessorSetStreamAutoProcessingMode(&vp, 0, false);

        let src_rect = RECT {
            left: 0,
            top: 0,
            right: in_w as i32,
            bottom: in_h as i32,
        };
        let dst_rect = RECT {
            left: 0,
            top: 0,
            right: out_w as i32,
            bottom: out_h as i32,
        };
        video_ctx.VideoProcessorSetStreamSourceRect(&vp, 0, true, Some(&src_rect));
        video_ctx.VideoProcessorSetStreamDestRect(&vp, 0, true, Some(&dst_rect));
        video_ctx.VideoProcessorSetOutputTargetRect(&vp, true, Some(&dst_rect));

        // Exact BT.709 color conversion: RGB Full (0..255) -> YCbCr Studio (16..235) BT.709.
        if let Ok(vctx1) = video_ctx.cast::<ID3D11VideoContext1>() {
            vctx1.VideoProcessorSetStreamColorSpace1(&vp, 0, DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709);
            vctx1.VideoProcessorSetOutputColorSpace1(&vp, DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709);
        } else {
            let in_cs = D3D11_VIDEO_PROCESSOR_COLOR_SPACE {
                _bitfield: (1 << 2) | (2 << 4), // RGB full, BT.709, nominal 0..255
            };
            let out_cs = D3D11_VIDEO_PROCESSOR_COLOR_SPACE {
                _bitfield: (1 << 2) | (1 << 4), // YCbCr BT.709, studio 16..235
            };
            video_ctx.VideoProcessorSetStreamColorSpace(&vp, 0, &in_cs);
            video_ctx.VideoProcessorSetOutputColorSpace(&vp, &out_cs);
        }

        let in_desc = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
            FourCC: 0,
            ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_VPIV {
                    MipSlice: 0,
                    ArraySlice: 0,
                },
            },
        };
        let mut in_view_opt = None;
        video_dev
            .CreateVideoProcessorInputView(input_bgra, &vp_enum, &in_desc, Some(&mut in_view_opt))
            .ok()?;
        let input_view = in_view_opt?;

        let out_desc = D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
            ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_VPOV { MipSlice: 0 },
            },
        };
        let mut output_views = Vec::with_capacity(nv12_ring.len());
        for tex in nv12_ring {
            let mut out_view_opt = None;
            video_dev
                .CreateVideoProcessorOutputView(tex, &vp_enum, &out_desc, Some(&mut out_view_opt))
                .ok()?;
            output_views.push(out_view_opt?);
        }

        Some(Self {
            video_ctx,
            vp,
            _vp_enum: vp_enum,
            input_view,
            output_views,
        })
    }

    unsafe fn convert_slot(&self, slot: usize, frame_idx: u32) -> bool {
        let mut streams = [D3D11_VIDEO_PROCESSOR_STREAM {
            Enable: BOOL(1),
            OutputIndex: 0,
            InputFrameOrField: 0,
            PastFrames: 0,
            FutureFrames: 0,
            ppPastSurfaces: null_mut(),
            pInputSurface: ManuallyDrop::new(Some(self.input_view.clone())),
            ppFutureSurfaces: null_mut(),
            ppPastSurfacesRight: null_mut(),
            pInputSurfaceRight: ManuallyDrop::new(None),
            ppFutureSurfacesRight: null_mut(),
        }];
        let ok = self
            .video_ctx
            .VideoProcessorBlt(&self.vp, &self.output_views[slot], frame_idx, &streams)
            .is_ok();
        ManuallyDrop::drop(&mut streams[0].pInputSurface);
        ok
    }
}

#[derive(Default)]
struct GdiFallbackState {
    screen_dc: HDC,
    mem_dc: HDC,
    dib: HBITMAP,
    old_bmp: HGDIOBJ,
    dib_bits: *mut std::ffi::c_void,
}

impl GdiFallbackState {
    unsafe fn ensure_init(&mut self, w: u32, h: u32) {
        if !self.screen_dc.is_invalid() && !self.dib_bits.is_null() {
            return;
        }
        self.screen_dc = GetDC(GetDesktopWindow());
        self.mem_dc = CreateCompatibleDC(self.screen_dc);
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                biHeight: -(h as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [Default::default(); 1],
        };
        let mut bits = null_mut();
        if let Ok(bmp) = CreateDIBSection(self.screen_dc, &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
            self.dib = bmp;
            self.dib_bits = bits;
            self.old_bmp = SelectObject(self.mem_dc, HGDIOBJ(self.dib.0));
        } else {
            // Release now, otherwise every frame re-acquires DCs until the GDI quota is gone.
            let _ = DeleteDC(self.mem_dc);
            ReleaseDC(GetDesktopWindow(), self.screen_dc);
            self.mem_dc = Default::default();
            self.screen_dc = Default::default();
        }
    }
}

impl Drop for GdiFallbackState {
    fn drop(&mut self) {
        unsafe {
            if !self.mem_dc.is_invalid() && !self.old_bmp.is_invalid() {
                SelectObject(self.mem_dc, self.old_bmp);
            }
            if !self.dib.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(self.dib.0));
            }
            if !self.mem_dc.is_invalid() {
                let _ = DeleteDC(self.mem_dc);
            }
            if !self.screen_dc.is_invalid() {
                ReleaseDC(GetDesktopWindow(), self.screen_dc);
            }
        }
    }
}

pub struct ReplayEngine {
    pub config: Mutex<ReplayConfig>,
    pub voice: Arc<VoiceRecognizer>,
    pub updater: Arc<crate::updater::UpdateManager>,
    pub displays: Mutex<Vec<DisplayTarget>>,
    pub mic_devices: Mutex<Vec<AudioInputDevice>>,
    pub active_mic_name: Mutex<String>,
    pub active_desktop_audio_label: Mutex<String>,
    pub gpu_name: Mutex<String>,
    pub codec_caps: Mutex<HardwareCodecCaps>,
    pub capture_mode_status: Mutex<String>,
    pub video_pipeline_status: Mutex<String>,
    pub status_message: Mutex<String>,
    pub last_saved_file: Mutex<String>,
    pub last_foreground_game: Mutex<String>,

    segments: Mutex<Vec<ReplaySegment>>,
    is_running: AtomicBool,
    is_stopping: AtomicBool,
    is_saving: AtomicBool,
    force_rotate: AtomicBool,
    rotate_notify: (Mutex<bool>, Condvar),
    consecutive_seg_failures: AtomicU32,

    current_fps_x10: AtomicU64,
    ui_hwnd: Mutex<isize>,
    capture_thread: Mutex<Option<JoinHandle<()>>>,
}

impl ReplayEngine {
    pub fn new(config: ReplayConfig) -> Arc<Self> {
        let voice = VoiceRecognizer::new(&config.voice_phrase);
        let caps = probe_hardware_codec_caps();
        let initial_game = detect_foreground_exe_path()
            .map(|p| resolve_game_name_for_exe_path(&p, &config.game_name_map))
            .unwrap_or_else(|| "Desktop".to_string());
        let engine = Arc::new(Self {
            config: Mutex::new(config),
            voice,
            updater: crate::updater::UpdateManager::new(),
            displays: Mutex::new(Vec::new()),
            mic_devices: Mutex::new(Vec::new()),
            active_mic_name: Mutex::new(String::new()),
            active_desktop_audio_label: Mutex::new(String::new()),
            gpu_name: Mutex::new("Hardware GPU Encoder".to_string()),
            codec_caps: Mutex::new(caps),
            capture_mode_status: Mutex::new("Standby".to_string()),
            video_pipeline_status: Mutex::new("Standby".to_string()),
            status_message: Mutex::new("Siap digunakan.".to_string()),
            last_saved_file: Mutex::new(String::new()),
            last_foreground_game: Mutex::new(initial_game),
            segments: Mutex::new(Vec::new()),
            is_running: AtomicBool::new(false),
            is_stopping: AtomicBool::new(false),
            is_saving: AtomicBool::new(false),
            force_rotate: AtomicBool::new(false),
            rotate_notify: (Mutex::new(false), Condvar::new()),
            consecutive_seg_failures: AtomicU32::new(0),
            current_fps_x10: AtomicU64::new(0),
            ui_hwnd: Mutex::new(0),
            capture_thread: Mutex::new(None),
        });
        engine.refresh_displays_and_gpu();
        engine.refresh_audio_inputs();
        engine
    }

    pub fn refresh_foreground_game(&self, custom_map: &str) -> String {
        if let Some(exe_path) = detect_foreground_exe_path() {
            let name = resolve_game_name_for_exe_path(&exe_path, custom_map);
            if !name.is_empty() {
                if let Ok(mut guard) = self.last_foreground_game.lock() {
                    *guard = name.clone();
                }
                return name;
            }
        }
        self.last_foreground_game
            .lock()
            .ok()
            .map(|g| {
                if g.trim().is_empty() {
                    "Desktop".to_string()
                } else {
                    g.clone()
                }
            })
            .unwrap_or_else(|| "Desktop".to_string())
    }

    pub fn set_ui_hwnd(&self, hwnd: HWND) {
        if let Ok(mut guard) = self.ui_hwnd.lock() {
            *guard = hwnd.0 as isize;
        }
        self.updater.set_ui_hwnd(hwnd);
    }

    fn notify_ui_status(&self) {
        let raw_hwnd = self.ui_hwnd.lock().map(|g| *g).unwrap_or(0);
        if raw_hwnd != 0 {
            unsafe {
                let _ = PostMessageW(HWND(raw_hwnd as *mut _), WM_APP_STATUS, WPARAM(0), LPARAM(0));
            }
        }
    }

    fn sync_active_mic_name(&self, mic_audio: &WasapiStream) {
        if let Ok(mut g) = self.active_mic_name.lock() {
            if mic_audio.is_available() {
                *g = mic_audio.active_device_name().to_string();
            } else {
                g.clear();
            }
        }
    }

    fn set_desktop_audio_label(&self, label: String) {
        if let Ok(mut g) = self.active_desktop_audio_label.lock() {
            if *g != label {
                *g = label;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn sync_active_game_audio(
        &self,
        desktop_audio: &mut WasapiStream,
        locked_game: &mut Option<(u32, String)>,
        fallback_all: &mut bool,
        game_name_map: &str,
        render_dirty: bool,
        last_desk_retry: &mut Instant,
        invalidated: bool,
    ) {
        if *fallback_all {
            Self::sync_audio_endpoint(
                desktop_audio,
                true,
                None,
                render_dirty,
                Duration::from_millis(400),
                last_desk_retry,
            );
            return;
        }

        if let Some((pid, _)) = locked_game.as_ref() {
            if !is_process_alive(*pid) {
                desktop_audio.stop_and_release();
                *locked_game = None;
                self.set_desktop_audio_label("Menunggu game aktif...".to_string());
                self.notify_ui_status();
            }
        }

        if let Some((fg_pid, fg_path)) = detect_foreground_process() {
            if !is_background_utility_exe(&fg_path) {
                let need_switch = locked_game.as_ref().map(|(p, _)| *p) != Some(fg_pid)
                    || invalidated
                    || !desktop_audio.is_available();
                if need_switch {
                    let name = resolve_game_name_for_exe_path(&fg_path, game_name_map);
                    if desktop_audio.initialize_process_loopback(fg_pid, &name) {
                        *locked_game = Some((fg_pid, name.clone()));
                        self.set_desktop_audio_label(format!("Game: {}", name));
                        self.notify_ui_status();
                    } else if is_process_alive(fg_pid) {
                        *fallback_all = true;
                        *locked_game = None;
                        *last_desk_retry = Instant::now();
                        desktop_audio.initialize_with_device(true, None);
                        self.set_desktop_audio_label("Fallback: Semua Desktop".to_string());
                        self.notify_ui_status();
                    }
                }
                return;
            }
        }

        if let Some((pid, name)) = locked_game.clone() {
            if (invalidated || !desktop_audio.is_available()) && is_process_alive(pid) {
                if desktop_audio.initialize_process_loopback(pid, &name) {
                    self.set_desktop_audio_label(format!("Game: {}", name));
                    self.notify_ui_status();
                } else {
                    *fallback_all = true;
                    *locked_game = None;
                    *last_desk_retry = Instant::now();
                    desktop_audio.initialize_with_device(true, None);
                    self.set_desktop_audio_label("Fallback: Semua Desktop".to_string());
                    self.notify_ui_status();
                }
            }
        } else {
            self.set_desktop_audio_label("Menunggu game aktif...".to_string());
        }
    }

    fn sync_audio_endpoint(
        stream: &mut WasapiStream,
        is_loopback: bool,
        pinned_device_id: Option<&str>,
        dirty: bool,
        retry_interval: Duration,
        last_retry: &mut Instant,
    ) -> bool {
        let invalidated = stream.take_invalidated();
        let retry_due = !stream.is_available() && last_retry.elapsed() >= retry_interval;
        if !(dirty || invalidated || retry_due) {
            return false;
        }

        let need_reinit = if !stream.is_available() || invalidated {
            true
        } else if pinned_device_id.is_none() {
            default_audio_endpoint_id(is_loopback).as_deref() != Some(stream.active_device_id())
        } else {
            false
        };
        if !need_reinit {
            return false;
        }

        *last_retry = Instant::now();
        stream.initialize_with_device(is_loopback, pinned_device_id);
        true
    }

    pub fn raise_status(&self, msg: String) {
        if let Ok(mut guard) = self.status_message.lock() {
            *guard = msg;
        }
        self.notify_ui_status();
    }

    pub fn is_saving(&self) -> bool {
        self.is_saving.load(Ordering::Acquire)
    }

    fn notify_saved(&self, path: String) {
        if let Ok(mut guard) = self.last_saved_file.lock() {
            *guard = path;
        }
        let raw_hwnd = self.ui_hwnd.lock().map(|g| *g).unwrap_or(0);
        if raw_hwnd != 0 {
            unsafe {
                let _ = PostMessageW(HWND(raw_hwnd as *mut _), WM_APP_SAVED, WPARAM(0), LPARAM(0));
            }
        }
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Acquire)
    }

    pub fn is_stopping(&self) -> bool {
        self.is_stopping.load(Ordering::Acquire)
    }

    pub fn current_fps(&self) -> f64 {
        self.current_fps_x10.load(Ordering::Relaxed) as f64 / 10.0
    }

    pub fn buffered_seconds(&self) -> f64 {
        let Ok(segs) = self.segments.lock() else {
            return 0.0;
        };
        segs.iter().map(|s| s.duration_seconds).sum()
    }

    pub fn buffer_size_bytes(&self) -> u64 {
        let Ok(segs) = self.segments.lock() else {
            return 0;
        };
        segs.iter().map(|s| s.size_bytes()).sum()
    }

    pub fn refresh_displays_and_gpu(&self) {
        let mut list = Vec::new();
        let mut best_gpu = "Hardware GPU Encoder".to_string();

        unsafe {
            if let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() {
                let mut adapter_idx: u32 = 0;
                while let Ok(adapter) = factory.EnumAdapters1(adapter_idx) {
                    if let Ok(desc) = adapter.GetDesc1() {
                        let adapter_title = utf16_slice_to_string(&desc.Description);
                        let vendor = vendor_tag(desc.VendorId);

                        if adapter_idx == 0
                            || desc.VendorId == 0x10DE
                            || desc.VendorId == 0x1002
                            || desc.VendorId == 0x8086
                        {
                            if best_gpu == "Hardware GPU Encoder"
                                || desc.VendorId == 0x10DE
                                || desc.VendorId == 0x1002
                            {
                                best_gpu = format!("{} ({})", adapter_title, vendor);
                            }
                        }

                        let mut output_idx: u32 = 0;
                        while let Ok(output) = adapter.EnumOutputs(output_idx) {
                            if let Ok(odesc) = output.GetDesc() {
                                if odesc.AttachedToDesktop.as_bool() {
                                    let dev_name = utf16_slice_to_string(&odesc.DeviceName);
                                    let r = odesc.DesktopCoordinates;
                                    let w = (r.right - r.left).max(640) as u32;
                                    let h = (r.bottom - r.top).max(360) as u32;
                                    let is_primary = r.left == 0 && r.top == 0;
                                    let hz = query_display_refresh_rate(&odesc.DeviceName);

                                    list.push(DisplayTarget {
                                        index: list.len(),
                                        adapter_index: adapter_idx,
                                        output_index: output_idx,
                                        device_name: dev_name,
                                        adapter_name: adapter_title.clone(),
                                        vendor_tag: vendor.to_string(),
                                        left: r.left,
                                        top: r.top,
                                        width: w,
                                        height: h,
                                        refresh_rate_hz: hz,
                                        is_primary,
                                    });
                                }
                            }
                            output_idx += 1;
                        }
                    }
                    adapter_idx += 1;
                }
            }

            if list.is_empty() {
                list = enum_monitors_gdi(&best_gpu);
            }
        }

        if let Ok(mut g) = self.gpu_name.lock() {
            *g = best_gpu;
        }
        if let Ok(mut d) = self.displays.lock() {
            *d = list;
        }
    }

    pub fn refresh_audio_inputs(&self) {
        let mics = enumerate_audio_input_devices();
        if let Ok(mut g) = self.mic_devices.lock() {
            *g = mics;
        }
    }

    pub fn start(self: &Arc<Self>) {
        if self.is_stopping.load(Ordering::Acquire) {
            return;
        }
        if self.is_running.swap(true, Ordering::AcqRel) {
            return;
        }
        self.clear_buffer();
        self.consecutive_seg_failures.store(0, Ordering::Release);
        self.force_rotate.store(false, Ordering::Release);

        let cfg = self.config.lock().unwrap().clone();
        let mode_str = if cfg.storage_mode == BufferStorageMode::RamRingBuffer {
            "RAM Buffer"
        } else {
            "Disk Buffer"
        };

        let engine_ref = Arc::clone(self);
        let handle = thread::Builder::new()
            .name("NVLiteReplayRust_Capture".to_string())
            .spawn(move || {
                Self::capture_loop(engine_ref);
            })
            .ok();

        if let Ok(mut t) = self.capture_thread.lock() {
            *t = handle;
        }
        self.raise_status(format!("Instant Replay Aktif ({})", mode_str));
    }

    fn finish_stop(&self) {
        if let Ok(mut guard) = self.capture_thread.lock() {
            if let Some(h) = guard.take() {
                let _ = h.join();
            }
        }

        // Quit calls stop() and then exits the process; let an in-flight merge finish first.
        // ponytail: 30s cap so quit can't hang forever on a stuck merge
        let deadline = Instant::now() + Duration::from_secs(30);
        while self.is_saving.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        self.clear_buffer();
        if let Ok(mut s) = self.capture_mode_status.lock() {
            *s = "Standby".to_string();
        }
        if let Ok(mut p) = self.video_pipeline_status.lock() {
            *p = "Standby".to_string();
        }
        self.set_desktop_audio_label(String::new());
        self.current_fps_x10.store(0, Ordering::Relaxed);
        trim_working_set();
        self.is_stopping.store(false, Ordering::Release);
        self.raise_status("Instant Replay Nonaktif".to_string());
    }

    pub fn stop_async(self: &Arc<Self>) {
        if !self.is_running.swap(false, Ordering::AcqRel) {
            return;
        }
        self.is_stopping.store(true, Ordering::Release);
        self.raise_status("Menonaktifkan Replay...".to_string());
        let engine_ref = Arc::clone(self);
        if thread::Builder::new()
            .name("NVLiteReplayRust_Stop".to_string())
            .spawn(move || {
                engine_ref.finish_stop();
            })
            .is_err()
        {
            self.finish_stop();
        }
    }

    pub fn stop(&self) {
        let was_running = self.is_running.swap(false, Ordering::AcqRel);
        if was_running {
            self.is_stopping.store(true, Ordering::Release);
            self.finish_stop();
        } else {
            while self.is_stopping.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(10));
            }
        }
    }

    pub fn clear_buffer(&self) {
        let drained: Vec<ReplaySegment> = if let Ok(mut segs) = self.segments.lock() {
            segs.drain(..).collect()
        } else {
            Vec::new()
        };
        // Dropping `drained` releases each segment's `Arc<DiskTempFile>` outside the mutex;
        // any file not pinned by an in-flight replay save is deleted immediately.
        drop(drained);
    }

    pub fn save_replay_async(self: &Arc<Self>) {
        if !self.is_running() || self.is_stopping.load(Ordering::Acquire) {
            self.raise_status("Instant Replay belum dinyalakan!".to_string());
            return;
        }
        if self.is_saving.swap(true, Ordering::AcqRel) {
            self.raise_status("Sedang menyimpan replay...".to_string());
            return;
        }

        let engine = Arc::clone(self);
        thread::spawn(move || {
            let _mf_guard = unsafe { ComMfGuard::new() };
            engine.raise_status("Menyimpan Instant Replay...".to_string());

            {
                let (lock, cv) = &engine.rotate_notify;
                if let Ok(mut done) = lock.lock() {
                    *done = false;
                    engine.force_rotate.store(true, Ordering::Release);
                    let _ = cv.wait_timeout_while(done, Duration::from_millis(3000), |d| !*d);
                } else {
                    engine.force_rotate.store(true, Ordering::Release);
                }
            }

            let cfg = engine.config.lock().unwrap().clone();
            let snapshot: Vec<ReplaySegment> = {
                let guard = engine.segments.lock().unwrap();
                let needed = cfg.replay_duration_seconds as f64;
                let mut acc = 0.0;
                let mut picked = Vec::new();
                for seg in guard.iter().rev() {
                    picked.push(seg.clone());
                    acc += seg.duration_seconds;
                    if acc >= needed {
                        break;
                    }
                }
                picked.reverse();
                picked
            };

            if snapshot.is_empty() {
                engine.raise_status("Buffer masih kosong, tunggu beberapa detik.".to_string());
                engine.is_saving.store(false, Ordering::Release);
                return;
            }

            let out_dir = Path::new(&cfg.output_folder);
            let _ = fs::create_dir_all(out_dir);

            let game_name = engine.refresh_foreground_game(&cfg.game_name_map);
            let output_path =
                unique_replay_output_path(out_dir, &cfg.clip_name_template, &game_name);
            let display_name = output_path
                .strip_prefix(out_dir)
                .unwrap_or(&output_path)
                .to_string_lossy()
                .replace('\\', "/");

            let t0 = Instant::now();
            let ok = merge_segments_pure_native(&snapshot, &output_path, cfg.fps);
            let elapsed_ms = t0.elapsed().as_millis();
            // Explicitly drop snapshot after merge so any evicted Arc<DiskTempFile> files clean up now.
            drop(snapshot);

            if ok {
                if let Ok(meta) = fs::metadata(&output_path) {
                    if meta.len() > 0 {
                        if cfg.play_sound_on_save {
                            unsafe {
                                let _ = Beep(880, 90);
                                let _ = Beep(1175, 120);
                            }
                        }
                        let size_mb = meta.len() as f64 / (1024.0 * 1024.0);
                        engine.raise_status(format!(
                            "Tersimpan: {} ({:.2} MB, {} ms)",
                            display_name, size_mb, elapsed_ms
                        ));
                        engine.notify_saved(output_path.to_string_lossy().into_owned());
                        engine.is_saving.store(false, Ordering::Release);
                        return;
                    }
                }
            }

            let _ = fs::remove_file(&output_path);
            engine.raise_status("Gagal menggabungkan segmen replay.".to_string());
            engine.is_saving.store(false, Ordering::Release);
        });
    }

    fn record_segment_failure(&self, mode: BufferStorageMode) {
        self.consecutive_seg_failures.fetch_add(1, Ordering::AcqRel);
        // If Disk Temp Buffer ran out of space while holding a full 3-5 min ring, evict the
        // oldest segment so subsequent segments have room instead of deadlocking forever.
        if mode == BufferStorageMode::DiskTempBuffer {
            let evicted_old = {
                if let Ok(mut segs) = self.segments.lock() {
                    if segs.len() > 2 {
                        Some(segs.remove(0))
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            drop(evicted_old);
        }
    }

    fn finalize_and_store_segment(
        self: &Arc<Self>,
        mut seg_writer: ActiveSegmentWriter,
        fps: u32,
        mode: BufferStorageMode,
    ) -> bool {
        if seg_writer.frames_written == 0 {
            let orphaned = seg_writer.disk_path.take();
            drop(seg_writer);
            if let Some(p) = orphaned {
                let _ = fs::remove_file(p);
            }
            return false;
        }
        unsafe {
            if !self.is_running.load(Ordering::Acquire) {
                let orphaned = seg_writer.disk_path.take();
                drop(seg_writer);
                if let Some(p) = orphaned {
                    let _ = fs::remove_file(p);
                }
                return false;
            }
            let seq_id = seg_writer.seq_id;
            let duration = seg_writer.frames_written as f64 / fps.max(1) as f64;
            if seg_writer.sink_writer.Finalize().is_err() {
                let orphaned = seg_writer.disk_path.take();
                drop(seg_writer);
                if let Some(p) = orphaned {
                    let _ = fs::remove_file(p);
                }
                self.record_segment_failure(mode);
                return false;
            }

            let mut seg = ReplaySegment {
                seq_id,
                ram_data: None,
                disk_file: None,
                size_bytes: 0,
                duration_seconds: duration,
            };

            let disk_path_opt = seg_writer.disk_path.take();
            if mode == BufferStorageMode::RamRingBuffer {
                if let Some(ref stream) = seg_writer.mem_stream {
                    let mut stat = STATSTG::default();
                    if stream.Stat(&mut stat, STATFLAG_NONAME).is_ok() && stat.cbSize > 0 {
                        let size = stat.cbSize as usize;
                        let mut data = vec![0u8; size];
                        let _ = stream.Seek(0, STREAM_SEEK_SET, None);
                        let mut bytes_read = 0u32;
                        let _ = stream.Read(data.as_mut_ptr() as *mut _, size as u32, Some(&mut bytes_read));
                        data.truncate(bytes_read as usize);
                        if !data.is_empty() {
                            seg.size_bytes = data.len() as u64;
                            seg.ram_data = Some(Arc::from(data.into_boxed_slice()));
                        }
                    }
                }
            }
            // Drop writer BEFORE checking disk metadata / pushing so MP4 file handles are closed & flushed.
            drop(seg_writer);

            if mode == BufferStorageMode::DiskTempBuffer {
                if let Some(path) = disk_path_opt {
                    match fs::metadata(&path) {
                        Ok(meta) if meta.len() > 0 => {
                            seg.size_bytes = meta.len();
                            seg.disk_file = Some(DiskTempFile::new(path));
                        }
                        _ => {
                            let _ = fs::remove_file(path);
                        }
                    }
                }
            }

            if seg.ram_data.is_none() && seg.disk_file.is_none() {
                self.record_segment_failure(mode);
                return false;
            }

            if !self.is_running.load(Ordering::Acquire) {
                drop(seg);
                return false;
            }
            let max_dur = {
                let cfg = self.config.lock().unwrap();
                (cfg.replay_duration_seconds + cfg.segment_duration_seconds) as f64
            };
            let mut evicted_segs = Vec::new();
            {
                let mut segs = self.segments.lock().unwrap();
                if !self.is_running.load(Ordering::Acquire) {
                    evicted_segs.push(seg);
                } else {
                    let pos = segs.partition_point(|s| s.seq_id < seg.seq_id);
                    segs.insert(pos, seg);
                    while segs.len() > 1 {
                        let total: f64 = segs.iter().map(|s| s.duration_seconds).sum();
                        if total <= max_dur {
                            break;
                        }
                        evicted_segs.push(segs.remove(0));
                    }
                }
            }
            // Drop evicted segments outside the mutex; Arc<DiskTempFile> deletes unpinned files immediately
            // while keeping files currently being merged by save_replay_async alive until merge completes.
            // Note: do NOT call EmptyWorkingSet here — trimming on every 5s eviction pages out active
            // ring-buffer RAM segments to pagefile.sys; working set is trimmed once in finish_stop().
            drop(evicted_segs);
            self.consecutive_seg_failures.store(0, Ordering::Release);
            true
        }
    }

    fn reset_standby_metrics(&self) {
        self.current_fps_x10.store(0, Ordering::Relaxed);
        if let Ok(mut s) = self.capture_mode_status.lock() {
            *s = "Standby".to_string();
        }
        if let Ok(mut p) = self.video_pipeline_status.lock() {
            *p = "Standby".to_string();
        }
        self.set_desktop_audio_label(String::new());
    }

    fn capture_loop(engine: Arc<Self>) {
        struct CaptureExitGuard {
            engine: Arc<ReplayEngine>,
        }
        impl Drop for CaptureExitGuard {
            fn drop(&mut self) {
                let (lock, cv) = &self.engine.rotate_notify;
                if let Ok(mut done) = lock.lock() {
                    *done = true;
                    cv.notify_all();
                }
                // Never leave `is_running == true` after the capture thread exits (prevents
                // zombie buffer state that would repeatedly save stale clips).
                if self.engine.is_running.swap(false, Ordering::AcqRel) {
                    self.engine.reset_standby_metrics();
                    self.engine.raise_status(
                        "Error: Capture terhenti (cek ruang disk temp / GPU). Nyalakan ulang Replay."
                            .to_string(),
                    );
                }
            }
        }

        let _com_mf_guard = unsafe { ComMfGuard::new() };
        let _mmcss_guard = MmcssThreadGuard::new("Capture");
        let _exit_guard = CaptureExitGuard {
            engine: Arc::clone(&engine),
        };

        engine.refresh_displays_and_gpu();
        let displays = engine.displays.lock().unwrap().clone();
        let cfg_init = engine.config.lock().unwrap().clone();

        if displays.is_empty() {
            engine.reset_standby_metrics();
            engine.raise_status("Error: Tidak ada monitor yang terdeteksi.".to_string());
            engine.is_running.store(false, Ordering::Release);
            return;
        }

        let mon_idx = cfg_init.monitor_index.min(displays.len() - 1);
        let target_mon = displays[mon_idx].clone();

        const MAX_RAPID_RESTARTS: u32 = 3;
        let mut next_seq_id: u64 = 1;
        let mut rapid_restarts: u32 = 0;

        while engine.is_running.load(Ordering::Acquire) {
            let session_start = Instant::now();
            engine.consecutive_seg_failures.store(0, Ordering::Release);

            match Self::run_capture_session(&engine, &target_mon, &cfg_init, &mut next_seq_id) {
                CaptureSessionOutcome::Stopped => break,
                CaptureSessionOutcome::Fatal(msg) => {
                    engine.reset_standby_metrics();
                    engine.raise_status(msg.to_string());
                    engine.is_running.store(false, Ordering::Release);
                    break;
                }
                CaptureSessionOutcome::NeedsRestart => {
                    if !engine.is_running.load(Ordering::Acquire) {
                        break;
                    }
                    if session_start.elapsed() >= Duration::from_secs(15) {
                        rapid_restarts = 0;
                    }
                    rapid_restarts += 1;
                    if rapid_restarts > MAX_RAPID_RESTARTS {
                        engine.reset_standby_metrics();
                        engine.raise_status(
                            "Error: Encoder/Disk Temp gagal pulih — cek ruang disk atau GPU."
                                .to_string(),
                        );
                        engine.is_running.store(false, Ordering::Release);
                        break;
                    }
                    engine.raise_status(format!(
                        "Memulihkan pipeline capture ({}/{})...",
                        rapid_restarts, MAX_RAPID_RESTARTS
                    ));
                    for _ in 0..8 {
                        if !engine.is_running.load(Ordering::Acquire) {
                            break;
                        }
                        thread::sleep(Duration::from_millis(50));
                    }
                }
            }
        }
    }

    fn run_capture_session(
        engine: &Arc<Self>,
        target_mon: &DisplayTarget,
        cfg_init: &ReplayConfig,
        next_seq_id: &mut u64,
    ) -> CaptureSessionOutcome {
        let cap_width = target_mon.width;
        let cap_height = target_mon.height;
        let cap_left = target_mon.left;
        let cap_top = target_mon.top;

        let out_width = if cfg_init.use_native_resolution {
            cap_width & !1
        } else {
            cfg_init.width & !1
        };
        let out_height = if cfg_init.use_native_resolution {
            cap_height & !1
        } else {
            cfg_init.height & !1
        };
        let fps = cfg_init.fps.max(1);
        let bitrate_bps = cfg_init.bitrate_mbps * 1_000_000;
        let rate_control = cfg_init.rate_control_mode;
        let storage_mode = cfg_init.storage_mode;
        let temp_dir = cfg_init.effective_temp_dir();
        let codec = cfg_init.codec;
        let frames_per_segment = cfg_init.segment_duration_seconds * fps;

        unsafe {
            set_capture_thread_dpi_aware();

            let mut target_adapter: Option<IDXGIAdapter1> = None;
            if let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() {
                target_adapter = factory.EnumAdapters1(target_mon.adapter_index).ok();
            }

            let mut device_opt: Option<ID3D11Device> = None;
            let mut context_opt: Option<ID3D11DeviceContext> = None;
            let flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT;
            let levels = [D3D_FEATURE_LEVEL_11_0];

            let hr_dev = if let Some(ref adap) = target_adapter {
                D3D11CreateDevice(
                    adap,
                    D3D_DRIVER_TYPE_UNKNOWN,
                    HMODULE::default(),
                    flags,
                    Some(&levels),
                    D3D11_SDK_VERSION,
                    Some(&mut device_opt),
                    None,
                    Some(&mut context_opt),
                )
            } else {
                Err(windows::core::Error::from_win32())
            };

            if hr_dev.is_err() || device_opt.is_none() {
                let _ = D3D11CreateDevice(
                    None,
                    D3D_DRIVER_TYPE_HARDWARE,
                    HMODULE::default(),
                    flags,
                    Some(&levels),
                    D3D11_SDK_VERSION,
                    Some(&mut device_opt),
                    None,
                    Some(&mut context_opt),
                );
            }

            let (Some(device), Some(context)) = (device_opt, context_opt) else {
                return CaptureSessionOutcome::Fatal("Gagal menginisialisasi D3D11 GPU device.");
            };

            if let Ok(mt) = device.cast::<ID3D11Multithread>() {
                let _ = mt.SetMultithreadProtected(true);
            }

            let mut reset_token: u32 = 0;
            let mut dev_mgr_opt: Option<IMFDXGIDeviceManager> = None;
            let _ = MFCreateDXGIDeviceManager(&mut reset_token, &mut dev_mgr_opt);
            if let Some(ref mgr) = dev_mgr_opt {
                let _ = mgr.ResetDevice(&device, reset_token);
            }

            // Tier 1: DXGI Desktop Duplication (Output5 DuplicateOutput1 -> Output1 DuplicateOutput)
            // with short startup retry window so pressing toggle mid-fullscreen/UAC never permanently
            // traps the session in GDI BitBlt (which sees through Flip-Model/MPO games like Valorant).
            let mut desk_dupl = try_create_output_duplication_with_retry(
                &device,
                &target_mon.device_name,
                target_mon.output_index,
                3,
                60,
            );
            // Tier 2: Windows.Graphics.Capture GPU monitor capture if DXGI is unavailable at startup.
            let mut wgc_cap: Option<WgcMonitorCapture> = if desk_dupl.is_none() {
                WgcMonitorCapture::new(&device, target_mon)
            } else {
                None
            };
            let mut wgc_has_frame = false;
            let mut wgc_started_at = Instant::now();
            let mut wgc_disabled_for_session = false;
            let mut dupl_lost_since: Option<Instant> = None;

            let tex_desc = D3D11_TEXTURE2D_DESC {
                Width: cap_width,
                Height: cap_height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };

            let mut staging_tex: Option<ID3D11Texture2D> = None;
            let _ = device.CreateTexture2D(&tex_desc, None, Some(&mut staging_tex));
            let mut staging_w = cap_width;
            let mut staging_h = cap_height;
            let mut need_ring_clear = false;

            if let Some(ref mut wgc) = wgc_cap {
                for _ in 0..12 {
                    if wgc.acquire_latest_frame(
                        &device,
                        &context,
                        &mut staging_tex,
                        &mut staging_w,
                        &mut staging_h,
                        &tex_desc,
                        cap_width,
                        cap_height,
                        &mut need_ring_clear,
                    ) {
                        wgc_has_frame = true;
                        break;
                    }
                    thread::sleep(Duration::from_millis(15));
                }
            }

            let safe_out_w = (out_width & !1).max(640);
            let safe_out_h = (out_height & !1).max(360);

            let want_audio = cfg_init.record_desktop_audio || cfg_init.record_microphone;
            let cfg_mic_id = cfg_init.effective_mic_device_id().map(str::to_string);
            let audio_monitor = AudioEndpointMonitor::new();
            let mut desktop_audio = WasapiStream::new();
            let mut mic_audio = WasapiStream::new();
            let use_active_game_audio = cfg_init.record_desktop_audio
                && cfg_init.desktop_audio_mode == DesktopAudioMode::ActiveGameOnly;
            let mut locked_game_audio: Option<(u32, String)> = None;
            let mut process_loopback_fallback_all = false;
            let mut last_desk_retry = Instant::now() - Duration::from_secs(10);

            if cfg_init.record_desktop_audio {
                if use_active_game_audio {
                    engine.sync_active_game_audio(
                        &mut desktop_audio,
                        &mut locked_game_audio,
                        &mut process_loopback_fallback_all,
                        &cfg_init.game_name_map,
                        false,
                        &mut last_desk_retry,
                        false,
                    );
                } else {
                    desktop_audio.initialize_with_device(true, None);
                    engine.set_desktop_audio_label("Semua Audio Desktop".to_string());
                }
            } else {
                engine.set_desktop_audio_label(String::new());
            }
            if cfg_init.record_microphone || cfg_init.voice_trigger_enabled {
                mic_audio.initialize_with_device(false, cfg_mic_id.as_deref());
                engine.sync_active_mic_name(&mic_audio);
                if cfg_init.voice_trigger_enabled && mic_audio.is_available() {
                    mic_audio.set_voice_tap_enabled(true);
                }
            } else if let Ok(mut g) = engine.active_mic_name.lock() {
                g.clear();
            }
            // Keep audio track active when user enabled audio recording so plugging in or switching
            // speakers/headphones mid-session resumes sound seamlessly on the same AAC track.
            let audio_active = want_audio;

            // Try ultra-low-VRAM GPU NV12 pipeline first (1 BGRA comp canvas + 4 compact NV12 targets + D3D11 VideoProcessor).
            const RING_SLOTS: usize = 4;
            let mut gpu_tex_ring: Vec<ID3D11Texture2D> = Vec::new();
            let mut gpu_rtv_ring: Vec<ID3D11RenderTargetView> = Vec::new();
            let mut dxgi_buf_ring: Vec<IMFMediaBuffer> = Vec::with_capacity(RING_SLOTS);
            let mut comp_bgra_tex: Option<ID3D11Texture2D> = None;
            let mut comp_bgra_rtv: Option<ID3D11RenderTargetView> = None;
            let mut nv12_tex_ring: Vec<ID3D11Texture2D> = Vec::new();
            let mut nv12_conv: Option<GpuNv12Converter> = None;
            let mut use_nv12 = false;

            let mut active_writer: Option<ActiveSegmentWriter> = None;

            {
                let mut ctex_opt: Option<ID3D11Texture2D> = None;
                let _ = device.CreateTexture2D(&tex_desc, None, Some(&mut ctex_opt));
                let crtv_opt = ctex_opt.as_ref().and_then(|ct| {
                    let mut rtv: Option<ID3D11RenderTargetView> = None;
                    let _ = device.CreateRenderTargetView(ct, None, Some(&mut rtv));
                    rtv
                });

                if let (Some(ctex), Some(crtv)) = (ctex_opt, crtv_opt) {
                    let nv12_desc = D3D11_TEXTURE2D_DESC {
                        Width: safe_out_w,
                        Height: safe_out_h,
                        MipLevels: 1,
                        ArraySize: 1,
                        Format: DXGI_FORMAT_NV12,
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: 1,
                            Quality: 0,
                        },
                        Usage: D3D11_USAGE_DEFAULT,
                        BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
                        CPUAccessFlags: 0,
                        MiscFlags: 0,
                    };
                    let mut tmp_nv12_tex = Vec::with_capacity(RING_SLOTS);
                    let mut tmp_nv12_buf = Vec::with_capacity(RING_SLOTS);
                    for _ in 0..RING_SLOTS {
                        let mut nt: Option<ID3D11Texture2D> = None;
                        let _ = device.CreateTexture2D(&nv12_desc, None, Some(&mut nt));
                        if let Some(ntex) = nt {
                            if let Ok(buf) =
                                MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, &ntex, 0, false)
                            {
                                let len = buf
                                    .GetMaxLength()
                                    .unwrap_or((safe_out_w * safe_out_h * 3) / 2);
                                let _ = buf.SetCurrentLength(len);
                                tmp_nv12_tex.push(ntex);
                                tmp_nv12_buf.push(buf);
                            }
                        }
                    }

                    if tmp_nv12_tex.len() == RING_SLOTS {
                        if let Some(conv) = GpuNv12Converter::new(
                            &device,
                            &context,
                            &ctex,
                            &tmp_nv12_tex,
                            cap_width,
                            cap_height,
                            safe_out_w,
                            safe_out_h,
                            fps,
                        ) {
                            if let Some(w_nv12) = create_segment_writer(
                                dev_mgr_opt.as_ref(),
                                safe_out_w,
                                safe_out_h,
                                safe_out_w,
                                safe_out_h,
                                fps,
                                bitrate_bps,
                                rate_control,
                                audio_active,
                                storage_mode,
                                &temp_dir,
                                codec,
                                true,
                                true,
                            ) {
                                comp_bgra_tex = Some(ctex);
                                comp_bgra_rtv = Some(crtv);
                                nv12_tex_ring = tmp_nv12_tex;
                                dxgi_buf_ring = tmp_nv12_buf;
                                nv12_conv = Some(conv);
                                active_writer = Some(w_nv12);
                                use_nv12 = true;
                            }
                        }
                    }
                }
            }

            if !use_nv12 {
                // Fallback path: 4-slot BGRA ring + Media Foundation ARGB32 input (still BT.709 + ICodecAPI tuned).
                gpu_tex_ring.reserve(RING_SLOTS);
                gpu_rtv_ring.reserve(RING_SLOTS);
                dxgi_buf_ring.clear();
                for _ in 0..RING_SLOTS {
                    let mut t: Option<ID3D11Texture2D> = None;
                    let _ = device.CreateTexture2D(&tex_desc, None, Some(&mut t));
                    if let Some(tex) = t {
                        let mut rtv: Option<ID3D11RenderTargetView> = None;
                        let _ = device.CreateRenderTargetView(&tex, None, Some(&mut rtv));
                        if let (Some(rtv), Ok(buf)) = (
                            rtv,
                            MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, &tex, 0, false),
                        ) {
                            let _ = buf.SetCurrentLength(cap_width * cap_height * 4);
                            gpu_tex_ring.push(tex);
                            gpu_rtv_ring.push(rtv);
                            dxgi_buf_ring.push(buf);
                        }
                    }
                }
                if gpu_tex_ring.len() < RING_SLOTS {
                    return CaptureSessionOutcome::Fatal("Gagal membuat GPU texture ring.");
                }
                active_writer = create_segment_writer(
                    dev_mgr_opt.as_ref(),
                    cap_width,
                    cap_height,
                    safe_out_w,
                    safe_out_h,
                    fps,
                    bitrate_bps,
                    rate_control,
                    audio_active,
                    storage_mode,
                    &temp_dir,
                    codec,
                    false,
                    true,
                );
            }

            let Some(mut first_writer) = active_writer else {
                return CaptureSessionOutcome::Fatal("Gagal menginisialisasi hardware video encoder.");
            };
            first_writer.seq_id = *next_seq_id;
            *next_seq_id += 1;
            let active_codec = first_writer.actual_codec;
            let rc_tuned = first_writer.rc_tuned;
            let writer_in_w = if use_nv12 { safe_out_w } else { cap_width };
            let writer_in_h = if use_nv12 { safe_out_h } else { cap_height };
            let mut active_writer = Some(first_writer);

            let mut cursor_comp = crate::cursor::DxgiCursorCompositor::new(&device);
            let _keep_nv12_ring = nv12_tex_ring;

            let standby_slot: Arc<Mutex<(bool, Option<ActiveSegmentWriter>)>> =
                Arc::new(Mutex::new((false, None)));
            let mut prewarm_handle: Option<JoinHandle<()>> = None;
            let mut prewarm_attempted = false;
            let mut finalizer_handles: Vec<JoinHandle<()>> = Vec::new();
            let mut outcome = CaptureSessionOutcome::Stopped;

            if let Ok(mut g) = engine.gpu_name.lock() {
                *g = format!("{} ({})", target_mon.adapter_name, target_mon.vendor_tag);
            }
            let codec_label = match active_codec {
                VideoCodecType::HardwareAV1 => "AV1",
                VideoCodecType::HardwareHEVC => "HEVC",
                VideoCodecType::HardwareH264 => "H.264 High",
            };
            let set_cap_status = |tag: &str| {
                if let Ok(mut s) = engine.capture_mode_status.lock() {
                    *s = format!(
                        "{}{}{}",
                        tag,
                        if use_nv12 { " NV12 + " } else { " Direct + " },
                        target_mon.vendor_tag
                    );
                }
                engine.notify_ui_status();
            };
            set_cap_status(if desk_dupl.is_some() {
                "DXGI"
            } else if wgc_cap.is_some() {
                "WGC GPU"
            } else {
                "GDI Fallback"
            });
            let rc_label = if rc_tuned {
                match rate_control {
                    RateControlMode::Cbr => "CBR+GOP",
                    RateControlMode::Vbr => "Peak-VBR+GOP",
                }
            } else {
                "Standard RC"
            };
            if let Ok(mut p) = engine.video_pipeline_status.lock() {
                *p = format!(
                    "{} BT.709 | {} | {}",
                    if use_nv12 { "NV12 GPU" } else { "ARGB32" },
                    rc_label,
                    codec_label
                );
            }

            let mut gdi_state = GdiFallbackState::default();
            let mut need_size_check = true;
            let mut last_dupl_retry = Instant::now();
            let mut last_valid_ring_slot: Option<usize> = None;
            let mut render_seq: usize = 0;

            let frame_dur_hns: i64 = 10_000_000 / fps as i64;
            let frame_interval_ms: f64 = 1000.0 / fps as f64;
            let pacing_timer = HighResWaitableTimer::new();
            let loop_clock = Instant::now();
            let mut fps_timer = Instant::now();
            let mut seg_audio_clock = Instant::now();
            let mut fps_counter: u32 = 0;
            let mut next_frame_tick: u64 = 0;

            const AUDIO_CHUNK_FRAMES: usize = 960;
            const AUDIO_CHUNK_FLOATS: usize = AUDIO_CHUNK_FRAMES * 2;
            let mut mixed_floats = vec![0.0f32; AUDIO_CHUNK_FLOATS];
            let mut mixed_bytes = vec![0u8; AUDIO_CHUNK_FLOATS * 2];
            let mut voice_mono_buf: Vec<f32> = Vec::with_capacity(2048);
            let mut last_game_audio_check = Instant::now();
            let mut last_mic_retry = Instant::now() - Duration::from_secs(10);

            while engine.is_running.load(Ordering::Acquire) {
                let target_ms = next_frame_tick as f64 * frame_interval_ms;
                let now_ms = loop_clock.elapsed().as_secs_f64() * 1000.0;
                if now_ms < target_ms {
                    pacing_timer.wait_ms(target_ms - now_ms);
                    next_frame_tick += 1;
                } else if now_ms > target_ms + 250.0 {
                    // Resync frame pacing after disk stalls, sleep/wake, or game load screens
                    // so we don't burst-encode hundreds of duplicate frames.
                    next_frame_tick = (now_ms / frame_interval_ms).floor() as u64 + 1;
                } else {
                    next_frame_tick += 1;
                }

                // Pre-warm next standby segment writer once, 1 second before rotation
                let aw_frames = active_writer.as_ref().map(|w| w.frames_written).unwrap_or(0);
                if !prewarm_attempted && aw_frames + fps >= frames_per_segment {
                    prewarm_attempted = true;
                    let mut st_guard = standby_slot.lock().unwrap();
                    if !st_guard.0 && st_guard.1.is_none() {
                        st_guard.0 = true;
                        let slot_clone = Arc::clone(&standby_slot);
                        let mgr_wrapped = SendDevMgr(dev_mgr_opt.clone());
                        let temp_dir_pw = temp_dir.clone();
                        if let Some(prev) = prewarm_handle.take() {
                            if prev.is_finished() {
                                let _ = prev.join();
                            } else {
                                finalizer_handles.push(prev);
                            }
                        }
                        prewarm_handle = thread::Builder::new()
                            .name("NVLiteReplayRust_Prewarm".to_string())
                            .spawn(move || {
                                let _mf_guard = ComMfGuard::new();
                                let mgr_wrapped = mgr_wrapped;
                                let built = create_segment_writer(
                                    mgr_wrapped.0.as_ref(),
                                    writer_in_w,
                                    writer_in_h,
                                    safe_out_w,
                                    safe_out_h,
                                    fps,
                                    bitrate_bps,
                                    rate_control,
                                    audio_active,
                                    storage_mode,
                                    &temp_dir_pw,
                                    active_codec,
                                    use_nv12,
                                    true,
                                );
                                if let Ok(mut g) = slot_clone.lock() {
                                    if let Some(old_w) = std::mem::replace(&mut g.1, built) {
                                        discard_segment_writer(old_w);
                                    }
                                    g.0 = false;
                                } else if let Some(w) = built {
                                    discard_segment_writer(w);
                                }
                            })
                            .ok();
                    }
                }

                // 1. Capture desktop/game frame (3-tier self-healing GPU pipeline: DXGI -> WGC -> GDI)
                let mut captured_via_gpu = false;
                let mut composite_dxgi_cursor = false;
                let mut gpu_screen_updated = false;
                let mut dupl_mouse_updated = false;

                if desk_dupl.is_none() && wgc_cap.is_none() {
                    if let Some(lost_at) = dupl_lost_since {
                        // Brief DXGI recovery window after ACCESS_LOST (fullscreen/Alt-Tab/UAC).
                        // Hold last GPU frame during transition so GDI never flashes Desktop behind game.
                        captured_via_gpu = true;
                        composite_dxgi_cursor = true;
                        if last_dupl_retry.elapsed() >= Duration::from_millis(250) {
                            last_dupl_retry = Instant::now();
                            desk_dupl = try_create_output_duplication(
                                &device,
                                &target_mon.device_name,
                                target_mon.output_index,
                            );
                            if desk_dupl.is_some() {
                                dupl_lost_since = None;
                                need_size_check = true;
                                last_valid_ring_slot = None;
                                if let Some(ref mut comp) = cursor_comp {
                                    comp.on_duplication_reset();
                                }
                                set_cap_status("DXGI");
                            } else if lost_at.elapsed() >= Duration::from_millis(1500) {
                                if device.GetDeviceRemovedReason().is_err() {
                                    outcome = CaptureSessionOutcome::NeedsRestart;
                                    break;
                                }
                                dupl_lost_since = None;
                                if !wgc_disabled_for_session {
                                    if let Some(wgc) = WgcMonitorCapture::new(&device, target_mon) {
                                        wgc_cap = Some(wgc);
                                        wgc_has_frame = false;
                                        wgc_started_at = Instant::now();
                                        last_valid_ring_slot = None;
                                        set_cap_status("WGC GPU");
                                    } else {
                                        captured_via_gpu = false;
                                        set_cap_status("GDI Fallback");
                                    }
                                } else {
                                    captured_via_gpu = false;
                                    set_cap_status("GDI Fallback");
                                }
                            }
                        }
                    } else if last_dupl_retry.elapsed() >= Duration::from_secs(1) {
                        // Never stay permanently trapped in GDI BitBlt: periodically upgrade back to
                        // DXGI (or WGC GPU if not already disabled) as soon as desktop allows it.
                        last_dupl_retry = Instant::now();
                        desk_dupl = try_create_output_duplication(
                            &device,
                            &target_mon.device_name,
                            target_mon.output_index,
                        );
                        if desk_dupl.is_some() {
                            need_size_check = true;
                            last_valid_ring_slot = None;
                            if let Some(ref mut comp) = cursor_comp {
                                comp.on_duplication_reset();
                            }
                            set_cap_status("DXGI");
                        } else if !wgc_disabled_for_session {
                            if let Some(wgc) = WgcMonitorCapture::new(&device, target_mon) {
                                wgc_cap = Some(wgc);
                                wgc_has_frame = false;
                                wgc_started_at = Instant::now();
                                last_valid_ring_slot = None;
                                set_cap_status("WGC GPU");
                            }
                        }
                    }
                }

                if let Some(ref dupl) = desk_dupl {
                    let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
                    let mut desk_res = None;
                    let acq_res = dupl.AcquireNextFrame(0, &mut frame_info, &mut desk_res);
                    match acq_res {
                        Ok(()) => {
                            if frame_info.LastMouseUpdateTime != 0 {
                                dupl_mouse_updated = true;
                                if let Some(ref mut comp) = cursor_comp {
                                    comp.update_from_dxgi(dupl, &frame_info, &device, &context);
                                }
                            }
                            if frame_info.LastPresentTime > 0 {
                                if let Some(res) = desk_res {
                                    if let Ok(dupl_tex) = res.cast::<ID3D11Texture2D>() {
                                        if need_size_check {
                                            need_size_check = false;
                                            let mut dupl_desc = D3D11_TEXTURE2D_DESC::default();
                                            dupl_tex.GetDesc(&mut dupl_desc);
                                            if dupl_desc.Width > 0
                                                && dupl_desc.Height > 0
                                                && (dupl_desc.Width != staging_w
                                                    || dupl_desc.Height != staging_h)
                                            {
                                                staging_w = dupl_desc.Width;
                                                staging_h = dupl_desc.Height;
                                                let mut new_desc = tex_desc;
                                                new_desc.Width = staging_w;
                                                new_desc.Height = staging_h;
                                                staging_tex = None;
                                                let _ = device.CreateTexture2D(
                                                    &new_desc,
                                                    None,
                                                    Some(&mut staging_tex),
                                                );
                                                need_ring_clear =
                                                    staging_w != cap_width || staging_h != cap_height;
                                            }
                                        }
                                        if let Some(ref stex) = staging_tex {
                                            context.CopyResource(stex, &dupl_tex);
                                            gpu_screen_updated = true;
                                        }
                                    }
                                }
                            }
                            let _ = dupl.ReleaseFrame();
                            if gpu_screen_updated || dupl_mouse_updated {
                                context.Flush();
                            }
                            captured_via_gpu = true;
                            composite_dxgi_cursor = true;
                        }
                        Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => {
                            captured_via_gpu = true;
                            composite_dxgi_cursor = true;
                        }
                        Err(e) if e.code() == DXGI_ERROR_ACCESS_LOST || e.code().is_err() => {
                            drop(desk_dupl.take());
                            last_dupl_retry = Instant::now();
                            desk_dupl = try_create_output_duplication(
                                &device,
                                &target_mon.device_name,
                                target_mon.output_index,
                            );
                            if desk_dupl.is_some() {
                                dupl_lost_since = None;
                                need_size_check = true;
                                last_valid_ring_slot = None;
                                if let Some(ref mut comp) = cursor_comp {
                                    comp.on_duplication_reset();
                                }
                            } else {
                                dupl_lost_since = Some(Instant::now());
                            }
                            captured_via_gpu = true;
                            composite_dxgi_cursor = true;
                        }
                        Err(_) => {}
                    }
                } else if let Some(ref mut wgc) = wgc_cap {
                    if wgc.acquire_latest_frame(
                        &device,
                        &context,
                        &mut staging_tex,
                        &mut staging_w,
                        &mut staging_h,
                        &tex_desc,
                        cap_width,
                        cap_height,
                        &mut need_ring_clear,
                    ) {
                        gpu_screen_updated = true;
                        wgc_has_frame = true;
                    }
                    if wgc_has_frame {
                        captured_via_gpu = true;
                        composite_dxgi_cursor = false;
                    } else if wgc_started_at.elapsed() >= Duration::from_millis(1500) {
                        // Isolated/headless desktop where WGC never produces frames: release WGC
                        // once for this session and fall back to GDI while continuing DXGI retries.
                        drop(wgc_cap.take());
                        wgc_disabled_for_session = true;
                        last_dupl_retry = Instant::now();
                        set_cap_status("GDI Fallback");
                    }
                }

                // Reuse the previous NV12/BGRA ring slot when neither desktop pixels nor cursor
                // changed (`LastPresentTime == 0 && LastMouseUpdateTime == 0`).
                // Encoder still receives a sample every tick (preserving CFR), while GPU skips
                // CopyResource, cursor composite shader, and VideoProcessorBlt BGRA -> NV12.
                let need_render_and_convert = !captured_via_gpu
                    || last_valid_ring_slot.is_none()
                    || need_ring_clear
                    || gpu_screen_updated
                    || dupl_mouse_updated;

                let ring_slot = if need_render_and_convert {
                    let slot = render_seq & (RING_SLOTS - 1);
                    render_seq = render_seq.wrapping_add(1);
                    let (dst_bgra_tex, dst_bgra_rtv) = if use_nv12 {
                        (
                            comp_bgra_tex.as_ref().unwrap(),
                            comp_bgra_rtv.as_ref().unwrap(),
                        )
                    } else {
                        (&gpu_tex_ring[slot], &gpu_rtv_ring[slot])
                    };

                    if captured_via_gpu {
                        if need_ring_clear {
                            need_ring_clear = false;
                            let clear_black = [0.0f32, 0.0, 0.0, 1.0];
                            if use_nv12 {
                                context.ClearRenderTargetView(dst_bgra_rtv, &clear_black);
                            } else {
                                for rtv in &gpu_rtv_ring {
                                    context.ClearRenderTargetView(rtv, &clear_black);
                                }
                            }
                        }

                        let mut cursor_off_x = 0i32;
                        let mut cursor_off_y = 0i32;
                        let mut copied_frame = false;
                        if let Some(ref stex) = staging_tex {
                            copied_frame = true;
                            if staging_w == cap_width && staging_h == cap_height {
                                context.CopyResource(dst_bgra_tex, stex);
                            } else {
                                let copy_w = staging_w.min(cap_width);
                                let copy_h = staging_h.min(cap_height);
                                let src_x = if staging_w > cap_width {
                                    (staging_w - cap_width) / 2
                                } else {
                                    0
                                };
                                let src_y = if staging_h > cap_height {
                                    (staging_h - cap_height) / 2
                                } else {
                                    0
                                };
                                let dst_x = if cap_width > staging_w {
                                    (cap_width - staging_w) / 2
                                } else {
                                    0
                                };
                                let dst_y = if cap_height > staging_h {
                                    (cap_height - staging_h) / 2
                                } else {
                                    0
                                };
                                cursor_off_x = dst_x as i32 - src_x as i32;
                                cursor_off_y = dst_y as i32 - src_y as i32;
                                let src_box = D3D11_BOX {
                                    left: src_x,
                                    top: src_y,
                                    front: 0,
                                    right: src_x + copy_w,
                                    bottom: src_y + copy_h,
                                    back: 1,
                                };
                                context.CopySubresourceRegion(
                                    dst_bgra_tex,
                                    0,
                                    dst_x,
                                    dst_y,
                                    0,
                                    stex,
                                    0,
                                    Some(&src_box),
                                );
                            }
                        }
                        if copied_frame && composite_dxgi_cursor {
                            if let Some(ref mut comp) = cursor_comp {
                                comp.composite_onto_target(
                                    &device,
                                    &context,
                                    dst_bgra_tex,
                                    dst_bgra_rtv,
                                    cap_width,
                                    cap_height,
                                    cap_left,
                                    cap_top,
                                    cursor_off_x,
                                    cursor_off_y,
                                );
                            }
                        }
                    } else {
                        gdi_state.ensure_init(cap_width, cap_height);
                        if !gdi_state.dib_bits.is_null() {
                            let _ = BitBlt(
                                gdi_state.mem_dc,
                                0,
                                0,
                                cap_width as i32,
                                cap_height as i32,
                                gdi_state.screen_dc,
                                cap_left,
                                cap_top,
                                SRCCOPY,
                            );
                            crate::cursor::draw_cursor_on_gdi_dc(
                                gdi_state.mem_dc,
                                cap_left,
                                cap_top,
                                cap_width as i32,
                                cap_height as i32,
                            );
                            context.UpdateSubresource(
                                dst_bgra_tex,
                                0,
                                None,
                                gdi_state.dib_bits,
                                cap_width * 4,
                                0,
                            );
                        }
                    }

                    if let Some(ref conv) = nv12_conv {
                        let _ = conv.convert_slot(slot, render_seq as u32);
                    }
                    last_valid_ring_slot = Some(slot);
                    slot
                } else {
                    last_valid_ring_slot.unwrap_or(0)
                };

                let Some(aw) = active_writer.as_mut() else {
                    outcome = CaptureSessionOutcome::NeedsRestart;
                    break;
                };

                // 2. Write Video Sample (track consecutive errors so broken GPU/disk streams self-heal)
                if let Ok(sample) = MFCreateSample() {
                    let sample_time = aw.frames_written as i64 * frame_dur_hns;
                    let _ = sample.SetSampleTime(sample_time);
                    let _ = sample.SetSampleDuration(frame_dur_hns);
                    let _ = sample.AddBuffer(&dxgi_buf_ring[ring_slot]);
                    match aw
                        .sink_writer
                        .WriteSample(aw.video_stream_index, &sample)
                    {
                        Ok(()) => aw.write_errors = 0,
                        Err(_) => aw.write_errors = aw.write_errors.saturating_add(1),
                    }
                } else {
                    aw.write_errors = aw.write_errors.saturating_add(1);
                }

                // 3. Drain & Write Audio Chunks + Voice Recognition Tap (with zero-poll endpoint hot-swap)
                let (live_voice_enabled, live_voice_sens, live_voice_cd) = {
                    let live_cfg = engine.config.lock().unwrap();
                    (
                        live_cfg.voice_trigger_enabled,
                        live_cfg.voice_sensitivity,
                        live_cfg.voice_cooldown_seconds,
                    )
                };
                let voice_enrolling = engine.voice.is_enrolling();
                let voice_want_mic = live_voice_enabled || voice_enrolling;

                let render_dirty = audio_monitor.take_render_dirty();
                if cfg_init.record_desktop_audio {
                    if use_active_game_audio && !process_loopback_fallback_all {
                        let desk_invalidated = desktop_audio.take_invalidated();
                        if desk_invalidated
                            || render_dirty
                            || last_game_audio_check.elapsed() >= Duration::from_secs(1)
                        {
                            last_game_audio_check = Instant::now();
                            engine.sync_active_game_audio(
                                &mut desktop_audio,
                                &mut locked_game_audio,
                                &mut process_loopback_fallback_all,
                                &cfg_init.game_name_map,
                                render_dirty,
                                &mut last_desk_retry,
                                desk_invalidated,
                            );
                        }
                    } else {
                        Self::sync_audio_endpoint(
                            &mut desktop_audio,
                            true,
                            None,
                            render_dirty,
                            Duration::from_millis(400),
                            &mut last_desk_retry,
                        );
                    }
                }

                let cap_dirty = audio_monitor.take_capture_dirty();
                if cap_dirty {
                    engine.refresh_audio_inputs();
                    engine.notify_ui_status();
                }
                if cfg_init.record_microphone || voice_want_mic {
                    if Self::sync_audio_endpoint(
                        &mut mic_audio,
                        false,
                        cfg_mic_id.as_deref(),
                        cap_dirty,
                        Duration::from_secs(1),
                        &mut last_mic_retry,
                    ) {
                        engine.sync_active_mic_name(&mic_audio);
                        engine.notify_ui_status();
                    }
                }
                mic_audio.set_voice_tap_enabled(voice_want_mic && mic_audio.is_available());

                if aw.has_audio {
                    if desktop_audio.is_available() {
                        desktop_audio.drain_packets();
                    }
                    if mic_audio.is_available() {
                        mic_audio.drain_packets();
                    }

                    let (desk_gain, mic_gain) = {
                        let live_cfg = engine.config.lock().unwrap();
                        (
                            if live_cfg.record_desktop_audio {
                                live_cfg.desktop_volume_percent as f32 / 100.0
                            } else {
                                0.0
                            },
                            if live_cfg.record_microphone && !live_cfg.mic_muted {
                                live_cfg.mic_volume_percent as f32 / 100.0
                            } else {
                                0.0
                            },
                        )
                    };

                    // Guard against clock jump after sleep/stall and cap per-frame iterations so
                    // audio writing can never hang `capture_loop` even if Media Foundation errors.
                    let expected_now =
                        (seg_audio_clock.elapsed().as_secs_f64() * 48_000.0) as i64;
                    if expected_now - aw.audio_samples_written > 48_000 {
                        aw.audio_samples_written = expected_now - AUDIO_CHUNK_FRAMES as i64;
                    }
                    for _ in 0..16 {
                        let expected_audio_frames =
                            (seg_audio_clock.elapsed().as_secs_f64() * 48_000.0) as i64;
                        if expected_audio_frames - aw.audio_samples_written
                            < AUDIO_CHUNK_FRAMES as i64
                        {
                            break;
                        }

                        mixed_floats.fill(0.0);
                        if desktop_audio.is_available() {
                            desktop_audio.mix_from_fifo(
                                &mut mixed_floats,
                                AUDIO_CHUNK_FRAMES,
                                desk_gain,
                            );
                        }
                        if mic_audio.is_available() {
                            mic_audio.mix_from_fifo(&mut mixed_floats, AUDIO_CHUNK_FRAMES, mic_gain);
                        }

                        for (idx, &f) in mixed_floats.iter().enumerate() {
                            let s16 = (f.clamp(-1.0, 1.0) * 32767.0) as i16;
                            let bytes = s16.to_le_bytes();
                            mixed_bytes[idx * 2] = bytes[0];
                            mixed_bytes[idx * 2 + 1] = bytes[1];
                        }

                        let atime = (aw.audio_samples_written * 10_000_000) / 48_000;
                        let adur = (AUDIO_CHUNK_FRAMES as i64 * 10_000_000) / 48_000;
                        aw.audio_samples_written += AUDIO_CHUNK_FRAMES as i64;

                        if let Ok(abuf) = MFCreateMemoryBuffer(mixed_bytes.len() as u32) {
                            let mut dst_ptr = null_mut();
                            if abuf.Lock(&mut dst_ptr, None, None).is_ok() && !dst_ptr.is_null() {
                                std::ptr::copy_nonoverlapping(
                                    mixed_bytes.as_ptr(),
                                    dst_ptr,
                                    mixed_bytes.len(),
                                );
                                let _ = abuf.Unlock();
                                let _ = abuf.SetCurrentLength(mixed_bytes.len() as u32);

                                if let Ok(asample) = MFCreateSample() {
                                    let _ = asample.SetSampleTime(atime);
                                    let _ = asample.SetSampleDuration(adur);
                                    let _ = asample.AddBuffer(&abuf);
                                    if aw
                                        .sink_writer
                                        .WriteSample(aw.audio_stream_index, &asample)
                                        .is_err()
                                    {
                                        aw.write_errors = aw.write_errors.saturating_add(1);
                                    }
                                }
                            }
                        }
                    }
                } else if voice_want_mic && mic_audio.is_available() {
                    mic_audio.drain_packets();
                }

                if voice_want_mic && mic_audio.is_available() {
                    mic_audio.drain_voice_tap_16k(&mut voice_mono_buf);
                    if !voice_mono_buf.is_empty()
                        && engine.voice.process_16k_chunk(
                            &voice_mono_buf,
                            live_voice_sens,
                            live_voice_cd,
                            live_voice_enabled,
                        )
                    {
                        let raw_hwnd = engine.ui_hwnd.lock().map(|g| *g).unwrap_or(0);
                        if raw_hwnd != 0 {
                            let _ = PostMessageW(
                                HWND(raw_hwnd as *mut _),
                                WM_APP_VOICE_TRIGGER,
                                WPARAM(0),
                                LPARAM(0),
                            );
                        }
                        engine.save_replay_async();
                    }
                }

                aw.frames_written += 1;
                let cur_aw_frames = aw.frames_written;
                fps_counter += 1;
                let fps_elapsed = fps_timer.elapsed().as_secs_f64();
                if fps_elapsed >= 1.0 {
                    let cur = ((fps_counter as f64 / fps_elapsed) * 10.0).round() as u64;
                    engine.current_fps_x10.store(cur, Ordering::Relaxed);
                    fps_counter = 0;
                    fps_timer = Instant::now();
                    let _ = engine.refresh_foreground_game(&cfg_init.game_name_map);
                }

                // 4. Segment Rotation + Self-Healing Error Detection
                let device_lost = aw.write_errors >= 5 && device.GetDeviceRemovedReason().is_err();
                let broken_pipeline = device_lost
                    || aw.write_errors >= 10
                    || engine.consecutive_seg_failures.load(Ordering::Acquire) >= 3;
                if broken_pipeline {
                    outcome = CaptureSessionOutcome::NeedsRestart;
                    break;
                }

                let forced = engine.force_rotate.load(Ordering::Acquire);
                if cur_aw_frames >= frames_per_segment || (forced && cur_aw_frames >= 10) {
                    let was_forced = engine.force_rotate.swap(false, Ordering::AcqRel);
                    let Some(finished) = active_writer.take() else {
                        outcome = CaptureSessionOutcome::NeedsRestart;
                        break;
                    };

                    // Always finish any in-flight prewarm before checking standby_slot so we never
                    // double-allocate encoder sessions or strand an open writer in standby_slot.
                    if let Some(h) = prewarm_handle.take() {
                        let _ = h.join();
                    }
                    let mut prewarmed = standby_slot.lock().ok().and_then(|mut g| g.1.take());
                    prewarm_attempted = false;

                    // Reap completed background finalizers and bound backlog on slow Disk Temp drives.
                    while let Some(pos) = finalizer_handles.iter().position(|h| h.is_finished()) {
                        let _ = finalizer_handles.remove(pos).join();
                    }
                    while finalizer_handles.len() >= 2 {
                        let _ = finalizer_handles.remove(0).join();
                    }

                    let next_w = if was_forced {
                        // Drain prior finalizers & finalize current segment FIRST so its encoder
                        // session and disk file handle are released before any fallback writer is
                        // created, and `save_replay_async` sees all segments up to this instant.
                        for h in finalizer_handles.drain(..) {
                            let _ = h.join();
                        }
                        let _ = engine.finalize_and_store_segment(finished, fps, storage_mode);
                        let (lock, cv) = &engine.rotate_notify;
                        if let Ok(mut done) = lock.lock() {
                            *done = true;
                            cv.notify_all();
                        }
                        prewarmed.take().or_else(|| {
                            try_create_segment_writer_with_retry(
                                dev_mgr_opt.as_ref(),
                                writer_in_w,
                                writer_in_h,
                                safe_out_w,
                                safe_out_h,
                                fps,
                                bitrate_bps,
                                rate_control,
                                audio_active,
                                storage_mode,
                                &temp_dir,
                                active_codec,
                                use_nv12,
                            )
                        })
                    } else if let Some(w) = prewarmed.take() {
                        // Fast zero-stall swap: finalize old segment asynchronously in background.
                        let fin_box = Arc::new(Mutex::new(Some(finished)));
                        let fin_bg = Arc::clone(&fin_box);
                        let eng_bg = Arc::clone(engine);
                        if let Ok(jh) = thread::Builder::new()
                            .name("NVLiteReplayRust_FinalizeSeg".to_string())
                            .spawn(move || {
                                let _mf_guard = ComMfGuard::new();
                                if let Some(fw) = fin_bg.lock().ok().and_then(|mut g| g.take()) {
                                    let _ = eng_bg.finalize_and_store_segment(fw, fps, storage_mode);
                                }
                            })
                        {
                            finalizer_handles.push(jh);
                        } else if let Some(fw) = fin_box.lock().ok().and_then(|mut g| g.take()) {
                            let _ = engine.finalize_and_store_segment(fw, fps, storage_mode);
                        }
                        Some(w)
                    } else {
                        // Standby writer unavailable: release `finished` + backlog FIRST to free
                        // encoder sessions & disk handles, then retry creating the next writer.
                        for h in finalizer_handles.drain(..) {
                            let _ = h.join();
                        }
                        let _ = engine.finalize_and_store_segment(finished, fps, storage_mode);
                        try_create_segment_writer_with_retry(
                            dev_mgr_opt.as_ref(),
                            writer_in_w,
                            writer_in_h,
                            safe_out_w,
                            safe_out_h,
                            fps,
                            bitrate_bps,
                            rate_control,
                            audio_active,
                            storage_mode,
                            &temp_dir,
                            active_codec,
                            use_nv12,
                        )
                    };

                    let Some(mut next_w) = next_w else {
                        outcome = CaptureSessionOutcome::NeedsRestart;
                        break;
                    };
                    next_w.seq_id = *next_seq_id;
                    *next_seq_id += 1;
                    active_writer = Some(next_w);
                    seg_audio_clock = Instant::now();
                }
            }

            // Clean up in-flight writers, prewarm thread, and background finalizers BEFORE
            // tearing down D3D11/Media Foundation/COM!
            {
                let (lock, cv) = &engine.rotate_notify;
                if let Ok(mut done) = lock.lock() {
                    *done = true;
                    cv.notify_all();
                }
            }
            if let Some(w) = active_writer.take() {
                discard_segment_writer(w);
            }
            if let Some(h) = prewarm_handle.take() {
                let _ = h.join();
            }
            if let Ok(mut g) = standby_slot.lock() {
                if let Some(w) = g.1.take() {
                    discard_segment_writer(w);
                }
            }
            for h in finalizer_handles.drain(..) {
                let _ = h.join();
            }
            drop(wgc_cap.take());
            drop(desk_dupl.take());
            outcome
        }
    }
}

fn discard_segment_writer(mut w: ActiveSegmentWriter) {
    let disk = w.disk_path.take();
    drop(w);
    if let Some(p) = disk {
        let _ = fs::remove_file(p);
    }
}

fn vendor_tag(vendor_id: u32) -> &'static str {
    match vendor_id {
        0x10DE => "NVENC",
        0x1002 | 0x1022 => "AMD AMF",
        0x8086 => "Intel QSV",
        _ => "Hardware MFT",
    }
}

fn utf16_slice_to_string(slice: &[u16]) -> String {
    let end = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
    String::from_utf16_lossy(&slice[..end]).trim().to_string()
}

unsafe fn query_display_refresh_rate(dev_name_utf16: &[u16]) -> u32 {
    let mut dm = DEVMODEW {
        dmSize: std::mem::size_of::<DEVMODEW>() as u16,
        ..Default::default()
    };
    if EnumDisplaySettingsW(PCWSTR(dev_name_utf16.as_ptr()), ENUM_CURRENT_SETTINGS, &mut dm).as_bool()
        && dm.dmDisplayFrequency > 1
    {
        return dm.dmDisplayFrequency;
    }
    60
}

unsafe fn enum_monitors_gdi(gpu_name: &str) -> Vec<DisplayTarget> {
    struct Ctx {
        gpu: String,
        items: Vec<DisplayTarget>,
    }
    unsafe extern "system" fn cb(
        hmon: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        let ctx = &mut *(data.0 as *mut Ctx);
        let mut mi = MONITORINFOEXW::default();
        mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(hmon, &mut mi.monitorInfo).as_bool() {
            let r = mi.monitorInfo.rcMonitor;
            let hz = query_display_refresh_rate(&mi.szDevice);
            ctx.items.push(DisplayTarget {
                index: ctx.items.len(),
                adapter_index: 0,
                output_index: ctx.items.len() as u32,
                device_name: utf16_slice_to_string(&mi.szDevice),
                adapter_name: ctx.gpu.clone(),
                vendor_tag: "Hardware".to_string(),
                left: r.left,
                top: r.top,
                width: (r.right - r.left).max(640) as u32,
                height: (r.bottom - r.top).max(360) as u32,
                refresh_rate_hz: hz,
                is_primary: (mi.monitorInfo.dwFlags & 1) != 0,
            });
        }
        BOOL(1)
    }

    let mut ctx = Ctx {
        gpu: gpu_name.to_string(),
        items: Vec::new(),
    };
    let _ = EnumDisplayMonitors(
        HDC::default(),
        None,
        Some(cb),
        LPARAM(&mut ctx as *mut _ as isize),
    );
    ctx.items
}

unsafe fn set_capture_thread_dpi_aware() {
    let _ = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
}

unsafe fn find_matching_dxgi_output(
    device: &ID3D11Device,
    target_device_name: &str,
    fallback_output_idx: u32,
) -> Option<IDXGIOutput> {
    let dxgi_dev: IDXGIDevice = device.cast().ok()?;
    let adap = dxgi_dev.GetAdapter().ok()?;
    if !target_device_name.is_empty() {
        let mut idx = 0u32;
        while let Ok(output) = adap.EnumOutputs(idx) {
            if let Ok(desc) = output.GetDesc() {
                if utf16_slice_to_string(&desc.DeviceName).eq_ignore_ascii_case(target_device_name)
                {
                    return Some(output);
                }
            }
            idx += 1;
        }
    }
    adap.EnumOutputs(fallback_output_idx).ok()
}

unsafe fn try_create_output_duplication(
    device: &ID3D11Device,
    target_device_name: &str,
    fallback_output_idx: u32,
) -> Option<IDXGIOutputDuplication> {
    let output = find_matching_dxgi_output(device, target_device_name, fallback_output_idx)?;
    if let Ok(out5) = output.cast::<IDXGIOutput5>() {
        let formats = [DXGI_FORMAT_B8G8R8A8_UNORM];
        if let Ok(dupl) = out5.DuplicateOutput1(device, 0, &formats) {
            return Some(dupl);
        }
    }
    let out1: IDXGIOutput1 = output.cast().ok()?;
    out1.DuplicateOutput(device).ok()
}

unsafe fn try_create_output_duplication_with_retry(
    device: &ID3D11Device,
    target_device_name: &str,
    fallback_output_idx: u32,
    attempts: u32,
    delay_ms: u64,
) -> Option<IDXGIOutputDuplication> {
    for attempt in 0..attempts.max(1) {
        if attempt > 0 {
            thread::sleep(Duration::from_millis(delay_ms));
        }
        if let Some(dupl) =
            try_create_output_duplication(device, target_device_name, fallback_output_idx)
        {
            return Some(dupl);
        }
    }
    None
}

unsafe fn resolve_target_hmonitor(
    device: &ID3D11Device,
    target_mon: &DisplayTarget,
) -> Option<HMONITOR> {
    if let Some(output) =
        find_matching_dxgi_output(device, &target_mon.device_name, target_mon.output_index)
    {
        if let Ok(desc) = output.GetDesc() {
            if !desc.Monitor.0.is_null() {
                return Some(desc.Monitor);
            }
        }
    }
    let pt = POINT {
        x: target_mon.left + (target_mon.width as i32 / 2),
        y: target_mon.top + (target_mon.height as i32 / 2),
    };
    let hmon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
    if hmon.0.is_null() {
        None
    } else {
        Some(hmon)
    }
}

/// Tier 2 GPU monitor capture via Windows.Graphics.Capture (Win10 1903+ / Win11).
/// Captures DirectX Flip-Model, Independent Flip, MPO, and fullscreen games (e.g. Valorant)
/// zero-copy on GPU even when DXGI Desktop Duplication is unavailable.
/// Caller thread must already have initialized WinRT/COM MTA via `ComMfGuard`.
struct WgcMonitorCapture {
    session: Option<GraphicsCaptureSession>,
    frame_pool: Option<Direct3D11CaptureFramePool>,
    _item: Option<GraphicsCaptureItem>,
    d3d_winrt_device: Option<IDirect3DDevice>,
    pool_w: i32,
    pool_h: i32,
}

impl WgcMonitorCapture {
    unsafe fn new(device: &ID3D11Device, target_mon: &DisplayTarget) -> Option<Self> {
        ensure_process_mta_pinned();
        if !GraphicsCaptureSession::IsSupported().unwrap_or(false) {
            return None;
        }
        let hmon = resolve_target_hmonitor(device, target_mon)?;
        let dxgi_dev: IDXGIDevice = device.cast().ok()?;
        let inspectable = CreateDirect3D11DeviceFromDXGIDevice(&dxgi_dev).ok()?;
        let d3d_winrt_device: IDirect3DDevice = inspectable.cast().ok()?;
        let class_name = HSTRING::from("Windows.Graphics.Capture.GraphicsCaptureItem");
        let interop: IGraphicsCaptureItemInterop = RoGetActivationFactory(&class_name).ok()?;
        let item: GraphicsCaptureItem = interop.CreateForMonitor(hmon).ok()?;
        let sz = item.Size().ok()?;
        if sz.Width <= 0 || sz.Height <= 0 {
            return None;
        }
        let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &d3d_winrt_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            sz,
        )
        .ok()?;
        let session = frame_pool.CreateCaptureSession(&item).ok()?;
        let _ = session.SetIsBorderRequired(false);
        let _ = session.SetIsCursorCaptureEnabled(true);
        session.StartCapture().ok()?;

        Some(Self {
            session: Some(session),
            frame_pool: Some(frame_pool),
            _item: Some(item),
            d3d_winrt_device: Some(d3d_winrt_device),
            pool_w: sz.Width,
            pool_h: sz.Height,
        })
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn acquire_latest_frame(
        &mut self,
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        staging_tex: &mut Option<ID3D11Texture2D>,
        staging_w: &mut u32,
        staging_h: &mut u32,
        tex_desc: &D3D11_TEXTURE2D_DESC,
        cap_width: u32,
        cap_height: u32,
        need_ring_clear: &mut bool,
    ) -> bool {
        let Some(ref frame_pool) = self.frame_pool else {
            return false;
        };
        let mut latest = None;
        while let Ok(frame) = frame_pool.TryGetNextFrame() {
            if let Some(prev) = latest.replace(frame) {
                let _ = prev.Close();
            }
        }
        let Some(frame) = latest else {
            return false;
        };

        if let Ok(csz) = frame.ContentSize() {
            if csz.Width > 0
                && csz.Height > 0
                && (csz.Width != self.pool_w || csz.Height != self.pool_h)
            {
                if let Some(ref d3d_dev) = self.d3d_winrt_device {
                    let _ = frame_pool.Recreate(
                        d3d_dev,
                        DirectXPixelFormat::B8G8R8A8UIntNormalized,
                        2,
                        csz,
                    );
                    self.pool_w = csz.Width;
                    self.pool_h = csz.Height;
                }
            }
        }

        let mut copied = false;
        if let Ok(surface) = frame.Surface() {
            if let Ok(access) = surface.cast::<IDirect3DDxgiInterfaceAccess>() {
                if let Ok(wgc_tex) = access.GetInterface::<ID3D11Texture2D>() {
                    let mut wgc_desc = D3D11_TEXTURE2D_DESC::default();
                    wgc_tex.GetDesc(&mut wgc_desc);
                    if wgc_desc.Width > 0
                        && wgc_desc.Height > 0
                        && wgc_desc.Format == DXGI_FORMAT_B8G8R8A8_UNORM
                    {
                        if staging_tex.is_none()
                            || wgc_desc.Width != *staging_w
                            || wgc_desc.Height != *staging_h
                        {
                            *staging_w = wgc_desc.Width;
                            *staging_h = wgc_desc.Height;
                            let mut new_desc = *tex_desc;
                            new_desc.Width = *staging_w;
                            new_desc.Height = *staging_h;
                            *staging_tex = None;
                            let _ = device.CreateTexture2D(&new_desc, None, Some(staging_tex));
                            *need_ring_clear = *staging_w != cap_width || *staging_h != cap_height;
                        }
                        if let Some(ref stex) = staging_tex {
                            context.CopyResource(stex, &wgc_tex);
                            context.Flush();
                            copied = true;
                        }
                    }
                }
            }
        }

        let _ = frame.Close();
        copied
    }
}

impl Drop for WgcMonitorCapture {
    fn drop(&mut self) {
        if let Some(s) = self.session.take() {
            let _ = s.Close();
            drop(s);
        }
        if let Some(fp) = self.frame_pool.take() {
            let _ = fp.Close();
            drop(fp);
        }
        self._item = None;
        self.d3d_winrt_device = None;
    }
}

pub fn probe_hardware_codec_caps() -> HardwareCodecCaps {
    unsafe {
        let _guard = ComMfGuard::new();
        HardwareCodecCaps {
            hevc_hw: has_hardware_video_encoder(MFVideoFormat_HEVC),
            av1_hw: has_hardware_video_encoder(MFVideoFormat_AV1),
        }
    }
}

unsafe fn has_hardware_video_encoder(subtype: GUID) -> bool {
    let out_info = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: subtype,
    };
    let mut activates: *mut Option<IMFActivate> = null_mut();
    let mut count: u32 = 0;
    let flags = MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER;
    if MFTEnumEx(
        MFT_CATEGORY_VIDEO_ENCODER,
        flags,
        None,
        Some(&out_info),
        &mut activates,
        &mut count,
    )
    .is_err()
    {
        return false;
    }
    let found = count > 0;
    if !activates.is_null() {
        for i in 0..count as usize {
            std::ptr::drop_in_place(activates.add(i));
        }
        CoTaskMemFree(Some(activates as *const _));
    }
    found
}

unsafe fn apply_bt709_color_attributes(mt: &IMFMediaType, studio_yuv: bool) {
    let _ = mt.SetUINT32(&MF_MT_VIDEO_PRIMARIES, MFVideoPrimaries_BT709.0 as u32);
    let _ = mt.SetUINT32(&MF_MT_TRANSFER_FUNCTION, MFVideoTransFunc_709.0 as u32);
    let _ = mt.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1u64);
    if studio_yuv {
        let _ = mt.SetUINT32(&MF_MT_YUV_MATRIX, MFVideoTransferMatrix_BT709.0 as u32);
        let _ = mt.SetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE, MFNominalRange_16_235.0 as u32);
    } else {
        let _ = mt.SetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE, MFNominalRange_0_255.0 as u32);
    }
}

/// Computes rate-control parameters for either Peak-Constrained VBR (calm scenes drop far
/// below target bitrate while heavy action stays bounded) or CBR (constant target bitrate).
/// Returns `(mean_bps, max_bps, vbv_buffer_bytes, gop_frames)`.
fn compute_rate_control_params(
    rate_control: RateControlMode,
    bitrate_bps: u32,
    fps: u32,
) -> (u32, u32, u32, u32) {
    let target_bps = bitrate_bps.clamp(2_000_000, 60_000_000);
    // Peak-VBR uses ~90% mean vs 100% peak to satisfy NVIDIA/AMD/Intel Peak-VBR (`max > mean`).
    // CBR locks both mean and VBV budget to the exact target bitrate (MaxBitRate is omitted on MFTs).
    let (mean_bps, max_bps) = match rate_control {
        RateControlMode::Cbr => (target_bps, target_bps),
        RateControlMode::Vbr => {
            let mean = ((target_bps as u64).saturating_mul(9) / 10).max(1_500_000) as u32;
            (mean, target_bps)
        }
    };
    // CODECAPI_AVEncCommonBufferSize is in BYTES (~0.5s VBV window prevents cold-start bursts).
    let vbv_buffer_bytes = (max_bps / 16).clamp(250_000, 4_000_000);
    let gop_frames = fps.saturating_mul(2).clamp(30, 240);
    (mean_bps, max_bps, vbv_buffer_bytes, gop_frames)
}

/// Builds `pEncodingParameters` passed directly to `IMFSinkWriter::SetInputMediaType`.
/// Hardware MFTs (notably NVIDIA NVENC) lock rate-control/GOP/VBV parameters inside
/// `SetInputMediaType` and silently ignore post-hoc `ICodecAPI::SetValue` calls.
unsafe fn build_encoder_attributes(
    codec: VideoCodecType,
    rate_control: RateControlMode,
    bitrate_bps: u32,
    fps: u32,
) -> Option<IMFAttributes> {
    let (mean_bps, max_bps, vbv_bytes, gop_frames) =
        compute_rate_control_params(rate_control, bitrate_bps, fps);
    let want_rc = match rate_control {
        RateControlMode::Cbr => eAVEncCommonRateControlMode_CBR.0 as u32,
        RateControlMode::Vbr => eAVEncCommonRateControlMode_PeakConstrainedVBR.0 as u32,
    };
    let mut attrs = None;
    MFCreateAttributes(&mut attrs, 8).ok()?;
    let attrs = attrs?;
    let _ = attrs.SetUINT32(&CODECAPI_AVEncCommonRateControlMode, want_rc);
    let _ = attrs.SetUINT32(&CODECAPI_AVEncCommonMeanBitRate, mean_bps);
    if rate_control == RateControlMode::Vbr {
        let _ = attrs.SetUINT32(&CODECAPI_AVEncCommonMaxBitRate, max_bps);
    }
    let _ = attrs.SetUINT32(&CODECAPI_AVEncCommonBufferSize, vbv_bytes);
    let _ = attrs.SetUINT32(&CODECAPI_AVEncMPVGOPSize, gop_frames);
    let _ = attrs.SetUINT32(&CODECAPI_AVEncCommonQualityVsSpeed, 70u32);
    if codec == VideoCodecType::HardwareH264 {
        let _ = attrs.SetUINT32(&CODECAPI_AVEncH264CABACEnable, 1u32);
    }
    Some(attrs)
}

/// Verifies (via `ICodecAPI::GetValue`) that the selected rate-control mode & 2s GOP took
/// effect, and falls back to `ICodecAPI::SetValue` on MFTs where `pEncodingParameters`
/// was rejected or deferred.
unsafe fn verify_or_configure_encoder_codec_api(
    sink_writer: &IMFSinkWriter,
    video_stream_index: u32,
    codec: VideoCodecType,
    rate_control: RateControlMode,
    bitrate_bps: u32,
    fps: u32,
    used_enc_params: bool,
) -> bool {
    let mut raw_api: *mut std::ffi::c_void = null_mut();
    let guid_null = GUID::zeroed();
    if sink_writer
        .GetServiceForStream(
            video_stream_index,
            &guid_null,
            &ICodecAPI::IID,
            &mut raw_api,
        )
        .is_err()
        || raw_api.is_null()
    {
        return used_enc_params;
    }
    let codec_api = ICodecAPI::from_raw(raw_api);
    let (mean_bps, max_bps, vbv_bytes, gop_frames) =
        compute_rate_control_params(rate_control, bitrate_bps, fps);
    let want_rc = match rate_control {
        RateControlMode::Cbr => eAVEncCommonRateControlMode_CBR.0 as u32,
        RateControlMode::Vbr => eAVEncCommonRateControlMode_PeakConstrainedVBR.0 as u32,
    };

    let read_u32 = |key: &GUID| -> Option<u32> {
        let val = codec_api.GetValue(key).ok()?;
        u32::try_from(&val).ok()
    };

    let mut rc_active = read_u32(&CODECAPI_AVEncCommonRateControlMode) == Some(want_rc);
    let mut gop_active = read_u32(&CODECAPI_AVEncMPVGOPSize) == Some(gop_frames);

    if !used_enc_params || (!rc_active && !gop_active) {
        let _ = codec_api.SetValue(&CODECAPI_AVEncCommonRateControlMode, &VARIANT::from(want_rc));
        let _ = codec_api.SetValue(&CODECAPI_AVEncCommonMeanBitRate, &VARIANT::from(mean_bps));
        if rate_control == RateControlMode::Vbr {
            let _ = codec_api.SetValue(&CODECAPI_AVEncCommonMaxBitRate, &VARIANT::from(max_bps));
        }
        let _ = codec_api.SetValue(&CODECAPI_AVEncCommonBufferSize, &VARIANT::from(vbv_bytes));
        let _ = codec_api.SetValue(&CODECAPI_AVEncMPVGOPSize, &VARIANT::from(gop_frames));
        let _ = codec_api.SetValue(&CODECAPI_AVEncCommonQualityVsSpeed, &VARIANT::from(70u32));
        if codec == VideoCodecType::HardwareH264 {
            let _ = codec_api.SetValue(&CODECAPI_AVEncH264CABACEnable, &VARIANT::from(1u32));
        }
        rc_active = read_u32(&CODECAPI_AVEncCommonRateControlMode) == Some(want_rc);
        gop_active = read_u32(&CODECAPI_AVEncMPVGOPSize) == Some(gop_frames);
    }

    rc_active || gop_active
}

#[allow(clippy::too_many_arguments)]
unsafe fn try_create_segment_writer_with_retry(
    dev_mgr: Option<&IMFDXGIDeviceManager>,
    input_w: u32,
    input_h: u32,
    out_w: u32,
    out_h: u32,
    fps: u32,
    bitrate_bps: u32,
    rate_control: RateControlMode,
    enable_audio: bool,
    mode: BufferStorageMode,
    temp_dir: &Path,
    codec: VideoCodecType,
    use_nv12: bool,
) -> Option<ActiveSegmentWriter> {
    for attempt in 0..3u32 {
        if attempt > 0 {
            thread::sleep(Duration::from_millis(80 * attempt as u64));
        }
        if let Some(w) = create_segment_writer(
            dev_mgr,
            input_w,
            input_h,
            out_w,
            out_h,
            fps,
            bitrate_bps,
            rate_control,
            enable_audio,
            mode,
            temp_dir,
            codec,
            use_nv12,
            true,
        ) {
            return Some(w);
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
unsafe fn create_segment_writer(
    dev_mgr: Option<&IMFDXGIDeviceManager>,
    input_w: u32,
    input_h: u32,
    out_w: u32,
    out_h: u32,
    fps: u32,
    bitrate_bps: u32,
    rate_control: RateControlMode,
    enable_audio: bool,
    mode: BufferStorageMode,
    temp_dir: &Path,
    codec: VideoCodecType,
    use_nv12: bool,
    force_profile: bool,
) -> Option<ActiveSegmentWriter> {
    let safe_out_w = (out_w & !1).max(640);
    let safe_out_h = (out_h & !1).max(360);

    let mut sw_attrs = None;
    MFCreateAttributes(&mut sw_attrs, 4).ok()?;
    let sw_attrs = sw_attrs?;
    let _ = sw_attrs.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1);
    if let Some(mgr) = dev_mgr {
        let _ = sw_attrs.SetUnknown(&MF_SINK_WRITER_D3D_MANAGER, mgr);
    }
    let _ = sw_attrs.SetUINT32(&MF_LOW_LATENCY, 1);

    let mut mem_stream: Option<IStream> = None;
    let mut mf_byte_stream: Option<IMFByteStream> = None;
    let mut disk_path: Option<PathBuf> = None;

    let sink_writer = if mode == BufferStorageMode::RamRingBuffer {
        let stream = SHCreateMemStream(None)?;
        let bs = MFCreateMFByteStreamOnStream(&stream).ok()?;
        let url = HSTRING::from("segment.mp4");
        let sw = MFCreateSinkWriterFromURL(&url, &bs, &sw_attrs).ok()?;
        mem_stream = Some(stream);
        mf_byte_stream = Some(bs);
        sw
    } else {
        let default_dir = std::env::temp_dir().join("NVLiteReplayRust");
        let mut chosen_dir = temp_dir.to_path_buf();
        if fs::create_dir_all(&chosen_dir).is_err() {
            chosen_dir = default_dir.clone();
            let _ = fs::create_dir_all(&chosen_dir);
        }
        let seg_file_id = NEXT_SEG_FILE_ID.fetch_add(1, Ordering::Relaxed);
        let p = chosen_dir.join(format!("seg_{}_{}.mp4", std::process::id(), seg_file_id));
        let url = HSTRING::from(p.to_string_lossy().as_ref());
        match MFCreateSinkWriterFromURL(&url, None, &sw_attrs) {
            Ok(sw) => {
                disk_path = Some(p);
                sw
            }
            Err(_) if chosen_dir != default_dir => {
                // Custom temp folder may be read-only / full / disconnected; fall back to default temp.
                let _ = fs::create_dir_all(&default_dir);
                let fb_id = NEXT_SEG_FILE_ID.fetch_add(1, Ordering::Relaxed);
                let fb_p = default_dir.join(format!("seg_{}_{}.mp4", std::process::id(), fb_id));
                let fb_url = HSTRING::from(fb_p.to_string_lossy().as_ref());
                let sw = MFCreateSinkWriterFromURL(&fb_url, None, &sw_attrs).ok()?;
                disk_path = Some(fb_p);
                sw
            }
            Err(_) => return None,
        }
    };

    let out_frame_size = ((safe_out_w as u64) << 32) | (safe_out_h as u64);
    let in_frame_size = ((input_w as u64) << 32) | (input_h as u64);
    let frame_rate = ((fps as u64) << 32) | 1;

    let video_out: IMFMediaType = MFCreateMediaType().ok()?;
    let _ = video_out.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video);
    let subtype = match codec {
        VideoCodecType::HardwareAV1 => MFVideoFormat_AV1,
        VideoCodecType::HardwareHEVC => MFVideoFormat_HEVC,
        VideoCodecType::HardwareH264 => MFVideoFormat_H264,
    };
    let (mean_bps, _, _, _) = compute_rate_control_params(rate_control, bitrate_bps, fps);
    let _ = video_out.SetGUID(&MF_MT_SUBTYPE, &subtype);
    let _ = video_out.SetUINT32(&MF_MT_AVG_BITRATE, mean_bps);
    let _ = video_out.SetUINT32(&MF_MT_INTERLACE_MODE, 2);
    let _ = video_out.SetUINT64(&MF_MT_FRAME_SIZE, out_frame_size);
    let _ = video_out.SetUINT64(&MF_MT_FRAME_RATE, frame_rate);
    apply_bt709_color_attributes(&video_out, true);
    if force_profile {
        match codec {
            VideoCodecType::HardwareH264 => {
                let _ = video_out.SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_High.0 as u32);
            }
            VideoCodecType::HardwareHEVC => {
                let _ = video_out
                    .SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH265VProfile_Main_420_8.0 as u32);
            }
            VideoCodecType::HardwareAV1 => {}
        }
    }

    let release_disk_on_fail = |sw: IMFSinkWriter, dp: Option<PathBuf>| {
        drop(sw);
        if let Some(p) = dp {
            let _ = fs::remove_file(p);
        }
    };

    let next_fallback = |c: VideoCodecType, fp: bool| -> Option<(VideoCodecType, bool)> {
        match c {
            VideoCodecType::HardwareAV1 => Some((VideoCodecType::HardwareHEVC, true)),
            VideoCodecType::HardwareHEVC => Some((VideoCodecType::HardwareH264, true)),
            VideoCodecType::HardwareH264 if fp => Some((VideoCodecType::HardwareH264, false)),
            VideoCodecType::HardwareH264 => None,
        }
    };

    let video_stream_index = match sink_writer.AddStream(&video_out) {
        Ok(idx) => idx,
        Err(_) => {
            release_disk_on_fail(sink_writer, disk_path);
            if let Some((fb_codec, fb_prof)) = next_fallback(codec, force_profile) {
                return create_segment_writer(
                    dev_mgr,
                    input_w,
                    input_h,
                    out_w,
                    out_h,
                    fps,
                    bitrate_bps,
                    rate_control,
                    enable_audio,
                    mode,
                    temp_dir,
                    fb_codec,
                    use_nv12,
                    fb_prof,
                );
            }
            return None;
        }
    };

    let video_in: IMFMediaType = MFCreateMediaType().ok()?;
    let _ = video_in.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video);
    let _ = video_in.SetGUID(
        &MF_MT_SUBTYPE,
        if use_nv12 {
            &MFVideoFormat_NV12
        } else {
            &MFVideoFormat_ARGB32
        },
    );
    let _ = video_in.SetUINT32(&MF_MT_INTERLACE_MODE, 2);
    let _ = video_in.SetUINT64(&MF_MT_FRAME_SIZE, in_frame_size);
    let _ = video_in.SetUINT64(&MF_MT_FRAME_RATE, frame_rate);
    apply_bt709_color_attributes(&video_in, use_nv12);

    let enc_attrs = build_encoder_attributes(codec, rate_control, bitrate_bps, fps);
    let mut used_enc_params = false;
    let set_in_ok = if let Some(ref ea) = enc_attrs {
        if sink_writer
            .SetInputMediaType(video_stream_index, &video_in, Some(ea))
            .is_ok()
        {
            used_enc_params = true;
            true
        } else {
            sink_writer
                .SetInputMediaType(video_stream_index, &video_in, None)
                .is_ok()
        }
    } else {
        sink_writer
            .SetInputMediaType(video_stream_index, &video_in, None)
            .is_ok()
    };

    if !set_in_ok {
        release_disk_on_fail(sink_writer, disk_path);
        if let Some((fb_codec, fb_prof)) = next_fallback(codec, force_profile) {
            return create_segment_writer(
                dev_mgr,
                input_w,
                input_h,
                out_w,
                out_h,
                fps,
                bitrate_bps,
                rate_control,
                enable_audio,
                mode,
                temp_dir,
                fb_codec,
                use_nv12,
                fb_prof,
            );
        }
        if !use_nv12 && dev_mgr.is_some() {
            return create_segment_writer(
                None,
                input_w,
                input_h,
                out_w,
                out_h,
                fps,
                bitrate_bps,
                rate_control,
                enable_audio,
                mode,
                temp_dir,
                VideoCodecType::HardwareH264,
                false,
                false,
            );
        }
        return None;
    }

    let rc_tuned = verify_or_configure_encoder_codec_api(
        &sink_writer,
        video_stream_index,
        codec,
        rate_control,
        bitrate_bps,
        fps,
        used_enc_params,
    );

    let mut has_audio = enable_audio;
    let mut audio_stream_index = 0u32;
    if enable_audio {
        if let Ok(audio_out) = MFCreateMediaType() {
            let _ = audio_out.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio);
            let _ = audio_out.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC);
            let _ = audio_out.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48_000);
            let _ = audio_out.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2);
            let _ = audio_out.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16);
            let _ = audio_out.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 24_000);
            if let Ok(aidx) = sink_writer.AddStream(&audio_out) {
                audio_stream_index = aidx;
                if let Ok(audio_in) = MFCreateMediaType() {
                    let _ = audio_in.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio);
                    let _ = audio_in.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM);
                    let _ = audio_in.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48_000);
                    let _ = audio_in.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2);
                    let _ = audio_in.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16);
                    let _ = audio_in.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 4);
                    let _ = audio_in.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 192_000);
                    if sink_writer.SetInputMediaType(aidx, &audio_in, None).is_err() {
                        has_audio = false;
                    }
                } else {
                    has_audio = false;
                }
            } else {
                has_audio = false;
            }
        } else {
            has_audio = false;
        }
    }

    if sink_writer.BeginWriting().is_err() {
        release_disk_on_fail(sink_writer, disk_path);
        return None;
    }

    Some(ActiveSegmentWriter {
        seq_id: 0,
        sink_writer,
        mem_stream,
        _mf_byte_stream: mf_byte_stream,
        disk_path,
        video_stream_index,
        audio_stream_index,
        has_audio,
        frames_written: 0,
        audio_samples_written: 0,
        write_errors: 0,
        actual_codec: codec,
        rc_tuned,
    })
}

fn convert_hvcc_to_annex_b(hvcc: &[u8]) -> Vec<u8> {
    if hvcc.len() >= 4 && hvcc[0] == 0 && hvcc[1] == 0 && hvcc[2] == 0 && hvcc[3] == 1 {
        return hvcc.to_vec();
    }
    // ISO/IEC 14496-15 HEVCDecoderConfigurationRecord: 22-byte header + numOfArrays at offset 22
    if hvcc.len() < 23 || hvcc[0] != 1 {
        return hvcc.to_vec();
    }
    let mut out = Vec::with_capacity(hvcc.len() + 24);
    let num_arrays = hvcc[22] as usize;
    let mut pos = 23usize;
    for _ in 0..num_arrays {
        if pos + 3 > hvcc.len() {
            break;
        }
        let num_nalus = ((hvcc[pos + 1] as usize) << 8) | (hvcc[pos + 2] as usize);
        pos += 3;
        for _ in 0..num_nalus {
            if pos + 2 > hvcc.len() {
                break;
            }
            let len = ((hvcc[pos] as usize) << 8) | (hvcc[pos + 1] as usize);
            pos += 2;
            if pos + len <= hvcc.len() {
                out.extend_from_slice(&[0, 0, 0, 1]);
                out.extend_from_slice(&hvcc[pos..pos + len]);
                pos += len;
            }
        }
    }
    out
}

fn convert_avcc_to_annex_b(avcc: &[u8]) -> Vec<u8> {
    if avcc.len() < 7 || avcc[0] != 1 {
        return avcc.to_vec();
    }
    let mut out = Vec::with_capacity(avcc.len() + 16);
    let mut pos = 5usize;
    let num_sps = (avcc[pos] & 0x1F) as usize;
    pos += 1;
    for _ in 0..num_sps {
        if pos + 2 > avcc.len() {
            break;
        }
        let len = ((avcc[pos] as usize) << 8) | (avcc[pos + 1] as usize);
        pos += 2;
        if pos + len <= avcc.len() {
            out.extend_from_slice(&[0, 0, 0, 1]);
            out.extend_from_slice(&avcc[pos..pos + len]);
            pos += len;
        }
    }
    if pos < avcc.len() {
        let num_pps = avcc[pos] as usize;
        pos += 1;
        for _ in 0..num_pps {
            if pos + 2 > avcc.len() {
                break;
            }
            let len = ((avcc[pos] as usize) << 8) | (avcc[pos + 1] as usize);
            pos += 2;
            if pos + len <= avcc.len() {
                out.extend_from_slice(&[0, 0, 0, 1]);
                out.extend_from_slice(&avcc[pos..pos + len]);
                pos += len;
            }
        }
    }
    out
}

unsafe fn convert_sample_length_prefix_to_annex_b(sample: &IMFSample) {
    let Ok(buf) = sample.ConvertToContiguousBuffer() else {
        return;
    };
    let mut p_data = null_mut();
    let mut cur_len = 0u32;
    if buf.Lock(&mut p_data, None, Some(&mut cur_len)).is_err() || p_data.is_null() {
        return;
    }

    let len = cur_len as usize;
    let data = std::slice::from_raw_parts_mut(p_data, len);
    let already_annex_b = len >= 4 && data[0] == 0 && data[1] == 0 && data[2] == 0 && data[3] == 1;
    if !already_annex_b {
        let mut pos = 0usize;
        while pos + 4 <= len {
            let nalu_len = ((data[pos] as usize) << 24)
                | ((data[pos + 1] as usize) << 16)
                | ((data[pos + 2] as usize) << 8)
                | (data[pos + 3] as usize);
            if nalu_len == 0 || pos + 4 + nalu_len > len {
                break;
            }
            data[pos] = 0;
            data[pos + 1] = 0;
            data[pos + 2] = 0;
            data[pos + 3] = 1;
            pos += 4 + nalu_len;
        }
    }
    let _ = buf.Unlock();
}

fn merge_segments_pure_native(segments: &[ReplaySegment], output_path: &Path, fps: u32) -> bool {
    if segments.is_empty() {
        return false;
    }
    if segments.len() == 1 {
        let seg = &segments[0];
        if let Some(ref ram) = seg.ram_data {
            return fs::write(output_path, ram.as_ref()).is_ok();
        }
        if let Some(p) = seg.disk_path() {
            return fs::copy(p, output_path).is_ok();
        }
        return false;
    }

    unsafe {
        let mut sink_writer_opt: Option<IMFSinkWriter> = None;
        let mut out_video_stream = 0u32;
        let mut out_audio_stream = 0u32;
        let mut has_audio_stream = false;
        let mut convert_nalu_annex_b = true;
        let mut video_time_offset: i64 = 0;
        let mut audio_time_offset: i64 = 0;
        let frame_step_hns: i64 = 10_000_000 / (fps.max(1) as i64);

        for seg in segments {
            let reader_res: Option<IMFSourceReader> = if let Some(ref ram) = seg.ram_data {
                if let Some(stream) = SHCreateMemStream(Some(ram.as_ref())) {
                    if let Ok(bs) = MFCreateMFByteStreamOnStream(&stream) {
                        MFCreateSourceReaderFromByteStream(&bs, None).ok()
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else if let Some(p) = seg.disk_path() {
                let url = HSTRING::from(p.to_string_lossy().as_ref());
                MFCreateSourceReaderFromURL(&url, None).ok()
            } else {
                None
            };

            let Some(reader) = reader_res else {
                continue;
            };

            let _ = reader.SetStreamSelection(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, true);
            let _ = reader.SetStreamSelection(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32, true);

            if sink_writer_opt.is_none() {
                let Ok(video_type) =
                    reader.GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32)
                else {
                    continue;
                };

                let v_sub = video_type.GetGUID(&MF_MT_SUBTYPE).unwrap_or(MFVideoFormat_H264);
                let is_h264 = v_sub == MFVideoFormat_H264;
                let is_hevc = v_sub == MFVideoFormat_HEVC;
                convert_nalu_annex_b = is_h264 || is_hevc;

                if convert_nalu_annex_b {
                    if let Ok(sz) = video_type.GetBlobSize(&MF_MT_MPEG_SEQUENCE_HEADER) {
                        if sz > 0 {
                            let mut seq = vec![0u8; sz as usize];
                            if video_type
                                .GetBlob(&MF_MT_MPEG_SEQUENCE_HEADER, &mut seq, None)
                                .is_ok()
                            {
                                let annex_b = if is_hevc {
                                    convert_hvcc_to_annex_b(&seq)
                                } else {
                                    convert_avcc_to_annex_b(&seq)
                                };
                                if !annex_b.is_empty() {
                                    let _ = video_type.SetBlob(&MF_MT_MPEG_SEQUENCE_HEADER, &annex_b);
                                }
                            }
                        }
                    }
                }
                apply_bt709_color_attributes(&video_type, true);

                let mut sw_attrs = None;
                if MFCreateAttributes(&mut sw_attrs, 2).is_err() {
                    return false;
                }
                let Some(sw_attrs) = sw_attrs else {
                    return false;
                };
                let _ = sw_attrs.SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1);

                let out_url = HSTRING::from(output_path.to_string_lossy().as_ref());
                let Ok(sw) = MFCreateSinkWriterFromURL(&out_url, None, &sw_attrs) else {
                    return false;
                };

                let Ok(v_idx) = sw.AddStream(&video_type) else {
                    return false;
                };
                out_video_stream = v_idx;
                let _ = sw.SetInputMediaType(out_video_stream, &video_type, None);

                if let Ok(audio_type) =
                    reader.GetCurrentMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32)
                {
                    if let Ok(a_idx) = sw.AddStream(&audio_type) {
                        if sw.SetInputMediaType(a_idx, &audio_type, None).is_ok() {
                            out_audio_stream = a_idx;
                            has_audio_stream = true;
                        }
                    }
                }

                if sw.BeginWriting().is_err() {
                    return false;
                }
                sink_writer_opt = Some(sw);
            }

            let Some(ref sw) = sink_writer_opt else {
                continue;
            };

            let mut max_video_end: i64 = 0;
            loop {
                let mut flags = 0u32;
                let mut timestamp = 0i64;
                let mut sample_opt = None;
                if reader
                    .ReadSample(
                        MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                        0,
                        None,
                        Some(&mut flags),
                        Some(&mut timestamp),
                        Some(&mut sample_opt),
                    )
                    .is_err()
                {
                    break;
                }
                if (flags & (MF_SOURCE_READERF_ERROR.0 as u32 | MF_SOURCE_READERF_ENDOFSTREAM.0 as u32)) != 0 {
                    break;
                }
                if let Some(sample) = sample_opt {
                    if convert_nalu_annex_b {
                        convert_sample_length_prefix_to_annex_b(&sample);
                    }
                    let dur = sample.GetSampleDuration().unwrap_or(frame_step_hns).max(1);
                    let _ = sample.SetSampleTime(video_time_offset + timestamp);
                    if timestamp + dur > max_video_end {
                        max_video_end = timestamp + dur;
                    }
                    let _ = sw.WriteSample(out_video_stream, &sample);
                }
            }

            let mut max_audio_end: i64 = 0;
            if has_audio_stream {
                loop {
                    let mut flags = 0u32;
                    let mut timestamp = 0i64;
                    let mut sample_opt = None;
                    if reader
                        .ReadSample(
                            MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32,
                            0,
                            None,
                            Some(&mut flags),
                            Some(&mut timestamp),
                            Some(&mut sample_opt),
                        )
                        .is_err()
                    {
                        break;
                    }
                    if (flags & (MF_SOURCE_READERF_ERROR.0 as u32 | MF_SOURCE_READERF_ENDOFSTREAM.0 as u32)) != 0 {
                        break;
                    }
                    if let Some(sample) = sample_opt {
                        let dur = sample.GetSampleDuration().unwrap_or(213_333).max(1);
                        let _ = sample.SetSampleTime(audio_time_offset + timestamp);
                        if timestamp + dur > max_audio_end {
                            max_audio_end = timestamp + dur;
                        }
                        let _ = sw.WriteSample(out_audio_stream, &sample);
                    }
                }
            }

            video_time_offset += max_video_end;
            audio_time_offset += if max_audio_end > 0 {
                max_audio_end
            } else {
                max_video_end
            };
        }

        if let Some(sw) = sink_writer_opt {
            if sw.Finalize().is_ok() {
                if let Ok(meta) = fs::metadata(output_path) {
                    return meta.len() > 1024;
                }
            }
        }
    }

    false
}

pub fn inspect_mp4_video_info(path: &Path) -> Option<(String, u32, u32, u32, u32)> {
    unsafe {
        let _guard = ComMfGuard::new();
        let url = HSTRING::from(path.to_string_lossy().as_ref());
        let reader = MFCreateSourceReaderFromURL(&url, None).ok()?;
        let v_stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
        let _ = reader.SetStreamSelection(v_stream, true);
        let vtype = reader.GetCurrentMediaType(v_stream).ok()?;
        let sub = vtype.GetGUID(&MF_MT_SUBTYPE).ok()?;
        let codec_name = if sub == MFVideoFormat_H264 {
            "H.264"
        } else if sub == MFVideoFormat_HEVC {
            "HEVC"
        } else if sub == MFVideoFormat_AV1 {
            "AV1"
        } else {
            "Other"
        }
        .to_string();
        let fs = vtype.GetUINT64(&MF_MT_FRAME_SIZE).unwrap_or(0);
        let w = (fs >> 32) as u32;
        let h = (fs & 0xFFFF_FFFF) as u32;
        let yuv_matrix = vtype.GetUINT32(&MF_MT_YUV_MATRIX).unwrap_or(0);

        let mut frames = 0u32;
        for _ in 0..2000 {
            let mut flags = 0u32;
            let mut ts = 0i64;
            let mut sample = None;
            if reader
                .ReadSample(v_stream, 0, None, Some(&mut flags), Some(&mut ts), Some(&mut sample))
                .is_err()
            {
                break;
            }
            if (flags & (MF_SOURCE_READERF_ERROR.0 as u32 | MF_SOURCE_READERF_ENDOFSTREAM.0 as u32)) != 0 {
                break;
            }
            if sample.is_some() {
                frames += 1;
            }
        }
        Some((codec_name, w, h, yuv_matrix, frames))
    }
}

#[link(name = "version")]
extern "system" {
    fn GetFileVersionInfoSizeW(lptstrfilename: PCWSTR, lpdwhandle: *mut u32) -> u32;
    fn GetFileVersionInfoW(
        lptstrfilename: PCWSTR,
        dwhandle: u32,
        dwlen: u32,
        lpdata: *mut std::ffi::c_void,
    ) -> BOOL;
    fn VerQueryValueW(
        pblock: *const std::ffi::c_void,
        lpsubblock: PCWSTR,
        lplpbuffer: *mut *mut std::ffi::c_void,
        pulen: *mut u32,
    ) -> BOOL;
}

fn is_usable_product_name(name: &str) -> bool {
    let trimmed = name
        .trim()
        .trim_matches(|c: char| c == '®' || c == '™' || c.is_whitespace());
    if trimmed.len() < 2 {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    !matches!(
        lower.as_str(),
        "microsoft® windows® operating system"
            | "microsoft windows operating system"
            | "windows operating system"
            | "bootstrappackagedgame"
            | "unrealgame"
            | "application"
    )
}

/// Queries the PE version resource (`GetFileVersionInfoW` / `VerQueryValueW`) of `exe_path`
/// for `ProductName` (or `FileDescription` fallback), ignoring generic OS/engine placeholders.
pub fn query_exe_version_product_name(exe_path: &Path) -> Option<String> {
    unsafe {
        let wide_path: Vec<u16> = exe_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut handle: u32 = 0;
        let size = GetFileVersionInfoSizeW(PCWSTR(wide_path.as_ptr()), &mut handle);
        if size == 0 || size > 4 * 1024 * 1024 {
            return None;
        }
        let mut buffer = vec![0u8; size as usize];
        if !GetFileVersionInfoW(
            PCWSTR(wide_path.as_ptr()),
            0,
            size,
            buffer.as_mut_ptr() as *mut std::ffi::c_void,
        )
        .as_bool()
        {
            return None;
        }

        let mut candidates: Vec<(u16, u16)> = Vec::new();
        let trans_sub: Vec<u16> = "\\VarFileInfo\\Translation\0".encode_utf16().collect();
        let mut trans_ptr: *mut std::ffi::c_void = null_mut();
        let mut trans_len: u32 = 0;
        if VerQueryValueW(
            buffer.as_ptr() as *const std::ffi::c_void,
            PCWSTR(trans_sub.as_ptr()),
            &mut trans_ptr,
            &mut trans_len,
        )
        .as_bool()
            && !trans_ptr.is_null()
            && trans_len >= 4
        {
            let count = (trans_len as usize) / 4;
            let pairs = std::slice::from_raw_parts(trans_ptr as *const [u16; 2], count);
            for pair in pairs {
                candidates.push((pair[0], pair[1]));
            }
        }

        for fallback in [
            (0x0409, 0x04B0),
            (0x0409, 0x04E4),
            (0x0000, 0x04B0),
            (0x0409, 0x0000),
        ] {
            if !candidates.contains(&fallback) {
                candidates.push(fallback);
            }
        }

        for field in ["ProductName", "FileDescription"] {
            for &(lang, cp) in &candidates {
                let query = format!("\\StringFileInfo\\{:04x}{:04x}\\{}\0", lang, cp, field);
                let query_w: Vec<u16> = query.encode_utf16().collect();
                let mut val_ptr: *mut std::ffi::c_void = null_mut();
                let mut val_len: u32 = 0;
                if VerQueryValueW(
                    buffer.as_ptr() as *const std::ffi::c_void,
                    PCWSTR(query_w.as_ptr()),
                    &mut val_ptr,
                    &mut val_len,
                )
                .as_bool()
                    && !val_ptr.is_null()
                    && val_len > 0
                {
                    let slice =
                        std::slice::from_raw_parts(val_ptr as *const u16, val_len as usize);
                    let s = utf16_slice_to_string(slice);
                    if is_usable_product_name(&s) {
                        let clean = sanitize_path_component(&s, "");
                        if !clean.is_empty() {
                            return Some(clean);
                        }
                    }
                }
            }
        }
        None
    }
}

/// Resolves a friendly folder/clip name for `exe_path`:
/// 1. Custom game name map (`msedge.exe -> Browser`, etc.)
/// 2. PE `GetFileVersionInfoW` `ProductName` / `FileDescription`
/// 3. Cleaned executable stem fallback
pub fn resolve_game_name_for_exe_path(exe_path: &Path, custom_map: &str) -> String {
    let path_str = exe_path.to_string_lossy();
    if let Some(mapped) = lookup_custom_game_name(custom_map, &path_str) {
        return mapped;
    }
    if let Some(prod) = query_exe_version_product_name(exe_path) {
        return prod;
    }
    clean_exe_stem_name(&path_str)
}

/// Queries the foreground window's `(pid, exe_path)` via
/// `GetForegroundWindow` -> `GetWindowThreadProcessId` -> `QueryFullProcessImageNameW`.
/// Returns `None` if the foreground window belongs to the current `NVLiteReplay` process.
pub fn detect_foreground_process() -> Option<(u32, PathBuf)> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 || pid == std::process::id() {
            return None;
        }
        let hproc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            hproc,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
        .is_ok();
        let _ = CloseHandle(hproc);
        if !ok || len == 0 {
            return None;
        }
        let path_str = String::from_utf16_lossy(&buf[..len as usize]);
        if path_str.trim().is_empty() {
            None
        } else {
            Some((pid, PathBuf::from(path_str)))
        }
    }
}

pub fn detect_foreground_exe_path() -> Option<PathBuf> {
    detect_foreground_process().map(|(_, p)| p)
}

pub fn is_process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    unsafe {
        let Ok(hproc) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let mut exit_code: u32 = 0;
        let ok = GetExitCodeProcess(hproc, &mut exit_code).is_ok();
        let _ = CloseHandle(hproc);
        ok && exit_code == STILL_ACTIVE.0 as u32
    }
}

/// Returns `true` for desktop shells, chat/music apps, and browsers so Alt-Tabbing to
/// them does not steal the "Active Game Only" audio loopback lock from a running game.
pub fn is_background_utility_exe(exe_path: &Path) -> bool {
    let Some(fname) = exe_path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    let lower = fname.trim().to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "explorer.exe"
            | "searchhost.exe"
            | "searchapp.exe"
            | "startmenuexperiencehost.exe"
            | "shellexperiencehost.exe"
            | "taskmgr.exe"
            | "applicationframehost.exe"
            | "lockapp.exe"
            | "dwm.exe"
            | "consent.exe"
            | "systemsettings.exe"
            | "snippingtool.exe"
            | "screenclippinghost.exe"
            | "discord.exe"
            | "discordptb.exe"
            | "discordcanary.exe"
            | "spotify.exe"
            | "teamspeak.exe"
            | "ts3client_win64.exe"
            | "obs64.exe"
            | "obs32.exe"
            | "msedge.exe"
            | "chrome.exe"
            | "firefox.exe"
            | "brave.exe"
            | "opera.exe"
            | "vivaldi.exe"
    )
}

pub fn unique_replay_output_path(out_dir: &Path, template: &str, game_name: &str) -> PathBuf {
    let (date_str, time_str) = unsafe {
        let st = GetLocalTime();
        (
            format!("{:04}-{:02}-{:02}", st.wYear, st.wMonth, st.wDay),
            format!("{:02}-{:02}-{:02}", st.wHour, st.wMinute, st.wSecond),
        )
    };
    let rel = format_clip_relative_path(template, game_name, &date_str, &time_str);
    let base_path = out_dir.join(&rel);
    if let Some(parent) = base_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let stem = base_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("Replay");
    let parent_dir = base_path.parent().unwrap_or(out_dir);

    let first = parent_dir.join(format!("{}.mp4", stem));
    if !first.exists() {
        return first;
    }
    for n in 2..=999u32 {
        let cand = parent_dir.join(format!("{}_{}.mp4", stem, n));
        if !cand.exists() {
            return cand;
        }
    }
    first
}

fn trim_working_set() {
    unsafe {
        let _ = EmptyWorkingSet(GetCurrentProcess());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repro_disk_hevc_stop() {
        let mut cfg = ReplayConfig::load();
        cfg.storage_mode = BufferStorageMode::DiskTempBuffer;
        cfg.codec = VideoCodecType::HardwareHEVC;
        cfg.fps = 60;
        cfg.replay_duration_seconds = 60;
        cfg.segment_duration_seconds = 5;
        cfg.bitrate_mbps = 12;
        cfg.use_native_resolution = true;
        cfg.play_sound_on_save = false;
        let temp_dir = std::env::temp_dir().join("NVLiteReplayRust_CustomTmpTest");
        let _ = fs::remove_dir_all(&temp_dir);
        cfg.temp_folder = temp_dir.to_string_lossy().to_string();

        let engine = ReplayEngine::new(cfg);
        eprintln!("[repro] Case 1: DiskTemp + HEVC + 60fps across segment rotation (7.2s)...");
        engine.start();
        std::thread::sleep(Duration::from_millis(7200));
        let buf_sec = engine.buffered_seconds();
        let buf_bytes = engine.buffer_size_bytes();
        eprintln!(
            "[repro] before stop: mode={}, fps={:.1}, buf={:.1}s, bytes={}",
            engine.capture_mode_status.lock().unwrap(),
            engine.current_fps(),
            buf_sec,
            buf_bytes
        );

        let (tx, rx) = std::sync::mpsc::channel();
        let eng2 = Arc::clone(&engine);
        std::thread::spawn(move || {
            let t0 = Instant::now();
            eng2.stop();
            let _ = tx.send(t0.elapsed());
        });

        let stop_dur = match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(elapsed) => {
                eprintln!("[repro] Case 1 stop() finished in {:?}", elapsed);
                elapsed
            }
            Err(_) => panic!("[repro] Case 1 HANG! stop() did not return within 10s!"),
        };
        assert!(buf_sec >= 4.0 && buf_bytes > 0, "Expected >=1 rotated HEVC segment, got buf_sec={buf_sec}, bytes={buf_bytes}");
        assert!(stop_dur < Duration::from_secs(5), "stop() too slow: {:?}", stop_dur);

        // Case 2: Non-blocking stop_async() right inside prewarm window (~4.3s)
        eprintln!("[repro] Case 2: DiskTemp + HEVC stop_async() during prewarm (~4.3s)...");
        engine.start();
        std::thread::sleep(Duration::from_millis(4300));
        let t_async = Instant::now();
        engine.stop_async();
        let dispatch_elapsed = t_async.elapsed();
        eprintln!("[repro] stop_async() returned in {:?}", dispatch_elapsed);
        assert!(dispatch_elapsed < Duration::from_millis(50), "stop_async blocked UI thread: {:?}", dispatch_elapsed);

        while engine.is_stopping() {
            assert!(t_async.elapsed() < Duration::from_secs(10), "stop_async did not settle within 10s!");
            std::thread::sleep(Duration::from_millis(20));
        }
        eprintln!("[repro] Case 2 stop_async settled in {:?}", t_async.elapsed());

        let pid_prefix = format!("seg_{}_", std::process::id());
        assert!(temp_dir.exists(), "Custom temp_folder was not created/used: {:?}", temp_dir);
        let leftovers: Vec<String> = fs::read_dir(&temp_dir)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|name| name.starts_with(&pid_prefix))
            .collect();
        let _ = fs::remove_dir_all(&temp_dir);
        assert!(leftovers.is_empty(), "Leftover temp segments after stop: {:?}", leftovers);
        eprintln!("[repro] PASS: zero crash, zero hang, zero leftover temp files!");
    }

    #[test]
    fn hvcc_to_annex_b_parses_vps_sps_pps() {
        // Minimal synthetic ISO/IEC 14496-15 hvcC record with 2 arrays (VPS + SPS)
        let mut hvcc = vec![0u8; 23];
        hvcc[0] = 1; // configurationVersion
        hvcc[21] = 0xFF; // lengthSizeMinusOne = 3
        hvcc[22] = 2; // numOfArrays = 2
        // Array 1 (VPS): type=32, numNalus=1, len=3, payload=[0x40, 0x01, 0xAA]
        hvcc.extend_from_slice(&[32, 0, 1, 0, 3, 0x40, 0x01, 0xAA]);
        // Array 2 (SPS): type=33, numNalus=1, len=2, payload=[0x42, 0x01]
        hvcc.extend_from_slice(&[33, 0, 1, 0, 2, 0x42, 0x01]);

        let annex_b = convert_hvcc_to_annex_b(&hvcc);
        assert_eq!(
            annex_b,
            vec![0, 0, 0, 1, 0x40, 0x01, 0xAA, 0, 0, 0, 1, 0x42, 0x01]
        );

        // Already Annex-B must pass through unchanged
        let already = vec![0, 0, 0, 1, 0x40, 0x01];
        assert_eq!(convert_hvcc_to_annex_b(&already), already);
    }

    #[test]
    fn disk_temp_pinned_arc_survives_eviction_during_merge() {
        let tmp = std::env::temp_dir().join("NVLiteReplayRust_PinnedEvictTest");
        let _ = fs::remove_dir_all(&tmp);
        let _ = fs::create_dir_all(&tmp);
        let p = tmp.join("seg_pinned.mp4");
        fs::write(&p, b"synthetic_mp4_payload").unwrap();

        let seg = ReplaySegment {
            seq_id: 1,
            ram_data: None,
            disk_file: Some(DiskTempFile::new(p.clone())),
            size_bytes: 21,
            duration_seconds: 5.0,
        };
        let mut ring = vec![seg];
        // Simulate save_replay_async snapshotting segments before merge
        let snapshot = ring.clone();
        // Simulate ring eviction during a long 3-minute Disk Temp merge
        ring.clear();
        assert!(p.exists(), "Evicted file must remain on disk while merge snapshot is alive");
        // Drop snapshot after merge completes -> RAII DiskTempFile deletes file automatically
        drop(snapshot);
        assert!(!p.exists(), "Evicted file must be deleted once merge snapshot drops");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn disk_temp_buffer_full_rollover_never_repeats_clips() {
        let mut cfg = ReplayConfig::default();
        cfg.storage_mode = BufferStorageMode::DiskTempBuffer;
        cfg.codec = VideoCodecType::HardwareH264;
        cfg.fps = 60;
        cfg.replay_duration_seconds = 4;
        cfg.segment_duration_seconds = 1;
        cfg.bitrate_mbps = 8;
        cfg.play_sound_on_save = false;

        let tmp = std::env::temp_dir().join("NVLiteReplayRust_DiskRolloverTest");
        let out_dir = tmp.join("out");
        let seg_dir = tmp.join("temp");
        let _ = fs::remove_dir_all(&tmp);
        cfg.output_folder = out_dir.to_string_lossy().to_string();
        cfg.temp_folder = seg_dir.to_string_lossy().to_string();

        let engine = ReplayEngine::new(cfg);
        engine.start();

        // Wait past full replay duration (4s) -> Clip #1
        std::thread::sleep(Duration::from_millis(5200));
        engine.save_replay_async();
        let t0 = Instant::now();
        while engine.is_saving.load(Ordering::Acquire) && t0.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(25));
        }
        let clip1_path = engine.last_saved_file.lock().unwrap().clone();
        let clip1_seqs: Vec<u64> = engine
            .segments
            .lock()
            .unwrap()
            .iter()
            .map(|s| s.seq_id)
            .collect();

        // Wait past ANOTHER full replay duration (4s + margin) -> Clip #2 must use 100% newer segments
        std::thread::sleep(Duration::from_millis(5600));
        engine.save_replay_async();
        let t1 = Instant::now();
        while engine.is_saving.load(Ordering::Acquire) && t1.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(25));
        }
        let clip2_path = engine.last_saved_file.lock().unwrap().clone();
        let clip2_seqs: Vec<u64> = engine
            .segments
            .lock()
            .unwrap()
            .iter()
            .map(|s| s.seq_id)
            .collect();

        assert!(engine.is_running(), "Engine must remain active across repeated saves");
        assert!(!clip1_path.is_empty() && !clip2_path.is_empty());
        assert_ne!(clip1_path, clip2_path, "Two saves must create distinct MP4 files");
        assert!(!clip1_seqs.is_empty() && !clip2_seqs.is_empty());
        assert!(
            clip1_seqs.windows(2).all(|w| w[0] < w[1]),
            "Clip 1 segments must stay strictly chronological: {:?}",
            clip1_seqs
        );
        assert!(
            clip2_seqs.windows(2).all(|w| w[0] < w[1]),
            "Clip 2 segments must stay strictly chronological: {:?}",
            clip2_seqs
        );
        let max_clip1 = *clip1_seqs.iter().max().unwrap();
        let min_clip2 = *clip2_seqs.iter().min().unwrap();
        assert!(
            min_clip2 > max_clip1,
            "After waiting > full replay duration, old segments must be evicted (clip1={:?}, clip2={:?})",
            clip1_seqs,
            clip2_seqs
        );

        let info1 = inspect_mp4_video_info(Path::new(&clip1_path)).expect("Clip 1 readable");
        let info2 = inspect_mp4_video_info(Path::new(&clip2_path)).expect("Clip 2 readable");
        assert!(info1.4 > 60 && info2.4 > 60, "Both clips must contain valid video frames");

        engine.stop();
        let pid_prefix = format!("seg_{}_", std::process::id());
        let leftovers: Vec<String> = fs::read_dir(&seg_dir)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|name| name.starts_with(&pid_prefix))
            .collect();
        let _ = fs::remove_dir_all(&tmp);
        assert!(leftovers.is_empty(), "No temp segment files should remain: {:?}", leftovers);
    }

    #[test]
    fn disk_temp_36_segments_3min_equiv_merge() {
        let tmp = std::env::temp_dir().join("NVLiteReplayRust_36SegMergeTest");
        let _ = fs::remove_dir_all(&tmp);
        let _ = fs::create_dir_all(&tmp);

        let mut segs36 = Vec::new();
        unsafe {
            let _com = ComMfGuard::new();
            let mut dev_opt: Option<ID3D11Device> = None;
            let mut ctx_opt: Option<ID3D11DeviceContext> = None;
            let levels = [D3D_FEATURE_LEVEL_11_0];
            let _ = D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&levels),
                D3D11_SDK_VERSION,
                Some(&mut dev_opt),
                None,
                Some(&mut ctx_opt),
            );
            let dev = dev_opt.expect("D3D11 device");
            let mut token = 0u32;
            let mut mgr_opt: Option<IMFDXGIDeviceManager> = None;
            let _ = MFCreateDXGIDeviceManager(&mut token, &mut mgr_opt);
            if let Some(ref mgr) = mgr_opt {
                let _ = mgr.ResetDevice(&dev, token);
            }
            let nv12_desc = D3D11_TEXTURE2D_DESC {
                Width: 1280,
                Height: 720,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_NV12,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };
            let mut nt: Option<ID3D11Texture2D> = None;
            let _ = dev.CreateTexture2D(&nv12_desc, None, Some(&mut nt));
            let buf = MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, nt.as_ref().unwrap(), 0, false).unwrap();
            let _ = buf.SetCurrentLength(1280 * 720 * 3 / 2);

            for i in 1..=36u64 {
                let mut wr = create_segment_writer(
                    mgr_opt.as_ref(),
                    1280,
                    720,
                    1280,
                    720,
                    60,
                    8_000_000,
                    RateControlMode::Vbr,
                    false,
                    BufferStorageMode::DiskTempBuffer,
                    &tmp,
                    VideoCodecType::HardwareH264,
                    true,
                    true,
                )
                .expect("create writer");
                for f in 0..30u32 {
                    let sample = MFCreateSample().unwrap();
                    let _ = sample.SetSampleTime(f as i64 * 166_666);
                    let _ = sample.SetSampleDuration(166_666);
                    let _ = sample.AddBuffer(&buf);
                    wr.sink_writer.WriteSample(wr.video_stream_index, &sample).expect("WriteSample");
                }
                wr.sink_writer.Finalize().expect("Finalize");
                let path = wr.disk_path.take().unwrap();
                drop(wr);
                let sz = fs::metadata(&path).unwrap().len();
                segs36.push(ReplaySegment {
                    seq_id: i,
                    ram_data: None,
                    disk_file: Some(DiskTempFile::new(path)),
                    size_bytes: sz,
                    duration_seconds: 5.0,
                });
            }
            let merged = tmp.join("merged_36.mp4");
            let ok = merge_segments_pure_native(&segs36, &merged, 60);
            let msz = fs::metadata(&merged).map(|m| m.len()).unwrap_or(0);
            assert!(ok && msz > 10_000, "36-segment merge failed (ok={ok}, size={msz})");
        }
        drop(segs36);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn vbr_params_cap_and_gop_bounds() {
        for (mbps, fps) in [(4, 30), (8, 60), (12, 60), (20, 60), (35, 120), (50, 120)] {
            let target = mbps * 1_000_000;
            let (mean, max, vbv, gop) = compute_rate_control_params(RateControlMode::Vbr, target, fps);
            assert!(mean < max, "Peak-VBR requires max > mean ({mean} vs {max})");
            assert_eq!(max, target, "Peak bitrate must not exceed user-selected bitrate");
            assert!(vbv >= 250_000 && vbv <= max / 8, "VBV buffer must be in bytes (<= 1s): {vbv}");
            assert_eq!(gop, (fps * 2).clamp(30, 240));

            let (cbr_mean, cbr_max, cbr_vbv, cbr_gop) =
                compute_rate_control_params(RateControlMode::Cbr, target, fps);
            assert_eq!(cbr_mean, target, "CBR mean bitrate must equal target");
            assert_eq!(cbr_max, target, "CBR max bitrate bound must equal target");
            assert!(cbr_vbv >= 250_000 && cbr_vbv <= cbr_max / 8, "CBR VBV buffer out of range: {cbr_vbv}");
            assert_eq!(cbr_gop, (fps * 2).clamp(30, 240));
        }
    }

    fn make_test_calm_and_action_frames(w: u32, h: u32) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
        let mut calm_frames = Vec::new();
        for idx in 0..2u32 {
            let mut f = vec![0u8; (w * h * 4) as usize];
            for y in 0..h {
                for x in 0..w {
                    let off = ((y * w + x) * 4) as usize;
                    f[off] = ((x * 255) / w) as u8;
                    f[off + 1] = ((y * 255) / h) as u8;
                    f[off + 2] = 50;
                    f[off + 3] = 255;
                }
            }
            for dy in 0..12usize {
                for dx in 0..12usize {
                    let off = (((100 + dy) * w as usize + 100 + idx as usize * 6 + dx) * 4) as usize;
                    f[off] = 255;
                    f[off + 1] = 255;
                    f[off + 2] = 255;
                }
            }
            calm_frames.push(f);
        }

        let mut act_frames = Vec::new();
        for idx in 0..4u32 {
            let mut f = vec![0u8; (w * h * 4) as usize];
            let mut rng: u32 = 0xDEAD_BEEF ^ idx.wrapping_mul(0x9E37_79B9);
            for y in 0..h {
                for x in 0..w {
                    rng ^= rng << 13;
                    rng ^= rng >> 17;
                    rng ^= rng << 5;
                    let off = ((y * w + x) * 4) as usize;
                    let vx = x.wrapping_add(idx * 23);
                    let vy = y.wrapping_add(idx * 11);
                    f[off] = (((vx ^ vy) & 0x7F) as u8).saturating_add((rng & 0x1F) as u8);
                    f[off + 1] = (((vx.wrapping_mul(3) + vy) & 0x7F) as u8)
                        .saturating_add(((rng >> 8) & 0x1F) as u8);
                    f[off + 2] = (((vx + vy.wrapping_mul(5)) & 0x7F) as u8)
                        .saturating_add(((rng >> 16) & 0x1F) as u8);
                    f[off + 3] = 255;
                }
            }
            act_frames.push(f);
        }
        (calm_frames, act_frames)
    }

    unsafe fn count_mp4_keyframes(path: &Path) -> u32 {
        use windows::Win32::Media::MediaFoundation::MFSampleExtension_CleanPoint;

        let url = HSTRING::from(path.to_string_lossy().as_ref());
        let mut keyframes = 0u32;
        if let Ok(r) = MFCreateSourceReaderFromURL(&url, None) {
            let vs = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
            let _ = r.SetStreamSelection(vs, true);
            loop {
                let mut fl = 0u32;
                let mut ts = 0i64;
                let mut sa = None;
                if r.ReadSample(vs, 0, None, Some(&mut fl), Some(&mut ts), Some(&mut sa)).is_err() {
                    break;
                }
                if (fl & (MF_SOURCE_READERF_ERROR.0 as u32 | MF_SOURCE_READERF_ENDOFSTREAM.0 as u32)) != 0 {
                    break;
                }
                if let Some(s) = sa {
                    if s.GetUINT32(&MFSampleExtension_CleanPoint).unwrap_or(0) != 0 {
                        keyframes += 1;
                    }
                }
            }
        }
        keyframes
    }

    #[test]
    fn hardware_encoder_applies_peak_vbr_gop_and_prevents_bitrate_bloat() {
        let tmp = std::env::temp_dir().join("NVLiteReplayRust_PeakVbrRegression");
        let _ = fs::remove_dir_all(&tmp);
        let _ = fs::create_dir_all(&tmp);

        unsafe {
            let _com = ComMfGuard::new();
            let mut dev_opt: Option<ID3D11Device> = None;
            let mut ctx_opt: Option<ID3D11DeviceContext> = None;
            let levels = [D3D_FEATURE_LEVEL_11_0];
            let _ = D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&levels),
                D3D11_SDK_VERSION,
                Some(&mut dev_opt),
                None,
                Some(&mut ctx_opt),
            );
            let dev = dev_opt.expect("D3D11 device");
            let ctx = ctx_opt.expect("D3D11 ctx");
            if let Ok(mt) = dev.cast::<ID3D11Multithread>() {
                let _ = mt.SetMultithreadProtected(true);
            }
            let mut token = 0u32;
            let mut mgr_opt: Option<IMFDXGIDeviceManager> = None;
            let _ = MFCreateDXGIDeviceManager(&mut token, &mut mgr_opt);
            if let Some(ref mgr) = mgr_opt {
                let _ = mgr.ResetDevice(&dev, token);
            }

            let w = 1920u32;
            let h = 1080u32;
            let bgra_desc = D3D11_TEXTURE2D_DESC {
                Width: w,
                Height: h,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };
            let nv12_desc = D3D11_TEXTURE2D_DESC {
                Width: w,
                Height: h,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_NV12,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };
            let mut ctex_opt: Option<ID3D11Texture2D> = None;
            dev.CreateTexture2D(&bgra_desc, None, Some(&mut ctex_opt)).unwrap();
            let ctex = ctex_opt.unwrap();
            let mut nv12_ring = Vec::new();
            let mut nv12_bufs = Vec::new();
            for _ in 0..4 {
                let mut nt: Option<ID3D11Texture2D> = None;
                dev.CreateTexture2D(&nv12_desc, None, Some(&mut nt)).unwrap();
                let ntex = nt.unwrap();
                let buf = MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, &ntex, 0, false).unwrap();
                let _ = buf.SetCurrentLength(w * h * 3 / 2);
                nv12_ring.push(ntex);
                nv12_bufs.push(buf);
            }
            let conv = GpuNv12Converter::new(&dev, &ctx, &ctex, &nv12_ring, w, h, w, h, 60).unwrap();

            let (calm_frames, act_frames) = make_test_calm_and_action_frames(w, h);

            let caps = probe_hardware_codec_caps();
            let mut codecs = vec![VideoCodecType::HardwareH264];
            if caps.hevc_hw {
                codecs.push(VideoCodecType::HardwareHEVC);
            }

            for codec in codecs {
                let target_mbps = 20u32;
                let target_bps = target_mbps * 1_000_000;
                let mut action_segs = Vec::new();
                let mut action_sum = 0u64;

                for (is_action, frames) in [(false, &calm_frames), (true, &act_frames)] {
                    let seg_count = if is_action { 2 } else { 1 };
                    for sidx in 0..seg_count {
                        let mut wr = create_segment_writer(
                            mgr_opt.as_ref(),
                            w,
                            h,
                            w,
                            h,
                            60,
                            target_bps,
                            RateControlMode::Vbr,
                            is_action,
                            BufferStorageMode::DiskTempBuffer,
                            &tmp,
                            codec,
                            true,
                            true,
                        )
                        .expect("create_segment_writer");
                        assert!(
                            wr.rc_tuned,
                            "{:?}: Peak-VBR / GOP tuning must actually take effect via pEncodingParameters",
                            codec
                        );

                        let dur_hns = 10_000_000 / 60i64;
                        let pcm = vec![0u8; 960 * 2 * 2];
                        let mut audio_written = 0i64;
                        for f in 0..300u32 {
                            let slot = (f as usize) & 3;
                            let pix = &frames[(f as usize) % frames.len()];
                            ctx.UpdateSubresource(&ctex, 0, None, pix.as_ptr() as *const _, w * 4, 0);
                            let _ = conv.convert_slot(slot, f);
                            let sample = MFCreateSample().unwrap();
                            let vts = f as i64 * dur_hns;
                            let _ = sample.SetSampleTime(vts);
                            let _ = sample.SetSampleDuration(dur_hns);
                            let _ = sample.AddBuffer(&nv12_bufs[slot]);
                            wr.sink_writer.WriteSample(wr.video_stream_index, &sample).unwrap();

                            // Interleave 20ms AAC chunks with video frames (matches capture_loop).
                            if is_action && wr.has_audio {
                                while audio_written * 200_000 <= vts {
                                    let abuf = MFCreateMemoryBuffer(pcm.len() as u32).unwrap();
                                    let mut ptr = null_mut();
                                    abuf.Lock(&mut ptr, None, None).unwrap();
                                    std::ptr::copy_nonoverlapping(pcm.as_ptr(), ptr, pcm.len());
                                    abuf.Unlock().unwrap();
                                    abuf.SetCurrentLength(pcm.len() as u32).unwrap();
                                    let asamp = MFCreateSample().unwrap();
                                    asamp.SetSampleTime(audio_written * 200_000).unwrap();
                                    asamp.SetSampleDuration(200_000).unwrap();
                                    asamp.AddBuffer(&abuf).unwrap();
                                    wr.sink_writer.WriteSample(wr.audio_stream_index, &asamp).unwrap();
                                    audio_written += 1;
                                }
                            }
                        }
                        wr.sink_writer.Finalize().unwrap();
                        let path = wr.disk_path.take().unwrap();
                        drop(wr);

                        let sz = fs::metadata(&path).unwrap().len();
                        let eff_mbps = (sz as f64 * 8.0) / 5.0 / 1_000_000.0;

                        // Verify GOP 2s in actual bitstream (<= 3 keyframes in a 5s segment, never 5!)
                        let keyframes = count_mp4_keyframes(&path);
                        assert!(
                            keyframes <= 3,
                            "{:?}: expected GOP 2s (<=3 keyframes in 5s), got {}",
                            codec,
                            keyframes
                        );

                        if !is_action {
                            assert!(
                                eff_mbps < (target_mbps as f64) * 0.50,
                                "{:?}: calm scene VBR should drop below 50% of {} Mbps, got {:.2} Mbps",
                                codec,
                                target_mbps,
                                eff_mbps
                            );
                            let _ = fs::remove_file(&path);
                        } else {
                            assert!(
                                eff_mbps <= (target_mbps as f64) * 1.12,
                                "{:?}: heavy action exceeded bitrate cap ({} Mbps), got {:.2} Mbps",
                                codec,
                                target_mbps,
                                eff_mbps
                            );
                            action_sum += sz;
                            action_segs.push(ReplaySegment {
                                seq_id: sidx as u64 + 1,
                                ram_data: None,
                                disk_file: Some(DiskTempFile::new(path)),
                                size_bytes: sz,
                                duration_seconds: 5.0,
                            });
                        }
                    }
                }

                let merged = tmp.join(format!("merged_{:?}.mp4", codec));
                assert!(
                    merge_segments_pure_native(&action_segs, &merged, 60),
                    "{:?}: merge failed",
                    codec
                );
                let merged_sz = fs::metadata(&merged).unwrap().len();
                let ratio = merged_sz as f64 / action_sum.max(1) as f64;
                assert!(
                    ratio <= 1.02 && ratio >= 0.95,
                    "{:?}: stream-copy merge size drift ratio={:.3} (sum={}, merged={})",
                    codec,
                    ratio,
                    action_sum,
                    merged_sz
                );
                drop(action_segs);
                let _ = fs::remove_file(&merged);
            }
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn hardware_encoder_applies_cbr_and_respects_gop_and_bitrate_cap() {
        let tmp = std::env::temp_dir().join("NVLiteReplayRust_CbrRegression");
        let _ = fs::remove_dir_all(&tmp);
        let _ = fs::create_dir_all(&tmp);

        unsafe {
            let _com = ComMfGuard::new();
            let mut dev_opt: Option<ID3D11Device> = None;
            let mut ctx_opt: Option<ID3D11DeviceContext> = None;
            let levels = [D3D_FEATURE_LEVEL_11_0];
            let _ = D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&levels),
                D3D11_SDK_VERSION,
                Some(&mut dev_opt),
                None,
                Some(&mut ctx_opt),
            );
            let dev = dev_opt.expect("D3D11 device");
            let ctx = ctx_opt.expect("D3D11 ctx");
            if let Ok(mt) = dev.cast::<ID3D11Multithread>() {
                let _ = mt.SetMultithreadProtected(true);
            }
            let mut token = 0u32;
            let mut mgr_opt: Option<IMFDXGIDeviceManager> = None;
            let _ = MFCreateDXGIDeviceManager(&mut token, &mut mgr_opt);
            if let Some(ref mgr) = mgr_opt {
                let _ = mgr.ResetDevice(&dev, token);
            }

            let w = 1920u32;
            let h = 1080u32;
            let bgra_desc = D3D11_TEXTURE2D_DESC {
                Width: w,
                Height: h,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };
            let nv12_desc = D3D11_TEXTURE2D_DESC {
                Width: w,
                Height: h,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_NV12,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };
            let mut ctex_opt: Option<ID3D11Texture2D> = None;
            dev.CreateTexture2D(&bgra_desc, None, Some(&mut ctex_opt)).unwrap();
            let ctex = ctex_opt.unwrap();
            let mut nv12_ring = Vec::new();
            let mut nv12_bufs = Vec::new();
            for _ in 0..4 {
                let mut nt: Option<ID3D11Texture2D> = None;
                dev.CreateTexture2D(&nv12_desc, None, Some(&mut nt)).unwrap();
                let ntex = nt.unwrap();
                let buf = MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, &ntex, 0, false).unwrap();
                let _ = buf.SetCurrentLength(w * h * 3 / 2);
                nv12_ring.push(ntex);
                nv12_bufs.push(buf);
            }
            let conv = GpuNv12Converter::new(&dev, &ctx, &ctex, &nv12_ring, w, h, w, h, 60).unwrap();
            let (calm_frames, act_frames) = make_test_calm_and_action_frames(w, h);

            let caps = probe_hardware_codec_caps();
            let mut codecs = vec![VideoCodecType::HardwareH264];
            if caps.hevc_hw {
                codecs.push(VideoCodecType::HardwareHEVC);
            }

            let target_mbps = 20u32;
            let target_bps = target_mbps * 1_000_000;
            let want_cbr = eAVEncCommonRateControlMode_CBR.0 as u32;

            for codec in codecs {
                for (is_action, frames) in [(false, &calm_frames), (true, &act_frames)] {
                    let wr = create_segment_writer(
                        mgr_opt.as_ref(),
                        w,
                        h,
                        w,
                        h,
                        60,
                        target_bps,
                        RateControlMode::Cbr,
                        false,
                        BufferStorageMode::DiskTempBuffer,
                        &tmp,
                        codec,
                        true,
                        true,
                    )
                    .expect("create_segment_writer (CBR)");
                    assert!(
                        wr.rc_tuned,
                        "{:?}: CBR / GOP tuning must take effect",
                        codec
                    );

                    let mut raw_api: *mut std::ffi::c_void = null_mut();
                    let guid_null = GUID::zeroed();
                    if wr
                        .sink_writer
                        .GetServiceForStream(
                            wr.video_stream_index,
                            &guid_null,
                            &ICodecAPI::IID,
                            &mut raw_api,
                        )
                        .is_ok()
                        && !raw_api.is_null()
                    {
                        let codec_api = ICodecAPI::from_raw(raw_api);
                        if let Ok(val) = codec_api.GetValue(&CODECAPI_AVEncCommonRateControlMode) {
                            if let Ok(actual_rc) = u32::try_from(&val) {
                                assert_eq!(
                                    actual_rc, want_cbr,
                                    "{:?}: expected CBR rate control mode ({}), got {}",
                                    codec, want_cbr, actual_rc
                                );
                            }
                        }
                    }

                    let dur_hns = 10_000_000 / 60i64;
                    for f in 0..300u32 {
                        let slot = (f as usize) & 3;
                        let pix = &frames[(f as usize) % frames.len()];
                        ctx.UpdateSubresource(&ctex, 0, None, pix.as_ptr() as *const _, w * 4, 0);
                        let _ = conv.convert_slot(slot, f);
                        let sample = MFCreateSample().unwrap();
                        let vts = f as i64 * dur_hns;
                        let _ = sample.SetSampleTime(vts);
                        let _ = sample.SetSampleDuration(dur_hns);
                        let _ = sample.AddBuffer(&nv12_bufs[slot]);
                        wr.sink_writer.WriteSample(wr.video_stream_index, &sample).unwrap();
                    }
                    wr.sink_writer.Finalize().unwrap();
                    let mut wr = wr;
                    let path = wr.disk_path.take().unwrap();
                    drop(wr);

                    let sz = fs::metadata(&path).unwrap().len();
                    let eff_mbps = (sz as f64 * 8.0) / 5.0 / 1_000_000.0;
                    let keyframes = count_mp4_keyframes(&path);
                    assert!(
                        keyframes <= 3,
                        "{:?}: CBR expected GOP 2s (<=3 keyframes in 5s), got {}",
                        codec,
                        keyframes
                    );
                    if is_action {
                        assert!(
                            (target_mbps as f64 * 0.70..=target_mbps as f64 * 1.15).contains(&eff_mbps),
                            "{:?}: CBR heavy action should stay tightly around {} Mbps, got {:.2} Mbps",
                            codec,
                            target_mbps,
                            eff_mbps
                        );
                    } else {
                        assert!(
                            eff_mbps <= target_mbps as f64 * 1.15,
                            "{:?}: CBR calm scene exceeded {} Mbps cap, got {:.2} Mbps",
                            codec,
                            target_mbps,
                            eff_mbps
                        );
                    }
                    let _ = fs::remove_file(&path);
                }
            }
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn high_res_waitable_timer_paces_60fps_and_120fps_without_spin() {
        use windows::Win32::Foundation::FILETIME;
        use windows::Win32::System::Threading::{GetCurrentThread, GetThreadTimes};

        let thread_cpu_ms = || -> f64 {
            unsafe {
                let mut create = FILETIME::default();
                let mut exit = FILETIME::default();
                let mut kernel = FILETIME::default();
                let mut user = FILETIME::default();
                let _ = GetThreadTimes(
                    GetCurrentThread(),
                    &mut create,
                    &mut exit,
                    &mut kernel,
                    &mut user,
                );
                let k = ((kernel.dwHighDateTime as u64) << 32) | (kernel.dwLowDateTime as u64);
                let u = ((user.dwHighDateTime as u64) << 32) | (user.dwLowDateTime as u64);
                (k + u) as f64 / 10_000.0
            }
        };

        unsafe {
            let timer = HighResWaitableTimer::new();
            assert!(
                timer.is_high_res(),
                "CREATE_WAITABLE_TIMER_HIGH_RESOLUTION should succeed on Windows 10 1803+ / Windows 11"
            );

            let cpu_before = thread_cpu_ms();
            for (fps, frames) in [(60u32, 30u64), (120u32, 60u64)] {
                let interval_ms = 1000.0 / fps as f64;
                let clock = Instant::now();
                for tick in 1..=frames {
                    let target_ms = tick as f64 * interval_ms;
                    let now_ms = clock.elapsed().as_secs_f64() * 1000.0;
                    if now_ms < target_ms {
                        timer.wait_ms(target_ms - now_ms);
                    }
                }
                let elapsed_ms = clock.elapsed().as_secs_f64() * 1000.0;
                let expected_ms = frames as f64 * interval_ms;
                assert!(
                    (elapsed_ms - expected_ms).abs() <= 35.0,
                    "{} FPS pacing drifted: elapsed={:.2} ms (expected {:.2} ms)",
                    fps,
                    elapsed_ms,
                    expected_ms
                );
            }
            let cpu_used_ms = thread_cpu_ms() - cpu_before;
            assert!(
                cpu_used_ms < 25.0,
                "Waitable timer should avoid spin-loop CPU burn: used {:.2} ms CPU over 1000 ms wall time",
                cpu_used_ms
            );
        }
    }

    #[test]
    fn ram_segment_clone_shares_arc_buffer_zero_copy() {
        let payload: Arc<[u8]> = Arc::from(vec![0x5Au8; 4 * 1024 * 1024].into_boxed_slice());
        let seg = ReplaySegment {
            seq_id: 42,
            ram_data: Some(Arc::clone(&payload)),
            disk_file: None,
            size_bytes: payload.len() as u64,
            duration_seconds: 5.0,
        };
        assert_eq!(Arc::strong_count(&payload), 2);
        let snapshot = vec![seg.clone()];
        assert_eq!(
            Arc::strong_count(&payload),
            3,
            "Cloning ReplaySegment in RAM mode must increment Arc refcount without duplicating buffer bytes"
        );
        assert!(Arc::ptr_eq(
            seg.ram_data.as_ref().unwrap(),
            snapshot[0].ram_data.as_ref().unwrap()
        ));
    }

    #[test]
    fn foreground_game_name_resolution_and_template_output_path() {
        use crate::config::DEFAULT_GAME_NAME_MAP;

        // 1. Custom map wins (e.g. msedge.exe -> Browser, explorer.exe -> Desktop)
        assert_eq!(
            resolve_game_name_for_exe_path(
                Path::new(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"),
                DEFAULT_GAME_NAME_MAP
            ),
            "Browser"
        );
        assert_eq!(
            resolve_game_name_for_exe_path(
                Path::new(r"C:\Windows\explorer.exe"),
                DEFAULT_GAME_NAME_MAP
            ),
            "Desktop"
        );

        // 2. GetFileVersionInfoW reads real Windows PE version info (FileDescription fallback when
        // ProductName is generic "Microsoft® Windows® Operating System")
        let cmd_path = Path::new(r"C:\Windows\System32\cmd.exe");
        if cmd_path.exists() {
            let info = query_exe_version_product_name(cmd_path);
            assert!(
                info.as_ref().map(|s| !s.is_empty()).unwrap_or(false),
                "Expected FileDescription/ProductName from cmd.exe via GetFileVersionInfoW, got {:?}",
                info
            );
        }

        // 3. unique_replay_output_path creates game subfolder & handles collision
        let tmp = std::env::temp_dir().join(format!("nvlite_tpl_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let p1 = unique_replay_output_path(&tmp, "{game}/Replay_{date}_{time}", "Valorant");
        assert_eq!(p1.parent().unwrap(), tmp.join("Valorant"));
        assert!(p1.parent().unwrap().is_dir());
        let name1 = p1.file_name().unwrap().to_string_lossy().to_string();
        assert!(name1.starts_with("Replay_") && name1.ends_with(".mp4"));

        // Touch p1 so the second call picks _2.mp4
        fs::write(&p1, b"test").unwrap();
        let p2 = unique_replay_output_path(&tmp, "{game}/Replay_{date}_{time}", "Valorant");
        assert_ne!(p1, p2);
        assert!(p2
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with("_2.mp4"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn mmcss_capture_thread_guard_registers_and_reverts_cleanly() {
        let guard = MmcssThreadGuard::new("Capture");
        assert!(
            guard.is_active(),
            "AvSetMmThreadCharacteristicsW(\"Capture\") should succeed on Windows"
        );
        drop(guard);
    }

    #[test]
    fn active_game_audio_filter_and_process_liveness() {
        assert!(is_process_alive(std::process::id()));
        assert!(!is_process_alive(0));

        for util in [
            r"C:\Users\User\AppData\Local\Discord\app-1.0.9000\Discord.exe",
            r"C:\Users\User\AppData\Roaming\Spotify\Spotify.exe",
            r"C:\Windows\explorer.exe",
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        ] {
            assert!(
                is_background_utility_exe(Path::new(util)),
                "{} should be treated as background utility so Alt-Tab doesn't steal audio lock",
                util
            );
        }

        for game in [
            r"D:\Riot Games\VALORANT\live\ShooterGame\Binaries\Win64\VALORANT-Win64-Shipping.exe",
            r"C:\Program Files (x86)\Steam\steamapps\common\Counter-Strike Global Offensive\game\bin\win64\cs2.exe",
        ] {
            assert!(
                !is_background_utility_exe(Path::new(game)),
                "{} should be allowed for Active Game Audio loopback lock",
                game
            );
        }
    }

    #[test]
    fn capture_helpers_and_wgc_fallback_init_cleanly() {
        let engine = ReplayEngine::new(ReplayConfig::default());
        let displays = engine.displays.lock().unwrap().clone();
        assert!(!displays.is_empty(), "at least one display must be enumerated");

        // Run across two sequential worker thread lifecycles to verify stop/start cycles
        // never dangle WinRT FactoryCache pointers after ComMfGuard drops.
        for _cycle in 0..2 {
            let mon = displays[0].clone();
            let h = thread::spawn(move || unsafe {
                let _com = ComMfGuard::new();
                set_capture_thread_dpi_aware();

                let mut dev_opt: Option<ID3D11Device> = None;
                let mut ctx_opt: Option<ID3D11DeviceContext> = None;
                let levels = [D3D_FEATURE_LEVEL_11_0];
                let _ = D3D11CreateDevice(
                    None,
                    D3D_DRIVER_TYPE_HARDWARE,
                    HMODULE::default(),
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    Some(&levels),
                    D3D11_SDK_VERSION,
                    Some(&mut dev_opt),
                    None,
                    Some(&mut ctx_opt),
                );
                let dev = dev_opt.expect("D3D11 hardware device");
                let _ctx = ctx_opt.expect("D3D11 context");

                let matched = find_matching_dxgi_output(&dev, &mon.device_name, mon.output_index)
                    .expect("find_matching_dxgi_output should locate primary monitor");
                let desc = matched.GetDesc().expect("IDXGIOutput::GetDesc");
                if !mon.device_name.is_empty() {
                    assert!(
                        utf16_slice_to_string(&desc.DeviceName)
                            .eq_ignore_ascii_case(&mon.device_name),
                        "matched output DeviceName should equal target monitor"
                    );
                }

                let hmon = resolve_target_hmonitor(&dev, &mon)
                    .expect("resolve_target_hmonitor should return non-null HMONITOR");
                assert!(!hmon.0.is_null());

                let dupl = try_create_output_duplication(&dev, &mon.device_name, mon.output_index);
                drop(dupl);

                if GraphicsCaptureSession::IsSupported().unwrap_or(false) {
                    let mut wgc = WgcMonitorCapture::new(&dev, &mon)
                        .expect("WgcMonitorCapture::new should succeed when WGC is supported");
                    let tex_desc = D3D11_TEXTURE2D_DESC {
                        Width: mon.width,
                        Height: mon.height,
                        MipLevels: 1,
                        ArraySize: 1,
                        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                        Usage: D3D11_USAGE_DEFAULT,
                        BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0)
                            as u32,
                        CPUAccessFlags: 0,
                        MiscFlags: 0,
                    };
                    let mut staging: Option<ID3D11Texture2D> = None;
                    let _ = dev.CreateTexture2D(&tex_desc, None, Some(&mut staging));
                    let mut sw = mon.width;
                    let mut sh = mon.height;
                    let mut clear = false;
                    for _ in 0..8 {
                        if wgc.acquire_latest_frame(
                            &dev,
                            &_ctx,
                            &mut staging,
                            &mut sw,
                            &mut sh,
                            &tex_desc,
                            mon.width,
                            mon.height,
                            &mut clear,
                        ) {
                            break;
                        }
                        thread::sleep(Duration::from_millis(15));
                    }
                    drop(wgc);
                }
            });
            h.join().expect("capture helper worker thread must not panic");
        }
    }
}
