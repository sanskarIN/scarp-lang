//! Experimental local application manifests, confined entrypoints, and scaffolding.

use crate::project_manifest::valid_name;
pub use crate::project_manifest::{Manifest, ManifestError, MAX_MANIFEST_BYTES};
use std::{
    fmt, fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

pub const MANIFEST_FILE: &str = "scarp.toml";
pub const APPLICATION_SOURCE: &str = "// Scarp application\nprintln(\"Hello from Scarp!\");\n";

#[derive(Debug)]
pub enum ProjectError {
    Io {
        path: PathBuf,
        operation: &'static str,
        error: io::Error,
    },
    Manifest {
        path: PathBuf,
        error: ManifestError,
    },
    Conflict(PathBuf),
    Policy {
        path: PathBuf,
        message: &'static str,
    },
    InvalidName,
}
impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {path, operation, error} => write!(f, "could not {operation} '{}': {error}", path.display()),
            Self::Manifest {path, error} => write!(f, "{}:{error}", path.display()),
            Self::Conflict(path) => write!(f, "'{}' already exists; no files were overwritten", path.display()),
            Self::Policy {path, message} => write!(f, "{}: {message}", path.display()),
            Self::InvalidName => f.write_str("invalid project name; use --name with 1–64 ASCII letters/digits/hyphens/underscores starting with a letter, excluding device names"),
        }
    }
}
impl std::error::Error for ProjectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { error, .. } => Some(error),
            Self::Manifest { error, .. } => Some(error),
            _ => None,
        }
    }
}
fn io_error(path: &Path, operation: &'static str, error: io::Error) -> ProjectError {
    ProjectError::Io {
        path: path.to_owned(),
        operation,
        error,
    }
}
fn policy(path: &Path, message: &'static str) -> ProjectError {
    ProjectError::Policy {
        path: path.to_owned(),
        message,
    }
}
fn linked(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        return metadata.file_attributes() & 0x400 != 0;
    }
    #[cfg(not(windows))]
    false
}
fn absent(path: &Path) -> Result<(), ProjectError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(ProjectError::Conflict(path.to_owned())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io_error(path, "inspect path", e)),
    }
}
fn directory(path: &Path) -> Result<(), ProjectError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|e| io_error(path, "inspect directory", e))?;
    if linked(&metadata) || !metadata.is_dir() {
        return Err(policy(
            path,
            "expected an ordinary directory, not a link or file",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Project {
    root: PathBuf,
    manifest: Manifest,
    entry: PathBuf,
}
impl Project {
    /// Load only this directory's manifest; never search ancestors or execute tasks.
    pub fn load(directory: &Path) -> Result<Self, ProjectError> {
        let root = fs::canonicalize(directory)
            .map_err(|e| io_error(directory, "resolve project directory", e))?;
        let path = root.join(MANIFEST_FILE);
        let metadata = fs::symlink_metadata(&path).map_err(|e| {
            io_error(
                &path,
                "read project manifest (run from the project root)",
                e,
            )
        })?;
        if linked(&metadata) || !metadata.is_file() {
            return Err(policy(&path, "manifest must be an ordinary file"));
        }
        let mut bytes = Vec::new();
        fs::File::open(&path)
            .and_then(|file| {
                file.take(MAX_MANIFEST_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)
            })
            .map_err(|e| io_error(&path, "read project manifest", e))?;
        let source = std::str::from_utf8(&bytes)
            .map_err(|_| policy(&path, "manifest must contain UTF-8"))?;
        let manifest = Manifest::parse(source).map_err(|error| ProjectError::Manifest {
            path: path.clone(),
            error,
        })?;
        let selected = root.join(manifest.entry());
        let entry = fs::canonicalize(&selected)
            .map_err(|e| io_error(&selected, "resolve project entrypoint", e))?;
        if !entry.starts_with(&root) {
            return Err(policy(
                &selected,
                "resolved entrypoint escapes the project root",
            ));
        }
        if !fs::metadata(&entry)
            .map_err(|e| io_error(&entry, "inspect entrypoint", e))?
            .is_file()
        {
            return Err(policy(&entry, "entrypoint must be a regular file"));
        }
        Ok(Self {
            root,
            manifest,
            entry,
        })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn name(&self) -> &str {
        self.manifest.name()
    }
    pub fn entry(&self) -> &Path {
        &self.entry
    }
}

#[derive(Debug, Clone)]
pub struct ScaffoldPlan {
    root: PathBuf,
    name: String,
    create_root: bool,
}
impl ScaffoldPlan {
    /// Plan a new directory without changing the filesystem.
    pub fn create(target: &Path, name: Option<&str>) -> Result<Self, ProjectError> {
        let absolute = std::env::current_dir()
            .map_err(|e| io_error(target, "read working directory", e))?
            .join(target);
        let leaf = absolute
            .file_name()
            .ok_or_else(|| policy(target, "choose a new child directory"))?;
        let parent = absolute
            .parent()
            .ok_or_else(|| policy(target, "missing parent directory"))?;
        let parent = fs::canonicalize(parent)
            .map_err(|e| io_error(parent, "resolve parent directory", e))?;
        let root = parent.join(leaf);
        absent(&root)?;
        let name = selected_name(&root, name)?;
        Ok(Self {
            root,
            name,
            create_root: true,
        })
    }
    /// Plan initialization of an existing directory without changing its files.
    pub fn init(target: &Path, name: Option<&str>) -> Result<Self, ProjectError> {
        let root = fs::canonicalize(target)
            .map_err(|e| io_error(target, "resolve project directory", e))?;
        directory(&root)?;
        if root.parent().is_none() {
            return Err(policy(
                &root,
                "initializing a filesystem root is not supported",
            ));
        }
        let name = selected_name(&root, name)?;
        let plan = Self {
            root,
            name,
            create_root: false,
        };
        plan.check_contents()?;
        Ok(plan)
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn name(&self) -> &str {
        &self.name
    }

    fn check_contents(&self) -> Result<(), ProjectError> {
        directory(&self.root)?;
        absent(&self.root.join(MANIFEST_FILE))?;
        let source = self.root.join("src");
        match fs::symlink_metadata(&source) {
            Ok(_) => {
                directory(&source)?;
                absent(&source.join("main.srp"))?;
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(io_error(&source, "inspect source directory", e)),
        }
        Ok(())
    }

    /// Recheck the plan, create files exclusively, and activate the manifest last.
    pub fn apply(&self) -> Result<(), ProjectError> {
        let mut created = Created::default();
        if self.create_root {
            fs::create_dir(&self.root).map_err(|e| {
                if e.kind() == io::ErrorKind::AlreadyExists {
                    ProjectError::Conflict(self.root.clone())
                } else {
                    io_error(&self.root, "create project directory", e)
                }
            })?;
            created.directories.push(self.root.clone());
        }
        self.check_contents()?;
        let source = self.root.join("src");
        match fs::create_dir(&source) {
            Ok(()) => created.directories.push(source.clone()),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => directory(&source)?,
            Err(e) => return Err(io_error(&source, "create source directory", e)),
        }
        created.write(&source.join("main.srp"), APPLICATION_SOURCE.as_bytes())?;
        let manifest = format!(
            "format = 1\n\n[project]\nname = \"{}\"\nentry = \"src/main.srp\"\n",
            self.name
        );
        created.write(&self.root.join(MANIFEST_FILE), manifest.as_bytes())?;
        created.committed = true;
        Ok(())
    }
}
fn selected_name(root: &Path, name: Option<&str>) -> Result<String, ProjectError> {
    let name = name
        .or_else(|| root.file_name().and_then(|name| name.to_str()))
        .ok_or(ProjectError::InvalidName)?;
    if !valid_name(name) {
        return Err(ProjectError::InvalidName);
    }
    Ok(name.to_owned())
}

#[derive(Default)]
struct Created {
    files: Vec<(PathBuf, Vec<u8>)>,
    directories: Vec<PathBuf>,
    committed: bool,
}
impl Created {
    fn write(&mut self, path: &Path, bytes: &[u8]) -> Result<(), ProjectError> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| {
                if e.kind() == io::ErrorKind::AlreadyExists {
                    ProjectError::Conflict(path.to_owned())
                } else {
                    io_error(path, "create project file", e)
                }
            })?;
        self.files.push((path.to_owned(), bytes.to_vec()));
        file.write_all(bytes)
            .and_then(|_| file.flush())
            .map_err(|e| io_error(path, "write project file", e))
    }
}
impl Drop for Created {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        for (path, expected) in self.files.iter().rev() {
            if let Ok(metadata) = fs::symlink_metadata(path) {
                if !linked(&metadata)
                    && metadata.is_file()
                    && metadata.len() <= expected.len() as u64
                {
                    if let Ok(actual) = fs::read(path) {
                        if expected.starts_with(&actual) {
                            let _ = fs::remove_file(path);
                        }
                    }
                }
            }
        }
        for path in self.directories.iter().rev() {
            if fs::symlink_metadata(path)
                .map(|m| !linked(&m) && m.is_dir())
                .unwrap_or(false)
            {
                let _ = fs::remove_dir(path);
            }
        }
    }
}
