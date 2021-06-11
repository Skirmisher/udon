use crate::{error::Error, source::{ChannelCount, SampleRate, Source}, session::{self, SampleFormat}};
use std::{mem, ptr::{self, NonNull}, slice};

use native::winapi::{
    // Generated
    Windows::Win32::{
        Data::HtmlHelp::E_NOTFOUND,
        Foundation::{CloseHandle, HANDLE},
        Media::{
            Audio::CoreAudio::*, // WASAPI
            Multimedia::{
                KSDATAFORMAT_SUBTYPE_PCM,
                KSDATAFORMAT_SUBTYPE_IEEE_FLOAT,
                WAVEFORMATEX,
                WAVEFORMATEXTENSIBLE,
                WAVE_FORMAT_PCM,
                WAVE_FORMAT_IEEE_FLOAT,
            },
        },
        System::{
            Threading::{CreateEventW, WaitForSingleObjectEx, WAIT_FAILED},
            WindowsProgramming::INFINITE,
        },
    },

    // Bonus
    com, Interface,
};

const CLSCTX_ALL: u32 = 23; // (CLSCTX_INPROC_SERVER | CLSCTX_INPROC_HANDLER | CLSCTX_LOCAL_SERVER | CLSCTX_REMOTE_SERVER)

pub struct Device {
    audio_client: IAudioClient,
    sample_format: SampleFormat,

    // Invariant: The channel count or sample rate must not be 0.
    wave_format: NonNull<WAVEFORMATEX>,
}

impl Device {
    pub fn channel_count(&self) -> ChannelCount {
        unsafe {
            ChannelCount::new_unchecked(self.wave_format.as_ref().nChannels)
        }
    }

    pub fn sample_rate(&self) -> SampleRate {
        unsafe {
            SampleRate::new_unchecked(self.wave_format.as_ref().nSamplesPerSec)
        }
    }

    pub fn default_output() -> Result<Self, Error> {
        unsafe {
            if com::initialize_sta_fast().is_err() {
                // This is the only exit point at which no cleanup is needed, since we only need to call CoUninitialize()
                // after a successful CoInitializeEx call. There's no good reason why this should fail.
                return Err(Error::Unknown);
            }

            let enumerator = match com::create_instance::<IMMDeviceEnumerator>(&MMDeviceEnumerator) {
                Ok(x) => x,
                Err(_err) => {
                    // Again, this really shouldn't fail.
                    return Err(Error::Unknown);
                },
            };

            let mut device_tmp = None;
            let device_err = enumerator.GetDefaultAudioEndpoint(
                eRender,
                eMultimedia,
                &mut device_tmp,
            );
            let device = match (device_tmp, device_err) {
                (Some(x), r) if r.is_ok() => x,
                (_, err) => return Err(match err {
                    x if x == E_NOTFOUND => Error::NoOutputDevice,
                    _ => Error::Unknown,
                }),
            };

            // TODO: IAudioClient2, IAudioClient3
            let mut audio_client_tmp: Option<NonNull<IAudioClient>> = None;
            let audio_client_err = device.Activate(
                &<IAudioClient as Interface>::IID,
                CLSCTX_ALL,
                ptr::null_mut(),
                mem::transmute(&mut audio_client_tmp),
            );
            let audio_client = match (audio_client_tmp, audio_client_err) {
                (Some(x), r) if r.is_ok() => {
                    let clone = x.as_ref().clone();
                    ptr::drop_in_place(x.as_ptr());
                    clone
                },
                (_, err) => return Err(match err {
                    x if x == AUDCLNT_E_DEVICE_INVALIDATED => Error::DeviceNotAvailable,
                    _ => Error::Unknown,
                }),
            };

            let mut wave_format_tmp: Option<NonNull<WAVEFORMATEX>> = None;
            let wave_format_err = audio_client.GetMixFormat(mem::transmute(&mut wave_format_tmp));
            let wave_format = match (wave_format_tmp, wave_format_err) {
                (Some(x), r) if r.is_ok() => x,
                (_, err) => return Err(match err {
                    x if x == AUDCLNT_E_DEVICE_INVALIDATED => Error::DeviceNotAvailable,
                    _ => Error::Unknown,
                }),
            };

            // TODO: What about *unsigned* 16-bit?
            let format_info = wave_format.as_ref();
            let sample_format = match (u32::from(format_info.wFormatTag), format_info.wBitsPerSample) {
                (WAVE_FORMAT_PCM, 16) => SampleFormat::I16,
                (WAVE_FORMAT_IEEE_FLOAT, 32) => SampleFormat::F32,
                (WAVE_FORMAT_EXTENSIBLE, bps) => {
                    let format_info_extended = &*(wave_format.as_ptr() as *mut WAVEFORMATEXTENSIBLE);
                    match (ptr::addr_of!(format_info_extended.SubFormat).read_unaligned(), bps) {
                        (x, 16) if x.eq(&KSDATAFORMAT_SUBTYPE_PCM) => SampleFormat::I16,
                        (x, 32) if x.eq(&KSDATAFORMAT_SUBTYPE_IEEE_FLOAT) => SampleFormat::F32,
                        _ => {
                            return Err(Error::DeviceNotUsable)
                        },
                    }
                },
                (_, _) => return Err(Error::DeviceNotUsable),
            };

            Ok(Device {
                audio_client,
                sample_format,
                wave_format,
            })
        }
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        com::task_free(self.wave_format.as_ptr());
    }
}

pub struct OutputStream {
    //kill_switch: AtomicBool,
    render_client: IAudioRenderClient,
    device: Device,
    event_handle: HANDLE,
}

unsafe fn write_source(
    format: SampleFormat,
    buffer: *mut u8,
    sample_count: usize,
    source: &mut dyn Source,
) -> usize {
    match format {
        SampleFormat::I16 => todo!(), // TODO: big
        SampleFormat::F32 => {
            let buf = slice::from_raw_parts_mut(buffer as *mut f32, sample_count);
            let count = source.write_samples(buf);
            if let Some(remaining) = buf.get_mut(count..) {
                remaining.iter_mut().for_each(|x| *x = 0.0);
            }
            count
        },
    }
}

impl OutputStream {
    pub fn new(device: session::Device) -> Result<session::OutputStream, Error> {
        #[allow(irrefutable_let_patterns)] // TODO: yeah only wasapi right now
        unsafe {
            if let session::Device(session::DeviceImpl::Wasapi(device)) = device {
                // TODO: `Box::try_new` once `allocator_api` hits
                session_wrap!(Self::new_(device), OutputStream(OutputStreamImpl), Wasapi)
            } else {
                todo!("piss off");
            }
        }
    }

    unsafe fn new_(device: Device) -> Result<Self, Error> {
        let mut default_period = 0;
        let mut min_period = 0;
        match device.audio_client.GetDevicePeriod(&mut default_period, &mut min_period) {
            x if x.is_ok() => (),
            x if x == AUDCLNT_E_DEVICE_INVALIDATED => return Err(Error::DeviceNotAvailable),
            _ => return Err(Error::Unknown),
        }

        // initialize audio client
        match device.audio_client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
            default_period,
            0, // not in exclusive mode
            device.wave_format.as_ref(),
            ptr::null_mut(),
        ) {
            x if x.is_ok() => (),
            x if x == AUDCLNT_E_DEVICE_INVALIDATED => return Err(Error::DeviceNotAvailable),
            x if x == AUDCLNT_E_UNSUPPORTED_FORMAT => return Err(Error::DeviceNotUsable),
            _ => return Err(Error::Unknown),
        }

        // create a nameless handle for audio render events, defaulted to non-signaled
        let event_handle = CreateEventW(ptr::null_mut(), false, false, None);
        if event_handle.is_null() {
            return Err(Error::Unknown);
        }
        if device.audio_client.SetEventHandle(event_handle).is_err() {
            CloseHandle(event_handle);
            return Err(Error::Unknown);
        }

        let render_client = match device.audio_client.GetService::<IAudioRenderClient>() {
            Ok(x) => x,
            Err(err) => {
                CloseHandle(event_handle);
                return Err(match err {
                    x if x.code() == AUDCLNT_E_DEVICE_INVALIDATED => Error::DeviceNotAvailable,
                    _ => Error::Unknown,
                })
            },
        };

        Ok(Self {
            render_client,
            device,
            event_handle,
        })
    }

    pub fn play(&self, mut source: impl Source + Send + 'static) -> Result<(), Error> {
        macro_rules! read_hresult {
            ($res:expr) => {
                match $res {
                    x if x.is_ok() => Ok(()),
                    x if x == AUDCLNT_E_DEVICE_INVALIDATED => Err(Error::DeviceNotAvailable),
                    _ => {
                        let _ = self.device.audio_client.Stop();
                        let _ = self.device.audio_client.Reset();
                        Err(Error::Unknown)
                    },
                }
            }
        }

        unsafe {
            // Query number of samples in WASAPI's buffer
            let mut buffer_frame_count = 0;
            read_hresult!(self.device.audio_client.GetBufferSize(&mut buffer_frame_count))?;

            // Write the first chunk before starting
            let mut buffer_data: *mut u8 = ptr::null_mut();
            read_hresult!(self.render_client.GetBuffer(buffer_frame_count, &mut buffer_data))?;
            let samples_to_write = (buffer_frame_count * u32::from(self.device.wave_format.as_ref().nChannels)) as usize;
            let samples_written = write_source(self.device.sample_format, buffer_data, samples_to_write, &mut source);
            let frames_written = (samples_written / self.device.wave_format.as_ref().nChannels as usize) as u32;
            read_hresult!(self.render_client.ReleaseBuffer(buffer_frame_count, 0))?;

            // Start playback
            read_hresult!(self.device.audio_client.Start())?;

            // Loop, filling the output buffer until the source is empty
            let mut silent_frames = if frames_written >= buffer_frame_count {
                loop {
                    // Wait for WASAPI wake up the thread when it wants us to send more samples
                    match WaitForSingleObjectEx(self.event_handle, INFINITE, false) {
                        x if x == WAIT_FAILED => return Err(Error::Unknown),
                        _ => (),
                    };

                    // Query how many samples are free in the WASAPI buffer
                    let mut padding = 0;
                    read_hresult!(self.device.audio_client.GetCurrentPadding(&mut padding))?;
                    let frame_count = buffer_frame_count - padding;

                    // Do nothing if there are 0 free...
                    if frame_count > 0 {
                        // Lock the free part of the buffer and write samples to it
                        let mut buffer_data: *mut u8 = ptr::null_mut();
                        read_hresult!(self.render_client.GetBuffer(frame_count, &mut buffer_data))?;
                        let samples_to_write = (frame_count * u32::from(self.device.wave_format.as_ref().nChannels)) as usize;
                        let frames_written = write_source(self.device.sample_format, buffer_data, samples_to_write, &mut source);
                        let frames_written = (frames_written / self.device.wave_format.as_ref().nChannels as usize) as u32;
                        read_hresult!(self.render_client.ReleaseBuffer(frame_count, 0))?;

                        // If our source ended (ie. frames_written < frame_count), then break,
                        // also indicating how much silence we wrote to the buffer after the end of the sound
                        if frames_written < frame_count {
                            break frame_count - frames_written;
                        }
                    }
                }
            } else {
                buffer_frame_count - frames_written
            };

            // Now we need to make sure WASAPI has played everything we put in the buffer before the sound ended.
            // Otherwise we'll stop and flush the buffer before it can play the last bit of the user's Source.
            while silent_frames < buffer_frame_count {
                // Wait for WASAPI wake up the thread when it wants us to send more samples
                match WaitForSingleObjectEx(self.event_handle, INFINITE, false) {
                    x if x == WAIT_FAILED => return Err(Error::Unknown),
                    _ => (),
                };

                // Get how much is free and add it to the count
                let mut padding = 0;
                read_hresult!(self.device.audio_client.GetCurrentPadding(&mut padding))?;
                let frame_count = buffer_frame_count - padding;
                silent_frames += frame_count;

                // Put silence in the remaining buffer to keep WASAPI happy
                let mut buffer_data: *mut u8 = ptr::null_mut();
                read_hresult!(self.render_client.GetBuffer(frame_count, &mut buffer_data))?;
                read_hresult!(self.render_client.ReleaseBuffer(frame_count, AUDCLNT_BUFFERFLAGS_SILENT.0 as _))?;
            }

            // Stop and flush the output buffer
            read_hresult!(self.device.audio_client.Stop())?;
            read_hresult!(self.device.audio_client.Reset())?;
            Ok(())
        }
    }
}

pub struct Session;

impl Session {
    pub fn new() -> Result<Self, Error> {
        Ok(Self)
    }

    pub fn default_output_device(&self) -> Result<session::Device, Error> {
        session_wrap!(Device::default_output(), Device(DeviceImpl), Wasapi)
    }

    pub fn open_output_stream(
        &self,
        device: session::Device,
    ) -> Result<session::OutputStream, Error> {
        OutputStream::new(device)
    }
}

impl Drop for OutputStream {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.event_handle); }
    }
}
