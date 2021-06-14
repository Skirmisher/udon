//! Wrapper for working with both `std` and `parking_lot`.
//! None of these functions should panic when used correctly.

#[cfg(not(feature = "parking-lot"))]
mod imp {
    pub use std::sync::{Condvar, Mutex, MutexGuard};
    use std::ptr;

    #[inline]
    pub fn condvar_notify1(cvar: &Condvar) {
        cvar.notify_one();
    }

    pub fn condvar_wait<T>(cvar: &Condvar, guard: &mut MutexGuard<T>) {
        // The signature in `std` is quite terrible and CONSUMES the guard
        // HACK: We "move it out" for the duration of the wait
        unsafe {
            let guard_copy = ptr::read(guard);
            let result = cvar.wait(guard_copy).expect("cvar mutex poisoned (this is a bug)");
            ptr::write(guard, result);
        }
    }

    pub fn mutex_lock<T>(mtx: &Mutex<T>) -> MutexGuard<T> {
        mtx.lock().expect("mutex poisoned (this is a bug)")
    }
}

#[cfg(feature = "parking-lot")]
mod imp {
    pub use parking_lot::{Condvar, Mutex, MutexGuard};

    #[inline]
    pub fn condvar_notify1(cvar: &Condvar) {
        let _ = cvar.notify_one();
    }

    #[inline]
    pub fn condvar_wait<T>(cvar: &Condvar, guard: &mut MutexGuard<T>) {
        cvar.wait(guard);
    }

    #[inline]
    pub fn mutex_lock<T>(mtx: &Mutex<T>) -> MutexGuard<T> {
        mtx.lock()
    }
}

pub use imp::*;
