use std::{
    fmt,
    path::{Path, PathBuf},
};

pub const MAX_MANIFEST_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    pub(super) name: String,
    pub(super) entry: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestError {
    pub line: usize,
    pub message: &'static str,
}
impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}
impl std::error::Error for ManifestError {}
fn error(line: usize, message: &'static str) -> ManifestError {
    ManifestError { line, message }
}
fn trim(text: &str) -> &str {
    text.trim_matches(|c| c == ' ' || c == '\t')
}

impl Manifest {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn entry(&self) -> &Path {
        &self.entry
    }

    /// Parse the documented single-application manifest profile, not general TOML.
    pub fn parse(source: &str) -> Result<Self, ManifestError> {
        if source.len() > MAX_MANIFEST_BYTES {
            return Err(error(1, "manifest exceeds 16 KiB"));
        }
        let (mut format, mut table, mut name, mut entry) = (false, false, None, None);
        for (offset, raw) in source
            .strip_prefix('\u{feff}')
            .unwrap_or(source)
            .lines()
            .enumerate()
        {
            let line = offset + 1;
            if raw.chars().any(|c| c.is_control() && c != '\t') {
                return Err(error(line, "unsupported control character"));
            }
            let text = trim(without_comment(raw));
            if text.is_empty() {
                continue;
            }
            if text.starts_with('[') {
                if text != "[project]" {
                    return Err(error(line, "only the [project] table is supported"));
                }
                if table {
                    return Err(error(line, "duplicate [project] table"));
                }
                table = true;
                continue;
            }
            let (key, value) = text
                .split_once('=')
                .ok_or_else(|| error(line, "expected key = value"))?;
            let (key, value) = (trim(key), trim(value));
            match (table, key) {
                (false, "format") => {
                    if format {
                        return Err(error(line, "duplicate format key"));
                    }
                    if value != "1" {
                        return Err(error(line, "supported manifest format is exactly 1"));
                    }
                    format = true;
                }
                (true, "name") => {
                    if name.is_some() {
                        return Err(error(line, "duplicate project name"));
                    }
                    let parsed = string(value).map_err(|message| error(line, message))?;
                    if !valid_name(&parsed) {
                        return Err(error(line, "invalid portable project name; use 1–64 letters/digits/hyphens/underscores starting with a letter"));
                    }
                    name = Some(parsed);
                }
                (true, "entry") => {
                    if entry.is_some() {
                        return Err(error(line, "duplicate project entry"));
                    }
                    let parsed = string(value).map_err(|message| error(line, message))?;
                    if !valid_entry(&parsed) {
                        return Err(error(
                            line,
                            "entry must be a portable relative path inside the project",
                        ));
                    }
                    entry = Some(PathBuf::from(parsed));
                }
                _ => return Err(error(line, "unknown key or key in the wrong table")),
            }
        }
        if !format || !table {
            return Err(error(
                1,
                "manifest requires format = 1 and a [project] table",
            ));
        }
        Ok(Self {
            name: name.ok_or_else(|| error(1, "manifest requires project.name"))?,
            entry: entry.ok_or_else(|| error(1, "manifest requires project.entry"))?,
        })
    }
}

fn without_comment(line: &str) -> &str {
    let (mut quote, mut escaped) = (None, false);
    for (index, character) in line.char_indices() {
        match quote {
            Some('"') if escaped => escaped = false,
            Some('"') if character == '\\' => escaped = true,
            Some(delimiter) if character == delimiter => quote = None,
            None if character == '#' => return &line[..index],
            None if character == '"' || character == '\'' => quote = Some(character),
            _ => {}
        }
    }
    line
}

fn string(text: &str) -> Result<String, &'static str> {
    let mut input = text.chars();
    let delimiter = input.next().ok_or("expected a quoted string")?;
    if delimiter != '"' && delimiter != '\'' {
        return Err("expected a basic or literal quoted string");
    }
    let mut value = String::new();
    while let Some(character) = input.next() {
        if character == delimiter {
            if input.any(|c| c != ' ' && c != '\t') {
                return Err("unexpected text after the string; multiline strings are unsupported");
            }
            return Ok(value);
        }
        if character == '\\' && delimiter == '"' {
            let escaped = match input.next().ok_or("unterminated string escape")? {
                '"' => '"',
                '\\' => '\\',
                'b' => '\u{8}',
                't' => '\t',
                'n' => '\n',
                'f' => '\u{c}',
                'r' => '\r',
                marker @ ('u' | 'U') => {
                    let mut scalar = 0u32;
                    for _ in 0..if marker == 'u' { 4 } else { 8 } {
                        scalar = scalar
                            .checked_mul(16)
                            .and_then(|n| input.next().and_then(|c| c.to_digit(16)).map(|d| n + d))
                            .ok_or("invalid Unicode escape")?;
                    }
                    char::from_u32(scalar).ok_or("Unicode escape is not a scalar value")?
                }
                _ => return Err("unsupported string escape"),
            };
            value.push(escaped);
        } else {
            value.push(character);
        }
    }
    Err("unterminated single-line string")
}

fn device(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "CONIN$"
            | "CONOUT$"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
            | "COM¹"
            | "COM²"
            | "COM³"
            | "LPT¹"
            | "LPT²"
            | "LPT³"
    )
}
pub(super) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.as_bytes()[0].is_ascii_alphabetic()
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        && !device(name)
}
fn valid_entry(entry: &str) -> bool {
    !entry.is_empty()
        && entry.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.ends_with('.')
                && !part.ends_with(' ')
                && !device(part)
                && !part
                    .chars()
                    .any(|c| c.is_control() || "\\:<>\"|?*".contains(c))
        })
}
