# Sample and Sequence Information

_SeqRow, InjectionData, ASInfo_

## 6. SeqRow

The Sequence Row contains sample identification and method file references from
the Xcalibur sequence table. Immediately follows the FileHeader.

### 6.1 Structure (Version 66)

| Order | Type | Field | Description |
|-------|------|-------|-------------|
| 1 | InjectionData | injection | Injection parameters (64 bytes, see §7) |
| 2 | PascalStringWin32 | unknown_text[a] | Usually empty |
| 3 | PascalStringWin32 | unknown_text[b] | Usually empty |
| 4 | PascalStringWin32 | id | Sample/sequence ID |
| 5 | PascalStringWin32 | comment | Sample comment (e.g. column description) |
| 6 | PascalStringWin32 | user_label[1] | User-defined label 1 |
| 7 | PascalStringWin32 | user_label[2] | User-defined label 2 |
| 8 | PascalStringWin32 | user_label[3] | User-defined label 3 |
| 9 | PascalStringWin32 | user_label[4] | User-defined label 4 |
| 10 | PascalStringWin32 | user_label[5] | User-defined label 5 |
| 11 | PascalStringWin32 | inst_method | Instrument method file path |
| 12 | PascalStringWin32 | proc_method | Processing method file path |
| 13 | PascalStringWin32 | file_name | Original RAW file path on acquisition system |
| 14 | PascalStringWin32 | path | Directory path of RAW file |
| 15 | PascalStringWin32 | vial | Autosampler vial position |
| 16 | PascalStringWin32 | unknown_text[c] | Usually empty |
| 17 | PascalStringWin32 | unknown_text[d] | Usually empty |
| 18 | UInt32 | unknown_long | |
| 19-33 | PascalStringWin32 | unknown_text[e-s] | 15 additional text fields (usually empty) |

### 6.2 Version Differences

| Version | Extra fields after common 14 |
|---------|------|
| v8 | None |
| v47, v57 | vial + unk_c + unk_d + unknown_long |
| v60-v66 | vial + unk_c + unk_d + unknown_long + unk_e through unk_s (15 more strings) |

---

## 7. InjectionData

The first sub-structure within SeqRow. Fixed size.

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| 0x00 | 4 | UInt32 | unknown_long[1] | |
| 0x04 | 4 | UInt32 | n | Sequence table row number (0-based in some versions, 1-based in others) |
| 0x08 | 4 | UInt32 | unknown_long[2] | |
| 0x0C | 12 | UTF16LE(12) | vial | Vial identifier (6 chars max) |
| 0x18 | 8 | Float64 | injection_volume | Injection volume (µL) |
| 0x20 | 8 | Float64 | weight | Sample weight |
| 0x28 | 8 | Float64 | volume | Sample volume |
| 0x30 | 8 | Float64 | istd_amount | Internal standard amount |
| 0x38 | 8 | Float64 | dilution_factor | Dilution factor |

**Total size**: 12 + 12 + 40 = **64 bytes**

---

## 8. ASInfo

Autosampler information. Immediately follows SeqRow.

| Order | Type | Field | Description |
|-------|------|-------|-------------|
| 1 | ASInfoPreamble | preamble | 24 bytes of numeric parameters |
| 2 | PascalStringWin32 | text | Autosampler tray description (e.g. "6x8 vials", "ANSI-48Vial2mLHolder/ANSI-48Vial2mLHolder") |

### 8.1 ASInfoPreamble

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0x00 | 4 | UInt32 | unknown_long[1] |
| 0x04 | 4 | UInt32 | unknown_long[2] |
| 0x08 | 4 | UInt32 | number_of_wells |
| 0x0C | 4 | UInt32 | unknown_long[3] |
| 0x10 | 4 | UInt32 | unknown_long[4] |
| 0x14 | 4 | UInt32 | unknown_long[5] |

**Total size**: 24 bytes

---

