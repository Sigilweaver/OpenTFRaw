use crate::error::Result;
use crate::reader::BinaryReader;
use std::io::{Read, Seek};

/// Precursor reaction info for MS2+ scans (32 bytes).
#[derive(Debug)]
pub struct Reaction {
    pub precursor_mz: f64,
    pub unknown_double: f64,
    pub energy: f64,
    pub unknown_long1: u32,
    pub unknown_long2: u32,
}

/// M/z acquisition range.
#[derive(Debug)]
pub struct FractionCollector {
    pub low_mz: f64,
    pub high_mz: f64,
}

/// Scan event preamble — byte array encoding scan parameters.
#[derive(Debug)]
pub struct ScanEventPreamble {
    pub bytes: Vec<u8>,
}

/// Complete scan event.
#[derive(Debug)]
pub struct ScanEvent {
    pub preamble: ScanEventPreamble,
    pub reactions: Vec<Reaction>,
    pub fraction_collectors: Vec<FractionCollector>,
    pub coefficients: Vec<f64>,
}

impl ScanEventPreamble {
    fn size_for_version(version: u32) -> usize {
        match version {
            0..=8 => 41,
            57 | 60 => 80,
            62 => 120,
            63 | 64 => 128,
            _ => 136, // v66+
        }
    }

    /// Polarity: byte 4.
    pub fn polarity(&self) -> Option<crate::Polarity> {
        self.bytes
            .get(4)
            .and_then(|&b| crate::Polarity::from_byte(b))
    }

    /// Scan mode (centroid/profile): byte 5.
    pub fn scan_mode(&self) -> Option<crate::ScanMode> {
        self.bytes
            .get(5)
            .and_then(|&b| crate::ScanMode::from_byte(b))
    }

    /// MS power: byte 6.
    pub fn ms_power(&self) -> Option<crate::MsPower> {
        self.bytes
            .get(6)
            .and_then(|&b| crate::MsPower::from_byte(b))
    }

    /// Scan type: byte 7.
    pub fn scan_type(&self) -> Option<crate::ScanType> {
        self.bytes
            .get(7)
            .and_then(|&b| crate::ScanType::from_byte(b))
    }

    /// Dependent scan flag: byte 10.
    pub fn is_dependent(&self) -> bool {
        self.bytes.get(10).copied() == Some(1)
    }

    /// Ionization mode: byte 11.
    pub fn ionization(&self) -> Option<crate::Ionization> {
        self.bytes
            .get(11)
            .and_then(|&b| crate::Ionization::from_byte(b))
    }

    /// Activation method: byte 24.
    pub fn activation(&self) -> Option<crate::Activation> {
        self.bytes
            .get(24)
            .and_then(|&b| crate::Activation::from_byte(b))
    }

    /// Analyzer type: byte 40.
    pub fn analyzer(&self) -> Option<crate::Analyzer> {
        self.bytes
            .get(40)
            .and_then(|&b| crate::Analyzer::from_byte(b))
    }
}

impl ScanEvent {
    pub(crate) fn read<R: Read + Seek>(r: &mut BinaryReader<R>, version: u32) -> Result<Self> {
        let preamble_size = ScanEventPreamble::size_for_version(version);
        let preamble_bytes = r.read_bytes(preamble_size)?;
        let preamble = ScanEventPreamble {
            bytes: preamble_bytes,
        };

        if version >= 66 {
            Self::read_v66(r, preamble)
        } else {
            Self::read_pre_v66(r, preamble)
        }
    }

    /// V66 scan events are fixed-size: 136-byte preamble + 96-byte body.
    /// Body layout depends on n_reactions at body+0:
    ///   MS1 (n_reactions=0): FC at body+0x08
    ///   MS2 (n_reactions>0): Reaction at body+0x04 (32 bytes each), FC at body+0x40
    fn read_v66<R: Read + Seek>(
        r: &mut BinaryReader<R>,
        preamble: ScanEventPreamble,
    ) -> Result<Self> {
        const BODY_SIZE: usize = 96;
        let body = r.read_bytes(BODY_SIZE)?;

        let n_reactions = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);

        let mut reactions = Vec::new();
        let mut fraction_collectors = Vec::new();

        if n_reactions > 0 {
            // Dependent scan (MS2+): Reaction at body+4, FC at body+0x40
            for i in 0..n_reactions as usize {
                let off = 4 + i * 32;
                if off + 32 <= BODY_SIZE {
                    let precursor_mz = f64::from_le_bytes(body[off..off + 8].try_into().unwrap());
                    let unknown_double =
                        f64::from_le_bytes(body[off + 8..off + 16].try_into().unwrap());
                    let energy = f64::from_le_bytes(body[off + 16..off + 24].try_into().unwrap());
                    let unknown_long1 =
                        u32::from_le_bytes(body[off + 24..off + 28].try_into().unwrap());
                    let unknown_long2 =
                        u32::from_le_bytes(body[off + 28..off + 32].try_into().unwrap());
                    reactions.push(Reaction {
                        precursor_mz,
                        unknown_double,
                        energy,
                        unknown_long1,
                        unknown_long2,
                    });
                }
            }
            if BODY_SIZE >= 0x50 {
                let low_mz = f64::from_le_bytes(body[0x40..0x48].try_into().unwrap());
                let high_mz = f64::from_le_bytes(body[0x48..0x50].try_into().unwrap());
                fraction_collectors.push(FractionCollector { low_mz, high_mz });
            }
        } else {
            // Primary scan (MS1): FC at body+0x08
            let low_mz = f64::from_le_bytes(body[0x08..0x10].try_into().unwrap());
            let high_mz = f64::from_le_bytes(body[0x10..0x18].try_into().unwrap());
            fraction_collectors.push(FractionCollector { low_mz, high_mz });
        }

        Ok(Self {
            preamble,
            reactions,
            fraction_collectors,
            coefficients: Vec::new(),
        })
    }

    fn read_pre_v66<R: Read + Seek>(
        r: &mut BinaryReader<R>,
        preamble: ScanEventPreamble,
    ) -> Result<Self> {
        let np = r.read_u32()?;
        let mut reactions = Vec::new();
        for _ in 0..np {
            reactions.push(Reaction::read(r)?);
        }
        let _unk1 = r.read_u32()?;
        let fc = FractionCollector::read(r)?;
        let nparam = r.read_u32()?;
        let mut coefficients = Vec::with_capacity(nparam as usize);
        for _ in 0..nparam {
            coefficients.push(r.read_f64()?);
        }
        let _unk2 = r.read_u32()?;
        let _unk3 = r.read_u32()?;

        Ok(Self {
            preamble,
            reactions,
            fraction_collectors: vec![fc],
            coefficients,
        })
    }
}

impl Reaction {
    fn read<R: Read + Seek>(r: &mut BinaryReader<R>) -> Result<Self> {
        let precursor_mz = r.read_f64()?;
        let unknown_double = r.read_f64()?;
        let energy = r.read_f64()?;
        let unknown_long1 = r.read_u32()?;
        let unknown_long2 = r.read_u32()?;
        Ok(Self {
            precursor_mz,
            unknown_double,
            energy,
            unknown_long1,
            unknown_long2,
        })
    }
}

impl FractionCollector {
    fn read<R: Read + Seek>(r: &mut BinaryReader<R>) -> Result<Self> {
        let low_mz = r.read_f64()?;
        let high_mz = r.read_f64()?;
        Ok(Self { low_mz, high_mz })
    }
}
