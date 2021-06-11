fn main() {
    // WASAPI
    #[cfg(all(feature = "wasapi", target_os = "windows"))]
    {
        windows::build!(
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
                    Threading::{CreateEventW, WaitForSingleObjectEx, WAIT_RETURN_CAUSE},
                    WindowsProgramming::INFINITE,
                },
            },
        );
    }
}
