//! Source identity and bounded reads; no lexer/parser filesystem access.
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceLoadKind {
    Io,
    InvalidUtf8,
    InvalidPath,
    Limit,
}

#[derive(Debug, Clone)]
pub struct SourceLoadError {
    pub kind: SourceLoadKind,
    pub message: String,
}
impl SourceLoadError {
    pub fn new(kind: SourceLoadKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for SourceLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for SourceLoadError {}

/// Implementations provide stable identities and honor the requested byte cap.
/// The compiler also checks returned source lengths. No source is executed here.
pub trait SourceLoader {
    fn resolve(&self, importer: Option<&str>, specifier: &str) -> Result<String, SourceLoadError>;
    fn load(&self, identity: &str, max_bytes: usize) -> Result<String, SourceLoadError>;
    fn display_name(&self, identity: &str) -> String {
        identity.to_string()
    }
}

fn path_error(message: impl Into<String>) -> SourceLoadError {
    SourceLoadError::new(SourceLoadKind::InvalidPath, message)
}

pub(super) fn validate_path(path: &str, import: bool) -> Result<(), SourceLoadError> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains(['\\', ':'])
        || path.chars().any(char::is_control)
        || path.split('/').any(str::is_empty)
        || (import && !path.ends_with(".srp"))
    {
        return Err(path_error(
            "use a relative .srp path with forward slashes and no drive, URI or control characters",
        ));
    }
    Ok(())
}

pub(super) fn relative_path(
    base: &str,
    path: &str,
    import: bool,
) -> Result<String, SourceLoadError> {
    validate_path(path, import)?;
    let mut parts: Vec<&str> = base.split('/').filter(|part| !part.is_empty()).collect();
    for part in path.split('/') {
        match part {
            "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err(path_error("module path escapes the trusted source root"));
                }
            }
            other => parts.push(other),
        }
    }
    if parts.is_empty() {
        return Err(path_error("module path must identify a source file"));
    }
    Ok(parts.join("/"))
}

/// Portable virtual paths, relative to an in-memory trusted root.
#[derive(Debug, Clone, Default)]
pub struct MemorySourceLoader {
    files: BTreeMap<String, String>,
}
impl MemorySourceLoader {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn insert(
        &mut self,
        name: impl AsRef<str>,
        source: impl Into<String>,
    ) -> Result<(), SourceLoadError> {
        let name = relative_path("", name.as_ref(), false)?;
        if self.files.contains_key(&name) {
            return Err(path_error(format!("source {name:?} is already registered")));
        }
        self.files.insert(name, source.into());
        Ok(())
    }
}
impl SourceLoader for MemorySourceLoader {
    fn resolve(&self, importer: Option<&str>, specifier: &str) -> Result<String, SourceLoadError> {
        let base = importer
            .and_then(|name| name.rsplit_once('/').map(|p| p.0))
            .unwrap_or("");
        let name = relative_path(base, specifier, importer.is_some())?;
        if !self.files.contains_key(&name) {
            return Err(SourceLoadError::new(
                SourceLoadKind::Io,
                format!("could not read {name:?}: source is not registered"),
            ));
        }
        Ok(name)
    }
    fn load(&self, identity: &str, max_bytes: usize) -> Result<String, SourceLoadError> {
        let source = self.files.get(identity).ok_or_else(|| {
            SourceLoadError::new(SourceLoadKind::Io, format!("could not read {identity:?}"))
        })?;
        if source.len() > max_bytes {
            return Err(SourceLoadError::new(
                SourceLoadKind::Limit,
                "module source byte limit exceeded",
            ));
        }
        Ok(source.clone())
    }
}

/// Canonical local source files restricted to an explicitly supplied directory.
#[derive(Debug, Clone)]
pub struct FileSourceLoader {
    root: PathBuf,
    native_paths: RefCell<BTreeMap<String, PathBuf>>,
}
impl FileSourceLoader {
    pub fn new(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = fs::canonicalize(root)?;
        if !root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "source root is not a directory",
            ));
        }
        Ok(Self {
            root,
            native_paths: RefCell::new(BTreeMap::new()),
        })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub(super) fn identify(&self, path: &Path) -> Result<String, SourceLoadError> {
        let path = self.checked(path)?;
        if let Some(name) = path.to_str() {
            return Ok(name.to_string());
        }
        // Preserve native non-Unicode entry paths without lossy identity keys.
        // Debug formatting escapes their native bytes/code units; actual paths
        // remain owned by this loader instance, never a mutable global cache.
        let identity = format!("@native:{path:?}");
        self.native_paths
            .borrow_mut()
            .insert(identity.clone(), path);
        Ok(identity)
    }
    fn path(&self, identity: &str) -> PathBuf {
        self.native_paths
            .borrow()
            .get(identity)
            .cloned()
            .unwrap_or_else(|| PathBuf::from(identity))
    }
    fn checked(&self, path: &Path) -> Result<PathBuf, SourceLoadError> {
        let path = fs::canonicalize(path).map_err(|error| {
            SourceLoadError::new(
                SourceLoadKind::Io,
                format!("could not read '{}': {error}", path.display()),
            )
        })?;
        if !path.starts_with(&self.root) {
            return Err(path_error(
                "canonical module path escapes the trusted source root",
            ));
        }
        if !path.is_file() {
            return Err(SourceLoadError::new(
                SourceLoadKind::Io,
                format!("could not read '{}': not a regular file", path.display()),
            ));
        }
        Ok(path)
    }
}
impl SourceLoader for FileSourceLoader {
    fn resolve(&self, importer: Option<&str>, specifier: &str) -> Result<String, SourceLoadError> {
        let base = if let Some(importer) = importer {
            let importing_path = self.path(importer);
            let parent = importing_path
                .parent()
                .ok_or_else(|| path_error("importing source has no parent"))?;
            parent
                .strip_prefix(&self.root)
                .map_err(|_| path_error("importer is outside the trusted source root"))?
                .to_str()
                .ok_or_else(|| path_error("source paths must be valid Unicode"))?
                .replace('\\', "/")
        } else {
            String::new()
        };
        let relative = relative_path(&base, specifier, importer.is_some())?;
        self.identify(&self.root.join(relative))
    }
    fn load(&self, identity: &str, max_bytes: usize) -> Result<String, SourceLoadError> {
        // Recheck canonical containment immediately before opening. This is a
        // trusted-tree loader, not a sandbox against concurrent hostile changes.
        let path = self.checked(&self.path(identity))?;
        let file = fs::File::open(&path).map_err(|error| {
            SourceLoadError::new(
                SourceLoadKind::Io,
                format!("could not read '{}': {error}", path.display()),
            )
        })?;
        let amount = file
            .metadata()
            .map_err(|error| SourceLoadError::new(SourceLoadKind::Io, error.to_string()))?
            .len();
        if amount > max_bytes as u64 {
            return Err(SourceLoadError::new(
                SourceLoadKind::Limit,
                "module source byte limit exceeded",
            ));
        }
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(amount as usize).map_err(|_| {
            SourceLoadError::new(SourceLoadKind::Limit, "cannot reserve source buffer")
        })?;
        file.take((max_bytes as u64).saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| {
                SourceLoadError::new(
                    SourceLoadKind::Io,
                    format!("could not read '{}': {error}", path.display()),
                )
            })?;
        if bytes.len() > max_bytes {
            return Err(SourceLoadError::new(
                SourceLoadKind::Limit,
                "module source grew beyond its byte limit",
            ));
        }
        String::from_utf8(bytes).map_err(|_| {
            SourceLoadError::new(
                SourceLoadKind::InvalidUtf8,
                format!(
                    "could not read '{}': source is not valid UTF-8",
                    path.display()
                ),
            )
        })
    }
    fn display_name(&self, identity: &str) -> String {
        self.path(identity).to_string_lossy().into_owned()
    }
}
