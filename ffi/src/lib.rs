#![allow(bad_style)]

#[cfg(all(feature = "wasapi", target_os = "windows"))]
pub mod winapi {
    windows::include_bindings!();
    pub use windows as extra;
}
