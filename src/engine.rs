use std::fs;
use std::mem::ManuallyDrop;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use windows::core::{Interface, GUID, HSTRING, PCWSTR, VARIANT};
use windows::Win32::Foundation::{BOOL, HMODULE, HWND, LPARAM, RECT, WPARAM};
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
    CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1, IDXGIOutput1, IDXGIOutputDuplication,
    DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO,
};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, EnumDisplayMonitors,
    EnumDisplaySettingsW, GetDC, GetMonitorInfoW, ReleaseDC, SelectObject, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DEVMODEW, DIB_RGB_COLORS, ENUM_CURRENT_SETTINGS, HBITMAP, HDC,
    HGDIOBJ, HMONITOR, MONITORINFOEXW, SRCCOPY,
};
use windows::Win32::Media::MediaFoundation::{
    eAVEncCommonRateControlMode_PeakConstrainedVBR, eAVEncH264VProfile_High,
    eAVEncH265VProfile_Main_420_8, ICodecAPI, IMFActivate, IMFByteStream, IMFDXGIDeviceManager,
    IMFMediaBuffer, IMFMediaType, IMFSample, IMFSinkWriter, IMFSourceReader, MFAudioFormat_AAC,
    MFAudioFormat_PCM, MFCreateAttributes, MFCreateDXGIDeviceManager, MFCreateDXGISurfaceBuffer,
    MFCreateMFByteStreamOnStream, MFCreateMediaType, MFCreateMemoryBuffer, MFCreateSample,
    MFCreateSinkWriterFromURL, MFCreateSourceReaderFromByteStream, MFCreateSourceReaderFromURL,
    MFMediaType_Audio, MFMediaType_Video, MFNominalRange_0_255, MFNominalRange_16_235, MFShutdown,
    MFStartup, MFTEnumEx, MFVideoFormat_ARGB32, MFVideoFormat_AV1, MFVideoFormat_H264,
    MFVideoFormat_HEVC, MFVideoFormat_NV12, MFVideoPrimaries_BT709, MFVideoTransFunc_709,
    MFVideoTransferMatrix_BT709, CODECAPI_AVEncCommonMaxBitRate, CODECAPI_AVEncCommonMeanBitRate,
    CODECAPI_AVEncCommonQualityVsSpeed, CODECAPI_AVEncCommonRateControlMode,
    CODECAPI_AVEncMPVGOPSize, MFT_CATEGORY_VIDEO_ENCODER, MFT_ENUM_FLAG_HARDWARE,
    MFT_ENUM_FLAG_SORTANDFILTER, MFT_REGISTER_TYPE_INFO, MF_LOW_LATENCY,
    MF_MT_AUDIO_AVG_BYTES_PER_SECOND, MF_MT_AUDIO_BITS_PER_SAMPLE, MF_MT_AUDIO_BLOCK_ALIGNMENT,
    MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND, MF_MT_AVG_BITRATE, MF_MT_FRAME_RATE,
    MF_MT_FRAME_SIZE, MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_MPEG2_PROFILE,
    MF_MT_MPEG_SEQUENCE_HEADER, MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SUBTYPE, MF_MT_TRANSFER_FUNCTION,
    MF_MT_VIDEO_NOMINAL_RANGE, MF_MT_VIDEO_PRIMARIES, MF_MT_YUV_MATRIX,
    MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, MF_SINK_WRITER_D3D_MANAGER,
    MF_SINK_WRITER_DISABLE_THROTTLING, MF_SOURCE_READERF_ENDOFSTREAM, MF_SOURCE_READERF_ERROR,
    MF_SOURCE_READER_FIRST_AUDIO_STREAM, MF_SOURCE_READER_FIRST_VIDEO_STREAM, MF_VERSION,
};
use windows::Win32::Media::{timeBeginPeriod, timeEndPeriod};
use windows::Win32::System::Com::{
    CoInitializeEx, CoTaskMemFree, CoUninitialize, IStream, COINIT_MULTITHREADED, STATFLAG_NONAME,
    STATSTG, STREAM_SEEK_SET,
};
use windows::Win32::System::Diagnostics::Debug::Beep;
use windows::Win32::System::ProcessStatus::EmptyWorkingSet;
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::System::Threading::GetCurrentProcess;
use windows::Win32::UI::Shell::SHCreateMemStream;
use windows::Win32::UI::WindowsAndMessaging::{GetDesktopWindow, PostMessageW};

use crate::audio::{enumerate_audio_input_devices, AudioInputDevice, WasapiStream};
use crate::config::{BufferStorageMode, ReplayConfig, VideoCodecType};
use crate::voice::VoiceRecognizer;

pub const WM_APP_STATUS: u32 = 0x8000 + 101;
pub const WM_APP_SAVED: u32 = 0x8000 + 102;
pub const WM_APP_VOICE_TRIGGER: u32 = 0x8000 + 103;

#[derive(Clone, Debug)]
pub struct DisplayTarget {
    pub index: usize,
    pub adapter_index: u32,
    pub output_index: u32,
    #[allow(dead_code)]
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

#[derive(Clone)]
pub struct ReplaySegment {
    pub ram_data: Option<Vec<u8>>,
    pub disk_path: Option<PathBuf>,
    pub size_bytes: u64,
    pub duration_seconds: f64,
    /// Capture order; background finalizers can complete out of order.
    pub seq: u32,
}

impl ReplaySegment {
    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
}

struct ComMfGuard {
    with_timer: bool,
    com_owned: bool,
}

impl ComMfGuard {
    unsafe fn new(with_timer: bool) -> Self {
        // RPC_E_CHANGED_MODE on an STA thread (UI) must not be paired with CoUninitialize.
        let com_owned = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        if with_timer {
            timeBeginPeriod(1);
        }
        let _ = MFStartup(MF_VERSION, 0);
        Self { with_timer, com_owned }
    }
}

impl Drop for ComMfGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
            if self.with_timer {
                timeEndPeriod(1);
            }
            if self.com_owned {
                CoUninitialize();
            }
        }
    }
}

struct ActiveSegmentWriter {
    sink_writer: IMFSinkWriter,
    mem_stream: Option<IStream>,
    _mf_byte_stream: Option<IMFByteStream>,
    disk_path: Option<PathBuf>,
    video_stream_index: u32,
    audio_stream_index: u32,
    has_audio: bool,
    frames_written: u32,
    audio_samples_written: i64,
    actual_codec: VideoCodecType,
    vbr_tuned: bool,
    seq: u32,
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
    pub displays: Mutex<Vec<DisplayTarget>>,
    pub mic_devices: Mutex<Vec<AudioInputDevice>>,
    pub active_mic_name: Mutex<String>,
    pub gpu_name: Mutex<String>,
    pub codec_caps: Mutex<HardwareCodecCaps>,
    pub capture_mode_status: Mutex<String>,
    pub video_pipeline_status: Mutex<String>,
    pub status_message: Mutex<String>,
    pub last_saved_file: Mutex<String>,

    segments: Mutex<Vec<ReplaySegment>>,
    is_running: AtomicBool,
    is_stopping: AtomicBool,
    is_saving: AtomicBool,
    force_rotate: AtomicBool,
    pending_finalize: AtomicUsize,
    rotate_notify: (Mutex<bool>, Condvar),

    current_fps_x10: AtomicU64,
    ui_hwnd: Mutex<isize>,
    capture_thread: Mutex<Option<JoinHandle<()>>>,
}

impl ReplayEngine {
    pub fn new(config: ReplayConfig) -> Arc<Self> {
        let voice = VoiceRecognizer::new(&config.voice_phrase);
        let caps = probe_hardware_codec_caps();
        let engine = Arc::new(Self {
            config: Mutex::new(config),
            voice,
            displays: Mutex::new(Vec::new()),
            mic_devices: Mutex::new(Vec::new()),
            active_mic_name: Mutex::new(String::new()),
            gpu_name: Mutex::new("Hardware GPU Encoder".to_string()),
            codec_caps: Mutex::new(caps),
            capture_mode_status: Mutex::new("Standby".to_string()),
            video_pipeline_status: Mutex::new("Standby".to_string()),
            status_message: Mutex::new("Siap digunakan.".to_string()),
            last_saved_file: Mutex::new(String::new()),
            segments: Mutex::new(Vec::new()),
            is_running: AtomicBool::new(false),
            is_stopping: AtomicBool::new(false),
            is_saving: AtomicBool::new(false),
            force_rotate: AtomicBool::new(false),
            pending_finalize: AtomicUsize::new(0),
            rotate_notify: (Mutex::new(false), Condvar::new()),
            current_fps_x10: AtomicU64::new(0),
            ui_hwnd: Mutex::new(0),
            capture_thread: Mutex::new(None),
        });
        engine.refresh_displays_and_gpu();
        engine.refresh_audio_inputs();
        engine
    }

    pub fn set_ui_hwnd(&self, hwnd: HWND) {
        if let Ok(mut guard) = self.ui_hwnd.lock() {
            *guard = hwnd.0 as isize;
        }
    }

    fn raise_status(&self, msg: String) {
        if let Ok(mut guard) = self.status_message.lock() {
            *guard = msg;
        }
        let raw_hwnd = self.ui_hwnd.lock().map(|g| *g).unwrap_or(0);
        if raw_hwnd != 0 {
            unsafe {
                let _ = PostMessageW(HWND(raw_hwnd as *mut _), WM_APP_STATUS, WPARAM(0), LPARAM(0));
            }
        }
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
        // Leftovers from a run that died on its own (seq/paths restart from scratch).
        self.clear_buffer();

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

        self.clear_buffer();
        if let Ok(mut s) = self.capture_mode_status.lock() {
            *s = "Standby".to_string();
        }
        if let Ok(mut p) = self.video_pipeline_status.lock() {
            *p = "Standby".to_string();
        }
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

    fn wait_for_save(&self) {
        // ponytail: 30s cap so quit can't hang forever on a stuck merge
        let deadline = Instant::now() + Duration::from_secs(30);
        while self.is_saving.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
    }

    pub fn clear_buffer(&self) {
        self.wait_for_save();
        let drained: Vec<ReplaySegment> = if let Ok(mut segs) = self.segments.lock() {
            segs.drain(..).collect()
        } else {
            Vec::new()
        };
        for seg in drained {
            if let Some(ref p) = seg.disk_path {
                let _ = fs::remove_file(p);
            }
        }
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
            let _mf_guard = unsafe { ComMfGuard::new(false) };
            engine.raise_status("Menyimpan Instant Replay...".to_string());

            {
                let (lock, cv) = &engine.rotate_notify;
                if let Ok(mut done) = lock.lock() {
                    *done = false;
                }
                engine.force_rotate.store(true, Ordering::Release);
                if let Ok(done) = lock.lock() {
                    let _ = cv.wait_timeout_while(done, Duration::from_millis(1200), |d| !*d);
                }
            }
            // A natural rotation just before the save may still be finalizing the previous
            // segment in the background; without it the clip has a gap.
            let fin_deadline = Instant::now() + Duration::from_secs(3);
            while engine.pending_finalize.load(Ordering::Acquire) > 0 && Instant::now() < fin_deadline {
                thread::sleep(Duration::from_millis(10));
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

            let mut file_name = local_replay_filename();
            let mut n = 2;
            while out_dir.join(&file_name).exists() {
                file_name = format!("{}_{}.mp4", local_replay_filename().trim_end_matches(".mp4"), n);
                n += 1;
            }
            let output_path = out_dir.join(&file_name);

            let t0 = Instant::now();
            let ok = merge_segments_pure_native(&snapshot, &output_path, cfg.fps);
            let elapsed_ms = t0.elapsed().as_millis();

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
                            file_name, size_mb, elapsed_ms
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

    fn finalize_and_store_segment(
        self: &Arc<Self>,
        mut seg_writer: ActiveSegmentWriter,
        fps: u32,
        mode: BufferStorageMode,
    ) {
        if seg_writer.frames_written == 0 {
            let orphaned = seg_writer.disk_path.take();
            drop(seg_writer);
            if let Some(p) = orphaned {
                let _ = fs::remove_file(p);
            }
            return;
        }
        unsafe {
            if !self.is_running.load(Ordering::Acquire) {
                let orphaned = seg_writer.disk_path.take();
                drop(seg_writer);
                if let Some(p) = orphaned {
                    let _ = fs::remove_file(p);
                }
                return;
            }
            if seg_writer.sink_writer.Finalize().is_err() {
                let orphaned = seg_writer.disk_path.take();
                drop(seg_writer);
                if let Some(p) = orphaned {
                    let _ = fs::remove_file(p);
                }
                return;
            }
            let duration = seg_writer.frames_written as f64 / fps.max(1) as f64;
            let mut seg = ReplaySegment {
                ram_data: None,
                disk_path: None,
                size_bytes: 0,
                duration_seconds: duration,
                seq: seg_writer.seq,
            };

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
                            seg.ram_data = Some(data);
                        }
                    }
                }
            } else if let Some(path) = seg_writer.disk_path.take() {
                if let Ok(meta) = fs::metadata(&path) {
                    seg.size_bytes = meta.len();
                    seg.disk_path = Some(path);
                }
            }
            // Drop writer explicitly before pushing/cleaning so MP4 file handles are closed
            drop(seg_writer);

            if seg.ram_data.is_some() || seg.disk_path.is_some() {
                if !self.is_running.load(Ordering::Acquire) {
                    if let Some(ref p) = seg.disk_path {
                        let _ = fs::remove_file(p);
                    }
                    return;
                }
                let (max_dur, cur_mode) = {
                    let cfg = self.config.lock().unwrap();
                    (
                        (cfg.replay_duration_seconds + cfg.segment_duration_seconds) as f64,
                        cfg.storage_mode,
                    )
                };
                let mut evicted = false;
                let mut evicted_paths = Vec::new();
                {
                    let mut segs = self.segments.lock().unwrap();
                    if !self.is_running.load(Ordering::Acquire) {
                        if let Some(p) = seg.disk_path.take() {
                            evicted_paths.push(p);
                        }
                    } else {
                        let pos = segs.partition_point(|s| s.seq < seg.seq);
                        segs.insert(pos, seg);
                        while segs.len() > 1 {
                            let total: f64 = segs.iter().map(|s| s.duration_seconds).sum();
                            if total <= max_dur {
                                break;
                            }
                            let old = segs.remove(0);
                            if let Some(p) = old.disk_path {
                                evicted_paths.push(p);
                            }
                            evicted = true;
                        }
                    }
                }
                for p in evicted_paths {
                    let _ = fs::remove_file(p);
                }
                if evicted && cur_mode == BufferStorageMode::RamRingBuffer {
                    trim_working_set();
                }
            }
        }
    }

    fn capture_loop(engine: Arc<Self>) {
        let _com_mf_guard = unsafe { ComMfGuard::new(true) };

        engine.refresh_displays_and_gpu();
        let displays = engine.displays.lock().unwrap().clone();
        let cfg_init = engine.config.lock().unwrap().clone();

        if displays.is_empty() {
            engine.raise_status("Error: Tidak ada monitor yang terdeteksi.".to_string());
            engine.is_running.store(false, Ordering::Release);
            return;
        }

        let mon_idx = cfg_init.monitor_index.min(displays.len() - 1);
        let target_mon = displays[mon_idx].clone();

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
        let storage_mode = cfg_init.storage_mode;
        let temp_dir = cfg_init.effective_temp_dir();
        let codec = cfg_init.codec;
        let frames_per_segment = cfg_init.segment_duration_seconds * fps;

        unsafe {
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
                engine.raise_status("Gagal menginisialisasi D3D11 GPU device.".to_string());
                engine.is_running.store(false, Ordering::Release);
                return;
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

            let mut desk_dupl =
                try_create_output_duplication(&target_adapter, target_mon.output_index, &device);
            let dupl_supported = desk_dupl.is_some();

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

            let safe_out_w = (out_width & !1).max(640);
            let safe_out_h = (out_height & !1).max(360);

            let want_audio = cfg_init.record_desktop_audio || cfg_init.record_microphone;
            let cfg_mic_id = cfg_init.effective_mic_device_id().map(str::to_string);
            let mut desktop_audio = WasapiStream::new();
            let mut mic_audio = WasapiStream::new();
            if cfg_init.record_desktop_audio {
                desktop_audio.initialize(true);
            }
            if cfg_init.record_microphone || cfg_init.voice_trigger_enabled {
                if mic_audio.initialize_with_device(false, cfg_mic_id.as_deref()) {
                    if let Ok(mut g) = engine.active_mic_name.lock() {
                        *g = mic_audio.active_device_name().to_string();
                    }
                } else if let Ok(mut g) = engine.active_mic_name.lock() {
                    g.clear();
                }
                if cfg_init.voice_trigger_enabled && mic_audio.is_available() {
                    mic_audio.set_voice_tap_enabled(true);
                }
            } else if let Ok(mut g) = engine.active_mic_name.lock() {
                g.clear();
            }
            let audio_active = want_audio
                && ((cfg_init.record_desktop_audio && desktop_audio.is_available())
                    || (cfg_init.record_microphone && mic_audio.is_available()));

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

            let mut seg_counter: u32 = 1;
            let mut active_seq: u32 = 0;
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
                                audio_active,
                                storage_mode,
                                &temp_dir,
                                codec,
                                true,
                                true,
                                seg_counter,
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
                    engine.raise_status("Gagal membuat GPU texture ring.".to_string());
                    engine.is_running.store(false, Ordering::Release);
                    return;
                }
                active_writer = create_segment_writer(
                    dev_mgr_opt.as_ref(),
                    cap_width,
                    cap_height,
                    safe_out_w,
                    safe_out_h,
                    fps,
                    bitrate_bps,
                    audio_active,
                    storage_mode,
                    &temp_dir,
                    codec,
                    false,
                    true,
                    seg_counter,
                );
            }

            let Some(first_writer) = active_writer else {
                engine.raise_status("Gagal menginisialisasi hardware video encoder.".to_string());
                engine.is_running.store(false, Ordering::Release);
                return;
            };
            let active_codec = first_writer.actual_codec;
            let vbr_tuned = first_writer.vbr_tuned;
            let writer_in_w = if use_nv12 { safe_out_w } else { cap_width };
            let writer_in_h = if use_nv12 { safe_out_h } else { cap_height };
            let mut active_writer = Some(first_writer);

            let mut cursor_comp = crate::cursor::DxgiCursorCompositor::new(&device);
            let _keep_nv12_ring = nv12_tex_ring;

            let standby_slot: Arc<Mutex<(bool, Option<ActiveSegmentWriter>)>> =
                Arc::new(Mutex::new((false, None)));
            let mut prewarm_handle: Option<JoinHandle<()>> = None;
            let mut finalizer_handles: Vec<JoinHandle<()>> = Vec::new();

            if let Ok(mut g) = engine.gpu_name.lock() {
                *g = format!("{} ({})", target_mon.adapter_name, target_mon.vendor_tag);
            }
            let codec_label = match active_codec {
                VideoCodecType::HardwareAV1 => "AV1",
                VideoCodecType::HardwareHEVC => "HEVC",
                VideoCodecType::HardwareH264 => "H.264 High",
            };
            if let Ok(mut s) = engine.capture_mode_status.lock() {
                *s = format!(
                    "{}{}{}",
                    if desk_dupl.is_some() {
                        "DXGI "
                    } else {
                        "D3D11 "
                    },
                    if use_nv12 { "NV12 + " } else { "Direct + " },
                    target_mon.vendor_tag
                );
            }
            if let Ok(mut p) = engine.video_pipeline_status.lock() {
                *p = format!(
                    "{} BT.709 | {} | {}",
                    if use_nv12 { "NV12 GPU" } else { "ARGB32" },
                    if vbr_tuned { "Peak-VBR+GOP" } else { "Standard RC" },
                    codec_label
                );
            }

            let mut gdi_state = GdiFallbackState::default();
            let mut staging_w = cap_width;
            let mut staging_h = cap_height;
            let mut need_size_check = true;
            let mut need_ring_clear = false;
            let mut last_dupl_retry = Instant::now();

            let frame_dur_hns: i64 = 10_000_000 / fps as i64;
            let frame_interval_ms: f64 = 1000.0 / fps as f64;
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
            let mut last_mic_retry = Instant::now() - Duration::from_secs(10);

            while engine.is_running.load(Ordering::Acquire) {
                let target_ms = next_frame_tick as f64 * frame_interval_ms;
                let now_ms = loop_clock.elapsed().as_secs_f64() * 1000.0;
                if now_ms < target_ms {
                    let sleep_ms = (target_ms - now_ms) as u64;
                    if sleep_ms > 1 {
                        thread::sleep(Duration::from_millis(sleep_ms - 1));
                    }
                    while loop_clock.elapsed().as_secs_f64() * 1000.0 < target_ms {
                        std::hint::spin_loop();
                    }
                }
                next_frame_tick += 1;

                // Pre-warm next standby segment writer 1 second before rotation
                let aw_frames = active_writer.as_ref().map(|w| w.frames_written).unwrap_or(0);
                if aw_frames + fps >= frames_per_segment {
                    let mut st_guard = standby_slot.lock().unwrap();
                    if !st_guard.0 && st_guard.1.is_none() {
                        st_guard.0 = true;
                        seg_counter += 1;
                        let next_id = seg_counter;
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
                                let _mf_guard = ComMfGuard::new(false);
                                let mgr_wrapped = mgr_wrapped;
                                let built = create_segment_writer(
                                    mgr_wrapped.0.as_ref(),
                                    writer_in_w,
                                    writer_in_h,
                                    safe_out_w,
                                    safe_out_h,
                                    fps,
                                    bitrate_bps,
                                    audio_active,
                                    storage_mode,
                                    &temp_dir_pw,
                                    active_codec,
                                    use_nv12,
                                    true,
                                    next_id,
                                );
                                if let Ok(mut g) = slot_clone.lock() {
                                    g.1 = built;
                                    g.0 = false;
                                }
                            })
                            .ok();
                    }
                }

                // 1. Capture desktop frame
                let mut captured_via_dupl = false;
                if desk_dupl.is_none() && dupl_supported {
                    captured_via_dupl = true;
                    if last_dupl_retry.elapsed() >= Duration::from_millis(250) {
                        last_dupl_retry = Instant::now();
                        desk_dupl = try_create_output_duplication(
                            &target_adapter,
                            target_mon.output_index,
                            &device,
                        );
                        if desk_dupl.is_some() {
                            need_size_check = true;
                            if let Some(ref mut comp) = cursor_comp {
                                comp.on_duplication_reset();
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
                                        }
                                    }
                                }
                            }
                            let _ = dupl.ReleaseFrame();
                            context.Flush();
                            captured_via_dupl = true;
                        }
                        Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => {
                            captured_via_dupl = true;
                        }
                        Err(e) if e.code() == DXGI_ERROR_ACCESS_LOST || e.code().is_err() => {
                            drop(desk_dupl.take());
                            last_dupl_retry = Instant::now();
                            desk_dupl = try_create_output_duplication(
                                &target_adapter,
                                target_mon.output_index,
                                &device,
                            );
                            if desk_dupl.is_some() {
                                need_size_check = true;
                                if let Some(ref mut comp) = cursor_comp {
                                    comp.on_duplication_reset();
                                }
                            }
                            captured_via_dupl = true;
                        }
                        Err(_) => {}
                    }
                }

                let ring_slot = (aw_frames as usize) & (RING_SLOTS - 1);
                let (dst_bgra_tex, dst_bgra_rtv) = if use_nv12 {
                    (
                        comp_bgra_tex.as_ref().unwrap(),
                        comp_bgra_rtv.as_ref().unwrap(),
                    )
                } else {
                    (&gpu_tex_ring[ring_slot], &gpu_rtv_ring[ring_slot])
                };

                if captured_via_dupl {
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
                    if copied_frame {
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
                    let _ = conv.convert_slot(ring_slot, aw_frames);
                }

                let Some(aw) = active_writer.as_mut() else {
                    break;
                };

                // 2. Write Video Sample
                if let Ok(sample) = MFCreateSample() {
                    let sample_time = aw.frames_written as i64 * frame_dur_hns;
                    let _ = sample.SetSampleTime(sample_time);
                    let _ = sample.SetSampleDuration(frame_dur_hns);
                    let _ = sample.AddBuffer(&dxgi_buf_ring[ring_slot]);
                    let _ = aw
                        .sink_writer
                        .WriteSample(aw.video_stream_index, &sample);
                }

                // 3. Drain & Write Audio Chunks + Voice Recognition Tap
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
                if voice_want_mic
                    && !mic_audio.is_available()
                    && last_mic_retry.elapsed() >= Duration::from_secs(3)
                {
                    if mic_audio.initialize_with_device(false, cfg_mic_id.as_deref()) {
                        if let Ok(mut g) = engine.active_mic_name.lock() {
                            *g = mic_audio.active_device_name().to_string();
                        }
                    }
                    last_mic_retry = Instant::now();
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

                    loop {
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
                                    let atime =
                                        (aw.audio_samples_written * 10_000_000) / 48_000;
                                    let adur = (AUDIO_CHUNK_FRAMES as i64 * 10_000_000) / 48_000;
                                    let _ = asample.SetSampleTime(atime);
                                    let _ = asample.SetSampleDuration(adur);
                                    let _ = asample.AddBuffer(&abuf);
                                    let _ = aw
                                        .sink_writer
                                        .WriteSample(aw.audio_stream_index, &asample);
                                    aw.audio_samples_written += AUDIO_CHUNK_FRAMES as i64;
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
                }

                // 4. Segment Rotation
                let forced = engine.force_rotate.load(Ordering::Acquire);
                if cur_aw_frames >= frames_per_segment || (forced && cur_aw_frames >= 10) {
                    let was_forced = engine.force_rotate.swap(false, Ordering::AcqRel);
                    let Some(finished) = active_writer.take() else {
                        break;
                    };

                    if let Some(h) = prewarm_handle.take() {
                        if h.is_finished() {
                            let _ = h.join();
                        } else {
                            finalizer_handles.push(h);
                        }
                    }

                    let prewarmed = standby_slot.lock().ok().and_then(|mut g| g.1.take());
                    active_writer = if let Some(w) = prewarmed {
                        Some(w)
                    } else {
                        seg_counter += 1;
                        let Some(fallback_w) = create_segment_writer(
                            dev_mgr_opt.as_ref(),
                            writer_in_w,
                            writer_in_h,
                            safe_out_w,
                            safe_out_h,
                            fps,
                            bitrate_bps,
                            audio_active,
                            storage_mode,
                            &temp_dir,
                            active_codec,
                            use_nv12,
                            true,
                            seg_counter,
                        ) else {
                            discard_segment_writer(finished);
                            break;
                        };
                        Some(fallback_w)
                    };
                    active_seq += 1;
                    if let Some(w) = active_writer.as_mut() {
                        w.seq = active_seq;
                    }
                    seg_audio_clock = Instant::now();

                    finalizer_handles.retain(|h| !h.is_finished());
                    if was_forced {
                        engine.finalize_and_store_segment(finished, fps, storage_mode);
                        let (lock, cv) = &engine.rotate_notify;
                        if let Ok(mut done) = lock.lock() {
                            *done = true;
                            cv.notify_all();
                        }
                    } else {
                        let eng_bg = Arc::clone(&engine);
                        engine.pending_finalize.fetch_add(1, Ordering::AcqRel);
                        match thread::Builder::new()
                            .name("NVLiteReplayRust_FinalizeSeg".to_string())
                            .spawn(move || {
                                let _mf_guard = ComMfGuard::new(false);
                                eng_bg.finalize_and_store_segment(finished, fps, storage_mode);
                                eng_bg.pending_finalize.fetch_sub(1, Ordering::AcqRel);
                            }) {
                            Ok(jh) => finalizer_handles.push(jh),
                            Err(_) => {
                                engine.pending_finalize.fetch_sub(1, Ordering::AcqRel);
                            }
                        }
                    }
                }
            }

            // Clean up in-flight writers, prewarm thread, and background finalizers BEFORE
            // `_com_mf_guard` tears down Media Foundation and COM!
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
            // Loop exited on its own (writer creation failed): don't keep reporting "running"
            // while saves export a frozen buffer.
            if engine.is_running.swap(false, Ordering::AcqRel) {
                engine.raise_status(
                    "Error: perekaman berhenti (encoder gagal). Nyalakan ulang Replay.".to_string(),
                );
            }
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

unsafe fn try_create_output_duplication(
    adapter: &Option<IDXGIAdapter1>,
    output_idx: u32,
    device: &ID3D11Device,
) -> Option<IDXGIOutputDuplication> {
    let adap = adapter.as_ref()?;
    let output = adap.EnumOutputs(output_idx).ok()?;
    let out1: IDXGIOutput1 = output.cast().ok()?;
    out1.DuplicateOutput(device).ok()
}

pub fn probe_hardware_codec_caps() -> HardwareCodecCaps {
    unsafe {
        let _guard = ComMfGuard::new(false);
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

/// Universal hardware MFT tuning via ICodecAPI (works across NVIDIA NVENC, AMD AMF, Intel QSV).
unsafe fn configure_encoder_codec_api(
    sink_writer: &IMFSinkWriter,
    video_stream_index: u32,
    bitrate_bps: u32,
    fps: u32,
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
        return false;
    }
    let codec_api = ICodecAPI::from_raw(raw_api);

    let v_rc = VARIANT::from(eAVEncCommonRateControlMode_PeakConstrainedVBR.0 as u32);
    let rc_ok = codec_api
        .SetValue(&CODECAPI_AVEncCommonRateControlMode, &v_rc)
        .is_ok();

    let v_mean = VARIANT::from(bitrate_bps);
    let _ = codec_api.SetValue(&CODECAPI_AVEncCommonMeanBitRate, &v_mean);

    let peak_bps = ((bitrate_bps as u64).saturating_mul(3) / 2)
        .clamp(bitrate_bps as u64, 100_000_000) as u32;
    let v_max = VARIANT::from(peak_bps);
    let _ = codec_api.SetValue(&CODECAPI_AVEncCommonMaxBitRate, &v_max);

    let gop_frames = fps.saturating_mul(2).clamp(30, 240);
    let v_gop = VARIANT::from(gop_frames);
    let gop_ok = codec_api.SetValue(&CODECAPI_AVEncMPVGOPSize, &v_gop).is_ok();

    let v_qvs = VARIANT::from(70u32);
    let _ = codec_api.SetValue(&CODECAPI_AVEncCommonQualityVsSpeed, &v_qvs);

    rc_ok || gop_ok
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
    enable_audio: bool,
    mode: BufferStorageMode,
    temp_dir: &Path,
    codec: VideoCodecType,
    use_nv12: bool,
    force_profile: bool,
    seg_counter: u32,
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
        let mut chosen_dir = temp_dir.to_path_buf();
        if fs::create_dir_all(&chosen_dir).is_err() {
            chosen_dir = std::env::temp_dir().join("NVLiteReplayRust");
            let _ = fs::create_dir_all(&chosen_dir);
        }
        let p = chosen_dir.join(format!("seg_{}_{}.mp4", std::process::id(), seg_counter));
        let url = HSTRING::from(p.to_string_lossy().as_ref());
        let sw = MFCreateSinkWriterFromURL(&url, None, &sw_attrs).ok()?;
        disk_path = Some(p);
        sw
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
    let _ = video_out.SetGUID(&MF_MT_SUBTYPE, &subtype);
    let _ = video_out.SetUINT32(&MF_MT_AVG_BITRATE, bitrate_bps);
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
                    enable_audio,
                    mode,
                    temp_dir,
                    fb_codec,
                    use_nv12,
                    fb_prof,
                    seg_counter,
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

    if sink_writer
        .SetInputMediaType(video_stream_index, &video_in, None)
        .is_err()
    {
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
                enable_audio,
                mode,
                temp_dir,
                fb_codec,
                use_nv12,
                fb_prof,
                seg_counter,
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
                enable_audio,
                mode,
                temp_dir,
                VideoCodecType::HardwareH264,
                false,
                false,
                seg_counter,
            );
        }
        return None;
    }

    let vbr_tuned = configure_encoder_codec_api(&sink_writer, video_stream_index, bitrate_bps, fps);

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
        sink_writer,
        mem_stream,
        _mf_byte_stream: mf_byte_stream,
        disk_path,
        video_stream_index,
        audio_stream_index,
        has_audio,
        frames_written: 0,
        audio_samples_written: 0,
        actual_codec: codec,
        vbr_tuned,
        seq: 0,
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
            return fs::write(output_path, ram).is_ok();
        }
        if let Some(ref p) = seg.disk_path {
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
                if let Some(stream) = SHCreateMemStream(Some(ram)) {
                    if let Ok(bs) = MFCreateMFByteStreamOnStream(&stream) {
                        MFCreateSourceReaderFromByteStream(&bs, None).ok()
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else if let Some(ref p) = seg.disk_path {
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
                    let _ = sample.SetSampleTime(video_time_offset + timestamp);
                    if timestamp + frame_step_hns > max_video_end {
                        max_video_end = timestamp + frame_step_hns;
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
                        let _ = sample.SetSampleTime(audio_time_offset + timestamp);
                        if timestamp + 200_000 > max_audio_end {
                            max_audio_end = timestamp + 200_000;
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
        let _guard = ComMfGuard::new(false);
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

fn local_replay_filename() -> String {
    unsafe {
        let st = GetLocalTime();
        format!(
            "Replay_{:04}-{:02}-{:02}_{:02}-{:02}-{:02}.mp4",
            st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond
        )
    }
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
}
