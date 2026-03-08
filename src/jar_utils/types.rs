use std::collections::BTreeMap;
use std::io::{Cursor, Read, Seek, Write};
use std::path::Path;

use binrw::{BinRead, BinWrite};
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

use crate::ClassFile;

use super::manifest::JarManifest;

const MANIFEST_PATH: &str = "META-INF/MANIFEST.MF";

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum JarError {
    Io(std::io::Error),
    Zip(zip::result::ZipError),
    ClassParse(binrw::Error),
    ManifestParse(String),
}

impl std::fmt::Display for JarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JarError::Io(e) => write!(f, "I/O error: {e}"),
            JarError::Zip(e) => write!(f, "ZIP error: {e}"),
            JarError::ClassParse(e) => write!(f, "class parse error: {e}"),
            JarError::ManifestParse(e) => write!(f, "manifest parse error: {e}"),
        }
    }
}

impl std::error::Error for JarError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            JarError::Io(e) => Some(e),
            JarError::Zip(e) => Some(e),
            JarError::ClassParse(e) => Some(e),
            JarError::ManifestParse(_) => None,
        }
    }
}

impl From<std::io::Error> for JarError {
    fn from(e: std::io::Error) -> Self {
        JarError::Io(e)
    }
}

impl From<zip::result::ZipError> for JarError {
    fn from(e: zip::result::ZipError) -> Self {
        JarError::Zip(e)
    }
}

impl From<binrw::Error> for JarError {
    fn from(e: binrw::Error) -> Self {
        JarError::ClassParse(e)
    }
}

pub type JarResult<T> = Result<T, JarError>;

// ---------------------------------------------------------------------------
// JarFile
// ---------------------------------------------------------------------------

/// In-memory representation of a JAR (ZIP) archive.
///
/// Entries are stored as a `BTreeMap<String, Vec<u8>>` mapping entry paths to
/// raw bytes. This avoids lifetime issues with `ZipArchive` and allows free
/// mutation before writing.
#[derive(Clone, Debug)]
pub struct JarFile {
    entries: BTreeMap<String, Vec<u8>>,
}

impl JarFile {
    /// Create an empty JAR.
    pub fn new() -> Self {
        JarFile {
            entries: BTreeMap::new(),
        }
    }

    // -- Reading --

    /// Read a JAR from any reader.
    pub fn read<R: Read + Seek>(reader: R) -> JarResult<Self> {
        let mut archive = zip::ZipArchive::new(reader)?;
        let mut entries = BTreeMap::new();

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            if file.is_dir() {
                continue;
            }
            let name = file.name().to_string();
            let mut data = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut data)?;
            entries.insert(name, data);
        }

        Ok(JarFile { entries })
    }

    /// Read a JAR from a byte slice.
    pub fn from_bytes(bytes: &[u8]) -> JarResult<Self> {
        Self::read(Cursor::new(bytes))
    }

    /// Read a JAR from a file path.
    pub fn open(path: impl AsRef<Path>) -> JarResult<Self> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        Self::read(reader)
    }

    // -- Writing --

    /// Write the JAR to any writer using Deflated compression.
    pub fn write<W: Write + Seek>(&self, writer: W) -> JarResult<()> {
        let mut zip_writer = zip::ZipWriter::new(writer);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        for (name, data) in &self.entries {
            zip_writer.start_file(name, options)?;
            zip_writer.write_all(data)?;
        }

        zip_writer.finish()?;
        Ok(())
    }

    /// Serialize the JAR to a byte vector.
    pub fn to_bytes(&self) -> JarResult<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());
        self.write(&mut buf)?;
        Ok(buf.into_inner())
    }

    /// Write the JAR to a file path.
    pub fn save(&self, path: impl AsRef<Path>) -> JarResult<()> {
        let file = std::fs::File::create(path)?;
        let writer = std::io::BufWriter::new(file);
        self.write(writer)
    }

    // -- Entry access --

    /// Iterate over all entry paths (sorted).
    pub fn entry_names(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(|s| s.as_str())
    }

    /// Iterate over `.class` entry paths only.
    pub fn class_names(&self) -> impl Iterator<Item = &str> {
        self.entry_names().filter(|n| n.ends_with(".class"))
    }

    /// Get the raw bytes of an entry.
    pub fn get_entry(&self, path: &str) -> Option<&[u8]> {
        self.entries.get(path).map(|v| v.as_slice())
    }

    /// Insert or replace an entry.
    pub fn set_entry(&mut self, path: impl Into<String>, data: Vec<u8>) {
        self.entries.insert(path.into(), data);
    }

    /// Remove an entry, returning its data if it existed.
    pub fn remove_entry(&mut self, path: &str) -> Option<Vec<u8>> {
        self.entries.remove(path)
    }

    /// Check whether an entry exists.
    pub fn contains_entry(&self, path: &str) -> bool {
        self.entries.contains_key(path)
    }

    // -- ClassFile integration --

    /// Parse a `.class` entry into a `ClassFile`.
    pub fn parse_class(&self, path: &str) -> JarResult<ClassFile> {
        let data = self.get_entry(path).ok_or_else(|| {
            JarError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("entry not found: {path}"),
            ))
        })?;
        let mut cursor = Cursor::new(data);
        let class_file = ClassFile::read(&mut cursor)?;
        Ok(class_file)
    }

    /// Parse all `.class` entries. Returns a vec of `(path, result)` pairs.
    pub fn parse_all_classes(&self) -> Vec<(String, JarResult<ClassFile>)> {
        self.class_names()
            .map(|name| name.to_string())
            .collect::<Vec<_>>()
            .into_iter()
            .map(|name| {
                let result = self.parse_class(&name);
                (name, result)
            })
            .collect()
    }

    /// Serialize a `ClassFile` and store it as an entry.
    pub fn set_class(&mut self, path: &str, class_file: &ClassFile) -> JarResult<()> {
        let mut buf = Cursor::new(Vec::new());
        class_file.write(&mut buf)?;
        self.set_entry(path.to_string(), buf.into_inner());
        Ok(())
    }

    // -- Manifest integration --

    /// Parse the `META-INF/MANIFEST.MF` entry if present.
    pub fn manifest(&self) -> JarResult<Option<JarManifest>> {
        match self.get_entry(MANIFEST_PATH) {
            Some(data) => Ok(Some(JarManifest::parse(data)?)),
            None => Ok(None),
        }
    }

    /// Serialize a `JarManifest` and store it as `META-INF/MANIFEST.MF`.
    pub fn set_manifest(&mut self, manifest: &JarManifest) {
        self.set_entry(MANIFEST_PATH.to_string(), manifest.to_bytes());
    }
}
