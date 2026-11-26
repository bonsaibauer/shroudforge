//! A block-compressed texture encoded on several threads is byte for byte the encode on one thread
//! (encode_in_strips.rs).
//!
//! Each BC block depends only on its own 4x4 pixels (block_compression makes a new compressor per
//! block), so the image is cut into strips of whole block rows and each strip is encoded on its
//! own. These tests hold that to kfc's serial `kfc::content::image::encode` for every BC format the
//! runtime encodes, for strips that do not divide evenly, more threads than block rows, the
//! one-block level and the error cases, and for helper threads that start late or never (a Windows
//! loader lock held elsewhere): the calling thread then encodes every strip itself.
use super::encode_in_strips::*;
use kfc::content::image::{PixelFormat, encode, size_of_format};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;

const BC: &[PixelFormat] = &[
    PixelFormat::BC1_RGB_unorm_block,
    PixelFormat::BC1_RGB_srgb_block,
    PixelFormat::BC1_RGBA_unorm_block,
    PixelFormat::BC1_RGBA_srgb_block,
    PixelFormat::BC2_unorm_block,
    PixelFormat::BC2_srgb_block,
    PixelFormat::BC3_unorm_block,
    PixelFormat::BC3_srgb_block,
    PixelFormat::BC4_unorm_block,
    PixelFormat::BC4_snorm_block,
    PixelFormat::BC5_unorm_block,
    PixelFormat::BC5_snorm_block,
    PixelFormat::BC6H_ufloat_block,
    PixelFormat::BC6H_sfloat_block, // not implemented in kfc: the same error on any thread count
    PixelFormat::BC7_unorm_block,
    PixelFormat::BC7_srgb_block,
];

/// A deterministic image with edges, gradients and alpha, so that every BC mode has something to
/// choose between.
fn rgba(width: usize, height: usize, seed: u64) -> Vec<u8> {
    let mut state = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
    let mut out = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let noise = (state >> 56) as u8;
            out.extend_from_slice(&[
                (x * 255 / width.max(1)) as u8 ^ (noise & 0x0f),
                (y * 255 / height.max(1)) as u8,
                if (x / 3 + y / 5) % 2 == 0 { 40 } else { 220 } ^ (noise >> 4),
                if x % 7 == 0 { 0 } else { 255 - (noise & 0x3f) },
            ]);
        }
    }
    out
}

fn outcome(
    run: impl FnOnce(&mut [u8]) -> anyhow::Result<()>,
    len: usize,
) -> Result<Vec<u8>, String> {
    let mut dst = vec![0xa5u8; len];
    match catch_unwind(AssertUnwindSafe(|| run(&mut dst))) {
        Ok(Ok(())) => Ok(dst),
        Ok(Err(e)) => Err(format!("error: {e}")),
        Err(_) => Err("panic".into()),
    }
}

/// kfc's encode on the calling thread.
fn serial(format: PixelFormat, w: usize, h: usize, src: &[u8]) -> Result<Vec<u8>, String> {
    outcome(
        |dst| encode(format, w, h, src, dst),
        size_of_format(format, w, h),
    )
}

/// The strip encode with real helper threads and no minimum strip size.
fn split(
    format: PixelFormat,
    w: usize,
    h: usize,
    src: &[u8],
    threads: usize,
) -> Result<Vec<u8>, String> {
    outcome(
        |dst| encode_split(format, w, h, src, dst, threads, 1, &OsThreads),
        size_of_format(format, w, h),
    )
}

#[test]
fn every_bc_format_encodes_the_same_bytes_on_any_number_of_threads() {
    // (width, height): one block; one block row; a square mip; 13 block rows (no thread count
    // divides them evenly); a tall strip; a wide one
    let sizes = [(4, 4), (16, 4), (64, 64), (128, 52), (8, 256), (256, 8)];
    for &format in BC {
        for (i, &(w, h)) in sizes.iter().enumerate() {
            let src = rgba(w, h, i as u64 + 1);
            let serial = serial(format, w, h, &src);
            if format != PixelFormat::BC6H_sfloat_block {
                let bytes = serial
                    .as_ref()
                    .unwrap_or_else(|e| panic!("{format:?} {w}x{h}: {e}"));
                assert!(
                    bytes.iter().any(|&b| b != 0xa5),
                    "{format:?} {w}x{h}: the serial encode wrote nothing"
                );
            }
            for threads in [2, 3, 4, 7, 16, 64] {
                let split = split(format, w, h, &src, threads);
                assert!(
                    split == serial,
                    "{format:?} {w}x{h} on {threads} threads differs from the serial encode"
                );
            }
        }
    }
}

#[test]
fn the_runtime_encode_with_the_cores_is_the_serial_encode() {
    // the path image.encode_texture takes (encode_with_threads: at least 64 blocks a strip, real
    // threads)
    for &format in &[
        PixelFormat::BC7_srgb_block,
        PixelFormat::BC1_RGBA_srgb_block,
        PixelFormat::BC5_unorm_block,
    ] {
        for &(w, h) in &[(256, 256), (512, 128), (64, 68), (32, 32)] {
            let src = rgba(w, h, 11);
            let ours = outcome(
                |dst| encode_with_threads(format, w, h, &src, dst, 16),
                size_of_format(format, w, h),
            );
            assert!(ours == serial(format, w, h, &src), "{format:?} {w}x{h}");
        }
    }
}

#[test]
fn sizes_that_are_not_whole_blocks_fail_as_the_serial_encode_does() {
    // block_compression asserts whole blocks; Lua callers pad a level under 4x4 themselves.
    // Whatever the serial encode does with such a size (it panics), the split one does too, and
    // never writes a partial result instead.
    for &format in BC {
        for (w, h) in [(6, 8), (8, 6), (2, 2), (1, 1), (0, 0)] {
            let src = rgba(w, h, 7);
            let serial = serial(format, w, h, &src);
            for threads in [2, 8] {
                assert_eq!(
                    split(format, w, h, &src, threads),
                    serial,
                    "{format:?} {w}x{h} x{threads}"
                );
            }
        }
    }
}

#[test]
fn uncompressed_formats_ignore_the_thread_count() {
    for format in [
        PixelFormat::R8G8B8A8_unorm,
        PixelFormat::R8_unorm,
        PixelFormat::R16G16B16A16_sfloat,
    ] {
        let src = rgba(32, 12, 3);
        assert_eq!(
            split(format, 32, 12, &src, 8),
            serial(format, 32, 12, &src),
            "{format:?}"
        );
    }
}

#[test]
fn helpers_that_never_start_leave_every_strip_to_the_calling_thread() {
    // Under a Windows loader lock that another thread holds (or this one, in a DllMain), a new
    // thread is created but does not run until the lock is free. The caller then encodes every
    // strip itself and never waits for a helper.
    let (w, h) = (64, 96);
    let src = rgba(w, h, 5);
    let expected = serial(PixelFormat::BC7_srgb_block, w, h, &src);
    let parked = Mutex::new(Vec::new());
    let never_runs = |job: Helper| {
        parked.lock().unwrap().push(job);
        true
    };
    let ours = outcome(
        |dst| {
            encode_split(
                PixelFormat::BC7_srgb_block,
                w,
                h,
                &src,
                dst,
                8,
                1,
                &never_runs,
            )
        },
        size_of_format(PixelFormat::BC7_srgb_block, w, h),
    );
    assert!(
        ours == expected,
        "the caller's own strips are the serial encode"
    );
    let parked = parked.into_inner().unwrap();
    assert_eq!(parked.len(), 7, "up to threads - 1 helpers were asked for");
    // the lock is free at last: they start, find no strip left and end, without touching the result
    for job in parked {
        job();
    }
    // a helper that cannot even be created: the same bytes
    let refused = |_: Helper| false;
    let ours = outcome(
        |dst| encode_split(PixelFormat::BC7_srgb_block, w, h, &src, dst, 8, 1, &refused),
        size_of_format(PixelFormat::BC7_srgb_block, w, h),
    );
    assert!(ours == expected);
}

#[test]
fn small_levels_are_not_cut_below_the_minimum_strip() {
    // A strip of fewer than `min_blocks` blocks costs more in a new thread than it saves: those
    // levels stay whole.
    assert_eq!(
        strips_for(64 / 4, 64 / 4, 16, 64),
        4,
        "256 blocks, at least 64 a strip"
    );
    assert_eq!(
        strips_for(8 / 4, 8 / 4, 16, 64),
        1,
        "4 blocks: one strip, the calling thread alone"
    );
    assert_eq!(
        strips_for(1024 / 4, 1024 / 4, 16, 64),
        16,
        "the thread count bounds it"
    );
    assert_eq!(
        strips_for(4096 / 4, 4 / 4, 16, 64),
        1,
        "a single block row is never cut"
    );
    assert_eq!(
        strips_for(16 / 4, 52 / 4, 64, 1),
        13,
        "at most one strip a block row"
    );
    assert_eq!(strips_for(16 / 4, 52 / 4, 1, 1), 1);
}

#[test]
fn the_pregame_pass_gets_the_cores() {
    // the PC's threads, at most 16
    assert_eq!(pregame_encode_threads(None, 16), 16);
    assert_eq!(pregame_encode_threads(None, 32), 16);
    assert_eq!(pregame_encode_threads(None, 4), 4);
    assert_eq!(pregame_encode_threads(None, 0), 1);
    // SHROUDFORGE_ENCODE_THREADS: 1 is the serial encode; another count is used as it is (at most
    // 64)
    assert_eq!(THREADS_VARIABLE, "SHROUDFORGE_ENCODE_THREADS");
    assert_eq!(pregame_encode_threads(Some("1"), 16), 1);
    assert_eq!(pregame_encode_threads(Some(" 3 "), 16), 3);
    assert_eq!(pregame_encode_threads(Some("40"), 16), 40);
    assert_eq!(pregame_encode_threads(Some("200"), 16), 64);
    for junk in ["0", "", "x", "-2", "2.5"] {
        assert_eq!(
            pregame_encode_threads(Some(junk), 6),
            6,
            "{junk:?} is not a count: the default"
        );
    }
}

#[test]
fn one_thread_unless_a_pass_says_otherwise_and_put_back_after_it() {
    // the only test in this binary that changes the process-wide setting: the in-game runtime (and
    // any caller that is not a pregame pass) keeps the serial encode
    assert_eq!(encode_threads(), 1);
    {
        let _outer = EncodeThreads::set(6);
        assert_eq!(encode_threads(), 6);
        {
            // two passes at once (tests of the loader run them side by side): the last one to end
            // puts it back
            let _inner = EncodeThreads::set(3);
            assert_eq!(encode_threads(), 3);
        }
        assert_eq!(encode_threads(), 3, "the other pass still runs");
    }
    assert_eq!(encode_threads(), 1);
    let _zero = EncodeThreads::set(0);
    assert_eq!(encode_threads(), 1, "0 is taken as 1");
}

#[test]
fn a_split_texture_is_counted() {
    // the loader's pass test reads this count to see that its pass cut textures into strips
    let before = textures_encoded_in_strips();
    let src = rgba(64, 64, 2);
    split(PixelFormat::BC7_srgb_block, 64, 64, &src, 4).unwrap();
    assert!(textures_encoded_in_strips() > before);
}
