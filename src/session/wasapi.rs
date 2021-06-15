use crate::{
    error::Error,
    session::{self, DeviceType},
    sync::{Condvar, condvar_notify1, condvar_wait, Mutex, mutex_lock},
};
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
            WindowsProgramming::INFINITE,
        },
    },

    // Re-exports
    deps::windows,
};
use std::{ffi::c_void, hint, mem, ops, ptr, sync::Arc};

pub struct Session {
    devices: DeviceThread,
}

impl Session {
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            devices: unsafe { DeviceThread::new()? },
        })
    }

    pub fn default_device(&self, device_type: DeviceType) -> Result<session::Device, Error> {
        Ok(rewrap_impl!(Wasapi, Device, DeviceImpl, self.devices.default_device(device_type)?))
    }
}

pub struct Device {
    device: IMMDevice,
}

impl Device {
    pub fn speak(&self) {
        // Proof of concept thing :p
        unsafe {
            let mut p = PWSTR::NULL;
            let _ = self.device.GetId(&mut p);
            let mut len = 0;
            let mut p2 = p.0;
            while *p2 != 0 {
                len += 1;
                p2 = p2.offset(1);
            }
            use std::ffi::OsString;
            use std::os::windows::ffi::OsStringExt;
            println!(
                "Device \"{}\" speaking!",
                OsString::from_wide(std::slice::from_raw_parts(p.0, len)).to_string_lossy().as_ref()
            );
            CoTaskMemFree(p.0.cast());
        }
    }
}

struct ThreadResult<T>(Condvar, Mutex<Option<Result<T, Error>>>);
impl<T> ThreadResult<T> {
    fn new() -> Self {
        Self(Condvar::new(), Mutex::new(None))
    }

    fn send(&mut self, res: Result<T, Error>) {
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
    queue: Arc<DeviceThreadQueue>,
    thread: HANDLE,
}
enum DeviceThreadMessage {
    Refresh(*mut ThreadResult<()>),
    GetDefault(DeviceType, *mut ThreadResult<Device>),
    Kill,
}
type DeviceThreadQueue = (Condvar, Mutex<Vec<DeviceThreadMessage>>);
struct DeviceThreadParams {
    queue: Arc<DeviceThreadQueue>,
    result: ThreadResult<()>,
}

impl DeviceThread {
    fn new() -> Result<Self, Error> {
        unsafe {
            let queue = Arc::new((Condvar::new(), Mutex::new(Vec::new())));
            let mut params = DeviceThreadParams {
                queue: Arc::clone(&queue),
                result: ThreadResult::new(),
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
            params.result.wait().map(|()| Self { queue, thread })
        }
    }

    fn default_device(&self, device_type: DeviceType) -> Result<Device, Error> {
        let (cvar, mutex) = &*self.queue;
        let mut guard = mutex_lock(mutex);
        let mut refresh_result = ThreadResult::new();
        let mut device_result = ThreadResult::new();
        guard.push(DeviceThreadMessage::Refresh(&mut refresh_result));
        guard.push(DeviceThreadMessage::GetDefault(device_type, &mut device_result));
        condvar_notify1(cvar);
        mem::drop(guard);

        refresh_result.wait()?;
        device_result.wait()
    }
}

unsafe extern "system" fn device_thread_proc(void_params: *mut c_void) -> u32 {
    let params = &mut *(void_params as *mut DeviceThreadParams);
    let queue = Arc::clone(&params.queue);
    let result = &mut params.result;

    // Initialize COM state for this thread in STA mode
    if CoInitializeEx(ptr::null_mut(), COINIT_APARTMENTTHREADED).is_err() {
        result.send(Err(Error::SystemResources));
        return 0
    }

    // Acquire a device enumerator to query audio devices
    // XXX: Type ascription
    let res = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL);
    let enumerator: IMMDeviceEnumerator = match res {
        Ok(x) => x,
        Err(_err) => {
            result.send(Err(Error::SystemResources));
            return 0
        },
    };

    // Query device state for the first time
    let mut state = match DeviceState::query(&enumerator) {
        Ok(x) => x,
        Err(err) => {
            result.send(Err(err));
            mem::drop(enumerator);
            return 0
        },
    };

    // Signal that we're OK to begin operating
    result.send(Ok(()));
    mem::drop(params);

    // Respond to messages... for the rest of time
    let (cvar, mutex) = &*queue;
    let mut guard = mutex_lock(mutex);
    'outer: loop {
        for message in &*guard {
            match message {
                // TODO: Detect a *need* to refresh with RegisterEndpointNotificationCallback
                DeviceThreadMessage::Refresh(response) => {
                    let response = &mut **response;
                    match DeviceState::query(&enumerator) {
                        Ok(x) => {
                            state = x;
                            response.send(Ok(()));
                        },
                        Err(err) => response.send(Err(err)),
                    }
                },
                DeviceThreadMessage::GetDefault(ty, response) => {
                    let response = &mut **response;
                    let maybe_device = match ty {
                        DeviceType::Input => &state.default_in,
                        DeviceType::Output => &state.default_out,
                    };
                    if let Some(device) = maybe_device {
                        response.send(Ok(Device { device: device.clone() }));
                    } else {
                        response.send(Err(Error::NoDeviceAvailable));
                    }
                },
                DeviceThreadMessage::Kill => break 'outer,
            }
        }
        guard.clear();
        condvar_wait(cvar, &mut guard);
    }

    // COM objects trying to drop after `CoUninitialize` will cause a very ugly segfault,
    // and it also unloads resources like DLLs so it's just better to not call it at all.
    0
}

impl ops::Drop for DeviceThread {
    fn drop(&mut self) {
        unsafe {
            let mut guard = mutex_lock(&self.queue.1);
            guard.push(DeviceThreadMessage::Kill);
            condvar_notify1(&self.queue.0);
            mem::drop(guard);

            WaitForSingleObjectEx(self.thread, INFINITE, false);
        }
    }
}
