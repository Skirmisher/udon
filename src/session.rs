use crate::error::Error;
use std::num::{NonZeroU16, NonZeroU32};

pub mod consts;

pub type ChannelCount = NonZeroU16;
pub type SampleRate = NonZeroU32;

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum DeviceType {
    Input,
    Output,
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum SampleType {
    // /// Unsigned 16-bit integer PCM
    // U16,

    /// Signed 16-bit integer PCM
    I16,

    /// IEEE 754 32-bit float PCM
    F32,
}

/// Generates API-specific repetitive stuff for sessions and all associated types.
macro_rules! sessions {
    (
        $( $(#[$outer:meta])* $mod:ident => $variant:ident $( if $cfg:meta )* ),* $(,)?
    ) => {
        const _PLEASE_CALL_THE_SESSIONS_GEN_MACRO_ONLY_ONCE: () = ();
        $( $(#[cfg($cfg)])* mod $mod; )*

        /// Represents a native API to create [`Session`] instances in.
        #[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
        #[non_exhaustive]
        pub enum Api {
            $( $(#[$outer])* $variant ),*
        }

        pub struct Device(pub(crate) DeviceImpl);
        pub enum DeviceImpl {
            $( $(#[cfg($cfg)])* $variant ( $mod::Device ) ),*
        }
        impl Device {
            pub fn speak(&self) {
                match self.0 {
                    $(
                        $(#[cfg($cfg)])*
                        DeviceImpl::$variant(ref imp) => imp.speak()
                    ),*,
                }
            }
        }

        pub struct Session(pub(crate) SessionImpl);
        pub enum SessionImpl {
            $( $(#[cfg($cfg)])* $variant ( $mod::Session ) ),*
        }
        impl Session {
            pub fn new(api: Api) -> Result<Self, Error> {
                match api {
                    $(Api::$variant => {
                        $(#[cfg($cfg)])*
                        { return $mod::Session::new().map(SessionImpl::$variant).map(Session); }
                        #[allow(unreachable_code)]
                        { return Err(Error::ApiNotAvailable); }
                    }),*,
                }
            }

            pub fn default_device(&self, device_type: DeviceType) -> Result<Device, Error> {
                match self.0 {
                    $(
                        $(#[cfg($cfg)])*
                        SessionImpl::$variant(ref imp) => imp.default_device(device_type)
                    ),*,
                }
            }
        }
    };
}

macro_rules! rewrap_impl {
    ($variant:ident, $ty:ident, $enum_ty:ident, $inner:expr) => {
        crate::session::$ty(crate::session::$enum_ty::$variant($inner))
    };
}

sessions! {
    /// Dummy (no-op backend)
    dummy => Dummy,

    /// Windows Audio Session API (WASAPI)
    wasapi => Wasapi if all(target_os = "windows", feature = "wasapi"),
}
