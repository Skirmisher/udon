use crate::error::Error;
use std::num::{NonZeroU16, NonZeroU32};

pub mod consts;

pub type ChannelCount = NonZeroU16;
pub type SampleRate = NonZeroU32;

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
        pub enum Api {
            $( $(#[$outer])* $variant ),*
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
        }
    };
}

sessions! {
    /// Windows Audio Session API (WASAPI)
    wasapi => Wasapi if all(target_os = "windows", feature = "wasapi"),
}
