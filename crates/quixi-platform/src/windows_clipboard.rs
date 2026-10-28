//! Native Unicode clipboard access. No credentials or application data are logged.
use std::{ffi::c_void, io, ptr};
type Handle = *mut c_void;

#[link(name = "user32")]
extern "system" {
    fn CreateWindowExW(ex_style: u32, class: *const u16, title: *const u16, style: u32,
        x: i32, y: i32, width: i32, height: i32, parent: Handle, menu: Handle,
        instance: Handle, parameter: Handle) -> Handle;
    fn DestroyWindow(window: Handle) -> i32;
    fn OpenClipboard(window: Handle) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn SetClipboardData(format: u32, memory: Handle) -> Handle;
    fn GetClipboardData(format: u32) -> Handle;
}
#[link(name = "kernel32")]
extern "system" {
    fn GlobalAlloc(flags: u32, bytes: usize) -> Handle;
    fn GlobalLock(memory: Handle) -> Handle;
    fn GlobalUnlock(memory: Handle) -> i32;
    fn GlobalFree(memory: Handle) -> Handle;
    fn GlobalSize(memory: Handle) -> usize;
}
const UNICODE_TEXT: u32 = 13;

struct Window(Handle);
impl Drop for Window { fn drop(&mut self) { unsafe { DestroyWindow(self.0); } } }
struct Clipboard;
impl Drop for Clipboard { fn drop(&mut self) { unsafe { CloseClipboard(); } } }
struct Memory(Handle);
impl Drop for Memory { fn drop(&mut self) { if !self.0.is_null() { unsafe { GlobalFree(self.0); } } } }

fn open(owner: Handle) -> io::Result<Clipboard> {
    for _ in 0..5 {
        if unsafe { OpenClipboard(owner) } != 0 { return Ok(Clipboard); }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    Err(io::Error::last_os_error())
}

pub fn write(text: &str) -> io::Result<()> {
    let encoded: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    // Allocate and fill the transferable buffer before clearing the old contents.
    let mut memory = Memory(unsafe { GlobalAlloc(0x0002, encoded.len() * 2) });
    if memory.0.is_null() { return Err(io::Error::last_os_error()); }
    let data = unsafe { GlobalLock(memory.0) } as *mut u16;
    if data.is_null() { return Err(io::Error::last_os_error()); }
    unsafe { ptr::copy_nonoverlapping(encoded.as_ptr(), data, encoded.len()); GlobalUnlock(memory.0); }
    // EmptyClipboard requires an actual owner window. A private message-only
    // window belongs to this worker thread and does not display any UI.
    let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
    let owner = Window(unsafe { CreateWindowExW(0, class.as_ptr(), class.as_ptr(), 0,
        0, 0, 0, 0, -3isize as Handle, ptr::null_mut(), ptr::null_mut(), ptr::null_mut()) });
    if owner.0.is_null() { return Err(io::Error::last_os_error()); }
    let _clipboard = open(owner.0)?;
    if unsafe { EmptyClipboard() } == 0 { return Err(io::Error::last_os_error()); }
    if unsafe { SetClipboardData(UNICODE_TEXT, memory.0) }.is_null() { return Err(io::Error::last_os_error()); }
    // The OS owns the allocation after a successful transfer.
    memory.0 = ptr::null_mut();
    Ok(())
}

pub fn read() -> io::Result<String> {
    let _clipboard = open(ptr::null_mut())?;
    let memory = unsafe { GetClipboardData(UNICODE_TEXT) };
    if memory.is_null() { return Ok(String::new()); }
    let size = unsafe { GlobalSize(memory) } / 2;
    if size > 16 * 1024 * 1024 { return Err(io::Error::new(io::ErrorKind::InvalidData, "Clipboard text exceeds 32 MB")); }
    let data = unsafe { GlobalLock(memory) } as *const u16;
    if data.is_null() { return Err(io::Error::last_os_error()); }
    let result = unsafe {
        let units = std::slice::from_raw_parts(data, size);
        let end = units.iter().position(|c| *c == 0).unwrap_or(size);
        let text = String::from_utf16_lossy(&units[..end]);
        GlobalUnlock(memory);
        text
    };
    Ok(result)
}
