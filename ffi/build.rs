fn main() {
    // WASAPI
    #[cfg(all(feature = "wasapi", target_os = "windows"))]
    {
        windows::build!(
            Windows::Win32::{
                Data::HtmlHelp::E_NOTFOUND,
                Foundation::{CloseHandle, HANDLE, PWSTR},
                Graphics::DirectDraw::E_OUTOFMEMORY,
                Media::{
                    Audio::CoreAudio::{
                        IMMDevice, IMMDeviceCollection, IMMDeviceEnumerator,
                        MMDeviceEnumerator, EDataFlow, ERole,
                        IAudioClient, IAudioClient2, IAudioClient3,
                        IAudioRenderClient, IAudioCaptureClient,

                        DEVICE_STATE_ACTIVE, DEVICE_STATE_DISABLED,
                        DEVICE_STATE_NOTPRESENT, DEVICE_STATE_UNPLUGGED,
                        DEVICE_STATEMASK_ALL,
                    },
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
                    Com::{CoInitializeEx, CoCreateInstance, CoTaskMemFree, CLSCTX, COINIT},
                    Diagnostics::Debug::{GetLastError, SetLastError, WIN32_ERROR},
                    Threading::{
                        CreateEventW, CreateThread, SetThreadPriority, WaitForSingleObjectEx,
                        THREAD_PRIORITY, WAIT_RETURN_CAUSE,
                    },
                    SystemServices::RTL_CONDITION_VARIABLE,
                    WindowsProgramming::INFINITE,
                },
            },
        );
    }
}
