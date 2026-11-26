//! A whole startup pass that encodes a block-compressed texture: the pass cuts it into strips on
//! several threads (mod-runtime/lua/bindings/image/encode_in_strips.rs), and the published blob is
//! byte for byte the serial encode.

use super::test_game::Game;
use super::*;
use kfc::container::KFCReader;
use kfc::guid::ContentHash;

fn published(game: &Game, hash: &ContentHash) -> Option<Vec<u8>> {
    KFCReader::new(&game.0, "enshrouded")
        .unwrap()
        .into_cursor()
        .unwrap()
        .read_content(hash)
        .unwrap()
}

/// The pixels the Lua below sets, as RGBA8.
fn pixels(width: usize, height: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            out.extend_from_slice(&[
                ((x * 37 + y * 11) % 256) as u8,
                ((x * x + 3 * y) % 256) as u8,
                if (x / 3 + y / 5) % 2 == 0 { 40 } else { 220 },
                if x % 7 == 0 {
                    0
                } else {
                    (255 - (x + y) % 64) as u8
                },
            ]);
        }
    }
    out
}

#[test]
fn a_texture_the_pass_encodes_on_several_threads_is_the_serial_encode() {
    // 768 BC7 blocks in 24 block rows: several strips of at least 64 blocks
    let (width, height) = (128usize, 96usize);
    let game = Game::new("encode-pass-texture");
    game.package(
        "painter",
        &format!(
            r#"
local img = image.create({width}, {height})
for y = 0, {height} - 1 do
  for x = 0, {width} - 1 do
    local a = 255 - (x + y) % 64
    if x % 7 == 0 then a = 0 end
    local b = 220
    if (x // 3 + y // 5) % 2 == 0 then b = 40 end
    img:set_pixel(x, y, (x * 37 + y * 11) % 256, (x * x + 3 * y) % 256, b, a)
  end
end
game.assets.create_content(image.encode_texture(img, "BC7_srgb_block"))
return {{}}
"#
        ),
        &[],
    );
    let before = shroudforge_api::textures_encoded_in_strips();
    run_startup(&game.0).unwrap();
    let record = game.record();
    assert_eq!(record["status"], "applied", "{record}\n{}", game.log());
    let format = kfc::content::image::PixelFormat::BC7_srgb_block;
    let mut serial = vec![0u8; kfc::content::image::size_of_format(format, width, height)];
    kfc::content::image::encode(format, width, height, &pixels(width, height), &mut serial)
        .unwrap();
    assert_eq!(
        published(&game, &ContentHash::from_data(&serial)).as_deref(),
        Some(serial.as_slice()),
        "the pass's texture is the serial encode, byte for byte"
    );
    // a PC with more than one thread: the pregame pass cut it into strips (other tests' passes
    // only add to the count)
    if std::thread::available_parallelism().map_or(1, |n| n.get()) > 1
        && std::env::var(shroudforge_api::ENCODE_THREADS_VARIABLE).is_err()
    {
        assert!(
            shroudforge_api::textures_encoded_in_strips() > before,
            "the pass encoded on one thread"
        );
    }
}
