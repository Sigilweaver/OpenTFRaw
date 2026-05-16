//! Instrument model + device family detection.
//!
//! Thermo RAW files embed the instrument model as a UTF-16LE string in the
//! metadata region that precedes the scan data (typically within the first
//! 8–16 KB of the file, inside the run-header prologue or instrument method
//! block).
//!
//! We detect the instrument by scanning that region for canonical model
//! names, preferring longer (more specific) matches over shorter ones so
//! that e.g. "Orbitrap Fusion Lumos" wins over "Orbitrap Fusion".
//!
//! This is more reliable than metadata like the audit-tag strings, which
//! frequently contain usernames or generic labels ("Thermo", "SYSTEM",
//! "admin") rather than the instrument model.
//!
//! The fallback path uses the audit-tag + instrument-method-path heuristic
//! and then the first-scan analyzer type as a last resort.

use crate::Analyzer;

/// Coarse instrument-family classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceFamily {
    /// LCQ Classic/Deca/Advantage/Fleet — 3D ion trap (legacy).
    LcqIonTrap,
    /// LTQ / LTQ XL / LTQ Velos / LTQ Velos Pro — 2D linear ion trap.
    LtqIonTrap,
    /// LTQ FT — ion trap coupled with FTICR (pre-Orbitrap era).
    LtqFt,
    /// LTQ Orbitrap family — ion trap + Orbitrap hybrids
    /// (Classic / XL / Discovery / Velos / Elite).
    LtqOrbitrap,
    /// Q Exactive family — quadrupole + C-trap + Orbitrap
    /// (Q Exactive / Plus / HF / HF-X / UHMR). No ion trap.
    QOrbitrap,
    /// Tribrid Orbitrap — quadrupole + linear ion trap + Orbitrap
    /// (Fusion / Fusion Lumos / Eclipse / Ascend).
    Tribrid,
    /// Single-stage Q-Orbitrap with advanced scan modes
    /// (Orbitrap Exploris 120 / 240 / 480).
    ExplorisOrbitrap,
    /// Orbitrap Astral hybrid — Orbitrap plus asymmetric-track lossless analyzer.
    OrbitrapAstral,
    /// Triple quadrupole — TSQ Vantage / Quantum / Quantiva / Altis / Endura.
    TripleQuad,
    /// Unknown / undetected.
    Unknown,
}

impl DeviceFamily {
    /// Human-readable family name.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::LcqIonTrap => "LCQ (3D ion trap)",
            Self::LtqIonTrap => "LTQ (linear ion trap)",
            Self::LtqFt => "LTQ FT (ion trap + FTICR)",
            Self::LtqOrbitrap => "LTQ Orbitrap hybrid",
            Self::QOrbitrap => "Q-Orbitrap (Q Exactive)",
            Self::Tribrid => "Tribrid Orbitrap",
            Self::ExplorisOrbitrap => "Orbitrap Exploris",
            Self::OrbitrapAstral => "Orbitrap Astral",
            Self::TripleQuad => "Triple quadrupole",
            Self::Unknown => "unknown",
        }
    }

    /// Whether the device is expected to use the flat-peaks (SRM) scan layout.
    pub fn uses_flat_peaks(self) -> bool {
        matches!(self, Self::TripleQuad)
    }
}

/// Canonical instrument-model registry.
///
/// Ordered such that longer (more specific) names come BEFORE any name that
/// is a proper prefix, so e.g. "Orbitrap Fusion Lumos" is matched in
/// preference to "Orbitrap Fusion" when both appear in the scan window.
const MODEL_REGISTRY: &[(&str, DeviceFamily)] = &[
    // --- Orbitrap Astral ---
    ("Orbitrap Astral", DeviceFamily::OrbitrapAstral),
    // --- Tribrid Orbitrap ---
    ("Orbitrap Ascend", DeviceFamily::Tribrid),
    ("Orbitrap Fusion Lumos", DeviceFamily::Tribrid),
    ("Orbitrap Eclipse", DeviceFamily::Tribrid),
    ("Orbitrap Fusion", DeviceFamily::Tribrid),
    // --- Exploris single-stage Q-Orbitrap ---
    ("Orbitrap Exploris 480", DeviceFamily::ExplorisOrbitrap),
    ("Orbitrap Exploris 240", DeviceFamily::ExplorisOrbitrap),
    ("Orbitrap Exploris 120", DeviceFamily::ExplorisOrbitrap),
    ("Orbitrap Exploris MX", DeviceFamily::ExplorisOrbitrap),
    ("Orbitrap Exploris GC 240", DeviceFamily::ExplorisOrbitrap),
    ("Orbitrap Exploris", DeviceFamily::ExplorisOrbitrap),
    // --- Q Exactive ---
    ("Q Exactive HF-X", DeviceFamily::QOrbitrap),
    ("Q Exactive UHMR", DeviceFamily::QOrbitrap),
    ("Q Exactive Plus", DeviceFamily::QOrbitrap),
    ("Q Exactive HF", DeviceFamily::QOrbitrap),
    ("Q Exactive GC", DeviceFamily::QOrbitrap),
    ("Q Exactive Focus", DeviceFamily::QOrbitrap),
    ("Q Exactive", DeviceFamily::QOrbitrap),
    // --- LTQ Orbitrap family ---
    ("LTQ Orbitrap Velos Pro", DeviceFamily::LtqOrbitrap),
    ("LTQ Orbitrap Velos ETD", DeviceFamily::LtqOrbitrap),
    ("LTQ Orbitrap Velos", DeviceFamily::LtqOrbitrap),
    ("LTQ Orbitrap Elite", DeviceFamily::LtqOrbitrap),
    ("LTQ Orbitrap Discovery", DeviceFamily::LtqOrbitrap),
    ("LTQ Orbitrap XL ETD", DeviceFamily::LtqOrbitrap),
    ("LTQ Orbitrap XL", DeviceFamily::LtqOrbitrap),
    ("LTQ Orbitrap", DeviceFamily::LtqOrbitrap),
    ("Orbitrap Elite", DeviceFamily::LtqOrbitrap),
    ("Orbitrap Velos Pro", DeviceFamily::LtqOrbitrap),
    ("Orbitrap Velos", DeviceFamily::LtqOrbitrap),
    ("Orbitrap Discovery", DeviceFamily::LtqOrbitrap),
    ("Orbitrap XL", DeviceFamily::LtqOrbitrap),
    // --- LTQ FT ---
    ("LTQ FT Ultra", DeviceFamily::LtqFt),
    ("LTQ FT", DeviceFamily::LtqFt),
    // --- LTQ linear ion trap ---
    ("LTQ Velos Pro", DeviceFamily::LtqIonTrap),
    ("LTQ Velos ETD", DeviceFamily::LtqIonTrap),
    ("LTQ Velos", DeviceFamily::LtqIonTrap),
    ("LTQ XL ETD", DeviceFamily::LtqIonTrap),
    ("LTQ XL", DeviceFamily::LtqIonTrap),
    ("LTQ", DeviceFamily::LtqIonTrap),
    // --- LCQ 3D trap ---
    ("LCQ Fleet", DeviceFamily::LcqIonTrap),
    ("LCQ Advantage", DeviceFamily::LcqIonTrap),
    ("LCQ Deca XP Plus", DeviceFamily::LcqIonTrap),
    ("LCQ Deca XP", DeviceFamily::LcqIonTrap),
    ("LCQ Deca", DeviceFamily::LcqIonTrap),
    ("LCQ Classic", DeviceFamily::LcqIonTrap),
    ("LCQ DUO", DeviceFamily::LcqIonTrap),
    ("LCQ", DeviceFamily::LcqIonTrap),
    // --- TSQ triple-quadrupoles ---
    ("TSQ Quantiva", DeviceFamily::TripleQuad),
    ("TSQ Quantum Ultra AM", DeviceFamily::TripleQuad),
    ("TSQ Quantum Ultra", DeviceFamily::TripleQuad),
    ("TSQ Quantum Access", DeviceFamily::TripleQuad),
    ("TSQ Quantum Discovery", DeviceFamily::TripleQuad),
    ("TSQ Quantum", DeviceFamily::TripleQuad),
    ("TSQ Vantage", DeviceFamily::TripleQuad),
    ("TSQ Endura", DeviceFamily::TripleQuad),
    ("TSQ Altis Plus", DeviceFamily::TripleQuad),
    ("TSQ Altis", DeviceFamily::TripleQuad),
    ("TSQ 8000 Evo", DeviceFamily::TripleQuad),
    ("TSQ 9000", DeviceFamily::TripleQuad),
    ("TSQ", DeviceFamily::TripleQuad),
];

/// Encode a str as UTF-16LE bytes.
fn utf16le(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() * 2);
    for u in s.encode_utf16() {
        out.extend_from_slice(&u.to_le_bytes());
    }
    out
}

/// True if `needle` appears in `haystack` bounded by non-word characters on
/// both sides (word boundary). Used to reject partial matches such as
/// "LCQ" inside "LCQ Header" (a generic section marker present in every
/// Thermo RAW file) or inside temp-file paths like "LCQ0rpvfvu1.tmp".
///
/// Since the file is UTF-16LE, a character is ASCII-alphanumeric iff its
/// low byte is alphanumeric and its high byte is 0. We treat everything
/// else (nulls, separators, non-ASCII) as a word boundary.
fn contains_word(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    let is_word_byte = |b: u8| b.is_ascii_alphanumeric();
    let is_word_char_at = |pos: usize| -> bool {
        // UTF-16LE code unit at `pos` (must be aligned). An ASCII-alphanumeric
        // code unit has high byte 0 and low byte in [0-9A-Za-z].
        if pos + 1 >= haystack.len() {
            return false;
        }
        haystack[pos + 1] == 0 && is_word_byte(haystack[pos])
    };

    for start in 0..=haystack.len() - needle.len() {
        if &haystack[start..start + needle.len()] != needle {
            continue;
        }
        // Left boundary: either at start of buffer, or previous UTF-16
        // code unit is not an ASCII-alphanumeric character.
        let left_ok = start < 2 || !is_word_char_at(start - 2);
        // Right boundary: either at end, or next UTF-16 code unit is not
        // ASCII-alphanumeric. Note: the match may be followed by a space
        // (e.g. "LCQ Header") — that's still a word boundary on the
        // needle's right edge, which is what we want.
        //
        // However for "LCQ" we want to REJECT "LCQ Header" because
        // "Header" is a generic section marker, not an instrument qualifier.
        // The word-boundary test alone accepts "LCQ " — to disambiguate
        // we also require: if the needle is a known ambiguous prefix
        // (LCQ/LTQ/TSQ with no model suffix), reject matches where the
        // next token is "Header".
        let after = start + needle.len();
        let right_ok = after >= haystack.len() || !is_word_char_at(after);

        if left_ok && right_ok {
            // Guard against "<PREFIX> Header" — a universal section marker.
            const HEADER_SUFFIX: &[u8] = b" \0H\0e\0a\0d\0e\0r\0";
            if haystack.len() >= after + HEADER_SUFFIX.len()
                && &haystack[after..after + HEADER_SUFFIX.len()] == HEADER_SUFFIX
            {
                continue;
            }
            return true;
        }
    }
    false
}

/// Detection result carrying the exact model (when found) and the coarse
/// family.
#[derive(Debug, Clone)]
pub struct DetectedInstrument {
    /// Exact canonical model name, e.g. "Orbitrap Fusion Lumos", if detected.
    pub model: Option<&'static str>,
    /// Coarse device family. Always populated (possibly `Unknown`).
    pub family: DeviceFamily,
}

impl DetectedInstrument {
    /// Unknown / undetected instrument.
    pub const fn unknown() -> Self {
        Self {
            model: None,
            family: DeviceFamily::Unknown,
        }
    }
}

impl DeviceFamily {
    /// Scan `metadata_bytes` (a raw prefix of the RAW file) for a canonical
    /// Thermo instrument model encoded as UTF-16LE, then fall back to the
    /// heuristic over `tag2` + `inst_method` + `first_analyzer`.
    pub fn detect_instrument(
        metadata_bytes: &[u8],
        tag2: &str,
        inst_method: &str,
        first_analyzer: Option<Analyzer>,
    ) -> DetectedInstrument {
        for (name, family) in MODEL_REGISTRY {
            let needle = utf16le(name);
            if contains_word(metadata_bytes, &needle) {
                return DetectedInstrument {
                    model: Some(name),
                    family: *family,
                };
            }
        }

        let family = Self::detect_heuristic(tag2, inst_method, first_analyzer);
        DetectedInstrument {
            model: None,
            family,
        }
    }

    /// Keyword heuristic over audit-tag + method path, with analyzer-type
    /// fallback. Retained as a secondary path when no model string is found.
    pub fn detect_heuristic(
        tag2: &str,
        inst_method: &str,
        first_analyzer: Option<Analyzer>,
    ) -> Self {
        let hay = format!("{} {}", tag2, inst_method).to_lowercase();

        let patterns: &[(&str, DeviceFamily)] = &[
            ("astral", Self::OrbitrapAstral),
            ("exploris", Self::ExplorisOrbitrap),
            ("ascend", Self::Tribrid),
            ("eclipse", Self::Tribrid),
            ("lumos", Self::Tribrid),
            ("fusion", Self::Tribrid),
            ("tribrid", Self::Tribrid),
            ("q exactive", Self::QOrbitrap),
            ("qexactive", Self::QOrbitrap),
            ("q-exactive", Self::QOrbitrap),
            ("exactive", Self::QOrbitrap),
            ("altis", Self::TripleQuad),
            ("quantiva", Self::TripleQuad),
            ("endura", Self::TripleQuad),
            ("vantage", Self::TripleQuad),
            ("tsq", Self::TripleQuad),
            ("ltq orbitrap", Self::LtqOrbitrap),
            ("ltq-orbitrap", Self::LtqOrbitrap),
            ("ltq_orbitrap", Self::LtqOrbitrap),
            ("orbitrap elite", Self::LtqOrbitrap),
            ("orbitrap velos", Self::LtqOrbitrap),
            ("orbitrap xl", Self::LtqOrbitrap),
            ("orbitrap discovery", Self::LtqOrbitrap),
            ("ltq ft", Self::LtqFt),
            ("ltq-ft", Self::LtqFt),
            ("ltqft", Self::LtqFt),
            ("ltq velos", Self::LtqIonTrap),
            ("ltq-velos", Self::LtqIonTrap),
            ("ltqvelos", Self::LtqIonTrap),
            ("ltq xl", Self::LtqIonTrap),
            ("ltqxl", Self::LtqIonTrap),
            ("ltq", Self::LtqIonTrap),
            ("lcq", Self::LcqIonTrap),
            ("orbitrap", Self::LtqOrbitrap),
        ];

        for (needle, family) in patterns {
            if hay.contains(needle) {
                return *family;
            }
        }

        match first_analyzer {
            Some(Analyzer::FTMS) => Self::LtqOrbitrap,
            Some(Analyzer::ITMS) => Self::LtqIonTrap,
            Some(Analyzer::TQMS) => Self::TripleQuad,
            Some(Analyzer::SQMS) => Self::TripleQuad,
            _ => Self::Unknown,
        }
    }

    /// Legacy compatibility wrapper (no metadata byte window).
    #[deprecated(
        note = "Use `DeviceFamily::detect_instrument` with the metadata byte window for reliable detection"
    )]
    pub fn detect(tag2: &str, inst_method: &str, first_analyzer: Option<Analyzer>) -> Self {
        Self::detect_heuristic(tag2, inst_method, first_analyzer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(s: &str) -> Vec<u8> {
        utf16le(s)
    }

    #[test]
    fn prefers_longer_model_names() {
        let mut hay = Vec::new();
        hay.extend_from_slice(&encode("Orbitrap Fusion Lumos"));
        hay.extend_from_slice(b"\x00\x00");
        hay.extend_from_slice(&encode("Orbitrap Fusion"));
        let det = DeviceFamily::detect_instrument(&hay, "", "", None);
        assert_eq!(det.model, Some("Orbitrap Fusion Lumos"));
        assert_eq!(det.family, DeviceFamily::Tribrid);
    }

    #[test]
    fn astral_wins_over_orbitrap() {
        let hay = encode("Orbitrap Astral");
        let det = DeviceFamily::detect_instrument(&hay, "", "", None);
        assert_eq!(det.model, Some("Orbitrap Astral"));
        assert_eq!(det.family, DeviceFamily::OrbitrapAstral);
    }

    #[test]
    fn falls_back_to_heuristic() {
        let det = DeviceFamily::detect_instrument(b"", "Q Exactive HF", "", None);
        assert_eq!(det.model, None);
        assert_eq!(det.family, DeviceFamily::QOrbitrap);
    }

    #[test]
    fn falls_back_to_analyzer() {
        let det = DeviceFamily::detect_instrument(b"", "", "", Some(Analyzer::TQMS));
        assert_eq!(det.family, DeviceFamily::TripleQuad);
    }

    #[test]
    fn unknown_when_no_signal() {
        let det = DeviceFamily::detect_instrument(b"", "", "", None);
        assert_eq!(det.family, DeviceFamily::Unknown);
    }

    #[test]
    fn rejects_lcq_header_section_marker() {
        // "LCQ Header" is a section marker present in every Thermo RAW file.
        // Matching bare "LCQ" against it would misclassify every non-LCQ
        // instrument as an LCQ.
        let hay = encode("LCQ Header");
        let det = DeviceFamily::detect_instrument(&hay, "", "", None);
        assert_eq!(det.model, None);
    }

    #[test]
    fn rejects_lcq_inside_tmp_path() {
        // Thermo temp files are named like "LCQ0rpvfvu1.tmp".
        let hay = encode("C:\\ProgramData\\Thermo\\Temp\\LCQ0rpvfvu1.tmp");
        let det = DeviceFamily::detect_instrument(&hay, "", "", None);
        assert_eq!(det.model, None);
    }

    #[test]
    fn accepts_lcq_classic_on_header_follow() {
        // "LCQ Classic" should still match even if "LCQ Header" is elsewhere.
        let mut hay = Vec::new();
        hay.extend_from_slice(&encode("LCQ Header\0"));
        hay.extend_from_slice(&encode(" some text "));
        hay.extend_from_slice(&encode("LCQ Classic"));
        hay.extend_from_slice(b"\x00\x00");
        let det = DeviceFamily::detect_instrument(&hay, "", "", None);
        assert_eq!(det.model, Some("LCQ Classic"));
    }
}
