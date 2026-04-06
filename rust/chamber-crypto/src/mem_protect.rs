//! Memory protection primitives for key material.
//!
//! On Android/Linux: mlock to prevent paging, MADV_DONTDUMP to exclude
//! from core dumps, prctl to deny ptrace.

/// Pin key bytes in physical RAM so they are never paged to swap.
///
/// On Android (Linux), calls mlock(2). On other platforms, this is a no-op
/// (the build target for this crate is aarch64-linux-android).
pub fn mlock_key(key: &[u8; 32]) {
    #[cfg(any(target_os = "android", target_os = "linux"))]
    {
        unsafe {
            libc::mlock(key.as_ptr() as *const libc::c_void, 32);
        }
    }
    #[cfg(not(any(target_os = "android", target_os = "linux")))]
    {
        let _ = key;
    }
}

/// Unlock previously mlocked memory.
pub fn munlock_key(key: &[u8; 32]) {
    #[cfg(any(target_os = "android", target_os = "linux"))]
    {
        unsafe {
            libc::munlock(key.as_ptr() as *const libc::c_void, 32);
        }
    }
    #[cfg(not(any(target_os = "android", target_os = "linux")))]
    {
        let _ = key;
    }
}

/// Advise the kernel not to include this memory region in core dumps.
pub fn madv_dontdump(ptr: *const u8, len: usize) {
    #[cfg(any(target_os = "android", target_os = "linux"))]
    {
        unsafe {
            libc::madvise(
                ptr as *mut libc::c_void,
                len,
                libc::MADV_DONTDUMP,
            );
        }
    }
    #[cfg(not(any(target_os = "android", target_os = "linux")))]
    {
        let _ = (ptr, len);
    }
}

/// On Android, use prctl to deny ptrace attachment.
pub fn deny_ptrace() {
    #[cfg(target_os = "android")]
    {
        unsafe {
            libc::prctl(libc::PR_SET_DUMPABLE, 0);
        }
    }
}

/// Apply all process-level hardening measures.
/// Called once at runtime initialization.
pub fn harden_process() {
    deny_ptrace();
}
