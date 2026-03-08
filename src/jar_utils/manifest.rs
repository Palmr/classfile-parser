use super::types::{JarError, JarResult};

/// Ordered collection of key-value pairs with case-insensitive key lookup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestAttributes {
    entries: Vec<(String, String)>,
}

impl ManifestAttributes {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Case-insensitive key lookup.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    /// Replace if a matching key exists (case-insensitive), otherwise append.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|(k, _)| k.eq_ignore_ascii_case(&key))
        {
            entry.0 = key;
            entry.1 = value;
        } else {
            self.entries.push((key, value));
        }
    }

    /// Remove the first entry matching the key (case-insensitive). Returns the value if found.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        let pos = self
            .entries
            .iter()
            .position(|(k, _)| k.eq_ignore_ascii_case(key))?;
        Some(self.entries.remove(pos).1)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case(key))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for ManifestAttributes {
    fn default() -> Self {
        Self::new()
    }
}

/// Structured representation of a JAR manifest (`META-INF/MANIFEST.MF`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JarManifest {
    pub main_attributes: ManifestAttributes,
    pub entries: std::collections::BTreeMap<String, ManifestAttributes>,
}

impl JarManifest {
    /// Parse a manifest from raw bytes (assumed UTF-8).
    pub fn parse(data: &[u8]) -> JarResult<Self> {
        let text = std::str::from_utf8(data)
            .map_err(|e| JarError::ManifestParse(format!("invalid UTF-8: {e}")))?;

        // Split into lines, handling both \r\n and \n
        let raw_lines: Vec<&str> = text.split('\n').collect();

        // Join continuation lines (lines starting with a single space)
        let mut logical_lines: Vec<String> = Vec::new();
        for raw in &raw_lines {
            let line = raw.strip_suffix('\r').unwrap_or(raw);
            if line.starts_with(' ') && !logical_lines.is_empty() {
                // Continuation line — append without the leading space
                let last = logical_lines.last_mut().unwrap();
                last.push_str(&line[1..]);
            } else {
                logical_lines.push(line.to_string());
            }
        }

        let mut main_attributes = ManifestAttributes::new();
        let mut entries = std::collections::BTreeMap::new();
        let mut current_section: Option<(String, ManifestAttributes)> = None;
        let mut in_main = true;

        for line in &logical_lines {
            if line.is_empty() {
                // Blank line separates sections
                if let Some((name, attrs)) = current_section.take() {
                    entries.insert(name, attrs);
                }
                in_main = false;
                continue;
            }

            // Split on first ": "
            let Some(colon_pos) = line.find(": ") else {
                // Lines without ": " are ignored (e.g. trailing whitespace)
                continue;
            };
            let key = &line[..colon_pos];
            let value = &line[colon_pos + 2..];

            if in_main {
                main_attributes.set(key, value);
            } else if key.eq_ignore_ascii_case("Name") && current_section.is_none() {
                current_section = Some((value.to_string(), ManifestAttributes::new()));
            } else if let Some((_, ref mut attrs)) = current_section {
                attrs.set(key, value);
            } else {
                // Name: starts a new section
                if key.eq_ignore_ascii_case("Name") {
                    current_section = Some((value.to_string(), ManifestAttributes::new()));
                }
            }
        }

        // Flush last section
        if let Some((name, attrs)) = current_section {
            entries.insert(name, attrs);
        }

        Ok(JarManifest {
            main_attributes,
            entries,
        })
    }

    /// Serialize to bytes with \r\n line endings and 72-byte line wrapping.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = String::new();

        // Main section
        write_section(&mut out, &self.main_attributes);

        // Per-entry sections
        for (name, attrs) in &self.entries {
            out.push_str("\r\n");
            write_wrapped_line(&mut out, "Name", name);
            write_section(&mut out, attrs);
        }

        out.into_bytes()
    }

    /// Shorthand for `main_attributes.get(key)`.
    pub fn main_attr(&self, key: &str) -> Option<&str> {
        self.main_attributes.get(key)
    }

    /// Shorthand for `main_attributes.set(key, value)`.
    pub fn set_main_attr(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.main_attributes.set(key, value);
    }

    /// Get a per-entry section by name.
    pub fn entry_section(&self, name: &str) -> Option<&ManifestAttributes> {
        self.entries.get(name)
    }

    /// Get or create a per-entry section by name (mutable).
    pub fn entry_section_mut(&mut self, name: impl Into<String>) -> &mut ManifestAttributes {
        self.entries.entry(name.into()).or_default()
    }

    /// Create a default manifest with `Manifest-Version: 1.0`.
    pub fn default_manifest() -> Self {
        let mut main_attributes = ManifestAttributes::new();
        main_attributes.set("Manifest-Version", "1.0");
        JarManifest {
            main_attributes,
            entries: std::collections::BTreeMap::new(),
        }
    }
}

/// Write all attributes in a section, each line wrapped at 72 bytes.
fn write_section(out: &mut String, attrs: &ManifestAttributes) {
    for (key, value) in attrs.iter() {
        write_wrapped_line(out, key, value);
    }
}

/// Write a single `Key: Value\r\n` with continuation wrapping at 72 bytes.
fn write_wrapped_line(out: &mut String, key: &str, value: &str) {
    let full = format!("{}: {}", key, value);
    let bytes = full.as_bytes();

    if bytes.len() <= 72 {
        out.push_str(&full);
        out.push_str("\r\n");
        return;
    }

    // First line: up to 72 bytes
    // Find a safe UTF-8 boundary at or before byte 72
    let first_end = safe_split_pos(&full, 72);
    out.push_str(&full[..first_end]);
    out.push_str("\r\n");

    let mut pos = first_end;
    while pos < bytes.len() {
        // Continuation lines: " " + up to 71 bytes of content = 72 bytes total
        let chunk_end = safe_split_pos(&full[pos..], 71);
        out.push(' ');
        out.push_str(&full[pos..pos + chunk_end]);
        out.push_str("\r\n");
        pos += chunk_end;
    }
}

/// Find the largest byte position <= max_bytes that is a valid UTF-8 char boundary.
fn safe_split_pos(s: &str, max_bytes: usize) -> usize {
    if max_bytes >= s.len() {
        return s.len();
    }
    let mut pos = max_bytes;
    while pos > 0 && !s.is_char_boundary(pos) {
        pos -= 1;
    }
    // Don't return 0 unless the string is empty — force at least one char
    if pos == 0 && !s.is_empty() {
        let first_char_len = s.chars().next().unwrap().len_utf8();
        return first_char_len;
    }
    pos
}
