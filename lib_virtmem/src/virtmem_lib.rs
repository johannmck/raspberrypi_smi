use std::fs::OpenOptions;
use std::os::unix::prelude::OpenOptionsExt;
use std::os::fd::AsRawFd;
use std::io::{self};
use libc::{mmap, munmap, PROT_READ, PROT_WRITE, MAP_SHARED, MAP_FAILED, O_SYNC, size_t};

const VCIO_PATH: &str = "/dev/mem";
const PAGE_SIZE: usize = 4096;

fn page_roundup(n: usize) -> usize {
    if n % PAGE_SIZE == 0 {
        n
    } else {
        (n + PAGE_SIZE) & !(PAGE_SIZE - 1)
    }
}

fn map_segment(addr: usize, size: usize) -> Result<*mut libc::c_void, String> {
    let rounded_size = page_roundup(size);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(O_SYNC)
        .open(VCIO_PATH)
        .map_err(|e| format!("Error: can't open /dev/mem: {}", e))?;
    
    let fd = file.as_raw_fd();
    let mapped_mem = unsafe {
        mmap(
            std::ptr::null_mut(),
            rounded_size as size_t,
            PROT_READ | PROT_WRITE,
            MAP_SHARED,
            fd,
            addr as libc::off_t,
        )
    };
    if mapped_mem == MAP_FAILED {
        Err(String::from("Error: can't map memory"))
    } else {
        #[cfg(debug_assertions)]
        println!("Map {:#x} -> {:?}", addr, mapped_mem);
        Ok(mapped_mem)
    }
}

// Unmap a memory segment
fn unmap_segment(mem: *mut libc::c_void, size: usize) -> Result<(), io::Error> {
    if !mem.is_null() {
        let rounded_size = page_roundup(size);
        let result = unsafe { munmap(mem, rounded_size) };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}
