//! `image.encode_texture` encodes a block-compressed texture (BC1-BC7) on several threads in a
//! pregame pass, with the bytes of the serial encode.
//!
//! BC7 is nearly all of a texture-heavy pass's time: EMF Beastmaster's three packs spent 397 of
//! their 401 s of Lua in it on one core. Each BC block depends only on its own 4x4 pixels
//! (block_compression makes a new compressor per block), so the image is cut into strips of whole
//! block rows and each strip goes through the same `kfc::content::image::encode`, with the image's
//! width and the strip's height. The strips are put back in order: byte for byte the serial encode
//! (encode_in_strips_tests.rs, the loader's encode_pass_tests.rs).
//!
//! The thread count is 1 (the serial encode) unless a pregame pass sets it
//! (`EncodeThreads::for_pregame_pass` in `run_with_api`): the PC's threads, at most 16, or
//! `SHROUDFORGE_ENCODE_THREADS`. The in-game runtime stays serial.
//!
//! The Windows loader lock: a thread created while another thread holds it (a DLL's initializer),
//! or while this one does (code called from a DllMain), does not run until the lock is free. So the
//! calling thread never waits for a helper to start: it takes strips itself from the same counter
//! as the helpers and waits only for strips a helper has already begun. With no helper running it
//! encodes every strip alone, in the time of the serial encode. While a helper made for an earlier
//! texture has still not started, no new helpers are made.
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::{
    Arc, Condvar, Mutex, MutexGuard, PoisonError, RwLock,
    atomic::{AtomicUsize, Ordering},
};

use kfc::content::image::{PixelFormat, encode, size_of_format};

/// The environment variable that sets a pregame pass's thread count (1: the serial encode).
pub const THREADS_VARIABLE: &str = "SHROUDFORGE_ENCODE_THREADS";
/// A pregame pass's default: the PC's threads, at most this many.
const DEFAULT_MAX_THREADS: usize = 16;
/// `SHROUDFORGE_ENCODE_THREADS` is used as it is, at most this many.
const MAX_THREADS: usize = 64;
/// A strip has at least this many blocks: a smaller one costs more in a new thread than it saves.
const MIN_STRIP_BLOCKS: usize = 64;

static THREADS: AtomicUsize = AtomicUsize::new(1);
/// (the `EncodeThreads` still alive, the count before the first of them)
static SCOPES: Mutex<(usize, usize)> = Mutex::new((0, 1));
static TEXTURES_IN_STRIPS: AtomicUsize = AtomicUsize::new(0);
/// Helper threads made that have not started yet.
static HELPERS_NOT_STARTED: AtomicUsize = AtomicUsize::new(0);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The thread count `image.encode_texture` gives a block-compressed texture now (1: the serial
/// encode).
pub fn encode_threads() -> usize {
    THREADS.load(Ordering::Relaxed).max(1)
}

/// How many textures were cut into strips in this process (tests read it to see that a pass used
/// threads).
pub fn textures_encoded_in_strips() -> usize {
    TEXTURES_IN_STRIPS.load(Ordering::Relaxed)
}

/// A pregame pass's thread count: `variable` (SHROUDFORGE_ENCODE_THREADS) when it is a count of at
/// least 1 (at most 64), otherwise the PC's threads (`available`), at most 16.
pub fn pregame_encode_threads(variable: Option<&str>, available: usize) -> usize {
    match variable
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|&n| n >= 1)
    {
        Some(n) => n.min(MAX_THREADS),
        None => available.clamp(1, DEFAULT_MAX_THREADS),
    }
}

/// The thread count for as long as this lives. When the last one alive ends, the count before the
/// first comes back.
#[must_use]
pub struct EncodeThreads(());

impl EncodeThreads {
    /// Sets the count (0 is taken as 1).
    pub fn set(threads: usize) -> Self {
        let mut scopes = lock(&SCOPES);
        let previous = THREADS.swap(threads.max(1), Ordering::Relaxed);
        if scopes.0 == 0 {
            scopes.1 = previous;
        }
        scopes.0 += 1;
        Self(())
    }

    /// A pregame pass's count (`pregame_encode_threads` with the environment and the PC's threads).
    pub fn for_pregame_pass() -> Self {
        let available = std::thread::available_parallelism().map_or(1, |n| n.get());
        Self::set(pregame_encode_threads(
            std::env::var(THREADS_VARIABLE).ok().as_deref(),
            available,
        ))
    }
}

impl Drop for EncodeThreads {
    fn drop(&mut self) {
        let mut scopes = lock(&SCOPES);
        scopes.0 = scopes.0.saturating_sub(1);
        if scopes.0 == 0 {
            THREADS.store(scopes.1, Ordering::Relaxed);
        }
    }
}

/// `image.encode_texture`'s encode: kfc's `encode`, block-compressed formats on `encode_threads()`
/// threads.
pub(super) fn encode_texture(
    format: PixelFormat,
    width: usize,
    height: usize,
    src: &[u8],
    dst: &mut [u8],
) -> anyhow::Result<()> {
    encode_with_threads(format, width, height, src, dst, encode_threads())
}

/// kfc's `encode`, with a block-compressed format cut into strips of at least 64 blocks on up to
/// `threads` threads. Byte for byte the serial encode.
pub fn encode_with_threads(
    format: PixelFormat,
    width: usize,
    height: usize,
    src: &[u8],
    dst: &mut [u8],
    threads: usize,
) -> anyhow::Result<()> {
    encode_split(
        format,
        width,
        height,
        src,
        dst,
        threads,
        MIN_STRIP_BLOCKS,
        &OsThreads,
    )
}

/// The formats whose blocks kfc encodes with block_compression, one block at a time.
fn splittable(format: PixelFormat) -> bool {
    use PixelFormat::*;
    matches!(
        format,
        BC1_RGB_unorm_block
            | BC1_RGB_srgb_block
            | BC1_RGBA_unorm_block
            | BC1_RGBA_srgb_block
            | BC2_unorm_block
            | BC2_srgb_block
            | BC3_unorm_block
            | BC3_srgb_block
            | BC4_unorm_block
            | BC4_snorm_block
            | BC5_unorm_block
            | BC5_snorm_block
            | BC6H_ufloat_block
            | BC7_unorm_block
            | BC7_srgb_block
    )
}

/// How many strips an image of `blocks_wide` x `block_rows` blocks is cut into: at most `threads`,
/// at most one a block row, and none under `min_blocks` blocks on average.
pub(super) fn strips_for(
    blocks_wide: usize,
    block_rows: usize,
    threads: usize,
    min_blocks: usize,
) -> usize {
    let by_size = blocks_wide.saturating_mul(block_rows) / min_blocks.max(1);
    threads.min(block_rows).min(by_size).max(1)
}

/// The work a helper thread is given.
pub(super) type Helper = Box<dyn FnOnce() + Send + 'static>;

/// Where helper threads come from. `stalled`: an earlier helper has not started yet, so make none
/// now.
pub(super) trait Spawner {
    fn stalled(&self) -> bool {
        false
    }
    /// Starts `helper` on a new thread; false when none could be made.
    fn spawn(&self, helper: Helper) -> bool;
}

/// Tests: any function that takes a helper is a spawner that never stalls.
impl<F: Fn(Helper) -> bool> Spawner for F {
    fn spawn(&self, helper: Helper) -> bool {
        self(helper)
    }
}

/// The operating system's threads.
pub(super) struct OsThreads;

impl Spawner for OsThreads {
    fn stalled(&self) -> bool {
        HELPERS_NOT_STARTED.load(Ordering::Acquire) > 0
    }

    fn spawn(&self, helper: Helper) -> bool {
        HELPERS_NOT_STARTED.fetch_add(1, Ordering::AcqRel);
        let made = std::thread::Builder::new()
            .name("sf-texture-encode".into())
            .spawn(move || {
                HELPERS_NOT_STARTED.fetch_sub(1, Ordering::AcqRel);
                helper()
            })
            .is_ok();
        if !made {
            HELPERS_NOT_STARTED.fetch_sub(1, Ordering::AcqRel);
        }
        made
    }
}

type StripResult = std::thread::Result<anyhow::Result<Vec<u8>>>;

struct Job {
    format: PixelFormat,
    width: usize,
    block_rows: usize,
    rows_per_strip: usize,
    strips: usize,
    /// The image's RGBA8 pixels; emptied once every strip is done, so a helper that starts late
    /// holds no copy.
    pixels: RwLock<Vec<u8>>,
    next: AtomicUsize,
    done: Mutex<(Vec<Option<StripResult>>, usize)>,
    all_done: Condvar,
}

impl Job {
    /// Takes strips until none is left (the calling thread and every helper).
    fn work(&self) {
        loop {
            let strip = self.next.fetch_add(1, Ordering::AcqRel);
            if strip >= self.strips {
                return;
            }
            let result = catch_unwind(AssertUnwindSafe(|| self.encode_strip(strip)));
            let mut done = lock(&self.done);
            done.0[strip] = Some(result);
            done.1 += 1;
            if done.1 == self.strips {
                self.all_done.notify_all();
            }
        }
    }

    fn encode_strip(&self, strip: usize) -> anyhow::Result<Vec<u8>> {
        let first = strip * self.rows_per_strip;
        let rows = self.rows_per_strip.min(self.block_rows - first);
        let row_bytes = self.width * 4 * 4; // one block row: four image rows of RGBA8
        let pixels = self.pixels.read().unwrap_or_else(PoisonError::into_inner);
        let src = &pixels[first * row_bytes..(first + rows) * row_bytes];
        let mut out = vec![0u8; size_of_format(self.format, self.width, rows * 4)];
        encode(self.format, self.width, rows * 4, src, &mut out)?;
        Ok(out)
    }
}

/// kfc's `encode` of the whole image, or for a block-compressed format of whole blocks: cut into
/// strips of whole block rows (`strips_for`), helpers from `spawner`, the calling thread taking
/// strips too. Anything else (another format, a size that is not whole blocks, buffers too short, a
/// single strip) is kfc's call unchanged, with its own checks and errors.
pub(super) fn encode_split(
    format: PixelFormat,
    width: usize,
    height: usize,
    src: &[u8],
    dst: &mut [u8],
    threads: usize,
    min_blocks: usize,
    spawner: &dyn Spawner,
) -> anyhow::Result<()> {
    let whole_blocks = splittable(format)
        && width > 0
        && width % 4 == 0
        && height % 4 == 0
        && src.len() >= width * height * 4
        && dst.len() >= size_of_format(format, width, height);
    let wanted = if whole_blocks {
        strips_for(width / 4, height / 4, threads, min_blocks)
    } else {
        1
    };
    if wanted <= 1 {
        return encode(format, width, height, src, dst);
    }
    TEXTURES_IN_STRIPS.fetch_add(1, Ordering::Relaxed);
    let block_rows = height / 4;
    let rows_per_strip = block_rows.div_ceil(wanted);
    let strips = block_rows.div_ceil(rows_per_strip);
    let job = Arc::new(Job {
        format,
        width,
        block_rows,
        rows_per_strip,
        strips,
        pixels: RwLock::new(src[..width * height * 4].to_vec()),
        next: AtomicUsize::new(0),
        done: Mutex::new(((0..strips).map(|_| None).collect(), 0)),
        all_done: Condvar::new(),
    });
    if !spawner.stalled() {
        for _ in 1..threads.min(strips) {
            let helper = Arc::clone(&job);
            if !spawner.spawn(Box::new(move || helper.work())) {
                break;
            }
        }
    }
    job.work();
    // every strip is taken; wait only for those a running helper still encodes
    let results = {
        let mut done = lock(&job.done);
        while done.1 < strips {
            done = job
                .all_done
                .wait(done)
                .unwrap_or_else(PoisonError::into_inner);
        }
        std::mem::take(&mut done.0)
    };
    std::mem::take(&mut *job.pixels.write().unwrap_or_else(PoisonError::into_inner));
    let row_bytes = size_of_format(format, width, 4); // the bytes of one block row
    for (strip, result) in results.into_iter().enumerate() {
        let bytes = match result.expect("every strip is done") {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(error)) => return Err(error),
            Err(panic) => resume_unwind(panic),
        };
        let start = strip * rows_per_strip * row_bytes;
        dst[start..start + bytes.len()].copy_from_slice(&bytes);
    }
    Ok(())
}
