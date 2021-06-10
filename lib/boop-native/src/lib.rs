#![allow(bad_style)]

#[cfg(all(feature = "wasapi", target_os = "windows"))]
pub mod winapi {
    windows::include_bindings!();

    pub mod com {
        use std::{ffi::c_void, ptr};

        #[link(name = "Ole32")]
        extern "system" {
            pub fn CoInitializeEx(pvReserved: *const c_void, dwCoInit: u32) -> i32;
            pub fn CoUninitialize();
            pub fn CoTaskMemFree(pv: *const c_void);
        }

        // For whatever reason it's fucking up generating these, so...
        pub use windows::create_instance;
        pub fn initialize_sta_fast() -> windows::HRESULT {
            unsafe {
                windows::HRESULT(CoInitializeEx(ptr::null(), 2 | 8) as _)
            }
        }

        // Bonus
        pub fn task_free<T>(ptr: *mut T) {
            unsafe { CoTaskMemFree(ptr.cast()) };
        }
    }

    pub use windows::{HRESULT, Interface, IUnknown};
}
