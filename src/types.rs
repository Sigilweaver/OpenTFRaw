//! Shared type aliases and enumerations used throughout the parser.

/// Analyzer type from ScanEventPreamble byte 40.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Analyzer {
    ITMS = 0,
    TQMS = 1,
    SQMS = 2,
    TOFMS = 3,
    FTMS = 4,
    Sector = 5,
}

/// Polarity from ScanEventPreamble byte 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Polarity {
    Negative = 0,
    Positive = 1,
}

/// Scan mode from ScanEventPreamble byte 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ScanMode {
    Centroid = 0,
    Profile = 1,
}

/// MS power (MSn order) from ScanEventPreamble byte 6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MsPower {
    Undefined = 0,
    Ms1 = 1,
    Ms2 = 2,
    Ms3 = 3,
    Ms4 = 4,
    Ms5 = 5,
    Ms6 = 6,
    Ms7 = 7,
    Ms8 = 8,
}

/// Scan type from ScanEventPreamble byte 7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ScanType {
    Full = 0,
    Zoom = 1,
    Sim = 2,
    Srm = 3,
    Crm = 4,
    Undefined = 5,
    Q1 = 6,
    Q3 = 7,
}

/// Ionization mode from ScanEventPreamble byte 11.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Ionization {
    EI = 0,
    CI = 1,
    FABI = 2,
    ESI = 3,
    APCI = 4,
    NSI = 5,
    TSI = 6,
    FDI = 7,
    MALDI = 8,
    GDI = 9,
}

/// Activation method from ScanEventPreamble byte 24.
///
/// Observed corpus values:
///   0 = no activation (MS1 scans; treated as None by `from_byte`)
///   1 = HCD (Q Exactive family; renders as "hcd")
///   4 = CID/HCD (Fusion/Exploris/Eclipse; renders as "cid" on ITMS, "hcd" on FTMS)
///
/// Values 2, 3, 5-10, 12 are defined from Xcalibur firmware documentation
/// but not yet confirmed across the current corpus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activation {
    /// Multi-photon induced dissociation (code 2).
    MPID,
    /// Electron transfer dissociation (code 3).
    ETD,
    /// Higher-energy collisional dissociation — Q Exactive style (code 1).
    HCD,
    /// Collision-induced dissociation / beam-type HCD — Fusion/Exploris style (code 4).
    /// Renders as "cid" on ITMS analyzers, "hcd" on FTMS analyzers.
    CID,
    /// Electron-capture dissociation (code 5).
    ECD,
    /// Infrared multiphoton dissociation (code 6).
    IRMPD,
    /// Proton transfer decay / activated-ion ETD (code 7).
    PD,
    /// Pulsed q dissociation (code 8).
    PQD,
    /// Ultraviolet photodissociation (code 9).
    UVPD,
    /// Surface-induced dissociation (code 10).
    SID,
    /// ETD with supplemental HCD (code 12).
    /// Filter string: `@etd<e>@hcd<se>` — two activation clauses.
    EThcD,
}

/// Generic data field type codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum GenericType {
    Gap = 0x0,
    Int8 = 0x1,
    Bool = 0x2,
    BoolYesNo = 0x3,
    BoolOnOff = 0x4,
    UInt8 = 0x5,
    Int16 = 0x6,
    UInt16 = 0x7,
    Int32 = 0x8,
    UInt32 = 0x9,
    Float32 = 0xA,
    Float64 = 0xB,
    AsciiString = 0xC,
    WideString = 0xD,
}

impl Analyzer {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0 => Some(Self::ITMS),
            1 => Some(Self::TQMS),
            2 => Some(Self::SQMS),
            3 => Some(Self::TOFMS),
            4 => Some(Self::FTMS),
            5 => Some(Self::Sector),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ITMS => "ITMS",
            Self::TQMS => "TQMS",
            Self::SQMS => "SQMS",
            Self::TOFMS => "TOFMS",
            Self::FTMS => "FTMS",
            Self::Sector => "Sector",
        }
    }
}

impl Polarity {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0 => Some(Self::Negative),
            1 => Some(Self::Positive),
            _ => None,
        }
    }

    pub fn symbol(&self) -> char {
        match self {
            Self::Negative => '-',
            Self::Positive => '+',
        }
    }
}

impl ScanMode {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0 => Some(Self::Centroid),
            1 => Some(Self::Profile),
            _ => None,
        }
    }

    pub fn symbol(&self) -> char {
        match self {
            Self::Centroid => 'c',
            Self::Profile => 'p',
        }
    }
}

impl MsPower {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0 => Some(Self::Undefined),
            1 => Some(Self::Ms1),
            2 => Some(Self::Ms2),
            3 => Some(Self::Ms3),
            4 => Some(Self::Ms4),
            5 => Some(Self::Ms5),
            6 => Some(Self::Ms6),
            7 => Some(Self::Ms7),
            8 => Some(Self::Ms8),
            _ => None,
        }
    }
}

impl ScanType {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0 => Some(Self::Full),
            1 => Some(Self::Zoom),
            2 => Some(Self::Sim),
            3 => Some(Self::Srm),
            4 => Some(Self::Crm),
            5 => Some(Self::Undefined),
            6 => Some(Self::Q1),
            7 => Some(Self::Q3),
            _ => None,
        }
    }
}

impl Ionization {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0 => Some(Self::EI),
            1 => Some(Self::CI),
            2 => Some(Self::FABI),
            3 => Some(Self::ESI),
            4 => Some(Self::APCI),
            5 => Some(Self::NSI),
            6 => Some(Self::TSI),
            7 => Some(Self::FDI),
            8 => Some(Self::MALDI),
            9 => Some(Self::GDI),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EI => "EI",
            Self::CI => "CI",
            Self::FABI => "FABI",
            Self::ESI => "ESI",
            Self::APCI => "APCI",
            Self::NSI => "NSI",
            Self::TSI => "TSI",
            Self::FDI => "FDI",
            Self::MALDI => "MALDI",
            Self::GDI => "GDI",
        }
    }
}

impl Activation {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            1 => Some(Self::HCD),
            2 => Some(Self::MPID),
            3 => Some(Self::ETD),
            4 => Some(Self::CID),
            5 => Some(Self::ECD),
            6 => Some(Self::IRMPD),
            7 => Some(Self::PD),
            8 => Some(Self::PQD),
            9 => Some(Self::UVPD),
            10 => Some(Self::SID),
            12 => Some(Self::EThcD),
            _ => None,
        }
    }

    /// Short identifier used in Thermo scan filter strings (e.g. "hcd", "etd").
    ///
    /// For [`Activation::CID`] on FTMS instruments the filter string conventionally
    /// uses "hcd" instead; callers should substitute via
    /// [`crate::scan_filter::activation_str`].
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HCD => "hcd",
            Self::MPID => "mpid",
            Self::ETD => "etd",
            Self::CID => "cid",
            Self::ECD => "ecd",
            Self::IRMPD => "irmpd",
            Self::PD => "pd",
            Self::PQD => "pqd",
            Self::UVPD => "uvpd",
            Self::SID => "sid",
            Self::EThcD => "ethcd",
        }
    }
}

impl GenericType {
    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            0x0 => Some(Self::Gap),
            0x1 => Some(Self::Int8),
            0x2 => Some(Self::Bool),
            0x3 => Some(Self::BoolYesNo),
            0x4 => Some(Self::BoolOnOff),
            0x5 => Some(Self::UInt8),
            0x6 => Some(Self::Int16),
            0x7 => Some(Self::UInt16),
            0x8 => Some(Self::Int32),
            0x9 => Some(Self::UInt32),
            0xA => Some(Self::Float32),
            0xB => Some(Self::Float64),
            0xC => Some(Self::AsciiString),
            0xD => Some(Self::WideString),
            _ => None,
        }
    }

    /// Size in bytes for fixed-size types. Returns None for strings and gaps.
    pub fn fixed_size(&self) -> Option<usize> {
        match self {
            Self::Gap => Some(0),
            Self::Int8 | Self::Bool | Self::BoolYesNo | Self::BoolOnOff | Self::UInt8 => Some(1),
            Self::Int16 | Self::UInt16 => Some(2),
            Self::Int32 | Self::UInt32 | Self::Float32 => Some(4),
            Self::Float64 => Some(8),
            Self::AsciiString | Self::WideString => None,
        }
    }
}
