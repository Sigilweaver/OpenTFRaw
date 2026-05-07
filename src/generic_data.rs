use crate::error::Result;
use crate::reader::BinaryReader;
use crate::types::GenericType;
use std::io::{Cursor, Read, Seek};

/// A field descriptor within a GenericDataHeader.
#[derive(Debug, Clone)]
pub struct GenericDataDescriptor {
    pub field_type: GenericType,
    pub length: u32,
    pub label: String,
}

/// Self-describing header for GenericRecord streams.
#[derive(Debug)]
pub struct GenericDataHeader {
    pub fields: Vec<GenericDataDescriptor>,
}

/// A typed value from a generic record.
#[derive(Debug, Clone)]
pub enum GenericValue {
    Gap,
    Int8(i8),
    Bool(bool),
    UInt8(u8),
    Int16(i16),
    UInt16(u16),
    Int32(i32),
    UInt32(u32),
    Float32(f32),
    Float64(f64),
    String(String),
}

/// A single record decoded using a GenericDataHeader.
#[derive(Debug)]
pub struct GenericRecord {
    pub values: Vec<(String, GenericValue)>,
}

impl GenericDataHeader {
    /// Try to read a GenericDataHeader. Returns `None` if the data at the
    /// current position does not look like a valid header (e.g. unreasonable
    /// field count or invalid type codes). The reader position is restored
    /// on failure.
    pub(crate) fn try_read<R: Read + Seek>(r: &mut BinaryReader<R>) -> Result<Option<Self>> {
        let saved_pos = r.position();
        let n = r.read_u32()?;
        // A genuine schema has at least a couple of fields and fewer than ~500.
        // The error-log "gap" region that precedes the schema in v64+ can
        // otherwise mislead us by looking like a 0- or 1-field header.
        if !(2..=500).contains(&n) {
            r.seek_to(saved_pos)?;
            return Ok(None);
        }
        let mut fields = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let type_code = r.read_u32()?;
            match GenericType::from_u32(type_code) {
                Some(field_type) => {
                    let length = r.read_u32()?;
                    // Character count of the label. Real Thermo labels are
                    // short and printable; require it to look sane or the
                    // whole header is bogus.
                    let label_start = r.position();
                    let char_count = r.read_u32()?;
                    if char_count > 200 {
                        r.seek_to(saved_pos)?;
                        return Ok(None);
                    }
                    r.seek_to(label_start)?;
                    let label = match r.read_pascal_string() {
                        Ok(s) => s,
                        Err(crate::error::Error::InvalidUtf16(_)) => {
                            r.seek_to(saved_pos)?;
                            return Ok(None);
                        }
                        Err(e) => return Err(e),
                    };
                    if !label_is_plausible(&label) {
                        r.seek_to(saved_pos)?;
                        return Ok(None);
                    }
                    fields.push(GenericDataDescriptor {
                        field_type,
                        length,
                        label,
                    });
                }
                None => {
                    r.seek_to(saved_pos)?;
                    return Ok(None);
                }
            }
        }
        let hdr = Self { fields };
        if !hdr.looks_meaningful() {
            r.seek_to(saved_pos)?;
            return Ok(None);
        }
        Ok(Some(hdr))
    }

    /// A schema is "meaningful" if it contains at least a few fields with
    /// real labels and has a non-trivial fixed record size. Used to reject
    /// false positives picked up by the forward scan.
    fn looks_meaningful(&self) -> bool {
        let named = self.fields.iter().filter(|f| !f.label.is_empty()).count();
        named >= 2 && self.fixed_record_size() > 0
    }

    /// Sum of fixed byte sizes contributed by each descriptor. For variable
    /// types (String/WideString) the descriptor's `length` field is used as
    /// the storage allocation — which is the fixed on-disk size per record.
    pub(crate) fn fixed_record_size(&self) -> usize {
        self.fields
            .iter()
            .map(|f| match f.field_type {
                GenericType::Gap => 0,
                GenericType::Int8
                | GenericType::Bool
                | GenericType::BoolYesNo
                | GenericType::BoolOnOff
                | GenericType::UInt8 => 1,
                GenericType::Int16 | GenericType::UInt16 => 2,
                GenericType::Int32 | GenericType::UInt32 | GenericType::Float32 => 4,
                GenericType::Float64 => 8,
                GenericType::AsciiString => f.length as usize,
                GenericType::WideString => f.length as usize * 2,
            })
            .sum()
    }

    /// Scan forward from the current position for a plausible GenericDataHeader
    /// in a bounded window. The v64+ error-log region contains padding bytes
    /// before the scan-parameters schema whose size isn't easily computed, so
    /// we locate the schema by scanning for a valid signature.
    pub(crate) fn find_forward<R: Read + Seek>(
        r: &mut BinaryReader<R>,
        max_scan: u64,
        expected_record_size: Option<usize>,
    ) -> Result<Option<Self>> {
        let start = r.position();
        let cap = max_scan.min(4 * 1024 * 1024) as usize;
        r.seek_to(start)?;
        let buf = r.read_bytes(cap)?;
        // Two passes: first require the schema's fixed record size to match
        // the tail; second accept any meaningful schema.
        //
        // Parse candidates entirely from the in-memory buffer using a Cursor so
        // that we never seek the underlying file reader for each false positive.
        // This avoids O(n) file seeks when the error-log gap is large (>1 MB).
        for pass in 0..2 {
            let mut offset = 0usize;
            while offset + 4 <= buf.len() {
                let n = u32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap());
                if (2..=500).contains(&n) {
                    let mut cursor = BinaryReader::new(Cursor::new(&buf[offset..]));
                    if let Some(hdr) = Self::try_read(&mut cursor)? {
                        let size_ok = match (pass, expected_record_size) {
                            (0, Some(want)) => hdr.fixed_record_size() == want,
                            _ => true,
                        };
                        if size_ok {
                            return Ok(Some(hdr));
                        }
                    }
                }
                offset += 2;
            }
            if expected_record_size.is_none() {
                break;
            }
        }
        r.seek_to(start)?;
        Ok(None)
    }
}

/// Heuristic: a GDH field label must either be empty or have reasonable
/// length. Labels are sometimes short single-character sentinels so we
/// don't require printability.
fn label_is_plausible(s: &str) -> bool {
    s.len() <= 200
}

impl GenericRecord {
    pub(crate) fn read<R: Read + Seek>(
        r: &mut BinaryReader<R>,
        header: &GenericDataHeader,
    ) -> Result<Self> {
        let mut values = Vec::with_capacity(header.fields.len());
        for desc in &header.fields {
            let label = desc.label.clone();
            let value = match desc.field_type {
                GenericType::Gap => GenericValue::Gap,
                GenericType::Int8 => GenericValue::Int8(r.read_i8()?),
                GenericType::Bool | GenericType::BoolYesNo | GenericType::BoolOnOff => {
                    GenericValue::Bool(r.read_u8()? != 0)
                }
                GenericType::UInt8 => GenericValue::UInt8(r.read_u8()?),
                GenericType::Int16 => GenericValue::Int16(r.read_i16()?),
                GenericType::UInt16 => GenericValue::UInt16(r.read_u16()?),
                GenericType::Int32 => GenericValue::Int32(r.read_i32()?),
                GenericType::UInt32 => GenericValue::UInt32(r.read_u32()?),
                GenericType::Float32 => GenericValue::Float32(r.read_f32()?),
                GenericType::Float64 => GenericValue::Float64(r.read_f64()?),
                GenericType::AsciiString => {
                    let s = if desc.length > 0 {
                        let bytes = r.read_bytes(desc.length as usize)?;
                        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
                        String::from_utf8_lossy(&bytes[..end]).into_owned()
                    } else {
                        String::new()
                    };
                    GenericValue::String(s)
                }
                GenericType::WideString => {
                    let s = if desc.length > 0 {
                        r.read_utf16_fixed(desc.length as usize * 2)?
                    } else {
                        String::new()
                    };
                    GenericValue::String(s)
                }
            };
            values.push((label, value));
        }
        Ok(Self { values })
    }

    /// Look up a field by label and return a reference to its value.
    pub fn get(&self, label: &str) -> Option<&GenericValue> {
        self.values.iter().find(|(l, _)| l == label).map(|(_, v)| v)
    }

    /// Get a float64 field by label.
    pub fn get_f64(&self, label: &str) -> Option<f64> {
        match self.get(label)? {
            GenericValue::Float64(v) => Some(*v),
            GenericValue::Float32(v) => Some(*v as f64),
            _ => None,
        }
    }

    /// Get a float32 field by label.
    pub fn get_f32(&self, label: &str) -> Option<f32> {
        match self.get(label)? {
            GenericValue::Float32(v) => Some(*v),
            GenericValue::Float64(v) => Some(*v as f32),
            _ => None,
        }
    }

    /// Get an i32 field by label.
    pub fn get_i32(&self, label: &str) -> Option<i32> {
        match self.get(label)? {
            GenericValue::Int32(v) => Some(*v),
            GenericValue::Int16(v) => Some(*v as i32),
            GenericValue::Int8(v) => Some(*v as i32),
            _ => None,
        }
    }

    /// Get a string field by label.
    pub fn get_string(&self, label: &str) -> Option<&str> {
        match self.get(label)? {
            GenericValue::String(v) => Some(v.as_str()),
            _ => None,
        }
    }
}

impl GenericValue {
    /// Get as f64, converting numeric types.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float64(v) => Some(*v),
            Self::Float32(v) => Some(*v as f64),
            Self::Int32(v) => Some(*v as f64),
            Self::UInt32(v) => Some(*v as f64),
            Self::Int16(v) => Some(*v as f64),
            Self::UInt16(v) => Some(*v as f64),
            Self::Int8(v) => Some(*v as f64),
            Self::UInt8(v) => Some(*v as f64),
            _ => None,
        }
    }
}
