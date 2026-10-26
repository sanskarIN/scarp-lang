//! Compile the supported C subset with an external Clang/GCC-style driver.

use crate::{c_backend, diagnostics::Diagnostic, ExecutionLimits};
use std::{
    env,
    ffi::OsString,
    fmt, fs, io,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectTarget {
    X86_64,
    Aarch64,
}

impl ObjectTarget {
    pub fn triple(self) -> &'static str {
        match self {
            Self::X86_64 => "x86_64-unknown-none-elf",
            Self::Aarch64 => "aarch64-unknown-none-elf",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BuildMode {
    Executable { limits: ExecutionLimits },
    Object { target: Option<ObjectTarget> },
    CSource,
}

#[derive(Clone, Debug)]
pub struct BuildOptions {
    pub output: PathBuf,
    /// A single executable path/name, without shell syntax or embedded flags.
    /// Unused for CSource, which does not invoke a compiler.
    pub compiler: OsString,
    pub mode: BuildMode,
}

#[derive(Debug)]
pub enum BuildError {
    Source(Diagnostic),
    OutputExists(PathBuf),
    File {
        operation: &'static str,
        error: io::Error,
    },
    CompilerStart(io::Error),
    CompilerFailed {
        status: ExitStatus,
        stdout: String,
        stderr: String,
    },
    MissingArtifact,
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(f),
            Self::OutputExists(path) => write!(f, "output '{}' already exists; choose a fresh path", path.display()),
            Self::File { operation, error } => write!(f, "could not {operation}: {error}"),
            Self::CompilerStart(error) => write!(f, "could not start the C compiler: {error}; install Clang/GCC or select its executable with --cc"),
            Self::CompilerFailed { status, stdout, stderr } => write!(f, "C compiler failed ({status})\n{stdout}{stderr}"),
            Self::MissingArtifact => f.write_str("C compiler succeeded without producing a nonempty regular artifact"),
        }
    }
}

impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::File { error, .. } | Self::CompilerStart(error) => Some(error),
            _ => None,
        }
    }
}

fn file_error(operation: &'static str, error: io::Error) -> BuildError {
    BuildError::File { operation, error }
}

struct Staging(PathBuf);
impl Staging {
    fn create(parent: &Path) -> io::Result<Self> {
        for _ in 0..32 {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let clock = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = parent.join(format!(".scarp-build-{}-{clock}-{id}", std::process::id()));
            let mut directory = fs::DirBuilder::new();
            directory.recursive(false);
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                directory.mode(0o700);
            }
            match directory.create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not reserve a build directory",
        ))
    }
}
impl Drop for Staging {
    fn drop(&mut self) {
        // Only remove a directory that this invocation successfully created.
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Build without executing the source or replacing any existing output.
/// The output parent must exist. Hard-link publication is atomic when supported;
/// other filesystems reserve a fresh file before copying and remove it on failure.
pub fn build(source: &str, options: &BuildOptions) -> Result<(), BuildError> {
    let mut unit = c_backend::emit_c(source).map_err(BuildError::Source)?;
    if let BuildMode::Executable { limits } = options.mode {
        use std::fmt::Write;
        writeln!(unit, "\n#define SCARP_MAX_STEPS UINT64_C({})\n#define SCARP_MAX_PRINTS ((size_t)UINT64_C({}))", limits.max_steps, limits.max_prints).unwrap();
        unit.push_str(include_str!("hosted_runtime.c"));
    }
    let output = env::current_dir()
        .map_err(|e| file_error("read the working directory", e))?
        .join(&options.output);
    match fs::symlink_metadata(&output) {
        Ok(_) => return Err(BuildError::OutputExists(output)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(file_error("inspect the output path", error)),
    }
    let staging = Staging::create(output.parent().unwrap_or(Path::new(".")))
        .map_err(|e| file_error("create the build staging directory", e))?;
    let input = staging.0.join("program.c");
    let artifact = staging.0.join(if cfg!(windows) {
        "artifact.exe"
    } else {
        "artifact"
    });
    fs::write(&input, unit).map_err(|e| file_error("write generated C", e))?;
    if matches!(options.mode, BuildMode::CSource) {
        let permissions = fs::metadata(&input)
            .map_err(|e| file_error("inspect generated C", e))?
            .permissions();
        return publish(&input, &output, permissions);
    }
    let mut command = Command::new(&options.compiler);
    command.args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror"]);
    if let BuildMode::Object { target } = options.mode {
        if let Some(target) = target {
            command.args(["-target", target.triple()]);
        }
        command.args([
            "-ffreestanding",
            "-fno-builtin",
            "-fno-stack-protector",
            "-c",
        ]);
    }
    let compiled = command
        .arg(&input)
        .arg("-o")
        .arg(&artifact)
        .output()
        .map_err(BuildError::CompilerStart)?;
    if !compiled.status.success() {
        return Err(BuildError::CompilerFailed {
            status: compiled.status,
            stdout: String::from_utf8_lossy(&compiled.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&compiled.stderr).into_owned(),
        });
    }
    let metadata = fs::symlink_metadata(&artifact).map_err(|_| BuildError::MissingArtifact)?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(BuildError::MissingArtifact);
    }
    // Same-filesystem staging preserves executable permissions. hard_link fails
    // atomically if another invocation creates the destination before this one.
    publish(&artifact, &output, metadata.permissions())
}

fn publish(artifact: &Path, output: &Path, permissions: fs::Permissions) -> Result<(), BuildError> {
    match fs::hard_link(artifact, output) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            Err(BuildError::OutputExists(output.to_owned()))
        }
        Err(_) => copy_new(artifact, output, permissions),
    }
}

fn copy_new(
    artifact: &Path,
    output: &Path,
    permissions: fs::Permissions,
) -> Result<(), BuildError> {
    let mut source =
        fs::File::open(artifact).map_err(|e| file_error("read the compiled artifact", e))?;
    let mut destination = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                BuildError::OutputExists(output.to_owned())
            } else {
                file_error("reserve the output file", error)
            }
        })?;
    let result = io::copy(&mut source, &mut destination)
        .and_then(|_| destination.flush())
        .and_then(|_| destination.set_permissions(permissions));
    drop(destination);
    if let Err(error) = result {
        let _ = fs::remove_file(output);
        return Err(file_error("copy the compiled artifact", error));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_copy_preserves_bytes_permissions_and_existing_outputs() {
        let staging = Staging::create(&env::temp_dir()).unwrap();
        let source = staging.0.join("source");
        let output = staging.0.join("output");
        fs::write(&source, b"compiled bytes").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&source, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let permissions = fs::metadata(&source).unwrap().permissions();
        copy_new(&source, &output, permissions.clone()).unwrap();
        assert_eq!(fs::read(&output).unwrap(), b"compiled bytes");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&output).unwrap().permissions().mode() & 0o777,
                0o755
            );
        }
        fs::write(&source, b"replacement").unwrap();
        assert!(matches!(
            copy_new(&source, &output, permissions),
            Err(BuildError::OutputExists(_))
        ));
        assert_eq!(fs::read(output).unwrap(), b"compiled bytes");
    }
}
