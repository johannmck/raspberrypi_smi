use std::fs::File;
use std::os::unix::io::{IntoRawFd, FromRawFd, RawFd};
use nix::ioctl_readwrite;
use std::io::{self};

const VCIO_PATH: &str = "/dev/vcio";
const PAGE_SIZE: u32 = 4096;

#[repr(C, align(16))]
pub struct VcMsg {
    len: u32,
    req: u32,
    tag: u32,
    blen: u32,
    dlen: u32,
    uints: [u32; 27],
}

ioctl_readwrite!(vc_ioctl, 100, 0, VcMsg);

pub struct VcMailbox {
    fd: RawFd,
}

impl VcMailbox {
    pub fn open() -> io::Result<Self> {
        let file = File::open(VCIO_PATH)?;
        Ok(Self {
            fd: file.into_raw_fd(),
        })
    }

    pub fn close(&self) {
        let _ = unsafe { File::from_raw_fd(self.fd) };
    }

    pub fn send_msg(&self, msg: &mut VcMsg) -> io::Result<u32> {
        unsafe {
            for i in (msg.dlen as usize / 4)..(msg.blen as usize / 4) {
                msg.uints[i] = 0;
            }
            msg.len = (msg.blen + 6) * 4;
            msg.req = 0;

            vc_ioctl(self.fd, msg).map_err(|e| io::Error::from_raw_os_error(e as i32))?;

            if msg.req & 0x80000000 == 0 {
                return Err(io::Error::new(io::ErrorKind::Other, "VC IOCTL error"));
            } else if msg.req == 0x80000001 {
                return Err(io::Error::new(io::ErrorKind::Other, "VC IOCTL partial error"));
            }

            Ok(msg.uints[0])
        }
    }

    pub fn alloc_mem(&self, size: u32, flags: u32) -> io::Result<u32> {
        let mut msg = VcMsg {
            len: 0,
            req: 0,
            tag: 0x3000c,
            blen: 12,
            dlen: 12,
            uints: {
                let mut arr = [0; 27];
                arr[0] = (size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
                arr[1] = PAGE_SIZE;
                arr[2] = flags;
                arr
            },
        };


        self.send_msg(&mut msg)
    }

    pub fn lock_mem(&self, handle: u32) -> io::Result<u32> {
        let mut msg = VcMsg {
            len: 0,
            req: 0,
            tag: 0x3000d,
            blen: 4,
            dlen: 4,
            uints: {
                let mut arr = [0; 27];
                arr[0] = handle;
                arr
            },
        };
        self.send_msg(&mut msg)
    }

    pub fn unlock_mem(&self, handle: u32) -> io::Result<u32> {
        let mut msg = VcMsg {
            len: 0,
            req: 0,
            tag: 0x3000e,
            blen: 4,
            dlen: 4,
            uints: {
                let mut arr = [0; 27];
                arr[0] = handle;
                arr
            },
        };
        self.send_msg(&mut msg)
    }

    pub fn free_mem(&self, handle: u32) -> io::Result<u32> {
        let mut msg = VcMsg {
            len: 0,
            req: 0,
            tag: 0x3000f,
            blen: 4,
            dlen: 4,
            uints: {
                let mut arr = [0; 27];
                arr[0] = handle;
                arr
            },
        };
        self.send_msg(&mut msg)
    }
}

impl Drop for VcMailbox {
    fn drop(&mut self) {
        self.close();
    }
}
