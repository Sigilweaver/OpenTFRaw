use std::io::{Read, Seek};
use crate::reader::BinaryReader;
use crate::error::Result;

/// Injection data sub-structure within SeqRow (64 bytes).
#[derive(Debug)]
pub struct InjectionData {
    pub row_number: u32,
    pub vial: String,
    pub injection_volume: f64,
    pub sample_weight: f64,
    pub sample_volume: f64,
    pub istd_amount: f64,
    pub dilution_factor: f64,
}

/// Sequence table row.
#[derive(Debug)]
pub struct SeqRow {
    pub injection: InjectionData,
    pub id: String,
    pub comment: String,
    pub user_labels: Vec<String>,
    pub inst_method: String,
    pub proc_method: String,
    pub file_name: String,
    pub path: String,
    pub vial: String,
}

impl SeqRow {
    pub(crate) fn read<R: Read + Seek>(r: &mut BinaryReader<R>, version: u32) -> Result<Self> {
        // InjectionData: 64 bytes
        let _unk1 = r.read_u32()?;
        let row_number = r.read_u32()?;
        let _unk2 = r.read_u32()?;
        let vial_inj = r.read_utf16_fixed(12)?;
        let injection_volume = r.read_f64()?;
        let sample_weight = r.read_f64()?;
        let sample_volume = r.read_f64()?;
        let istd_amount = r.read_f64()?;
        let dilution_factor = r.read_f64()?;

        let injection = InjectionData {
            row_number,
            vial: vial_inj,
            injection_volume,
            sample_weight,
            sample_volume,
            istd_amount,
            dilution_factor,
        };

        // PascalString fields
        let _unk_a = r.read_pascal_string()?;
        let _unk_b = r.read_pascal_string()?;
        let id = r.read_pascal_string()?;
        let comment = r.read_pascal_string()?;
        let mut user_labels = Vec::new();
        for _ in 0..5 {
            user_labels.push(r.read_pascal_string()?);
        }
        let inst_method = r.read_pascal_string()?;
        let proc_method = r.read_pascal_string()?;
        let file_name = r.read_pascal_string()?;
        let path = r.read_pascal_string()?;
        let vial = r.read_pascal_string()?;

        if version >= 57 {
            // unk_c, unk_d
            let _unk_c = r.read_pascal_string()?;
            let _unk_d = r.read_pascal_string()?;
            let _unk_long = r.read_u32()?;
        }

        if version >= 60 {
            // 15 additional text fields (unk_e through unk_s)
            for _ in 0..15 {
                let _ = r.read_pascal_string()?;
            }
        }

        Ok(Self {
            injection,
            id,
            comment,
            user_labels,
            inst_method,
            proc_method,
            file_name,
            path,
            vial,
        })
    }
}
