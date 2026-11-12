use super::*;
use anyhow::{Context, ensure};

impl AppState {
    /// Validate the actual generated index and records before publication. Old
    /// content mappings and chunk metadata must survive unchanged; only resources
    /// serialized during this session and its newly created content may differ.
    pub(crate) fn validate_generated_assets(
        &self,
        expected: &[(ResourceId, Vec<u8>)],
    ) -> anyhow::Result<()> {
        if !self.generated_asset_transaction {
            return Ok(());
        }
        if self.prepared_asset_output {
            shroudforge_game_assets::transaction::validate_generated_prepared(
                &self.asset_input_root,
                &self.asset_output_root,
                &self.asset_stem,
            )?;
        }
        let stage = shroudforge_game_assets::transaction::directory(
            &self.asset_output_root,
            &self.asset_stem,
        );
        let generated = KFCFile::from_path(stage.join(format!("{}.kfc", self.asset_stem)), false)?;
        let original = self.ref_file.as_ref();
        ensure!(
            generated.game_version() == original.game_version(),
            "generated game version changed"
        );
        ensure!(
            generated.containers().len() >= original.containers().len(),
            "existing containers removed"
        );
        for (before, after) in original.containers().iter().zip(generated.containers()) {
            ensure!(
                before.size == after.size && before.count == after.count,
                "existing container metadata changed"
            );
        }
        ensure!(
            generated.resource_chunks().len() >= original.resource_chunks().len(),
            "existing chunks removed"
        );
        for (a, b) in original
            .resource_chunks()
            .iter()
            .zip(generated.resource_chunks())
        {
            ensure!(
                (
                    a.offset,
                    a.size,
                    a.compressed_size,
                    a.uncompressed_offset,
                    a.uncompressed_size
                ) == (
                    b.offset,
                    b.size,
                    b.compressed_size,
                    b.uncompressed_offset,
                    b.uncompressed_size
                ),
                "existing resource chunk metadata changed"
            );
        }
        let changed: HashSet<_> = expected.iter().map(|(id, _)| *id).collect();
        for (id, before) in original.resources().iter() {
            let after = generated
                .resources()
                .get(id)
                .context("existing resource removed")?;
            if !changed.contains(id) {
                ensure!(
                    before.offset == after.offset && before.size == after.size,
                    "unrelated resource changed"
                );
            }
        }
        for id in generated.resources().keys() {
            ensure!(
                original.resources().contains_key(id) || changed.contains(id),
                "unexpected resource added"
            );
        }
        // Bundle indices may be reordered by serialization, but type membership
        // must agree with every generated resource key.
        let mut members: HashMap<_, HashSet<_>> = HashMap::new();
        for id in generated.resources().keys() {
            members.entry(id.type_hash()).or_default().insert(*id);
        }
        for (kind, expected_members) in members {
            let actual: HashSet<_> = generated.resources_by_type(kind).copied().collect();
            ensure!(
                actual == expected_members,
                "generated resource type bundle mismatch"
            );
        }
        for (id, before) in original.contents().iter() {
            let after = generated
                .contents()
                .get(id)
                .context("existing content mapping removed")?;
            ensure!(
                (before.offset, before.flags, before.container_index)
                    == (after.offset, after.flags, after.container_index),
                "existing content mapping changed"
            );
        }
        let contents = self.new_contents.borrow();
        for id in generated.contents().keys() {
            ensure!(
                original.contents().contains_key(id) || contents.contains_key(id),
                "unexpected content added"
            );
        }
        let mut staged_reader = KFCReader::new(&stage, &self.asset_stem)?.into_cursor()?;
        for (id, bytes) in expected {
            ensure!(
                staged_reader
                    .read_resource(id)?
                    .context("generated resource missing")?
                    == bytes.as_slice(),
                "generated resource readback differs from serialized value"
            );
        }
        for (id, bytes) in contents.iter() {
            let actual = if original.contents().contains_key(id) {
                self.reader
                    .borrow_mut()
                    .as_mut()
                    .context(GAME_FILE_HANDLES_RELEASED)?
                    .read_content(id)?
            } else {
                staged_reader.read_content(id)?
            }
            .context("generated content missing")?;
            ensure!(actual == *bytes, "generated content readback mismatch");
            ensure!(
                ContentHash::from_data(&actual) == *id,
                "generated content key mismatch"
            );
        }
        Ok(())
    }
}
