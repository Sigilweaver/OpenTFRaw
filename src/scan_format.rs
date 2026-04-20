//! Scan-data format classification and dispatch.
//!
//! Thermo RAW files use one of three distinct scan-data layouts depending on
//! the file version and instrument type. This module classifies those layouts
//! and provides a single entry point ([`RawFileReader::read_scan_peaks`]) that
//! dispatches to the correct decoder.
//!
//! # Format matrix
//!
//! | Format           | Versions     | Instruments              | Record shape                |
//! |------------------|--------------|--------------------------|-----------------------------|
//! | `PacketHeader`   | 57, 63, 64, 66 | Orbitrap / ion trap / Q-Orbitrap | Variable, self-describing |
//! | `FlatV63`        | 63            | TSQ Vantage (SRM)        | Variable; offset = cumulative end |
//! | `FlatV66`        | 64, 66        | TSQ Quantiva / Altis (SRM) | Fixed size; offset = start, header+windows+peaks |

/// The scan-data encoding used by a RAW file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanDataFormat {
    /// Self-describing `PacketHeader` records with optional profile + centroid
    /// sections. Used by all ion-trap, Orbitrap, Q-Orbitrap and Tribrid instruments.
    PacketHeader,

    /// Flat peaks, variable record size, offsets are cumulative end positions.
    /// TSQ Vantage (file version 63).
    ///
    /// Each record ends in `peak_count` peaks of `(f32 mz, f32 intensity)`
    /// followed by `peak_count` flag bytes.
    FlatV63,

    /// Flat peaks, fixed record size, offsets are start positions.
    /// TSQ Quantiva / Altis (file version 64+).
    ///
    /// Layout per record:
    /// - bytes 0–3: `u32 n_peaks`
    /// - bytes 32..32+8·n: m/z window table (2 × f32 per channel)
    /// - bytes 32+8·n..: peak triplets `(u32 channel, f32 mz, f32 intensity)`
    FlatV66,
}

impl ScanDataFormat {
    /// Classify a RAW file's scan layout from its version and flat-peaks flag.
    ///
    /// `flat_peaks` is the heuristic computed during [`crate::RawFileReader::open`]:
    /// it is `true` when the RunHeader has no scan-event trailer, which is the
    /// signature of a triple-quadrupole SRM acquisition.
    pub fn detect(version: u32, flat_peaks: bool) -> Self {
        if !flat_peaks {
            return Self::PacketHeader;
        }
        if version <= 63 {
            Self::FlatV63
        } else {
            Self::FlatV66
        }
    }

    /// Whether this format uses the flat (SRM) peak layout.
    pub fn is_flat(self) -> bool {
        !matches!(self, Self::PacketHeader)
    }

    /// Human-readable name suitable for logging.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::PacketHeader => "PacketHeader",
            Self::FlatV63 => "FlatV63 (TSQ Vantage SRM)",
            Self::FlatV66 => "FlatV66 (TSQ Quantiva/Altis SRM)",
        }
    }
}
