use std::fmt;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceEditKind {
    Formatting,
    SyntaxMigration,
    QuickFix,
    Refactor,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceEdit {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
    pub kind: SourceEditKind,
    pub code: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceSuggestion {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
    pub kind: SourceEditKind,
    pub code: String,
    pub message: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePatch {
    pub version: u8,
    pub source_fingerprint: String,
    pub source_bytes: usize,
    #[serde(default)]
    pub edits: Vec<SourceEdit>,
    #[serde(default)]
    pub unresolved: Vec<SourceSuggestion>,
}

#[derive(Debug)]
pub struct SourcePatchError(&'static str);

impl fmt::Display for SourcePatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

pub fn fingerprint(source: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in source.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

pub fn create(source: &str, replacement: &str, kind: SourceEditKind, code: &str) -> SourcePatch {
    let before = source.as_bytes();
    let after = replacement.as_bytes();
    let mut start = before.iter().zip(after).take_while(|(a, b)| a == b).count();
    while start > 0 && (!source.is_char_boundary(start) || !replacement.is_char_boundary(start)) {
        start -= 1;
    }
    let (mut old_end, mut new_end) = (before.len(), after.len());
    while old_end > start && new_end > start && before[old_end - 1] == after[new_end - 1] {
        old_end -= 1;
        new_end -= 1;
    }
    while !source.is_char_boundary(old_end) || !replacement.is_char_boundary(new_end) {
        old_end += 1;
        new_end += 1;
    }
    let edits = if source == replacement {
        Vec::new()
    } else {
        vec![SourceEdit {
            start,
            end: old_end,
            replacement: replacement[start..new_end].into(),
            kind,
            code: code.into(),
        }]
    };
    SourcePatch {
        version: 1,
        source_fingerprint: fingerprint(source),
        source_bytes: source.len(),
        edits,
        unresolved: Vec::new(),
    }
}

pub fn apply(source: &str, patch: &SourcePatch) -> Result<String, SourcePatchError> {
    if patch.version != 1 {
        return Err(SourcePatchError("unsupported source patch version"));
    }
    if patch.source_bytes != source.len() || patch.source_fingerprint != fingerprint(source) {
        return Err(SourcePatchError(
            "source patch precondition does not match the source",
        ));
    }
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    for edit in &patch.edits {
        if edit.start < cursor
            || edit.end < edit.start
            || edit.end > source.len()
            || !source.is_char_boundary(edit.start)
            || !source.is_char_boundary(edit.end)
            || edit.code.is_empty()
        {
            return Err(SourcePatchError(
                "source patch edits must be sorted, non-overlapping UTF-8 byte ranges",
            ));
        }
        output.push_str(&source[cursor..edit.start]);
        output.push_str(&edit.replacement);
        cursor = edit.end;
    }
    output.push_str(&source[cursor..]);
    Ok(output)
}
