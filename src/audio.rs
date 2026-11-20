use std::ptr::null_mut;
use windows::core::{GUID, PCWSTR, PWSTR};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::{
    eCapture, eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDevice,
    IMMDeviceEnumerator, MMDeviceEnumerator, AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_LOOPBACK, DEVICE_STATE_ACTIVE, WAVEFORMATEX, WAVEFORMATEXTENSIBLE,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_MULTITHREADED, STGM_READ,
};

const KSDATAFORMAT_SUBTYPE_IEEE_FLOAT: GUID =
    GUID::from_u128(0x00000003_0000_0010_8000_00aa00389b71);

const FIFO_CAPACITY: usize = 192_000; // 96,000 stereo frames = 2.0 sec @ 48 kHz
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

    #[allow(dead_code)]
    pub fn active_device_name(&self) -> &str {
        &self.active_device_name
    }

    #[allow(dead_code)]
    pub fn initialize(&mut self, is_loopback: bool) -> bool {
        self.initialize_with_device(is_loopback, None)
    }

    pub fn initialize_with_device(&mut self, is_loopback: bool, device_id: Option<&str>) -> bool {
        self.initialized = false;
        self.capture_client = None;
        self.silence_client = None;
        self.audio_client = None;
        self.active_device_name.clear();
        self.fifo_read = 0;
        self.fifo_write = 0;
        self.fifo_count = 0;
        self.resample_phase = 0.0;
        self.last_left = 0.0;
        self.last_right = 0.0;
        unsafe {
            if !self.mix_format_ptr.is_null() {
                CoTaskMemFree(Some(self.mix_format_ptr as *const _));
                self.mix_format_ptr = null_mut();
            }

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
                return false;
            }

            let Ok(capture_client): windows::core::Result<IAudioCaptureClient> =
                audio_client.GetService()
            else {
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
                return false;
            }

            // Initial 35ms (1680 stereo frames) anti-jitter cushion
            self.push_stereo_sample(0.0, 0.0, 1680);

            self.audio_client = Some(audio_client);
            self.capture_client = Some(capture_client);
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
                let Ok(packet_len) = capture.GetNextPacketSize() else {
                    break;
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
                    break;
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
        self.initialized = false;
        unsafe {
            if let Some(ref c) = self.audio_client {
                let _ = c.Stop();
            }
            if let Some(ref s) = self.silence_client {
                let _ = s.Stop();
            }
            if !self.mix_format_ptr.is_null() {
                CoTaskMemFree(Some(self.mix_format_ptr as *const _));
                self.mix_format_ptr = null_mut();
            }
        }
        self.capture_client = None;
        self.silence_client = None;
        self.audio_client = None;
    }
}
