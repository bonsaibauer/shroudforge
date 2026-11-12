//! Qualified original Greeneye dynamic loading. No original scan is invoked directly.
use super::*;
use mod_loader::dll_manager::{DllLoadEntry, DllLoadMode};

const HOST_BYTES: u64 = 294912;
const HOST_HASH: &str = "973a2284ba4faf24e55c9e4b165c136c81dec4d3b1706b3f21f9cbd314c26eed";
const SCAN_INPUTS: [(&str, u64, &str); 4] = [
    (
        "greeneye/find/mod.json",
        266,
        "837c9a800d651f164cebbbf5d2404acb3dfbef548a54255ceb865ff5df264438",
    ),
    (
        "greeneye/find/find.dll",
        548352,
        "f292d6859543770425f1313df4fe3838516cf829074436e44fc12ce036920edd",
    ),
    (
        "greeneye/map-teleport/mod.json",
        280,
        "db7964ed0ee2c45e63f847bd93c65c6115e64638280c513d086fe7573734331a",
    ),
    (
        "greeneye/map-teleport/map-teleport.dll",
        466432,
        "911ecc2e80cc2f75ff22efb7dbfdf16ac6673755785d15b7fb25fc35efe2784b",
    ),
];

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TicketV1 {
    pub bytes: u32,
    pub version: u32,
    pub kind: u32,
    pub reserved: u32,
    pub binding: [u8; 32],
}
pub(super) struct Work {
    pub entry: DllLoadEntry,
    pub preloads: Vec<DllLoadEntry>,
    pub ticket: TicketV1,
    pub taken: bool,
    pub loading: bool,
    pub handle: Option<isize>,
    pub confirmed: bool,
    retained_inputs: BTreeSet<PathBuf>,
    pub _directories: Vec<fs::File>,
}

fn matches_file(path: &Path, bytes: u64, hash: &str) -> Result<(), String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() != bytes {
        return Err("Original loader input size differs".into());
    }
    let data = fs::read(path).map_err(|e| e.to_string())?;
    if data.len() as u64 != bytes || format!("{:x}", Sha256::digest(&data)) != hash {
        return Err(format!(
            "Original loader input has no qualified profile: {}",
            path.display()
        ));
    }
    Ok(())
}

fn namespace(root: &Path) -> Result<Vec<PathBuf>, String> {
    if root
        .join("dbgcore.dll")
        .try_exists()
        .map_err(|e| e.to_string())?
    {
        return Err("Root Greeneye proxy conflicts with selected dynamic loading".into());
    }
    let scan = root.join("greeneye");
    let mut directories = vec![scan.clone()];
    canonical(&scan)?;
    for item in fs::read_dir(&scan).map_err(|e| e.to_string())? {
        let item = item.map_err(|e| e.to_string())?;
        let name = item.file_name();
        canonical(&item.path())?;
        if name == "loader.log" && item.file_type().map_err(|e| e.to_string())?.is_file() {
            continue;
        }
        if (name != "find" && name != "map-teleport")
            || !item.file_type().map_err(|e| e.to_string())?.is_dir()
        {
            return Err("Unqualified Greeneye scan directory or hotreload input".into());
        }
        directories.push(item.path());
    }
    for name in ["find", "map-teleport"] {
        if !scan.join(name).is_dir() {
            return Err("Required Greeneye scan directory is missing".into());
        }
    }
    Ok(directories)
}

/// Profiles are fixed implementation evidence, never supplied by a caller's JSON.
pub(super) fn inspect(root: &Path, entries: &[DllLoadEntry]) -> Result<BTreeSet<PathBuf>, String> {
    if entries.is_empty() {
        return Ok(BTreeSet::new());
    }
    if entries.len() > 2
        || entries
            .iter()
            .any(|entry| entry.mode != DllLoadMode::Library)
        || !entries
            .last()
            .unwrap()
            .path
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("dbgcore.dll"))
    {
        return Err("Original native loader selection has no qualified startup adapter".into());
    }
    // The qualified composition loads Creative 3.2.0 before the original
    // Greeneye scan. Preserve declared order; do not guess or reorder DLLs.
    if entries.len() == 2 {
        matches_file(
            &canonical(&entries[0].path)?,
            6069760,
            "1bed93fda0427fd5f7158310efabbbcf6ff6dded04b49f87f3ca80a860f180e7",
        )?;
    }
    namespace(root)?;
    let host = canonical(&entries.last().unwrap().path)?;
    if !host.starts_with(root) {
        return Err("Original loader escapes the installation".into());
    }
    matches_file(&host, HOST_BYTES, HOST_HASH)?;
    let mut files = BTreeSet::from([host]);
    for entry in entries {
        let path = canonical(&entry.path)?;
        if !path.starts_with(root) {
            return Err("Original loader escapes the installation".into());
        }
        files.insert(path);
    }
    for (name, bytes, hash) in SCAN_INPUTS {
        let path = canonical(&root.join(name))?;
        matches_file(&path, bytes, hash)?;
        files.insert(path);
    }
    Ok(files)
}

impl Work {
    pub(super) fn retains(&self, path: &Path) -> bool {
        self.retained_inputs.contains(path)
    }
    pub(super) fn capture(
        root: &Path,
        mut entries: Vec<DllLoadEntry>,
        binding: [u8; 32],
    ) -> Result<Self, String> {
        let retained_inputs = inspect(root, &entries)?;
        let mut directories = namespace(root)?;
        for entry in &entries {
            not_loaded(&entry.path)?;
            for parent in entry.path.ancestors().skip(1) {
                if parent == root {
                    break;
                }
                if !parent.starts_with(root) {
                    return Err("Original loader ancestor escapes root".into());
                }
                directories.push(parent.to_path_buf());
            }
        }
        let entry = entries.pop().ok_or("No original loader selected")?;
        let mut held = Vec::new();
        for path in directories {
            canonical(&path)?;
            let mut options = fs::OpenOptions::new();
            options.read(true);
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                options.share_mode(3).custom_flags(0x02000000);
            }
            held.push(options.open(path).map_err(|e| e.to_string())?);
        }
        Ok(Self {
            entry,
            preloads: entries,
            ticket: TicketV1 {
                bytes: 48,
                version: 1,
                kind: 2,
                reserved: 0,
                binding,
            },
            taken: false,
            loading: false,
            handle: None,
            confirmed: false,
            retained_inputs,
            _directories: held,
        })
    }
}

#[cfg(windows)]
fn not_loaded(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    if !unsafe { windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(wide.as_ptr()) }
        .is_null()
    {
        return Err("Original host was loaded before its admitted continuation".into());
    }
    Ok(())
}
#[cfg(not(windows))]
fn not_loaded(_: &Path) -> Result<(), String> {
    Err("Original native loading requires Windows".into())
}

pub(super) fn load(root: &Path, entry: &DllLoadEntry) -> (Option<isize>, Result<(), String>) {
    if let Err(error) = crate::native_dlls::pin_native_runtime_module() {
        return (None, Err(error));
    }
    if let Err(error) = not_loaded(&entry.path).and_then(|_| namespace(root).map(|_| ())) {
        return (None, Err(error));
    }
    let (handle, result) = crate::native_dlls::load_library(root, entry);
    let result = result.and_then(|_| {
        let handle = handle.ok_or("Original loader returned no module")?;
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStringExt;
            use windows_sys::Win32::System::LibraryLoader::{
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_PIN,
                GetModuleFileNameW, GetModuleHandleExW,
            };
            let mut name = vec![0u16; 32768];
            let length =
                unsafe { GetModuleFileNameW(handle as _, name.as_mut_ptr(), name.len() as u32) }
                    as usize;
            if length == 0 || length >= name.len() {
                return Err("Original module path is unavailable".into());
            }
            let path = PathBuf::from(std::ffi::OsString::from_wide(&name[..length]));
            if canonical(&path)? != canonical(&entry.path)? {
                return Err("Original loader returned a different module".into());
            }
            let mut pinned = std::ptr::null_mut();
            if unsafe {
                GetModuleHandleExW(
                    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
                    handle as *const u16,
                    &mut pinned,
                )
            } == 0
                || pinned as isize != handle
            {
                return Err("Could not retain original module residency".into());
            }
        }
        #[cfg(not(windows))]
        let _ = handle;
        Ok(())
    });
    // A failed witness can already have published callbacks. Never FreeLibrary.
    (handle, result)
}
