//! A whole startup pass where one package reads a blob (content) that an earlier package of the
//! same pass created. `game.assets.get_content` answers it, as EML's `has_content` does: the
//! pass's new contents first, then the game's header. EMF Beastmaster's beasts pack writes the
//! English text table back as a new blob and the wilds pack's harness script reads it; with only
//! the game's header asked, it got nil and the pass went on without the Llama Harness, Camel
//! Saddle and Mule Harness.

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

fn ids(list: &serde_json::Value) -> Vec<&str> {
    list.as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item["id"].as_str())
                .collect()
        })
        .unwrap_or_default()
}

const TEXT: &str = "keenglish: Llama Harness\tCamel Saddle\tMule Harness";

#[test]
fn a_blob_one_package_creates_is_there_for_the_next_package_of_the_pass() {
    let game = Game::new("same-pass-blob");
    let text = ContentHash::from_data(TEXT.as_bytes());
    let derived = format!("seen: {TEXT}");
    let unknown = ContentHash::from_data(b"no package made this");
    // the beasts pack: writes the text table back as a new blob
    game.package(
        "beasts",
        &format!("game.assets.create_content(buffer.wrap({TEXT:?}))\nreturn {{}}"),
        &[],
    );
    // the wilds pack: reads that blob by its hash (in the real pack, from the resource the beasts
    // pack changed)
    game.package(
        "wilds",
        &format!(
            r#"
local content = game.assets.get_content("{text}")
if content == nil then error("the blob the beasts pack made is not there") end
if content.size ~= {size} then error("size " .. tostring(content.size)) end
local data = content:read_data():to_string()
if data ~= {TEXT:?} then error("other bytes: " .. data) end
if game.assets.get_content("{unknown}") ~= nil then error("a blob nobody made") end
game.assets.create_content(buffer.wrap("seen: " .. data))
return {{}}
"#,
            size = TEXT.len()
        ),
        &["beasts"],
    );
    run_startup(&game.0).unwrap();
    let record = game.record();
    assert_eq!(record["status"], "applied", "{record}\n{}", game.log());
    assert_eq!(
        ids(&record["mods"]),
        ["beasts", "wilds"],
        "{record}\n{}",
        game.log()
    );
    assert_eq!(published(&game, &text).as_deref(), Some(TEXT.as_bytes()));
    assert_eq!(
        published(&game, &ContentHash::from_data(derived.as_bytes())).as_deref(),
        Some(derived.as_bytes()),
        "the reader's own blob, made from what it read"
    );
    assert_eq!(published(&game, &unknown), None);
}
