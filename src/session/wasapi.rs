use crate::{error::Error, sync::{Condvar, condvar_notify1, condvar_wait, Mutex, mutex_lock}};
use ffi::winapi::{
    // Generated
    Windows::Win32::{
        Data::HtmlHelp::E_NOTFOUND,
        Foundation::{CloseHandle, HANDLE, PWSTR},
        Graphics::DirectDraw::E_OUTOFMEMORY,
        Media::{
            Audio::CoreAudio::{
                IMMDevice, IMMDeviceCollection, IMMDeviceEnumerator,
                MMDeviceEnumerator, eCapture, eRender, eAll, eMultimedia,
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
            Com::{
                CoInitializeEx, CoCreateInstance, CoTaskMemFree,
                CLSCTX_ALL, COINIT_APARTMENTTHREADED, COINIT_SPEED_OVER_MEMORY,
            },
            Diagnostics::Debug::{GetLastError, SetLastError, ERROR_SUCCESS},
            Threading::{
                CreateEventW, CreateThread, SetThreadPriority, WaitForSingleObjectEx,
                THREAD_PRIORITY_BELOW_NORMAL, WAIT_RETURN_CAUSE,
            },
            SystemServices::RTL_CONDITION_VARIABLE,
            WindowsProgramming::INFINITE,
        },
    },

    // Re-exports
    deps::windows,
};
use std::{ffi::c_void, hint, mem, ops, ptr, sync::Arc};

pub struct Session {
    device_thread: DeviceThread,
}

impl Session {
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            device_thread: unsafe { DeviceThread::new()? },
        })
    }
}

struct ThreadResult<T>(Condvar, Mutex<Option<Result<T, Error>>>);
impl<T> ThreadResult<T> {
    fn set(&mut self, res: Result<T, Error>) {
        let mut guard = mutex_lock(&self.1);
        *guard = Some(res);
        condvar_notify1(&self.0);
        mem::drop(guard);
    }

    fn wait(&mut self) -> Result<T, Error> {
        let mut guard = mutex_lock(&self.1);
        loop {
            match guard.take() {
                Some(res) => break res,
                None => condvar_wait(&self.0, &mut guard),
            }
        }
    }
}

struct DeviceState {
    default_in: Option<IMMDevice>,
    default_out: Option<IMMDevice>,
    devices: IMMDeviceCollection,
}

impl DeviceState {
    unsafe fn query(enumerator: &IMMDeviceEnumerator) -> Result<Self, Error> {
        let mut default_in = None;
        let mut default_out = None;
        let mut devices = None;

        match enumerator.GetDefaultAudioEndpoint(eCapture, eMultimedia, &mut default_in) {
            x if x.is_ok() || x == E_NOTFOUND => (),
            x if x == windows::HRESULT(E_OUTOFMEMORY as u32) => return Err(Error::SystemResources),
            _ => return Err(Error::Unknown),
        }
        match enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia, &mut default_out) {
            x if x.is_ok() || x == E_NOTFOUND => (),
            x if x == windows::HRESULT(E_OUTOFMEMORY as u32) => return Err(Error::SystemResources),
            _ => return Err(Error::Unknown),
        }
        match enumerator.EnumAudioEndpoints(eAll, DEVICE_STATEMASK_ALL, &mut devices) {
            x if x.is_ok() => (),
            x if x == windows::HRESULT(E_OUTOFMEMORY as u32) => return Err(Error::SystemResources),
            _ => return Err(Error::Unknown),
        }

        let devices = devices.unwrap_or_else(|| hint::unreachable_unchecked());
        Ok(Self { default_in, default_out, devices })
    }
}

struct DeviceThread {
    thread: HANDLE,
}
struct DeviceThreadParams {
    // control: ...,
    result: ThreadResult<()>,
}

impl DeviceThread {
    unsafe fn new() -> Result<Self, Error> {
        let mut params = DeviceThreadParams {
            result: ThreadResult(Condvar::new(), Mutex::new(None)),
        };
        let mut thread_id: u32 = Default::default();
        let thread = CreateThread(
            ptr::null_mut(),
            0,
            Some(device_thread_proc),
            (&mut params as *mut DeviceThreadParams).cast(),
            Default::default(),
            &mut thread_id,
        );
        if thread.is_null() {
            // TODO: Debug error better
            let _ = GetLastError();
            return Err(Error::SystemResources);
        }
        SetThreadPriority(thread, THREAD_PRIORITY_BELOW_NORMAL);
        params.result.wait().map(|()| Self { thread })
    }
}

unsafe extern "system" fn device_thread_proc(void_params: *mut c_void) -> u32 {
    let params = &mut *(void_params as *mut DeviceThreadParams);
    let thread_result = &mut params.result;

    // Initialize COM state for this thread in STA mode
    if CoInitializeEx(ptr::null_mut(), COINIT_APARTMENTTHREADED).is_err() {
        thread_result.set(Err(Error::SystemResources));
        return 0
    }

    // Acquire a device enumerator to query audio devices
    // XXX: Type ascription
    let res = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL);
    let enumerator: IMMDeviceEnumerator = match res {
        Ok(x) => x,
        Err(_err) => {
            thread_result.set(Err(Error::SystemResources));
            return 0
        },
    };

    // Query device state for the first time
    let state = match DeviceState::query(&enumerator) {
        Ok(x) => x,
        Err(err) => {
            thread_result.set(Err(err));
            mem::drop(enumerator);
            return 0
        },
    };

    let mut devices = 0;
    let _ = state.devices.GetCount(&mut devices);
    for i in 0..devices {
        let mut device = None;
        let device = match state.devices.Item(i, &mut device) {
            x if x.is_ok() => device.unwrap(),
            _ => panic!(),
        };
        let mut p = PWSTR::NULL;
        let _ = device.GetId(&mut p);
        let mut len = 0;
        let mut p2 = p.0;
        while *p2 != 0 {
            len += 1;
            p2 = p2.offset(1);
        }
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        println!(
            "Device #{}: {}",
            i,
            OsString::from_wide(std::slice::from_raw_parts(p.0, len)).to_string_lossy().as_ref()
        );
        CoTaskMemFree(p.0.cast());
    }

    thread_result.set(Ok(()));
    mem::drop(thread_result);

    // COM objects trying to drop after `CoUninitialize` will cause a very ugly segfault,
    // and it also unloads resources like DLLs so it's just better to not call it at all.
    0
}

impl ops::Drop for DeviceThread {
    fn drop(&mut self) {
        unsafe {
            WaitForSingleObjectEx(self.thread, INFINITE, false);
        }
    }
}
