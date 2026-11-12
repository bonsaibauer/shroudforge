use crate::{
    logging::{debug, info},
    session::{AppFeatures, AppState},
};
pub(super) fn commit_asset_changes(lua: &mlua::Lua, app_state: &AppState) -> anyhow::Result<()> {
    if !app_state.has_feature(AppFeatures::PATCH) {
        return Ok(());
    }
    debug!("Committing typed asset changes...");

    let mut buf = Vec::new();
    let mut writer = app_state.take_writer()?;
    let resources = app_state.get_cached_resources();
    let mut applied_resources = 0;
    let mut expected_resources = Vec::new();

    for resource in resources.iter() {
        let value = resource.apply(lua)?;

        if let Some(value) = value {
            let r#type = kfc::kfc_format::type_for_resource(
                app_state.type_registry(),
                &resource.resource_id,
            )
            .expect("resource type must exist in current game registry");

            buf.clear();
            value
                .write_into(app_state.type_registry(), r#type, &mut buf)
                .unwrap();
            writer.write_resource(&resource.resource_id, &buf)?;
            expected_resources.push((resource.resource_id, buf.clone()));
            applied_resources += 1;
        }
    }

    writer.finalize()?;
    app_state.validate_generated_assets(&expected_resources)?;
    app_state.commit_assets()?;

    info!("Committed {} changed resources", applied_resources);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{env::Buffer, session::asset_tests::Fixture};
    use kfc::{container::KFCReader, guid::ContentHash};

    #[test]
    fn lua_generated_assets_roundtrip_in_prepared_and_published_modes() {
        for prepared in [true, false] {
            let fixture = Fixture::generated(prepared);
            let input = fixture.root.join("input");
            let original_index = std::fs::read(input.join("fixture.kfc")).unwrap();
            let original_dat = std::fs::read(input.join("fixture_000.dat")).unwrap();
            let payload = b"generated texture content";
            fixture
                .lua
                .globals()
                .set("payload", Buffer::wrap(payload.to_vec()))
                .unwrap();
            let resource: String = fixture
                .lua
                .load(
                    r#"
                local resource = game.assets.create_resource({x=37}, 'test::Base')
                game.assets.create_content(payload)
                return resource.guid
            "#,
                )
                .eval()
                .unwrap();
            let state = fixture.lua.app_data_ref::<AppState>().unwrap();
            let id = state.get_cached_resources()[0].resource_id;
            assert_eq!(id.guid().to_string(), resource);
            commit_asset_changes(&fixture.lua, &state).unwrap();
            drop(state);
            let root = if prepared {
                assert_eq!(
                    std::fs::read(input.join("fixture.kfc")).unwrap(),
                    original_index
                );
                let output = fixture.root.join("output");
                assert!(
                    shroudforge_game_assets::transaction::commit_generated(&output, "fixture")
                        .is_err()
                );
                shroudforge_game_assets::transaction::directory(&output, "fixture")
            } else {
                input.clone()
            };
            let mut reader = KFCReader::new(root, "fixture")
                .unwrap()
                .into_cursor()
                .unwrap();
            assert_eq!(
                reader
                    .read_content(&ContentHash::from_data(payload))
                    .unwrap()
                    .unwrap(),
                payload
            );
            assert_eq!(
                reader.read_resource(&id).unwrap().unwrap(),
                37u32.to_le_bytes()
            );
            assert_eq!(
                std::fs::read(input.join("fixture_000.dat")).unwrap(),
                original_dat
            );
            assert!(!input.join("fixture.kfc.bak").exists());
        }
    }
}
