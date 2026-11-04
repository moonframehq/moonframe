//! The ring in shared memory, read and written.

#![allow(
    unsafe_code,
    reason = "mapping shared memory and reading what another process writes; see each SAFETY"
)]

use {
    crate::{Format, Pattern, valid_name},
    std::{
        ffi::CString,
        io, ptr,
        sync::atomic::{AtomicU64, Ordering, fence},
    },
};

const MAGIC: &[u8; 8] = b"MFRING1\0";
const HEADER: usize = 64;
const SLOT_HEADER: usize = 64;
/// More than any ring needs; a larger header is not one of ours.
const MAX_SLOTS: u32 = 1024;
const OFFSET_SLOTS: usize = 8;
const OFFSET_SLOT_SIZE: usize = 16;
const OFFSET_LATEST: usize = 24;

/// A frame as its slot describes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameInfo {
    pub number: u64,
    pub width: u32,
    pub height: u32,
    pub format: Format,
    pub stride: u32,
    /// Capture time, nanoseconds since the Unix epoch.
    pub time: u64,
    pub pattern: Pattern,
}

/// Shared memory mapped into this process.
struct Mapping {
    start: *mut u8,
    size: usize,
}

// SAFETY: the mapping is plain memory that lives as long as this value;
// every access to what another thread may change goes through atomics or
// is checked afterwards.
unsafe impl Send for Mapping {}
// SAFETY: as for `Send`.
unsafe impl Sync for Mapping {}

impl Drop for Mapping {
    fn drop(&mut self) {
        // SAFETY: `start` and `size` are what `mmap` returned and took.
        unsafe { libc::munmap(self.start.cast(), self.size) };
    }
}

impl Mapping {
    fn atomic(&self, offset: usize) -> &AtomicU64 {
        debug_assert!(offset.is_multiple_of(8) && offset + 8 <= self.size);
        // SAFETY: in the mapping, and 8-byte aligned: the mapping starts on
        // a page, and every offset used is a multiple of 8.
        unsafe { &*self.start.add(offset).cast::<AtomicU64>() }
    }

    fn u32(&self, offset: usize) -> u32 {
        debug_assert!(offset + 4 <= self.size);
        // SAFETY: in the mapping; a torn value is caught by the frame's
        // number or refused by the checks that follow.
        unsafe { self.start.add(offset).cast::<u32>().read_volatile() }
    }

    fn u64(&self, offset: usize) -> u64 {
        debug_assert!(offset + 8 <= self.size);
        // SAFETY: as for `u32`.
        unsafe { self.start.add(offset).cast::<u64>().read_volatile() }
    }

    fn set_u32(&self, offset: usize, value: u32) {
        // SAFETY: in the mapping, which a writer alone writes.
        unsafe { self.start.add(offset).cast::<u32>().write_volatile(value) }
    }

    fn set_u64(&self, offset: usize, value: u64) {
        // SAFETY: as for `set_u32`.
        unsafe { self.start.add(offset).cast::<u64>().write_volatile(value) }
    }
}

fn path(name: &str) -> io::Result<CString> {
    if !valid_name(name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("`{name}` is not a frame source's name"),
        ));
    }
    CString::new(format!("/{name}")).map_err(io::Error::other)
}

/// Maps `size` bytes of `descriptor`, and closes it.
fn map(descriptor: libc::c_int, size: usize, writable: bool) -> io::Result<Mapping> {
    let protection = if writable {
        libc::PROT_READ | libc::PROT_WRITE
    } else {
        libc::PROT_READ
    };
    // SAFETY: a fresh mapping at an address the system picks.
    let start = unsafe {
        libc::mmap(
            ptr::null_mut(),
            size,
            protection,
            libc::MAP_SHARED,
            descriptor,
            0,
        )
    };
    let error = io::Error::last_os_error();
    // SAFETY: the descriptor is ours, and the mapping outlives it.
    unsafe { libc::close(descriptor) };
    if start == libc::MAP_FAILED {
        return Err(error);
    }
    Ok(Mapping {
        start: start.cast(),
        size,
    })
}

/// The size of the memory behind `descriptor`, once it proves to be this
/// user's alone. Names in shared memory are one namespace for the whole
/// machine, so another user could otherwise make a ring first and have
/// their pixels taken for the engine's.
fn own_size(descriptor: libc::c_int) -> io::Result<usize> {
    // SAFETY: `fstat` fills the zeroed struct it is given.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: as above.
    if unsafe { libc::fstat(descriptor, &raw mut stat) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: takes nothing and cannot fail.
    if stat.st_uid != unsafe { libc::geteuid() } {
        return Err(invalid("a ring another user made"));
    }
    if stat.st_mode & 0o077 != 0 {
        return Err(invalid("a ring other users can reach"));
    }
    usize::try_from(stat.st_size).map_err(io::Error::other)
}

/// Takes the newest frames from a ring another process writes.
pub struct Reader {
    mapping: Mapping,
    slots: u32,
    slot_size: usize,
}

impl Reader {
    /// Opens the ring `name`. It fails until the writer made one; refuses
    /// memory that isn't this user's, isn't a ring, or says it is larger
    /// than it is. The writer is trusted not to shrink the memory while
    /// it is mapped, which would crash this process; only the user's own
    /// programs can reach it.
    pub fn open(name: &str) -> io::Result<Self> {
        let path = path(name)?;
        // SAFETY: `path` is a valid C string.
        let descriptor = unsafe { libc::shm_open(path.as_ptr(), libc::O_RDONLY, 0) };
        if descriptor < 0 {
            return Err(io::Error::last_os_error());
        }
        let size = match own_size(descriptor) {
            Ok(size) if size >= HEADER => size,
            Ok(_) => {
                // SAFETY: the descriptor is ours.
                unsafe { libc::close(descriptor) };
                return Err(invalid("too small to be a ring"));
            },
            Err(error) => {
                // SAFETY: as above.
                unsafe { libc::close(descriptor) };
                return Err(error);
            },
        };
        let mapping = map(descriptor, size, false)?;
        // SAFETY: the mapping holds at least the 64-byte header.
        let magic = unsafe { std::slice::from_raw_parts(mapping.start, MAGIC.len()) };
        if magic != MAGIC {
            return Err(invalid("not a Moonframe ring"));
        }
        let slots = mapping.u32(OFFSET_SLOTS);
        let slot_size = usize::try_from(mapping.u64(OFFSET_SLOT_SIZE)).unwrap_or(usize::MAX);
        let fits = (slots as usize)
            .checked_mul(slot_size)
            .and_then(|bytes| bytes.checked_add(HEADER))
            .is_some_and(|bytes| bytes <= size);
        if slots == 0
            || slots > MAX_SLOTS
            || slot_size <= SLOT_HEADER
            || !slot_size.is_multiple_of(8)
            || !fits
        {
            return Err(invalid("a ring's header that does not add up"));
        }
        Ok(Self {
            mapping,
            slots,
            slot_size,
        })
    }

    /// The number of the newest finished frame; 0 before the first.
    pub fn latest(&self) -> u64 {
        self.mapping.atomic(OFFSET_LATEST).load(Ordering::Acquire)
    }

    fn slot(&self, number: u64) -> usize {
        let index = (number - 1) % u64::from(self.slots);
        HEADER + usize::try_from(index).unwrap_or(0) * self.slot_size
    }

    /// Reads frame `number` with `read`, handed its description and
    /// pixels. `None` if the writer has lapped it, before or during the
    /// read, or its description makes no sense: a viewer goes on to the
    /// newest frame then. What `read` was given is unreliable if this
    /// returns `None`, so whatever it made must be dropped.
    pub fn read<T>(
        &self,
        number: u64,
        read: impl FnOnce(&FrameInfo, *const u8, usize) -> T,
    ) -> Option<T> {
        if number == 0 {
            return None;
        }
        let slot = self.slot(number);
        let seal = self.mapping.atomic(slot);
        if seal.load(Ordering::Acquire) != number {
            return None;
        }
        let info = self.info(number, slot)?;
        let bytes = info.stride as usize * info.height as usize;
        // SAFETY: `info` checked that the rows fit the slot, in the mapping.
        let pixels = unsafe { self.mapping.start.add(slot + SLOT_HEADER) };
        let made = read(&info, pixels.cast_const(), bytes);
        fence(Ordering::Acquire);
        (seal.load(Ordering::Relaxed) == number).then_some(made)
    }

    fn info(&self, number: u64, slot: usize) -> Option<FrameInfo> {
        let mapping = &self.mapping;
        let (width, height) = (mapping.u32(slot + 8), mapping.u32(slot + 12));
        let format = Format::from_raw(mapping.u32(slot + 16))?;
        let stride = mapping.u32(slot + 20);
        let row = (width as usize).checked_mul(format.bytes_per_pixel())?;
        let bytes = (stride as usize).checked_mul(height as usize)?;
        if width == 0 || height == 0 || (stride as usize) < row {
            return None;
        }
        if bytes > self.slot_size - SLOT_HEADER {
            return None;
        }
        Some(FrameInfo {
            number,
            width,
            height,
            format,
            stride,
            time: mapping.u64(slot + 24),
            pattern: Pattern::from_raw(mapping.u32(slot + 32)),
        })
    }
}

fn invalid(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what.to_owned())
}

/// Writes frames into a ring of its own, for an engine in Rust. The ring
/// goes when this does.
pub struct Writer {
    mapping: Mapping,
    name: CString,
    slots: u32,
    slot_size: usize,
    number: u64,
}

impl Writer {
    /// Makes the ring `name`, of `slots` slots holding `bytes` of pixels
    /// each, replacing one left behind by a run that ended badly.
    pub fn create(name: &str, slots: u32, bytes: usize) -> io::Result<Self> {
        if slots == 0 || slots > MAX_SLOTS {
            return Err(invalid("a ring has 1 to 1024 slots"));
        }
        let path = path(name)?;
        let slot_size = (SLOT_HEADER + bytes).next_multiple_of(8);
        let size = HEADER + slots as usize * slot_size;
        // SAFETY: `path` is a valid C string.
        unsafe { libc::shm_unlink(path.as_ptr()) };
        // SAFETY: as above; the mode is a plain integer, widened as the
        // variadic call requires.
        let descriptor = unsafe {
            libc::shm_open(
                path.as_ptr(),
                libc::O_CREAT | libc::O_EXCL | libc::O_RDWR,
                0o600 as libc::c_uint,
            )
        };
        if descriptor < 0 {
            return Err(io::Error::last_os_error());
        }
        let length = libc::off_t::try_from(size).map_err(io::Error::other)?;
        // SAFETY: the descriptor is ours.
        if unsafe { libc::ftruncate(descriptor, length) } != 0 {
            let error = io::Error::last_os_error();
            // SAFETY: as above.
            unsafe { libc::close(descriptor) };
            return Err(error);
        }
        let mapping = map(descriptor, size, true)?;
        mapping.set_u32(OFFSET_SLOTS, slots);
        mapping.set_u64(OFFSET_SLOT_SIZE, slot_size as u64);
        // SAFETY: the mapping holds the header, and only this writes it.
        unsafe { ptr::copy_nonoverlapping(MAGIC.as_ptr(), mapping.start, MAGIC.len()) };
        Ok(Self {
            mapping,
            name: path,
            slots,
            slot_size,
            number: 0,
        })
    }

    /// Writes a frame of `pixels`; its number, or an error if it doesn't
    /// fit a slot.
    pub fn write(
        &mut self,
        width: u32,
        height: u32,
        format: Format,
        pattern: Pattern,
        time: u64,
        pixels: &[u8],
    ) -> io::Result<u64> {
        if pixels.len() > self.slot_size - SLOT_HEADER {
            return Err(invalid("a frame larger than its slot"));
        }
        let stride =
            u32::try_from(width as usize * format.bytes_per_pixel()).map_err(io::Error::other)?;
        if stride as usize * height as usize != pixels.len() {
            return Err(invalid("pixels that aren't width by height"));
        }
        let number = self.number + 1;
        let index = usize::try_from((number - 1) % u64::from(self.slots)).unwrap_or(0);
        let slot = HEADER + index * self.slot_size;
        let mapping = &self.mapping;
        mapping.atomic(slot).store(0, Ordering::Release);
        fence(Ordering::Release);
        mapping.set_u32(slot + 8, width);
        mapping.set_u32(slot + 12, height);
        mapping.set_u32(slot + 16, format.raw());
        mapping.set_u32(slot + 20, stride);
        mapping.set_u64(slot + 24, time);
        mapping.set_u32(slot + 32, pattern.raw());
        // SAFETY: checked above to fit the slot, in the mapping.
        unsafe {
            ptr::copy_nonoverlapping(
                pixels.as_ptr(),
                mapping.start.add(slot + SLOT_HEADER),
                pixels.len(),
            );
        }
        mapping.atomic(slot).store(number, Ordering::Release);
        mapping
            .atomic(OFFSET_LATEST)
            .store(number, Ordering::Release);
        self.number = number;
        Ok(number)
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        // SAFETY: a valid C string; a viewer's mapping stays until it goes.
        unsafe { libc::shm_unlink(self.name.as_ptr()) };
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, unsafe_code)]
mod tests {
    use super::*;

    fn name(test: &str) -> String {
        format!("mf{}-{test}", std::process::id())
    }

    fn pixels(reader: &Reader, number: u64) -> Option<(FrameInfo, Vec<u8>)> {
        reader.read(number, |info, pixels, bytes| {
            // SAFETY: `read` hands out `bytes` readable bytes.
            let copy = unsafe { std::slice::from_raw_parts(pixels, bytes) }.to_vec();
            (*info, copy)
        })
    }

    #[test]
    fn a_reader_gets_what_was_written() {
        let name = name("read");
        let mut writer = Writer::create(&name, 3, 16).unwrap();
        let reader = Reader::open(&name).unwrap();
        assert_eq!(reader.latest(), 0);
        assert!(pixels(&reader, 0).is_none());
        let bytes: Vec<u8> = (0..16).collect();
        let number = writer
            .write(2, 2, Format::Rgba8, Pattern::Rggb, 77, &bytes)
            .unwrap();
        assert_eq!(number, 1);
        assert_eq!(reader.latest(), 1);
        let (info, read) = pixels(&reader, 1).unwrap();
        assert_eq!((info.width, info.height, info.stride), (2, 2, 8));
        assert_eq!((info.format, info.time), (Format::Rgba8, 77));
        assert_eq!(read, bytes);
    }

    #[test]
    fn a_lapped_frame_is_not_read_and_the_writer_never_waits() {
        let name = name("lap");
        let mut writer = Writer::create(&name, 2, 8).unwrap();
        let reader = Reader::open(&name).unwrap();
        for _ in 0..5 {
            writer
                .write(1, 1, Format::Rgba16, Pattern::Rggb, 0, &[0; 8])
                .unwrap();
        }
        assert_eq!(reader.latest(), 5);
        assert!(pixels(&reader, 1).is_none());
        assert!(pixels(&reader, 5).is_some());
    }

    #[test]
    fn a_frame_overwritten_during_the_read_is_dropped() {
        let name = name("torn");
        let mut writer = Writer::create(&name, 1, 8).unwrap();
        let reader = Reader::open(&name).unwrap();
        writer
            .write(1, 1, Format::Rgba16, Pattern::Rggb, 0, &[0; 8])
            .unwrap();
        let read = reader.read(1, |_, _, _| {
            writer
                .write(1, 1, Format::Rgba16, Pattern::Rggb, 0, &[1; 8])
                .unwrap();
        });
        assert!(read.is_none());
    }

    #[test]
    fn a_frame_that_does_not_fit_is_refused() {
        let name = name("big");
        let mut writer = Writer::create(&name, 1, 8).unwrap();
        assert!(
            writer
                .write(4, 4, Format::Rgba8, Pattern::Rggb, 0, &[0; 64])
                .is_err()
        );
        assert!(
            writer
                .write(9, 9, Format::Rgba8, Pattern::Rggb, 0, &[0; 8])
                .is_err()
        );
    }

    #[test]
    fn a_ring_other_users_can_reach_is_refused() {
        let name = name("open");
        let writer = Writer::create(&name, 1, 8).unwrap();
        // SAFETY: a valid C string and mode, widened as `fchmod` takes it.
        unsafe {
            let descriptor = libc::shm_open(writer.name.as_ptr(), libc::O_RDWR, 0);
            assert!(descriptor >= 0);
            if libc::fchmod(descriptor, 0o666) == 0 {
                let opened = Reader::open(&name);
                assert!(opened.is_err());
            }
            libc::close(descriptor);
        }
    }

    #[test]
    fn opening_what_is_not_there_or_not_a_ring_fails() {
        assert!(Reader::open(&name("absent")).is_err());
        assert!(Reader::open("../etc/passwd").is_err());
    }

    #[test]
    fn dimensions_past_the_slot_are_refused_as_if_lapped() {
        let name = name("lies");
        let mut writer = Writer::create(&name, 1, 8).unwrap();
        let reader = Reader::open(&name).unwrap();
        writer
            .write(1, 1, Format::Rgba16, Pattern::Rggb, 0, &[0; 8])
            .unwrap();
        // An engine with a bug says the frame is enormous.
        writer.mapping.set_u32(HEADER + 12, 1_000_000);
        assert!(pixels(&reader, 1).is_none());
    }
}
