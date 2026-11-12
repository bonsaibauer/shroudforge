use std::rc::Rc;

use kfc::{
    container::{KFCCursor, KFCFile, KFCReader, KFCReaderOptions, KFCWriteOptions, KFCWriter},
    reflection::TypeRegistry,
};

use crate::{logging::error, type_aliases::Path};

pub fn load_kfc_file(kfc_path: impl AsRef<Path>) -> Result<Rc<KFCFile>, ()> {
    let kfc_path = kfc_path.as_ref();

    match KFCFile::from_path(kfc_path, false) {
        Ok(file) => Ok(Rc::new(file)),
        Err(e) => {
            error!(
                error = %e,
                path = ?kfc_path,
                "Failed to open KFC file",
            );
            Err(())
        }
    }
}

pub fn create_reader(dir: &Path, file_name: &str) -> Result<KFCCursor<KFCReader>, ()> {
    create_reader_with_extension(dir, file_name, "kfc.bak")
}

pub fn create_reader_with_extension(
    dir: &Path,
    file_name: &str,
    extension: &str,
) -> Result<KFCCursor<KFCReader>, ()> {
    match KFCReader::new_with_options(
        dir,
        file_name,
        KFCReaderOptions {
            kfc_extension: extension,
            ..Default::default()
        },
    )
    .and_then(|reader| reader.into_cursor())
    {
        Ok(reader) => Ok(reader),
        Err(e) => {
            error!(
                path = ?dir,
                file_name = %file_name,
                error = %e,
                "Failed to create KFC reader",
            );
            Err(())
        }
    }
}

pub fn create_writer_with_policy(
    dir: &Path,
    file_name: &str,
    type_registry: &Rc<TypeRegistry>,
    ref_file: &Rc<KFCFile>,
    preserve_current: bool,
) -> Result<KFCWriter<Rc<KFCFile>, Rc<TypeRegistry>>, ()> {
    let writer = match KFCWriter::new_incremental_with_options(
        dir,
        file_name,
        ref_file.clone(),
        type_registry.clone(),
        KFCWriteOptions {
            overwrite_containers: !preserve_current,
            append_after_resource_file: preserve_current,
            ..Default::default()
        },
    ) {
        Ok(writer) => writer,
        Err(e) => {
            error!(
                path = ?dir,
                file_name = %file_name,
                error = %e,
                "Failed to create KFC writer",
            );
            return Err(());
        }
    };

    Ok(writer)
}
