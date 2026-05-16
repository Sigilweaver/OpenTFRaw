# File Layout

_File Layout, FileHeader, AuditTag_

## 3. File Layout

The file is organized as a sequence of structures, some at fixed positions
relative to the start and some at positions indicated by pointer fields in
earlier structures. The reading order is:

```
Offset 0x0000:
  ├── FileHeader                          (fixed size: 1356 bytes)
  ├── SeqRow                              (variable size)
  ├── ASInfo                              (variable size)
  ├── RawFileInfo                         (variable size)
  │     └── RawFileInfoPreamble
  │           ├── data_addr  ──────────────────┐
  │           └── run_header_addr  ────────┐   │
  ├── InstID[0..n]                         │   │
  ├── MethodFile (OLE2 container)          │   │
  │                                        │   │
  ├── [data_addr] ◄────────────────────────│───┘
  │     └── ScanData stream                │
  │           ├── ScanDataPacket[0]        │
  │           ├── ScanDataPacket[1]        │
  │           └── ...                      │
  │                                        │
  ├── [run_header_addr] ◄──────────────────┘
  │     └── RunHeader
  │           ├── SampleInfo (embedded)
  │           ├── file_name fields
  │           ├── scan_index_addr  ───► ScanIndex stream
  │           ├── scan_trailer_addr ──► ScanEvent trailer stream
  │           ├── scan_params_addr  ──► ScanParameters stream
  │           ├── inst_log_addr  ─────► Instrument log stream
  │           └── error_log_addr  ────► Error log stream
  │
  ├── [scan_index_addr]
  │     └── ScanIndexEntry[0..n]
  │
  ├── [scan_trailer_addr]
  │     ├── UInt32: count
  │     └── ScanEvent[0..count-1]
  │
  ├── [scan_params_addr]
  │     ├── GenericDataHeader
  │     └── GenericRecord[0..n]
  │
  ├── [inst_log_addr]
  │     ├── GenericDataHeader
  │     └── GenericRecord[0..n]
  │
  └── [error_log_addr]
        └── Error[0..n]
```

### 3.1 Validated Offsets (Version 66)

From 6 real-world samples (LTQ Orbitrap, LTQ Orbitrap Elite, Orbitrap Fusion,
Orbitrap Fusion Lumos, Q Exactive HF, LTQ), all version 66:

| Structure | Offset | Notes |
|-----------|--------|-------|
| FileHeader | 0x0000 | Always at file start, 1356 bytes |
| SeqRow | 0x054C | Immediately follows FileHeader |
| ASInfo | Variable | Immediately follows SeqRow |
| RawFileInfo | Variable | Immediately follows ASInfo |
| InstID | Variable | Immediately follows RawFileInfo |
| MethodFile | Variable | OLE2 container after InstID(s) |
| ScanData | `data_addr` | Pointed to by RawFileInfoPreamble |
| RunHeader | `run_header_addr` | Pointed to by RawFileInfoPreamble |

---

## 4. FileHeader

The file header is the first structure in every RAW file. It is always 1356
bytes and always starts at offset 0.

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| 0x0000 | 2 | UInt16 | magic | Must be `0xa101` |
| 0x0002 | 18 | UTF16LE(18) | signature | Must be `"Finnigan"` (8 chars + null, 9 code units) |
| 0x0014 | 4 | UInt32 | unknown_long[1] | Always 0 in observed samples |
| 0x0018 | 4 | UInt32 | unknown_long[2] | Always 0 |
| 0x001C | 4 | UInt32 | unknown_long[3] | Always 0 |
| 0x0020 | 4 | UInt32 | unknown_long[4] | Observed: 524288 (0x80000) |
| 0x0024 | 4 | UInt32 | version | File format version (e.g. 66) |
| 0x0028 | 112 | AuditTag | audit_start | Acquisition start audit tag |
| 0x0098 | 112 | AuditTag | audit_end | Acquisition end audit tag |
| 0x0108 | 4 | UInt32 | unknown_long[5] | Always 0 |
| 0x010C | 60 | RawBytes(60) | unknown_area | Padding/reserved |
| 0x0148 | 1028 | UTF16LE(1028) | tag | Header tag (usually empty) |

**Total size**: 2 + 18 + 16 + 4 + 112 + 112 + 4 + 60 + 1028 = **1356 bytes** (0x054C)

### 4.1 Validation

A valid RAW file must satisfy:
1. `magic == 0xa101`
2. `signature == "Finnigan"` (when decoded as UTF-16-LE)
3. `version` is one of: 8, 47, 57, 60, 62, 63, 64, 66

### 4.2 Quick Detection

To quickly identify a file as Thermo RAW:
```python
magic = data[0:2]  # b'\x01\xa1'
signature = data[2:20].decode('utf-16-le')[:8]  # "Finnigan"
```

---

## 5. AuditTag

A sub-structure embedded twice in FileHeader (audit_start, audit_end). Contains
a timestamp and operator/system identification tags.

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| 0x00 | 8 | WindowsFileTime | time | Timestamp |
| 0x08 | 50 | UTF16LE(50) | tag[1] | Primary tag (usually Xcalibur user or operator) |
| 0x3A | 50 | UTF16LE(50) | tag[2] | Secondary tag (instrument model, user label, or duplicate of tag[1]) |
| 0x6C | 4 | UInt32 | unknown_long | Possibly CRC-32 or session identifier |

**Total size**: 8 + 50 + 50 + 4 = **112 bytes**

### 5.1 Observed Values

- `audit_start.tag[1]`: Almost always `"Xcalibur_System"` (the acquisition software identity)
- `audit_start.tag[2]`: Varies — seen: `"Orbitrap Elite"`, `"Fusion"`, `"Thermo Scientific"`, `"protein"`, `"admin"`, `"Thermo"`. This carries either the instrument model name or the Windows user account name.
- `audit_end.tag[1]`: Almost always `"Xcalibur_System"`
- `audit_end.tag[2]`: Almost always `"Xcalibur_System"`
- `audit_start.time`: Acquisition start time
- `audit_end.time`: Acquisition end time (may be slightly before start time in some edge cases — possibly clock discrepancies)

---

