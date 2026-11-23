//! Import and validate trainer executables by reference.

use std::{
    fs,
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedTrainer {
    pub path: PathBuf,
    pub name: String,
    pub file_size: u64,
}
#[derive(Debug, thiserror::Error)]
pub enum TrainerError {
    #[error("trainer file could not be read at {path}: {source}")]
    Unreadable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("trainer must be an absolute path")]
    RelativePath,
    #[error("trainer must be a Windows .exe file: {0}")]
    NotExecutable(PathBuf),
    #[error("trainer path does not point to a regular file: {0}")]
    NotFile(PathBuf),
    #[error("trainer does not contain a valid Windows PE signature: {0}")]
    InvalidExecutable(PathBuf),
}
impl ImportedTrainer {
    pub fn validate(path: impl AsRef<Path>) -> Result<Self, TrainerError> {
        let path = path.as_ref();
        if !path.is_absolute() {
            return Err(TrainerError::RelativePath);
        }
        let canonical = fs::canonicalize(path).map_err(|source| TrainerError::Unreadable {
            path: path.to_path_buf(),
            source,
        })?;
        let metadata = fs::metadata(&canonical).map_err(|source| TrainerError::Unreadable {
            path: canonical.clone(),
            source,
        })?;
        if !metadata.is_file() {
            return Err(TrainerError::NotFile(canonical));
        }
        if !canonical
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
        {
            return Err(TrainerError::NotExecutable(canonical));
        }
        if !has_pe_signature(&canonical).map_err(|source| TrainerError::Unreadable {
            path: canonical.clone(),
            source,
        })? {
            return Err(TrainerError::InvalidExecutable(canonical));
        }
        let name = canonical
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Trainer")
            .to_owned();
        Ok(Self {
            path: canonical,
            name,
            file_size: metadata.len(),
        })
    }
}

fn has_pe_signature(path: &Path) -> io::Result<bool> {
    const DOS_HEADER_SIZE: usize = 64;
    let mut file = fs::File::open(path)?;
    let length = file.metadata()?.len();
    if length < DOS_HEADER_SIZE as u64 {
        return Ok(false);
    }
    let mut dos_header = [0_u8; DOS_HEADER_SIZE];
    match file.read_exact(&mut dos_header) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(false),
        Err(error) => return Err(error),
    }
    if &dos_header[..2] != b"MZ" {
        return Ok(false);
    }
    let pe_offset = u32::from_le_bytes(dos_header[0x3c..0x40].try_into().unwrap()) as u64;
    match pe_offset.checked_add(4) {
        Some(end) if end <= length => {}
        _ => return Ok(false),
    }
    file.seek(SeekFrom::Start(pe_offset))?;
    let mut pe_signature = [0_u8; 4];
    match file.read_exact(&mut pe_signature) {
        Ok(()) => Ok(&pe_signature == b"PE\0\0"),
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write_pe(path: &Path) {
        let mut bytes = vec![0_u8; 68];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&64_u32.to_le_bytes());
        bytes[64..68].copy_from_slice(b"PE\0\0");
        fs::write(path, bytes).unwrap();
    }
    #[test]
    fn validates_by_reference() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("Trainer.exe");
        write_pe(&p);
        let t = ImportedTrainer::validate(&p).unwrap();
        assert_eq!(t.file_size, 68);
        assert_eq!(t.path, p);
    }
    #[test]
    fn rejects_exe_extension_with_only_mz_prefix() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("fake.exe");
        fs::write(&p, b"MZ but not a PE image").unwrap();
        assert!(matches!(
            ImportedTrainer::validate(p),
            Err(TrainerError::InvalidExecutable(_))
        ));
    }
    #[test]
    fn requires_absolute_path_before_accessing_files() {
        assert!(matches!(
            ImportedTrainer::validate("relative.exe"),
            Err(TrainerError::RelativePath)
        ));
    }
}
