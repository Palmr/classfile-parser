use std::io::{Read, Seek};
use std::path::Path;

use crate::ClassFile;
use crate::jar_utils::{JarError, JarFile, JarResult};

use super::classpath_idx::ClasspathIndex;
use super::layers_idx::LayersIndex;

// ---------------------------------------------------------------------------
// SpringBootFormat
// ---------------------------------------------------------------------------

/// The packaging format of a Spring Boot fat JAR/WAR.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpringBootFormat {
    /// Standard JAR layout — application classes in `BOOT-INF/`.
    Jar,
    /// WAR layout — application classes in `WEB-INF/`.
    War,
}

impl SpringBootFormat {
    /// The top-level prefix directory (`BOOT-INF` or `WEB-INF`).
    pub fn prefix(&self) -> &'static str {
        match self {
            SpringBootFormat::Jar => "BOOT-INF",
            SpringBootFormat::War => "WEB-INF",
        }
    }

    /// The directory containing application classes.
    pub fn classes_dir(&self) -> &'static str {
        match self {
            SpringBootFormat::Jar => "BOOT-INF/classes/",
            SpringBootFormat::War => "WEB-INF/classes/",
        }
    }

    /// The directory containing dependency JARs.
    pub fn lib_dir(&self) -> &'static str {
        match self {
            SpringBootFormat::Jar => "BOOT-INF/lib/",
            SpringBootFormat::War => "WEB-INF/lib/",
        }
    }
}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// Known Spring Boot launcher main classes.
const JAR_LAUNCHERS: &[&str] = &[
    "org.springframework.boot.loader.JarLauncher",
    "org.springframework.boot.loader.launch.JarLauncher",
];

const WAR_LAUNCHERS: &[&str] = &[
    "org.springframework.boot.loader.WarLauncher",
    "org.springframework.boot.loader.launch.WarLauncher",
];

const PROPERTIES_LAUNCHERS: &[&str] = &[
    "org.springframework.boot.loader.PropertiesLauncher",
    "org.springframework.boot.loader.launch.PropertiesLauncher",
];

/// Detect the Spring Boot format from a `JarFile` by inspecting the manifest.
///
/// Returns `None` if this is not a Spring Boot fat JAR/WAR.
pub fn detect_format(jar: &JarFile) -> Option<SpringBootFormat> {
    let manifest = jar.manifest().ok()??;

    // Must have Start-Class
    manifest.main_attr("Start-Class")?;

    let main_class = manifest.main_attr("Main-Class")?;

    if JAR_LAUNCHERS.iter().any(|&l| l == main_class) {
        return Some(SpringBootFormat::Jar);
    }
    if WAR_LAUNCHERS.iter().any(|&l| l == main_class) {
        return Some(SpringBootFormat::War);
    }
    if PROPERTIES_LAUNCHERS.iter().any(|&l| l == main_class) {
        // Infer format from directory structure
        if jar.entry_names().any(|n| n.starts_with("BOOT-INF/")) {
            return Some(SpringBootFormat::Jar);
        }
        if jar.entry_names().any(|n| n.starts_with("WEB-INF/")) {
            return Some(SpringBootFormat::War);
        }
        // Default to Jar for PropertiesLauncher
        return Some(SpringBootFormat::Jar);
    }

    None
}

// ---------------------------------------------------------------------------
// SpringBootJar
// ---------------------------------------------------------------------------

/// A Spring Boot fat JAR/WAR wrapping a `JarFile`.
#[derive(Clone, Debug)]
pub struct SpringBootJar {
    jar: JarFile,
    format: SpringBootFormat,
}

impl SpringBootJar {
    // -- Construction / Detection --

    /// Wrap an existing `JarFile` if it is a Spring Boot fat JAR/WAR.
    pub fn from_jar(jar: JarFile) -> Option<Self> {
        let format = detect_format(&jar)?;
        Some(SpringBootJar { jar, format })
    }

    /// Read a JAR and detect Spring Boot format.
    /// Returns `Ok(None)` if it is not a Spring Boot fat JAR.
    pub fn read<R: Read + Seek>(reader: R) -> JarResult<Option<Self>> {
        let jar = JarFile::read(reader)?;
        Ok(Self::from_jar(jar))
    }

    /// Read from bytes and detect.
    pub fn from_bytes(bytes: &[u8]) -> JarResult<Option<Self>> {
        let jar = JarFile::from_bytes(bytes)?;
        Ok(Self::from_jar(jar))
    }

    /// Open from a file path and detect.
    pub fn open(path: impl AsRef<Path>) -> JarResult<Option<Self>> {
        let jar = JarFile::open(path)?;
        Ok(Self::from_jar(jar))
    }

    // -- Accessors --

    /// The underlying `JarFile`.
    pub fn jar(&self) -> &JarFile {
        &self.jar
    }

    /// Mutable access to the underlying `JarFile`.
    pub fn jar_mut(&mut self) -> &mut JarFile {
        &mut self.jar
    }

    /// Consume the wrapper, returning the inner `JarFile`.
    pub fn into_jar(self) -> JarFile {
        self.jar
    }

    /// The detected packaging format.
    pub fn format(&self) -> SpringBootFormat {
        self.format
    }

    // -- Manifest shortcuts --

    /// The `Start-Class` manifest attribute (the actual application main class).
    pub fn start_class(&self) -> JarResult<Option<String>> {
        Ok(self
            .jar
            .manifest()?
            .and_then(|m| m.main_attr("Start-Class").map(|s| s.to_string())))
    }

    /// The `Spring-Boot-Version` manifest attribute.
    pub fn spring_boot_version(&self) -> JarResult<Option<String>> {
        Ok(self
            .jar
            .manifest()?
            .and_then(|m| m.main_attr("Spring-Boot-Version").map(|s| s.to_string())))
    }

    /// The `Spring-Boot-Classes` manifest attribute.
    pub fn spring_boot_classes_path(&self) -> JarResult<Option<String>> {
        Ok(self
            .jar
            .manifest()?
            .and_then(|m| m.main_attr("Spring-Boot-Classes").map(|s| s.to_string())))
    }

    /// The `Spring-Boot-Lib` manifest attribute.
    pub fn spring_boot_lib_path(&self) -> JarResult<Option<String>> {
        Ok(self
            .jar
            .manifest()?
            .and_then(|m| m.main_attr("Spring-Boot-Lib").map(|s| s.to_string())))
    }

    // -- Application classes --

    /// Iterate over `.class` file paths under the classes directory.
    pub fn app_class_names(&self) -> impl Iterator<Item = &str> {
        let classes_dir = self.format.classes_dir();
        self.jar
            .entry_names()
            .filter(move |n| n.starts_with(classes_dir) && n.ends_with(".class"))
    }

    /// Iterate over non-`.class` resource paths under the classes directory.
    pub fn app_resource_names(&self) -> impl Iterator<Item = &str> {
        let classes_dir = self.format.classes_dir();
        self.jar
            .entry_names()
            .filter(move |n| n.starts_with(classes_dir) && !n.ends_with(".class"))
    }

    /// Parse a `.class` file from the classes directory.
    pub fn parse_app_class(&self, path: &str) -> JarResult<ClassFile> {
        self.jar.parse_class(path)
    }

    /// Parse all `.class` files under the classes directory.
    pub fn parse_all_app_classes(&self) -> Vec<(String, JarResult<ClassFile>)> {
        self.app_class_names()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .into_iter()
            .map(|name| {
                let result = self.jar.parse_class(&name);
                (name, result)
            })
            .collect()
    }

    // -- Loader classes --

    /// Iterate over Spring Boot loader class paths.
    pub fn loader_class_names(&self) -> impl Iterator<Item = &str> {
        self.jar
            .entry_names()
            .filter(|n| n.starts_with("org/springframework/boot/loader/") && n.ends_with(".class"))
    }

    // -- Nested JARs --

    /// Iterate over nested JAR paths under the lib directory.
    pub fn nested_jar_names(&self) -> impl Iterator<Item = &str> {
        let lib_dir = self.format.lib_dir();
        self.jar
            .entry_names()
            .filter(move |n| n.starts_with(lib_dir) && n.ends_with(".jar"))
    }

    /// Open a nested JAR by its path within the fat JAR.
    pub fn open_nested_jar(&self, path: &str) -> JarResult<JarFile> {
        let data = self.jar.get_entry(path).ok_or_else(|| {
            JarError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("nested JAR not found: {path}"),
            ))
        })?;
        JarFile::from_bytes(data)
    }

    /// Open all nested JARs. Returns `(path, result)` pairs.
    pub fn open_all_nested_jars(&self) -> Vec<(String, JarResult<JarFile>)> {
        self.nested_jar_names()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .into_iter()
            .map(|name| {
                let result = self.open_nested_jar(&name);
                (name, result)
            })
            .collect()
    }

    /// Parse a `.class` file from inside a nested JAR.
    pub fn parse_nested_class(&self, jar_path: &str, class_path: &str) -> JarResult<ClassFile> {
        let nested = self.open_nested_jar(jar_path)?;
        nested.parse_class(class_path)
    }

    // -- Index file access --

    /// Parse the `classpath.idx` file if present.
    pub fn classpath_index(&self) -> JarResult<Option<ClasspathIndex>> {
        let idx_path = format!("{}/classpath.idx", self.format.prefix());
        match self.jar.get_entry(&idx_path) {
            Some(data) => Ok(Some(ClasspathIndex::parse(data)?)),
            None => Ok(None),
        }
    }

    /// Parse the `layers.idx` file if present.
    pub fn layers_index(&self) -> JarResult<Option<LayersIndex>> {
        let idx_path = format!("{}/layers.idx", self.format.prefix());
        match self.jar.get_entry(&idx_path) {
            Some(data) => Ok(Some(LayersIndex::parse(data)?)),
            None => Ok(None),
        }
    }
}
