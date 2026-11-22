use std::ffi::c_void;
use std::mem::ManuallyDrop;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use windows::core::{Interface, GUID, HRESULT, PCWSTR, PROPVARIANT, PWSTR};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::{E_NOINTERFACE, S_OK};
use windows::Win32::Media::Audio::{
    eCapture, eConsole, eMultimedia, eRender, ActivateAudioInterfaceAsync, EDataFlow, ERole,
    IActivateAudioInterfaceAsyncOperation, IActivateAudioInterfaceCompletionHandler,
    IActivateAudioInterfaceCompletionHandler_Vtbl, IAudioCaptureClient, IAudioClient, IMMDevice,
    IMMDeviceEnumerator, IMMNotificationClient, IMMNotificationClient_Vtbl, MMDeviceEnumerator,
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
    AUDCLNT_STREAMFLAGS_LOOPBACK, AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
    AUDIOCLIENT_ACTIVATION_PARAMS, AUDIOCLIENT_ACTIVATION_PARAMS_0,
    AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK, AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS,
    DEVICE_STATE, DEVICE_STATE_ACTIVE, PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
    VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, WAVEFORMATEX, WAVEFORMATEXTENSIBLE,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_MULTITHREADED, STGM_READ,
};
use windows::Win32::UI::Shell::PropertiesSystem::PROPERTYKEY;

const KSDATAFORMAT_SUBTYPE_IEEE_FLOAT: GUID =
    GUID::from_u128(0x00000003_0000_0010_8000_00aa00389b71);

const FIFO_CAPACITY: usize = 192_000; // 96,000 stereo frames = 2.0 sec @ 48 kHz
const WAVE_FORMAT_PCM_TAG: u16 = 1;
const WAVE_FORMAT_IEEE_FLOAT_TAG: u16 = 3;
const WAVE_FORMAT_EXTENSIBLE_TAG: u16 = 0xFFFE;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioInputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

unsafe fn pwstr_to_string_and_free(pwstr: PWSTR) -> String {
    if pwstr.0.is_null() {
        return String::new();
    }
    let len = (0usize..).find(|&i| *pwstr.0.add(i) == 0).unwrap_or(0);
    let s = String::from_utf16_lossy(std::slice::from_raw_parts(pwstr.0, len));
    CoTaskMemFree(Some(pwstr.0 as *const _));
    s
}

unsafe fn device_friendly_name(dev: &IMMDevice) -> String {
    if let Ok(store) = dev.OpenPropertyStore(STGM_READ) {
        if let Ok(val) = store.GetValue(&PKEY_Device_FriendlyName) {
            let s = val.to_string();
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    "Mikrofon Windows".to_string()
}

pub fn enumerate_audio_input_devices() -> Vec<AudioInputDevice> {
    let mut devices = Vec::new();
    unsafe {
        // Initialize COM only if caller thread has not initialized COM yet (avoids clobbering STA UI thread).
        let need_uninit = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let res = (|| -> Option<()> {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
            let default_id = enumerator
                .GetDefaultAudioEndpoint(eCapture, eConsole)
                .ok()
                .and_then(|d| d.GetId().ok())
                .map(|p| pwstr_to_string_and_free(p))
                .filter(|s| !s.is_empty());

            let collection = enumerator
                .EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)
                .ok()?;
            let count = collection.GetCount().unwrap_or(0);
            for i in 0..count {
                let Ok(dev) = collection.Item(i) else {
                    continue;
                };
                let Ok(raw_id) = dev.GetId() else {
                    continue;
                };
                let id = pwstr_to_string_and_free(raw_id);
                if id.is_empty() {
                    continue;
                }
                let name = device_friendly_name(&dev);
                let is_default = default_id.as_deref() == Some(id.as_str());
                devices.push(AudioInputDevice {
                    id,
                    name,
                    is_default,
                });
            }
            Some(())
        })();
        let _ = res;
        if need_uninit {
            CoUninitialize();
        }
    }
    devices.sort_by(|a, b| b.is_default.cmp(&a.is_default).then_with(|| a.name.cmp(&b.name)));
    devices
}

/// Queries the current Windows default endpoint ID for render (`is_loopback == true`) or capture.
pub fn default_audio_endpoint_id(is_loopback: bool) -> Option<String> {
    unsafe {
        let need_uninit = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let res = (|| -> Option<String> {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
            let flow = if is_loopback { eRender } else { eCapture };
            let dev = enumerator.GetDefaultAudioEndpoint(flow, eConsole).ok()?;
            let raw_id = dev.GetId().ok()?;
            let id = pwstr_to_string_and_free(raw_id);
            if id.is_empty() {
                None
            } else {
                Some(id)
            }
        })();
        if need_uninit {
            CoUninitialize();
        }
        res
    }
}

const IID_IAGILE_OBJECT: GUID = GUID::from_u128(0x94ea2b94_e9cc_49e0_c0ff_ee64ca8f5b90);

#[repr(C)]
struct ComAudioNotifier {
    vtable: *const IMMNotificationClient_Vtbl,
    ref_count: AtomicU32,
    render_dirty: Arc<AtomicBool>,
    capture_dirty: Arc<AtomicBool>,
}

static AUDIO_NOTIFIER_VTABLE: IMMNotificationClient_Vtbl = IMMNotificationClient_Vtbl {
    base__: windows::core::IUnknown_Vtbl {
        QueryInterface: notifier_query_interface,
        AddRef: notifier_add_ref,
        Release: notifier_release,
    },
    OnDeviceStateChanged: notifier_on_state_changed,
    OnDeviceAdded: notifier_on_device_added,
    OnDeviceRemoved: notifier_on_device_removed,
    OnDefaultDeviceChanged: notifier_on_default_changed,
    OnPropertyValueChanged: notifier_on_prop_changed,
};

unsafe extern "system" fn notifier_query_interface(
    this: *mut c_void,
    iid: *const GUID,
    interface: *mut *mut c_void,
) -> HRESULT {
    if interface.is_null() || iid.is_null() {
        return E_NOINTERFACE;
    }
    let id = *iid;
    if id == windows::core::IUnknown::IID
        || id == IMMNotificationClient::IID
        || id == IID_IAGILE_OBJECT
    {
        *interface = this;
        notifier_add_ref(this);
        return S_OK;
    }
    *interface = null_mut();
    E_NOINTERFACE
}

unsafe extern "system" fn notifier_add_ref(this: *mut c_void) -> u32 {
    let obj = &*(this as *const ComAudioNotifier);
    obj.ref_count.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn notifier_release(this: *mut c_void) -> u32 {
    let obj = &*(this as *const ComAudioNotifier);
    let prev = obj.ref_count.fetch_sub(1, Ordering::Release);
    if prev == 1 {
        std::sync::atomic::fence(Ordering::Acquire);
        drop(Box::from_raw(this as *mut ComAudioNotifier));
    }
    prev - 1
}

unsafe extern "system" fn notifier_on_state_changed(
    this: *mut c_void,
    _device_id: PCWSTR,
    _new_state: DEVICE_STATE,
) -> HRESULT {
    let obj = &*(this as *const ComAudioNotifier);
    obj.render_dirty.store(true, Ordering::Release);
    obj.capture_dirty.store(true, Ordering::Release);
    S_OK
}

unsafe extern "system" fn notifier_on_device_added(
    _this: *mut c_void,
    _device_id: PCWSTR,
) -> HRESULT {
    S_OK
}

unsafe extern "system" fn notifier_on_device_removed(
    this: *mut c_void,
    _device_id: PCWSTR,
) -> HRESULT {
    let obj = &*(this as *const ComAudioNotifier);
    obj.render_dirty.store(true, Ordering::Release);
    obj.capture_dirty.store(true, Ordering::Release);
    S_OK
}

unsafe extern "system" fn notifier_on_default_changed(
    this: *mut c_void,
    flow: EDataFlow,
    role: ERole,
    _default_device_id: PCWSTR,
) -> HRESULT {
    if role == eConsole || role == eMultimedia {
        let obj = &*(this as *const ComAudioNotifier);
        if flow == eRender {
            obj.render_dirty.store(true, Ordering::Release);
        } else if flow == eCapture {
            obj.capture_dirty.store(true, Ordering::Release);
        }
    }
    S_OK
}

unsafe extern "system" fn notifier_on_prop_changed(
    _this: *mut c_void,
    _device_id: PCWSTR,
    _key: PROPERTYKEY,
) -> HRESULT {
    S_OK
}

/// Zero-polling COM notification watcher for Windows default audio output/input switches.
pub struct AudioEndpointMonitor {
    enumerator: Option<IMMDeviceEnumerator>,
    client: Option<IMMNotificationClient>,
    render_dirty: Arc<AtomicBool>,
    capture_dirty: Arc<AtomicBool>,
}

impl AudioEndpointMonitor {
    pub fn new() -> Self {
        let render_dirty = Arc::new(AtomicBool::new(false));
        let capture_dirty = Arc::new(AtomicBool::new(false));
        let mut enumerator_opt = None;
        let mut client_opt = None;

        unsafe {
            if let Ok(enumerator) = CoCreateInstance::<_, IMMDeviceEnumerator>(
                &MMDeviceEnumerator,
                None,
                CLSCTX_ALL,
            ) {
                let raw = Box::into_raw(Box::new(ComAudioNotifier {
                    vtable: &AUDIO_NOTIFIER_VTABLE,
                    ref_count: AtomicU32::new(1),
                    render_dirty: Arc::clone(&render_dirty),
                    capture_dirty: Arc::clone(&capture_dirty),
                }));
                let client = IMMNotificationClient::from_raw(raw as *mut c_void);
                if enumerator
                    .RegisterEndpointNotificationCallback(&client)
                    .is_ok()
                {
                    enumerator_opt = Some(enumerator);
                    client_opt = Some(client);
                }
            }
        }

        Self {
            enumerator: enumerator_opt,
            client: client_opt,
            render_dirty,
            capture_dirty,
        }
    }

    pub fn take_render_dirty(&self) -> bool {
        self.render_dirty.swap(false, Ordering::AcqRel)
    }

    pub fn take_capture_dirty(&self) -> bool {
        self.capture_dirty.swap(false, Ordering::AcqRel)
    }
}

impl Drop for AudioEndpointMonitor {
    fn drop(&mut self) {
        if let (Some(ref enumerator), Some(ref client)) = (&self.enumerator, &self.client) {
            unsafe {
                let _ = enumerator.UnregisterEndpointNotificationCallback(client);
            }
        }
        self.client = None;
        self.enumerator = None;
    }
}

#[repr(C)]
struct RawPropVariantBlob {
    vt: u16, // 65 = VT_BLOB
    w_reserved1: u16,
    w_reserved2: u16,
    w_reserved3: u16,
    cb_size: u32,
    _pad: u32,
    p_blob_data: *mut u8,
}

#[repr(C)]
struct ComActivateHandler {
    vtable: *const IActivateAudioInterfaceCompletionHandler_Vtbl,
    ref_count: AtomicU32,
    done: Arc<(Mutex<Option<windows::core::Result<IAudioClient>>>, Condvar)>,
}

static ACTIVATE_HANDLER_VTABLE: IActivateAudioInterfaceCompletionHandler_Vtbl =
    IActivateAudioInterfaceCompletionHandler_Vtbl {
        base__: windows::core::IUnknown_Vtbl {
            QueryInterface: activate_query_interface,
            AddRef: activate_add_ref,
            Release: activate_release,
        },
        ActivateCompleted: activate_completed,
    };

unsafe extern "system" fn activate_query_interface(
    this: *mut c_void,
    iid: *const GUID,
    interface: *mut *mut c_void,
) -> HRESULT {
    if interface.is_null() || iid.is_null() {
        return E_NOINTERFACE;
    }
    let id = *iid;
    if id == windows::core::IUnknown::IID
        || id == IActivateAudioInterfaceCompletionHandler::IID
        || id == IID_IAGILE_OBJECT
    {
        *interface = this;
        activate_add_ref(this);
        return S_OK;
    }
    *interface = null_mut();
    E_NOINTERFACE
}

unsafe extern "system" fn activate_add_ref(this: *mut c_void) -> u32 {
    let obj = &*(this as *const ComActivateHandler);
    obj.ref_count.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn activate_release(this: *mut c_void) -> u32 {
    let obj = &*(this as *const ComActivateHandler);
    let prev = obj.ref_count.fetch_sub(1, Ordering::Release);
    if prev == 1 {
        std::sync::atomic::fence(Ordering::Acquire);
        drop(Box::from_raw(this as *mut ComActivateHandler));
    }
    prev - 1
}

unsafe extern "system" fn activate_completed(
    this: *mut c_void,
    operation: *mut c_void,
) -> HRESULT {
    let obj = &*(this as *const ComActivateHandler);
    let res = if operation.is_null() {
        Err(windows::core::Error::from(HRESULT(-2147467261i32)))
    } else {
        let op = ManuallyDrop::new(IActivateAudioInterfaceAsyncOperation::from_raw(operation));
        let mut hr = HRESULT(0);
        let mut unk: Option<windows::core::IUnknown> = None;
        match op.GetActivateResult(&mut hr, &mut unk) {
            Ok(()) => {
                if hr.is_err() {
                    Err(windows::core::Error::from(hr))
                } else if let Some(u) = unk {
                    u.cast::<IAudioClient>()
                } else {
                    Err(windows::core::Error::from(HRESULT(-2147467259i32)))
                }
            }
            Err(e) => Err(e),
        }
    };

    if let Ok(mut guard) = obj.done.0.lock() {
        *guard = Some(res);
        obj.done.1.notify_all();
    }
    S_OK
}

pub struct WasapiStream {
    audio_client: Option<IAudioClient>,
    silence_client: Option<IAudioClient>,
    capture_client: Option<IAudioCaptureClient>,
    mix_format_ptr: *mut WAVEFORMATEX,

    sample_rate: u32,
    channels: usize,
    bits_per_sample: u16,
    block_align: usize,
    is_float: bool,

    fifo: Vec<f32>,
    fifo_read: usize,
    fifo_write: usize,
    fifo_count: usize,

    // Optional lightweight 48k -> 16k mono decimator tap for pure-Rust voice recognition
    voice_tap_enabled: bool,
    voice_tap_16k: Vec<f32>,
    voice_accum: f32,
    voice_sub_idx: u8,

    resample_phase: f64,
    last_left: f32,
    last_right: f32,
    initialized: bool,
    device_invalidated: bool,
    process_loopback_pid: Option<u32>,
    active_device_id: String,
    active_device_name: String,
}

unsafe impl Send for WasapiStream {}

impl WasapiStream {
    pub fn new() -> Self {
        Self {
            audio_client: None,
            silence_client: None,
            capture_client: None,
            mix_format_ptr: null_mut(),
            sample_rate: 48_000,
            channels: 2,
            bits_per_sample: 32,
            block_align: 8,
            is_float: true,
            fifo: vec![0.0f32; FIFO_CAPACITY],
            fifo_read: 0,
            fifo_write: 0,
            fifo_count: 0,
            voice_tap_enabled: false,
            voice_tap_16k: Vec::with_capacity(4096),
            voice_accum: 0.0,
            voice_sub_idx: 0,
            resample_phase: 0.0,
            last_left: 0.0,
            last_right: 0.0,
            initialized: false,
            device_invalidated: false,
            process_loopback_pid: None,
            active_device_id: String::new(),
            active_device_name: String::new(),
        }
    }

    pub fn set_voice_tap_enabled(&mut self, enabled: bool) {
        self.voice_tap_enabled = enabled;
        if !enabled {
            self.voice_tap_16k.clear();
            self.voice_accum = 0.0;
            self.voice_sub_idx = 0;
        }
    }

    pub fn drain_voice_tap_16k(&mut self, out: &mut Vec<f32>) {
        out.clear();
        if !self.voice_tap_16k.is_empty() {
            out.append(&mut self.voice_tap_16k);
        }
    }

    pub fn is_available(&self) -> bool {
        self.initialized
    }

    pub fn active_device_id(&self) -> &str {
        &self.active_device_id
    }

    pub fn take_invalidated(&mut self) -> bool {
        let was = self.device_invalidated;
        self.device_invalidated = false;
        was
    }

    #[allow(dead_code)]
    pub fn active_device_name(&self) -> &str {
        &self.active_device_name
    }

    #[cfg(test)]
    pub fn active_process_loopback_pid(&self) -> Option<u32> {
        self.process_loopback_pid
    }

    pub fn stop_and_release(&mut self) {
        unsafe {
            self.release_clients();
        }
        self.device_invalidated = false;
        self.active_device_id.clear();
        self.active_device_name.clear();
    }

    unsafe fn release_clients(&mut self) {
        self.initialized = false;
        self.process_loopback_pid = None;
        if let Some(ref c) = self.audio_client {
            let _ = c.Stop();
        }
        if let Some(ref s) = self.silence_client {
            let _ = s.Stop();
        }
        self.capture_client = None;
        self.silence_client = None;
        self.audio_client = None;
        if !self.mix_format_ptr.is_null() {
            CoTaskMemFree(Some(self.mix_format_ptr as *const _));
            self.mix_format_ptr = null_mut();
        }
    }

    /// Initializes Per-Application WASAPI Process Loopback (`VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK`)
    /// capturing audio exclusively from `target_pid` and its child process tree.
    pub fn initialize_process_loopback(&mut self, target_pid: u32, label: &str) -> bool {
        if target_pid == 0 {
            return false;
        }
        let had_fifo = self.fifo_count > 0;
        self.device_invalidated = false;
        self.active_device_id.clear();
        self.active_device_name.clear();
        self.resample_phase = 0.0;
        self.last_left = 0.0;
        self.last_right = 0.0;

        if self.fifo_count > 1200 * 2 {
            let mut drop_count = self.fifo_count - (1200 * 2);
            if drop_count % 2 != 0 {
                drop_count -= 1;
            }
            self.fifo_read = (self.fifo_read + drop_count) % FIFO_CAPACITY;
            self.fifo_count -= drop_count;
        }

        unsafe {
            self.release_clients();

            let mut params = AUDIOCLIENT_ACTIVATION_PARAMS {
                ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
                Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
                    ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                        TargetProcessId: target_pid,
                        ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
                    },
                },
            };

            let raw_pv = RawPropVariantBlob {
                vt: 65, // VT_BLOB
                w_reserved1: 0,
                w_reserved2: 0,
                w_reserved3: 0,
                cb_size: std::mem::size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32,
                _pad: 0,
                p_blob_data: &mut params as *mut _ as *mut u8,
            };

            let done = Arc::new((Mutex::new(None), Condvar::new()));
            let raw_handler = Box::into_raw(Box::new(ComActivateHandler {
                vtable: &ACTIVATE_HANDLER_VTABLE,
                ref_count: AtomicU32::new(1),
                done: Arc::clone(&done),
            }));
            let handler =
                IActivateAudioInterfaceCompletionHandler::from_raw(raw_handler as *mut c_void);

            let Ok(_op) = ActivateAudioInterfaceAsync(
                VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
                &IAudioClient::IID,
                Some(&raw_pv as *const RawPropVariantBlob as *const PROPVARIANT),
                &handler,
            ) else {
                return false;
            };

            let client_opt = {
                let (lock, cv) = &*done;
                let Ok(guard) = lock.lock() else {
                    return false;
                };
                match cv.wait_timeout_while(guard, Duration::from_millis(1500), |g| g.is_none()) {
                    Ok((mut g, _)) => g.take(),
                    Err(_) => None,
                }
            };

            let Some(Ok(audio_client)) = client_opt else {
                return false;
            };

            let wfx_float = WAVEFORMATEX {
                wFormatTag: WAVE_FORMAT_IEEE_FLOAT_TAG,
                nChannels: 2,
                nSamplesPerSec: 48_000,
                nAvgBytesPerSec: 384_000,
                nBlockAlign: 8,
                wBitsPerSample: 32,
                cbSize: 0,
            };
            let wfx_pcm16 = WAVEFORMATEX {
                wFormatTag: WAVE_FORMAT_PCM_TAG,
                nChannels: 2,
                nSamplesPerSec: 48_000,
                nAvgBytesPerSec: 192_000,
                nBlockAlign: 4,
                wBitsPerSample: 16,
                cbSize: 0,
            };

            let attempts = [
                (wfx_float, AUDCLNT_STREAMFLAGS_LOOPBACK, 32u16, 8usize, true),
                (wfx_pcm16, AUDCLNT_STREAMFLAGS_LOOPBACK, 16u16, 4usize, false),
                (
                    wfx_pcm16,
                    AUDCLNT_STREAMFLAGS_LOOPBACK
                        | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                        | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
                    16u16,
                    4usize,
                    false,
                ),
            ];

            let mut chosen: Option<(u16, usize, bool)> = None;
            for (fmt, flags, bits, align, is_flt) in attempts {
                if audio_client
                    .Initialize(AUDCLNT_SHAREMODE_SHARED, flags, 2_000_000, 0, &fmt, None)
                    .is_ok()
                {
                    chosen = Some((bits, align, is_flt));
                    break;
                }
            }
            let Some((bits, align, is_flt)) = chosen else {
                return false;
            };

            let Ok(capture_client): windows::core::Result<IAudioCaptureClient> =
                audio_client.GetService()
            else {
                return false;
            };

            if audio_client.Start().is_err() {
                return false;
            }

            if !had_fifo {
                self.push_stereo_sample(0.0, 0.0, 1680);
            } else if self.fifo_count < 720 * 2 {
                self.push_stereo_sample(0.0, 0.0, 720);
            }

            self.sample_rate = 48_000;
            self.channels = 2;
            self.bits_per_sample = bits;
            self.block_align = align;
            self.is_float = is_flt;
            self.audio_client = Some(audio_client);
            self.capture_client = Some(capture_client);
            self.process_loopback_pid = Some(target_pid);
            self.active_device_id = format!("pid:{}", target_pid);
            self.active_device_name = label.to_string();
            self.initialized = true;
            true
        }
    }

    #[allow(dead_code)]
    pub fn initialize(&mut self, is_loopback: bool) -> bool {
        self.initialize_with_device(is_loopback, None)
    }

    pub fn initialize_with_device(&mut self, is_loopback: bool, device_id: Option<&str>) -> bool {
        let had_fifo = self.fifo_count > 0;
        self.device_invalidated = false;
        self.active_device_id.clear();
        self.active_device_name.clear();
        self.resample_phase = 0.0;
        self.last_left = 0.0;
        self.last_right = 0.0;

        // Trim any stale excess FIFO on hot-swap while keeping up to ~25ms for clickless transition.
        if self.fifo_count > 1200 * 2 {
            let mut drop_count = self.fifo_count - (1200 * 2);
            if drop_count % 2 != 0 {
                drop_count -= 1;
            }
            self.fifo_read = (self.fifo_read + drop_count) % FIFO_CAPACITY;
            self.fifo_count -= drop_count;
        }

        unsafe {
            self.release_clients();

            let Ok(enumerator): windows::core::Result<IMMDeviceEnumerator> =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
            else {
                return false;
            };

            let flow = if is_loopback { eRender } else { eCapture };
            let custom_id = device_id.map(str::trim).filter(|s| !s.is_empty());
            let device = if !is_loopback {
                if let Some(id_str) = custom_id {
                    let mut wide: Vec<u16> = id_str.encode_utf16().collect();
                    wide.push(0);
                    match enumerator.GetDevice(PCWSTR(wide.as_ptr())) {
                        Ok(dev) => dev,
                        Err(_) => {
                            let Ok(def) = enumerator.GetDefaultAudioEndpoint(flow, eConsole) else {
                                return false;
                            };
                            def
                        }
                    }
                } else {
                    let Ok(def) = enumerator.GetDefaultAudioEndpoint(flow, eConsole) else {
                        return false;
                    };
                    def
                }
            } else {
                let Ok(def) = enumerator.GetDefaultAudioEndpoint(flow, eConsole) else {
                    return false;
                };
                def
            };

            let resolved_id = device
                .GetId()
                .ok()
                .map(|p| pwstr_to_string_and_free(p))
                .unwrap_or_default();
            let resolved_name = device_friendly_name(&device);

            let Ok(audio_client): windows::core::Result<IAudioClient> =
                device.Activate(CLSCTX_ALL, None)
            else {
                return false;
            };

            let Ok(mix_fmt) = audio_client.GetMixFormat() else {
                return false;
            };
            if mix_fmt.is_null() {
                return false;
            }
            self.mix_format_ptr = mix_fmt;

            let wfx = &*mix_fmt;
            self.sample_rate = wfx.nSamplesPerSec.max(8_000);
            self.channels = (wfx.nChannels as usize).max(1);
            self.bits_per_sample = wfx.wBitsPerSample;
            self.block_align = (wfx.nBlockAlign as usize).max(1);

            if wfx.wFormatTag == WAVE_FORMAT_IEEE_FLOAT_TAG {
                self.is_float = true;
            } else if wfx.wFormatTag == WAVE_FORMAT_EXTENSIBLE_TAG {
                let ext = &*(mix_fmt as *const WAVEFORMATEXTENSIBLE);
                let sub_fmt = std::ptr::addr_of!(ext.SubFormat).read_unaligned();
                self.is_float = sub_fmt == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
            } else {
                self.is_float = false;
            }

            let flags = if is_loopback {
                AUDCLNT_STREAMFLAGS_LOOPBACK
            } else {
                0
            };

            if audio_client
                .Initialize(AUDCLNT_SHAREMODE_SHARED, flags, 2_000_000, 0, mix_fmt, None)
                .is_err()
            {
                self.release_clients();
                return false;
            }

            let Ok(capture_client): windows::core::Result<IAudioCaptureClient> =
                audio_client.GetService()
            else {
                self.release_clients();
                return false;
            };

            if is_loopback {
                if let Ok(sil_client) = device.Activate::<IAudioClient>(CLSCTX_ALL, None) {
                    if sil_client
                        .Initialize(AUDCLNT_SHAREMODE_SHARED, 0, 2_000_000, 0, mix_fmt, None)
                        .is_ok()
                    {
                        let _ = sil_client.Start();
                        self.silence_client = Some(sil_client);
                    }
                }
            }

            if audio_client.Start().is_err() {
                self.release_clients();
                return false;
            }

            // Push initial 35ms anti-jitter cushion on fresh start (or small 15ms cushion on hot-swap).
            if !had_fifo {
                self.push_stereo_sample(0.0, 0.0, 1680);
            } else if self.fifo_count < 720 * 2 {
                self.push_stereo_sample(0.0, 0.0, 720);
            }

            self.audio_client = Some(audio_client);
            self.capture_client = Some(capture_client);
            self.active_device_id = resolved_id;
            self.active_device_name = resolved_name;
            self.initialized = true;
            true
        }
    }

    #[inline]
    fn push_stereo_sample(&mut self, left: f32, right: f32, repeat: usize) {
        if self.voice_tap_enabled {
            let mono_in = (left + right) * 0.5;
            if repeat == 1 {
                self.voice_accum += mono_in;
                self.voice_sub_idx += 1;
                if self.voice_sub_idx == 3 {
                    if self.voice_tap_16k.len() < 32_000 {
                        self.voice_tap_16k.push(self.voice_accum * (1.0 / 3.0));
                    }
                    self.voice_accum = 0.0;
                    self.voice_sub_idx = 0;
                }
            } else {
                for _ in 0..repeat {
                    self.voice_accum += mono_in;
                    self.voice_sub_idx += 1;
                    if self.voice_sub_idx == 3 {
                        if self.voice_tap_16k.len() < 32_000 {
                            self.voice_tap_16k.push(self.voice_accum * (1.0 / 3.0));
                        }
                        self.voice_accum = 0.0;
                        self.voice_sub_idx = 0;
                    }
                }
            }
        }
        for _ in 0..repeat {
            if self.fifo_count + 2 > FIFO_CAPACITY {
                self.fifo_read = (self.fifo_read + 2) % FIFO_CAPACITY;
                self.fifo_count -= 2;
            }
            self.fifo[self.fifo_write] = left;
            self.fifo[(self.fifo_write + 1) % FIFO_CAPACITY] = right;
            self.fifo_write = (self.fifo_write + 2) % FIFO_CAPACITY;
            self.fifo_count += 2;
        }
    }

    #[inline]
    unsafe fn read_stereo_frame(&self, base_ptr: *const u8, frame_idx: usize) -> (f32, f32) {
        let p = base_ptr.add(frame_idx * self.block_align);
        if self.is_float && self.bits_per_sample == 32 {
            let l = (p as *const f32).read_unaligned();
            let r = if self.channels >= 2 {
                (p.add(4) as *const f32).read_unaligned()
            } else {
                l
            };
            (l, r)
        } else if self.bits_per_sample == 16 {
            let l = (p as *const i16).read_unaligned() as f32 / 32768.0;
            let r = if self.channels >= 2 {
                (p.add(2) as *const i16).read_unaligned() as f32 / 32768.0
            } else {
                l
            };
            (l, r)
        } else if self.bits_per_sample == 24 {
            let b0 = *p as i32;
            let b1 = *p.add(1) as i32;
            let b2 = (*p.add(2) as i8) as i32;
            let l24 = (b0 << 8) | (b1 << 16) | (b2 << 24);
            let l = l24 as f32 / 2147483648.0;
            let r = if self.channels >= 2 {
                let r0 = *p.add(3) as i32;
                let r1 = *p.add(4) as i32;
                let r2 = (*p.add(5) as i8) as i32;
                let r24 = (r0 << 8) | (r1 << 16) | (r2 << 24);
                r24 as f32 / 2147483648.0
            } else {
                l
            };
            (l, r)
        } else if self.bits_per_sample == 32 {
            let l = (p as *const i32).read_unaligned() as f32 / 2147483648.0;
            let r = if self.channels >= 2 {
                (p.add(4) as *const i32).read_unaligned() as f32 / 2147483648.0
            } else {
                l
            };
            (l, r)
        } else {
            (0.0, 0.0)
        }
    }

    pub fn drain_packets(&mut self) {
        if !self.initialized {
            return;
        }
        let Some(capture) = self.capture_client.clone() else {
            return;
        };

        unsafe {
            loop {
                let packet_len = match capture.GetNextPacketSize() {
                    Ok(len) => len,
                    Err(_) => {
                        drop(capture);
                        self.release_clients();
                        self.device_invalidated = true;
                        return;
                    }
                };
                if packet_len == 0 {
                    break;
                }

                let mut data_ptr: *mut u8 = null_mut();
                let mut num_frames: u32 = 0;
                let mut flags: u32 = 0;
                if capture
                    .GetBuffer(&mut data_ptr, &mut num_frames, &mut flags, None, None)
                    .is_err()
                {
                    drop(capture);
                    self.release_clients();
                    self.device_invalidated = true;
                    return;
                }

                if num_frames > 0 && self.fifo_count == 0 {
                    self.push_stereo_sample(0.0, 0.0, 1440);
                }

                let is_silent = (flags & (AUDCLNT_BUFFERFLAGS_SILENT.0 as u32)) != 0 || data_ptr.is_null();
                let frames = num_frames as usize;

                if self.sample_rate == 48_000 {
                    if is_silent {
                        self.push_stereo_sample(0.0, 0.0, frames);
                        self.last_left = 0.0;
                        self.last_right = 0.0;
                    } else {
                        for i in 0..frames {
                            let (l, r) = self.read_stereo_frame(data_ptr, i);
                            self.push_stereo_sample(l, r, 1);
                        }
                    }
                } else if self.sample_rate == 192_000 {
                    if is_silent {
                        self.push_stereo_sample(0.0, 0.0, frames / 4);
                        self.last_left = 0.0;
                        self.last_right = 0.0;
                    } else {
                        let mut i = 0;
                        while i + 3 < frames {
                            let (l0, r0) = self.read_stereo_frame(data_ptr, i);
                            let (l1, r1) = self.read_stereo_frame(data_ptr, i + 1);
                            let (l2, r2) = self.read_stereo_frame(data_ptr, i + 2);
                            let (l3, r3) = self.read_stereo_frame(data_ptr, i + 3);
                            self.push_stereo_sample(
                                (l0 + l1 + l2 + l3) * 0.25,
                                (r0 + r1 + r2 + r3) * 0.25,
                                1,
                            );
                            i += 4;
                        }
                    }
                } else if self.sample_rate == 96_000 {
                    if is_silent {
                        self.push_stereo_sample(0.0, 0.0, frames / 2);
                    } else {
                        let mut i = 0;
                        while i + 1 < frames {
                            let (l0, r0) = self.read_stereo_frame(data_ptr, i);
                            let (l1, r1) = self.read_stereo_frame(data_ptr, i + 1);
                            self.push_stereo_sample((l0 + l1) * 0.5, (r0 + r1) * 0.5, 1);
                            i += 2;
                        }
                    }
                } else {
                    let step = (self.sample_rate as f64 / 48_000.0).max(0.05);
                    for i in 0..frames {
                        let (cur_l, cur_r) = if is_silent {
                            (0.0, 0.0)
                        } else {
                            self.read_stereo_frame(data_ptr, i)
                        };
                        while self.resample_phase <= 1.0 {
                            let frac = self.resample_phase as f32;
                            let out_l = self.last_left + (cur_l - self.last_left) * frac;
                            let out_r = self.last_right + (cur_r - self.last_right) * frac;
                            self.push_stereo_sample(out_l, out_r, 1);
                            self.resample_phase += step;
                        }
                        self.resample_phase -= 1.0;
                        self.last_left = cur_l;
                        self.last_right = cur_r;
                    }
                }

                let _ = capture.ReleaseBuffer(num_frames);
            }
        }
    }

    pub fn mix_from_fifo(&mut self, mix_buffer: &mut [f32], stereo_frames: usize, gain: f32) {
        if !self.initialized {
            return;
        }

        let floats_needed = (stereo_frames * 2).min(mix_buffer.len());
        let floats_avail = floats_needed.min(self.fifo_count);

        for out in mix_buffer.iter_mut().take(floats_avail) {
            let sample = self.fifo[self.fifo_read];
            self.fifo_read = (self.fifo_read + 1) % FIFO_CAPACITY;
            if gain > 0.0001 {
                *out += sample * gain;
            }
        }
        self.fifo_count -= floats_avail;

        if self.fifo_count > 4800 * 2 {
            let mut drop_count = self.fifo_count - (1920 * 2);
            if drop_count % 2 != 0 {
                drop_count -= 1;
            }
            self.fifo_read = (self.fifo_read + drop_count) % FIFO_CAPACITY;
            self.fifo_count -= drop_count;
        }
    }
}

impl Drop for WasapiStream {
    fn drop(&mut self) {
        unsafe {
            self.release_clients();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_endpoint_monitor_and_hotswap_safe() {
        unsafe {
            let need_uninit = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
            {
                let mon = AudioEndpointMonitor::new();
                assert!(!mon.take_render_dirty());
                assert!(!mon.take_capture_dirty());
            }

            let mut loop_stream = WasapiStream::new();
            if let Some(def_out) = default_audio_endpoint_id(true) {
                assert!(loop_stream.initialize_with_device(true, None));
                assert_eq!(loop_stream.active_device_id(), def_out);
                loop_stream.drain_packets();
                // Hot-swap re-initialization must stop old clients cleanly & preserve availability.
                assert!(loop_stream.initialize_with_device(true, None));
                assert!(loop_stream.is_available());
            }
            drop(loop_stream);

            if need_uninit {
                CoUninitialize();
            }
        }
    }

    #[test]
    fn process_loopback_activates_and_drains_cleanly() {
        assert_eq!(
            std::mem::size_of::<RawPropVariantBlob>(),
            std::mem::size_of::<PROPVARIANT>()
        );
        unsafe {
            let need_uninit = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
            let mut stream = WasapiStream::new();
            let pid = std::process::id();
            assert!(
                stream.initialize_process_loopback(pid, "SelfTest"),
                "Per-application process loopback should succeed on Windows 11 Build 26200"
            );
            assert!(stream.is_available());
            assert_eq!(stream.active_process_loopback_pid(), Some(pid));
            stream.drain_packets();
            stream.stop_and_release();
            assert!(!stream.is_available());
            assert_eq!(stream.active_process_loopback_pid(), None);

            if need_uninit {
                CoUninitialize();
            }
        }
    }
}
