use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::sync::Notify;
use tracing::{debug, warn};

/// OS-level recursive directory watcher that notifies `changed` on filesystem modifications.
/// Dropping the handle cancels the underlying `ReadDirectoryChangesW` call and releases the directory handle.
pub struct DirWatcher {
    stop: Arc<AtomicBool>,
    #[cfg(windows)]
    handle: Arc<AtomicUsize>,
}

impl DirWatcher {
    pub fn spawn(root: &Path, changed: Arc<Notify>) -> Option<Self> {
        #[cfg(windows)]
        {
            spawn_windows_watcher(root, changed)
        }
        #[cfg(not(windows))]
        {
            let _ = (root, changed);
            None
        }
    }
}

impl Drop for DirWatcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        #[cfg(windows)]
        {
            let raw = self.handle.load(Ordering::SeqCst);
            if raw != 0 {
                unsafe {
                    cancel_io_ex(raw as *mut std::ffi::c_void);
                }
            }
        }
    }
}

#[cfg(windows)]
unsafe fn cancel_io_ex(handle: *mut std::ffi::c_void) {
    #[link(name = "kernel32")]
    extern "system" {
        fn CancelIoEx(hFile: *mut std::ffi::c_void, lpOverlapped: *mut std::ffi::c_void) -> i32;
    }
    let _ = CancelIoEx(handle, std::ptr::null_mut());
}

#[cfg(windows)]
fn spawn_windows_watcher(root: &Path, changed: Arc<Notify>) -> Option<DirWatcher> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;

    const FILE_LIST_DIRECTORY: u32 = 0x0001;
    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_WRITE: u32 = 0x0000_0002;
    const FILE_SHARE_DELETE: u32 = 0x0000_0004;
    const OPEN_EXISTING: u32 = 3;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const INVALID_HANDLE_VALUE: isize = -1;

    const FILE_NOTIFY_CHANGE_FILE_NAME: u32 = 0x0000_0001;
    const FILE_NOTIFY_CHANGE_DIR_NAME: u32 = 0x0000_0002;
    const FILE_NOTIFY_CHANGE_SIZE: u32 = 0x0000_0008;
    const FILE_NOTIFY_CHANGE_LAST_WRITE: u32 = 0x0000_0010;
    const NOTIFY_FILTER: u32 = FILE_NOTIFY_CHANGE_FILE_NAME
        | FILE_NOTIFY_CHANGE_DIR_NAME
        | FILE_NOTIFY_CHANGE_SIZE
        | FILE_NOTIFY_CHANGE_LAST_WRITE;

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateFileW(
            lpFileName: *const u16,
            dwDesiredAccess: u32,
            dwShareMode: u32,
            lpSecurityAttributes: *mut c_void,
            dwCreationDisposition: u32,
            dwFlagsAndAttributes: u32,
            hTemplateFile: *mut c_void,
        ) -> *mut c_void;
        fn ReadDirectoryChangesW(
            hDirectory: *mut c_void,
            lpBuffer: *mut u8,
            nBufferLength: u32,
            bWatchSubtree: i32,
            dwNotifyFilter: u32,
            lpBytesReturned: *mut u32,
            lpOverlapped: *mut c_void,
            lpCompletionRoutine: *mut c_void,
        ) -> i32;
        fn CloseHandle(hObject: *mut c_void) -> i32;
    }

    let wide: Vec<u16> = root
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let raw_handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_LIST_DIRECTORY,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null_mut(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        )
    };
    if raw_handle as isize == INVALID_HANDLE_VALUE || raw_handle.is_null() {
        warn!(path = %root.display(), "Failed to open directory for ReadDirectoryChangesW");
        return None;
    }

    let raw_usize = raw_handle as usize;
    let stop = Arc::new(AtomicBool::new(false));
    let handle_slot = Arc::new(AtomicUsize::new(raw_usize));
    let stop_clone = Arc::clone(&stop);
    let handle_clone = Arc::clone(&handle_slot);
    let root_display = root.display().to_string();

    std::thread::Builder::new()
        .name("turbo-watch".into())
        .spawn(move || {
            let raw_handle = raw_usize as *mut c_void;
            let mut buf = vec![0u8; 64 * 1024];
            while !stop_clone.load(Ordering::SeqCst) {
                let mut bytes_returned: u32 = 0;
                let ok = unsafe {
                    ReadDirectoryChangesW(
                        raw_handle,
                        buf.as_mut_ptr(),
                        buf.len() as u32,
                        1,
                        NOTIFY_FILTER,
                        &mut bytes_returned,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )
                };
                if ok == 0 || stop_clone.load(Ordering::SeqCst) {
                    break;
                }
                changed.notify_one();
            }
            handle_clone.store(0, Ordering::SeqCst);
            unsafe {
                CloseHandle(raw_handle);
            }
            debug!(path = %root_display, "Directory watcher stopped");
        })
        .ok()?;

    Some(DirWatcher {
        stop,
        handle: handle_slot,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn watcher_notifies_on_file_write_and_stops_cleanly() {
        let dir = std::env::temp_dir().join(format!("nanodock-watch-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let notify = Arc::new(Notify::new());
        let watcher = DirWatcher::spawn(&dir, Arc::clone(&notify)).expect("watcher should start");
        tokio::time::sleep(Duration::from_millis(50)).await;

        std::fs::write(dir.join("hello.txt"), "world").unwrap();
        tokio::time::timeout(Duration::from_secs(2), notify.notified())
            .await
            .expect("expected filesystem notification");

        drop(watcher);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
