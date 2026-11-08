use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::ptr::NonNull;

use crate::canvas::{Canvas, H, W};

const KDSETMODE: libc::Ioctl = 0x4b3a;
const KD_TEXT: libc::c_ulong = 0;
const KD_GRAPHICS: libc::c_ulong = 1;

/// The SSD130x framebuffer with the console switched to graphics mode.
pub struct Display {
    tty: File,
    map: NonNull<u8>,
    len: usize,
    stride: usize,
    last: Box<[bool; W * H]>,
}

fn find() -> io::Result<String> {
    for entry in fs::read_dir("/sys/class/graphics")? {
        let path = entry?.path();
        if fs::read_to_string(path.join("name")).is_ok_and(|n| n.trim() == "ssd130xdrmfb") {
            return Ok(path.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    Err(io::Error::other("ssd130x framebuffer not found"))
}

impl Display {
    pub fn open() -> io::Result<Self> {
        let name = find()?;
        let sys = format!("/sys/class/graphics/{name}");
        let read = |f: &str| -> io::Result<usize> {
            fs::read_to_string(format!("{sys}/{f}"))?
                .trim()
                .parse()
                .map_err(io::Error::other)
        };
        if read("bits_per_pixel")? != 32 {
            return Err(io::Error::other("expected a 32 bpp framebuffer"));
        }
        let stride = read("stride")?;
        let len = stride * H;
        let fb = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/dev/{name}"))?;
        // SAFETY: mapping a framebuffer device of at least `len` bytes; checked below.
        let map = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fb.as_raw_fd(),
                0,
            )
        };
        if map == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        let tty = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty1")?;
        // SAFETY: KDSETMODE takes an integer argument.
        if unsafe { libc::ioctl(tty.as_raw_fd(), KDSETMODE, KD_GRAPHICS) } < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut display = Self {
            tty,
            map: NonNull::new(map.cast()).unwrap(),
            len,
            stride,
            last: Box::new([true; W * H]),
        };
        display.show(&Canvas([false; W * H]));
        Ok(display)
    }

    pub fn show(&mut self, canvas: &Canvas) {
        if *self.last == canvas.0 {
            return;
        }
        // SAFETY: the mapping is `len` bytes and lives as long as `self`.
        let fb = unsafe { std::slice::from_raw_parts_mut(self.map.as_ptr(), self.len) };
        for y in 0..H {
            for x in 0..W {
                let v = if canvas.0[y * W + x] { 0xff } else { 0 };
                let i = y * self.stride + x * 4;
                fb[i..i + 3].fill(v);
            }
        }
        *self.last = canvas.0;
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        // SAFETY: undoing the mapping and mode set from `open`.
        unsafe {
            libc::ioctl(self.tty.as_raw_fd(), KDSETMODE, KD_TEXT);
            libc::munmap(self.map.as_ptr().cast(), self.len);
        }
    }
}
