use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufReader, Error, ErrorKind, Read, Result},
};

use walkdir::WalkDir;
use zip::ZipArchive;

use crate::{
    alias::{Path, PathBuf},
    log::debug,
};

pub trait FileRead: Read {}

impl<R: Read> FileRead for R {}

#[derive(Debug)]
pub enum FileSystem {
    Disk(DiskFileSystem),
    Zip(ZipFileSystem),
    Empty,
}

impl FileSystem {
    pub fn new_disk<P: AsRef<Path>>(root: P) -> Result<Self> {
        DiskFileSystem::new(root.as_ref()).map(Self::Disk)
    }

    pub fn new_zip<P: AsRef<Path>>(path: P) -> Result<Self> {
        ZipFileSystem::new(path.as_ref()).map(Self::Zip)
    }

    pub fn root(&self) -> &Path {
        match self {
            Self::Disk(fs) => fs.root(),
            Self::Zip(fs) => fs.root(),
            Self::Empty => Path::new(""),
        }
    }

    pub fn files(&self) -> Vec<Result<PathBuf>> {
        let mut files = match self {
            Self::Disk(fs) => fs.files(),
            Self::Zip(fs) => fs.files(),
            Self::Empty => Vec::new(),
        };
        if let Self::Disk(fs) = self {
            for path in fs.overrides.keys() {
                if !files
                    .iter()
                    .any(|entry| entry.as_ref().is_ok_and(|item| item == path))
                {
                    files.push(Ok(path.clone()));
                }
            }
        }

        debug!(
            event = "files",
            result = ?files,
            root = self.root().as_str(),
        );

        files
    }

    pub fn read_directory<P: AsRef<Path>>(&self, path: P) -> Result<Vec<Result<PathBuf>>> {
        let mut files = match self {
            Self::Disk(fs) => fs.read_directory(path.as_ref()),
            Self::Zip(fs) => fs.read_directory(path.as_ref()),
            Self::Empty => Err(Error::new(ErrorKind::NotFound, "No file system available")),
        };
        if let (Self::Disk(fs), Ok(entries)) = (self, &mut files) {
            let mut names: HashSet<_> = entries
                .iter()
                .filter_map(|entry| entry.as_ref().ok().cloned())
                .collect();
            for override_path in fs.overrides.keys() {
                if override_path.parent() == Some(path.as_ref())
                    && names.insert(override_path.clone())
                {
                    entries.push(Ok(override_path.clone()));
                }
            }
        }

        debug!(
            event = "read_directory",
            result = ?files,
            path = path.as_ref().as_str(),
            root = self.root().as_str(),
        );

        files
    }

    pub fn read_file<P: AsRef<Path>>(&mut self, path: P) -> Result<Box<dyn FileRead + '_>> {
        let root = if tracing::enabled!(tracing::Level::DEBUG) {
            self.root().to_owned()
        } else {
            PathBuf::new()
        };

        let file = self.open_package_file(path.as_ref());

        debug!(
            event = "read_file",
            result = file.is_ok(),
            path = path.as_ref().as_str(),
            root = root.as_str(),
        );

        file
    }

    // Revision polling observes package contents without turning each unchanged
    // observation into activity. Keep the same filesystem and path checks used
    // by the public, logged read operation.
    pub(crate) fn open_package_file<P: AsRef<Path>>(
        &mut self,
        path: P,
    ) -> Result<Box<dyn FileRead + '_>> {
        match self {
            Self::Disk(fs) => {
                if let Some(bytes) = fs.overrides.get(path.as_ref()) {
                    return Ok(Box::new(std::io::Cursor::new(bytes.clone())));
                }
                fs.read_file(path.as_ref())
            }
            Self::Zip(fs) => fs.read_file(path.as_ref()),
            Self::Empty => Err(Error::new(ErrorKind::NotFound, "No file system available")),
        }
    }

    pub fn exists<P: AsRef<Path>>(&self, path: P) -> bool {
        let result = match self {
            Self::Disk(fs) => {
                fs.exists(path.as_ref())
                    || fs.overrides.contains_key(path.as_ref())
                    || fs
                        .overrides
                        .keys()
                        .any(|item| item.starts_with(path.as_ref()))
            }
            Self::Zip(fs) => fs.exists(path.as_ref()),
            Self::Empty => false,
        };

        debug!(
            event = "exists",
            result,
            path = path.as_ref().as_str(),
            root = self.root().as_str(),
        );

        result
    }

    pub fn is_file<P: AsRef<Path>>(&mut self, path: P) -> bool {
        let result = match self {
            Self::Disk(fs) => fs.overrides.contains_key(path.as_ref()) || fs.is_file(path.as_ref()),
            Self::Zip(fs) => fs.is_file(path.as_ref()),
            Self::Empty => false,
        };

        debug!(
            event = "is_file",
            result,
            exists = self.exists(path.as_ref()),
            path = path.as_ref().as_str(),
            root = self.root().as_str(),
        );

        result
    }

    pub fn is_directory<P: AsRef<Path>>(&mut self, path: P) -> bool {
        let result = match self {
            Self::Disk(fs) => {
                fs.overrides
                    .keys()
                    .any(|item| item.starts_with(path.as_ref()))
                    || fs.is_directory(path.as_ref())
            }
            Self::Zip(fs) => fs.is_directory(path.as_ref()),
            Self::Empty => false,
        };

        debug!(
            event = "is_directory",
            result,
            exists = self.exists(path.as_ref()),
            path = path.as_ref().as_str(),
            root = self.root().as_str(),
        );

        result
    }

    pub fn absolute_path<P: AsRef<Path>>(&self, path: P) -> Result<Option<PathBuf>> {
        let abs_path = match self {
            Self::Disk(fs) => Some(sanitize_path(fs.root(), path.as_ref())?),
            Self::Zip(_) => None,
            Self::Empty => None,
        };

        debug!(
            event = "absolute_path",
            result = ?abs_path,
            path = path.as_ref().as_str(),
            root = self.root().as_str(),
        );

        Ok(abs_path)
    }

    pub(crate) fn apply_profile_files(&mut self, files: &HashMap<String, String>) {
        if let Self::Disk(fs) = self {
            for (path, contents) in files {
                fs.overrides
                    .insert(PathBuf::from(path.as_str()), contents.as_bytes().to_vec());
            }
        }
    }
}

#[derive(Debug)]
pub struct DiskFileSystem {
    root: PathBuf,
    overrides: HashMap<PathBuf, Vec<u8>>,
}

impl DiskFileSystem {
    fn new(root: &Path) -> Result<Self> {
        Ok(Self {
            root: root.canonicalize_utf8()?,
            overrides: HashMap::new(),
        })
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn files(&self) -> Vec<Result<PathBuf>> {
        WalkDir::new(self.root.as_std_path())
            .follow_links(false)
            .follow_root_links(false)
            .into_iter()
            .map(|entry| match entry {
                Ok(e) => PathBuf::try_from(e.into_path()).map_err(Error::other),
                Err(e) => Err(e
                    .into_io_error()
                    .expect("Failed to convert error to IO error")),
            })
            .collect::<Vec<_>>()
    }

    fn read_directory(&self, path: &Path) -> Result<Vec<Result<PathBuf>>> {
        let path = sanitize_path(&self.root, path)?;

        if !path.is_dir() {
            return Err(Error::from(ErrorKind::NotADirectory));
        }

        let mut results = Vec::new();

        for entry in path.read_dir()? {
            match entry {
                Ok(e) => {
                    let p = match PathBuf::try_from(e.path()) {
                        Ok(p) => p,
                        Err(e) => {
                            results.push(Err(Error::other(e)));
                            continue;
                        }
                    };

                    let p = match p.strip_prefix(&self.root) {
                        Ok(p) => p.to_path_buf(),
                        Err(e) => {
                            results.push(Err(Error::other(e)));
                            continue;
                        }
                    };

                    results.push(Ok(p));
                }
                Err(e) => {
                    results.push(Err(e));
                }
            }
        }

        Ok(results)
    }

    fn read_file(&mut self, path: &Path) -> Result<Box<dyn FileRead>> {
        let path = sanitize_path(&self.root, path)?;

        if !path.is_file() {
            return Err(Error::from(ErrorKind::NotFound));
        }

        Ok(Box::new(File::open(path)?))
    }

    fn exists(&self, path: &Path) -> bool {
        sanitize_path(&self.root, path)
            .map(|p| p.exists())
            .unwrap_or(false)
    }

    fn is_file(&self, path: &Path) -> bool {
        sanitize_path(&self.root, path)
            .map(|p| p.is_file())
            .unwrap_or(false)
    }

    fn is_directory(&self, path: &Path) -> bool {
        sanitize_path(&self.root, path)
            .map(|p| p.is_dir())
            .unwrap_or(false)
    }
}

#[derive(Debug)]
pub struct ZipFileSystem {
    root: PathBuf,
    archive: ZipArchive<BufReader<File>>,
    node: TreeNode,
}

impl ZipFileSystem {
    fn new(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let mut archive = ZipArchive::new(BufReader::new(file))?;

        let mut node = TreeNode::new();

        for i in 0..archive.len() {
            let file = archive.by_index(i)?;

            let path = file.name().map_err(Error::other)?;
            let is_file = file.is_file();

            let components: Vec<&str> = Path::new(path.as_ref())
                .components()
                .map(|c| c.as_str())
                .filter(|s| !s.is_empty())
                .collect();

            node.insert_path(&components, Some(i), is_file)?;
        }

        Ok(Self {
            root: path.into(),
            archive,
            node,
        })
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn files(&self) -> Vec<Result<PathBuf>> {
        self.archive
            .file_names()
            .map(|name| {
                name.map(|name| PathBuf::from(name.into_owned()))
                    .map_err(Error::other)
            })
            .collect()
    }

    fn read_directory(&self, path: &Path) -> Result<Vec<Result<PathBuf>>> {
        let node = self
            .node
            .get(path)
            .ok_or_else(|| Error::from(ErrorKind::NotFound))?;

        Ok(node
            .iter()
            .map(|name| path.join(name))
            .map(|name| Ok(PathBuf::from(name)))
            .collect::<Vec<_>>())
    }

    fn read_file(&mut self, path: &Path) -> Result<Box<dyn FileRead + '_>> {
        let node = self
            .node
            .get(path)
            .ok_or_else(|| Error::from(ErrorKind::NotFound))?;
        let index = node.index.ok_or_else(|| Error::from(ErrorKind::NotFound))?;
        let file = self.archive.by_index(index).map_err(Error::other)?;

        Ok(Box::new(file))
    }

    fn exists(&self, path: &Path) -> bool {
        self.node.get(path).is_some()
    }

    fn is_file(&mut self, path: &Path) -> bool {
        self.node.get(path).map(|n| n.is_file).unwrap_or(false)
    }

    fn is_directory(&mut self, path: &Path) -> bool {
        self.node.get(path).map(|n| !n.is_file).unwrap_or(false)
    }
}

#[derive(Debug)]
struct TreeNode {
    children: HashMap<String, TreeNode>,
    index: Option<usize>,
    is_file: bool,
}

impl TreeNode {
    fn new() -> Self {
        Self {
            children: HashMap::new(),
            index: None,
            is_file: false,
        }
    }

    fn insert_path(
        &mut self,
        components: &[&str],
        index: Option<usize>,
        is_file: bool,
    ) -> Result<()> {
        if components.is_empty() {
            return Ok(());
        }

        let component = components[0];

        if let Some(existing) = self.children.get_mut(component) {
            if components.len() == 1 {
                if existing.is_file != is_file {
                    return Err(Error::other(format!(
                        "Path conflict between file and directory: {}",
                        components.join("/"),
                    )));
                }

                existing.index = index;
                existing.is_file = is_file;
            } else {
                existing.insert_path(&components[1..], index, is_file)?;
            }
        } else {
            let mut node = Self::new();

            if components.len() == 1 {
                node.index = index;
                node.is_file = is_file;
            } else {
                node.insert_path(&components[1..], index, is_file)?;
            }

            self.children.insert(component.to_string(), node);
        }

        Ok(())
    }

    fn get(&self, path: impl AsRef<Path>) -> Option<&Self> {
        let path = path.as_ref();
        let components: Vec<&str> = path
            .components()
            .map(|c| c.as_str())
            .filter(|s| !s.is_empty())
            .collect();

        self.get_internal(&components)
    }

    fn get_internal(&self, components: &[&str]) -> Option<&Self> {
        if components.is_empty() {
            return Some(self);
        }

        let component = components[0];

        if let Some(child) = self.children.get(component) {
            child.get_internal(&components[1..])
        } else {
            None
        }
    }

    fn iter(&self) -> impl Iterator<Item = &String> {
        self.children.keys()
    }
}

fn sanitize_path(root: &Path, path: &Path) -> Result<PathBuf> {
    assert!(root.is_absolute(), "Root path must be absolute");

    let path = root.join(path).canonicalize_utf8()?;

    if !path.starts_with(root) {
        return Err(Error::from(ErrorKind::PermissionDenied));
    }

    if path.is_symlink() {
        return Err(Error::from(ErrorKind::PermissionDenied));
    }

    Ok(path)
}
