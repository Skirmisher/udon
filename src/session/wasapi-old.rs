use crate::{error::Error, source::{ChannelCount, SampleRate, Source}, sessiona::{self, SampleFormat}};
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
            Com::{
                CoInitializeEx, CoCreateInstance, CoTaskMemFree, CoUninitialize,
                CLSCTX_ALL, COINIT_APARTMENTTHREADED, COINIT_SPEED_OVER_MEMORY,
            },
            Threading::{CreateEventW, WaitForSingleObjectEx, WAIT_FAILED},
            WindowsProgramming::INFINITE,
        },
    },
};

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
        todo!()
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            CoTaskMemFree(self.wave_format.as_ptr().cast());
        }
    }
}

pub struct OutputStream {
    // //kill_switch: AtomicBool,
    // render_client: IAudioRenderClient,
    // device: Device,
    // event_handle: HANDLE,
}

// unsafe fn write_source(
//     format: SampleFormat,
//     buffer: *mut u8,
//     sample_count: usize,
//     source: &mut dyn Source,
// ) -> usize {
//     match format {
//         SampleFormat::I16 => todo!(), // TODO: big
//         SampleFormat::F32 => {
//             let buf = slice::from_raw_parts_mut(buffer as *mut f32, sample_count);
//             let count = source.write_samples(buf);
//             if let Some(remaining) = buf.get_mut(count..) {
//                 remaining.iter_mut().for_each(|x| *x = 0.0);
//             }
//             count
//         },
//     }
// }

impl OutputStream {
    pub fn new(device: sessiona::Device) -> Result<sessiona::OutputStream, Error> {
        #[allow(irrefutable_let_patterns)] // TODO: yeah only wasapi right now
        unsafe {
            if let sessiona::Device(sessiona::DeviceImpl::Wasapi(device)) = device {
                // TODO: `Box::try_new` once `allocator_api` hits
                session_wrap!(Self::new_(device), OutputStream(OutputStreamImpl), Wasapi)
            } else {
                todo!("piss off");
            }
        }
    }

    unsafe fn new_(device: Device) -> Result<Self, Error> {
        todo!()
    }

    pub fn play(&self, mut source: impl Source + Send + 'static) -> Result<(), Error> {
        todo!()
    }
}

pub struct Session;

impl Session {
    pub fn new() -> Result<Self, Error> {
        todo!()
    }

    pub fn default_output_device(&self) -> Result<sessiona::Device, Error> {
        session_wrap!(Device::default_output(), Device(DeviceImpl), Wasapi)
    }

    pub fn open_output_stream(
        &self,
        device: sessiona::Device,
    ) -> Result<sessiona::OutputStream, Error> {
        OutputStream::new(device)
    }
}

impl Drop for OutputStream {
    fn drop(&mut self) {
        // unsafe { CloseHandle(self.event_handle); }
    }
}
