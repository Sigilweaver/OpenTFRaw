//! Device family detection.
//!
//! A best-effort classification of the instrument that produced a RAW file.
//! This is informational only — the scan-data format routing lives in
//! [`crate::scan_format`] and does not depend on the device family.
//!
//! Detection sources (in priority order):
//! 1. `audit_tag2` keywords (most reliable when present)
//! 2. `seq_row.inst_method` path (often contains the instrument name)
//! 3. First-scan analyzer type hint
//!
//! When no clear signal is found, the family falls back to [`DeviceFamily::Unknown`].

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

    /// Detect from available metadata.
    ///
    /// `tag2` is the FileHeader audit-tag2 string (often the instrument model
    /// or a username). `inst_method` is the `.meth` file path stored in SeqRow.
    /// `first_analyzer` is the analyzer type from the first scan event, if any.
    pub fn detect(tag2: &str, inst_method: &str, first_analyzer: Option<Analyzer>) -> Self {
        // Combine candidates into one lowercase haystack.
        // inst_method paths often look like: C:\Xcalibur\methods\...\something.meth
        let hay = format!("{} {}", tag2, inst_method).to_lowercase();

        // Order matters: more specific patterns first, because many names
        // contain substrings of broader families
        // (e.g. "Orbitrap Fusion Lumos" contains "orbitrap fusion").
        let patterns: &[(&str, DeviceFamily)] = &[
            // — Astral first (most specific) —
            ("astral", Self::OrbitrapAstral),
            // — Exploris —
            ("exploris", Self::ExplorisOrbitrap),
            // — Tribrid —
            ("ascend", Self::Tribrid),
            ("eclipse", Self::Tribrid),
            ("lumos", Self::Tribrid),
            ("fusion", Self::Tribrid),
            ("tribrid", Self::Tribrid),
            // — Q-Orbitrap —
            ("q exactive", Self::QOrbitrap),
            ("qexactive", Self::QOrbitrap),
            ("q-exactive", Self::QOrbitrap),
            ("exactive", Self::QOrbitrap),
            // — Triple quads —
            ("altis", Self::TripleQuad),
            ("quantiva", Self::TripleQuad),
            ("endura", Self::TripleQuad),
            ("vantage", Self::TripleQuad),
            ("tsq", Self::TripleQuad),
            // — LTQ Orbitrap (must come before plain LTQ) —
            ("ltq orbitrap", Self::LtqOrbitrap),
            ("ltq-orbitrap", Self::LtqOrbitrap),
            ("ltq_orbitrap", Self::LtqOrbitrap),
            ("orbitrap elite", Self::LtqOrbitrap),
            ("orbitrap velos", Self::LtqOrbitrap),
            ("orbitrap xl", Self::LtqOrbitrap),
            ("orbitrap discovery", Self::LtqOrbitrap),
            // — LTQ FT —
            ("ltq ft", Self::LtqFt),
            ("ltq-ft", Self::LtqFt),
            ("ltqft", Self::LtqFt),
            // — LTQ ion trap —
            ("ltq velos", Self::LtqIonTrap),
            ("ltq-velos", Self::LtqIonTrap),
            ("ltqvelos", Self::LtqIonTrap),
            ("ltq xl", Self::LtqIonTrap),
            ("ltqxl", Self::LtqIonTrap),
            ("ltq", Self::LtqIonTrap),
            // — LCQ —
            ("lcq", Self::LcqIonTrap),
            // — Bare "orbitrap" — generic, least specific —
            ("orbitrap", Self::LtqOrbitrap),
        ];

        for (needle, family) in patterns {
            if hay.contains(needle) {
                return *family;
            }
        }

        // Last-resort: use the first scan's analyzer type.
        match first_analyzer {
            Some(Analyzer::FTMS) => Self::LtqOrbitrap,
            Some(Analyzer::ITMS) => Self::LtqIonTrap,
            Some(Analyzer::TQMS) => Self::TripleQuad,
            Some(Analyzer::SQMS) => Self::TripleQuad,
            _ => Self::Unknown,
        }
    }
}
