# Scan Parameters (Generic Records)

_ScanParameters, GenericDataHeader/Descriptor/Record_

## 25. ScanParameters

Per-scan metadata (also called "trailer extra" in Thermo parlance). Located at
`RunHeader.scan_params_addr`.

This stream is **self-describing**: a GenericDataHeader defines the field
layout, followed by one GenericRecord per scan. The GenericDataHeader is NOT
stored at `scan_params_addr` itself; it is located somewhere between
`RunHeader.error_log_addr` and `RunHeader.scan_trailer_addr` in the file,
typically near the instrument log. It is found by scanning forward from
`error_log_addr` looking for a valid GDH whose `fixed_record_size()` matches
`(file_size − scan_params_addr) / num_scans`.

### 25.1 Stream Layout at `scan_params_addr`

Records begin **directly** at `scan_params_addr` — there is no preamble u32 or
any other header at this offset. The file may contain a few trailing bytes
(typically 4–8) after the last record; these are not part of any scan.

| Byte offset | Description |
|-------------|-------------|
| 0 | First byte of Record[0] (first scan) |
| record_size | First byte of Record[1] |
| … | … |
| (num_scans − 1) × record_size | First byte of Record[num_scans − 1] |
| num_scans × record_size | Optional trailing bytes (instrument-dependent) |

### 25.2 Locating the GenericDataHeader

The GDH for the ScanParameters stream is stored in the region bounded by
`RunHeader.error_log_addr` (inclusive) and `RunHeader.scan_trailer_addr`
(exclusive). Because the instrument log, error log, and ScanParameters GDH may
be interleaved in this region, the reader locates the GDH by linear forward
scan:

1. Compute `expected_record_size = (file_size − scan_params_addr) / num_scans`
   (integer division).
2. Scan from `error_log_addr` toward `scan_trailer_addr`, attempting to decode
   a GenericDataHeader at each plausible alignment.
3. Accept the first GDH whose `fixed_record_size()` equals `expected_record_size`
   (first pass). If no exact match is found, accept any structurally valid GDH
   (second pass).

### 25.3 Common Fields

The exact field set and order vary by instrument family and firmware version.
Representative fields:

| Label | Type | Description |
|-------|------|-------------|
| `"Ion Injection Time (ms):"` | Float32 or Float64 | Fill time in ms |
| `"Charge State:"` | Int32 | Precursor charge (0 = MS1 / unknown) |
| `"Monoisotopic M/Z:"` | Float64 | Monoisotopic precursor m/z (0 = not determined) |
| `"Master Scan Number:"` | Int32 | Parent MS1 scan number (−1 = none) |
| `"Master Index:"` | Int32 | Alternative master scan reference (newer firmware) |
| `"Micro Scan Count:"` | Int32 | Number of micro-scans averaged |
| `"Scan Segment:"` | Int32 | Segment index |
| `"Scan Event:"` | Int32 | Event index within segment |
| `"Orbitrap Resolution:"` | Int32 | Resolving power (older firmware) |
| `"FT Resolution:"` | Int32 | Resolving power (newer firmware) |
| `"HCD Energy:"` | AsciiString | Collision energy (may be stepped, e.g. `"27 30 33"`) |
| `"Elapsed Scan Time (sec):"` | Float32 or Float64 | Scan duration |
| `"AGC:"` | Bool or AsciiString | AGC on/off (`"On "` / `"Off"`) |
| `"AGC Target:"` | Int32 | Target ion count |
| `"Max. Ion Time (ms):"` | Float64 | Maximum allowed fill time |
| `"Number of LM Found:"` | Int32 | Number of lock masses matched |
| `"LM Correction (ppm):"` | Float64 | Applied lock-mass correction |

Older LTQ Orbitrap instruments prepend one or more empty-label `AsciiString`
fields (containing `\t`-separated internal state) before the human-readable
fields above.

---

## 26. GenericDataHeader

Self-describing header that defines the schema for GenericRecord streams.

| Order | Type | Field | Description |
|-------|------|-------|-------------|
| 1 | UInt32 | n | Number of field descriptors |
| 2 | GenericDataDescriptor[n] | fields | Array of field descriptors |

After decoding, the header produces a **template list** used to decode each
subsequent GenericRecord.

---

## 27. GenericDataDescriptor

A single field descriptor within a GenericDataHeader.

| Order | Type | Field | Description |
|-------|------|-------|-------------|
| 1 | UInt32 | type | Data type code (see table below) |
| 2 | UInt32 | length | Byte length (for string types) |
| 3 | PascalStringWin32 | label | Human-readable field name |

### 27.1 Type Codes

| Code | Type | Size | Description |
|------|------|------|-------------|
| 0x0 | — | 0 | Gap/section separator (no data) |
| 0x1 | Int8 | 1 | Signed byte |
| 0x2 | UInt8 | 1 | Boolean (true/false) |
| 0x3 | UInt8 | 1 | Boolean (yes/no) |
| 0x4 | UInt8 | 1 | Boolean (on/off) |
| 0x5 | UInt8 | 1 | Unsigned byte |
| 0x6 | Int16 | 2 | Signed short |
| 0x7 | UInt16 | 2 | Unsigned short |
| 0x8 | Int32 | 4 | Signed long |
| 0x9 | UInt32 | 4 | Unsigned long |
| 0xA | Float32 | 4 | Single-precision float |
| 0xB | Float64 | 8 | Double-precision float |
| 0xC | String | `length` | ASCIIZ string |
| 0xD | WString | `length × 2` | UTF-16-LE null-terminated string |

---

## 28. GenericRecord

A single data record decoded using the templates generated by a
GenericDataHeader. The fields are read sequentially in the order defined by the
header's descriptor array.

Each field's type and size are determined by the corresponding
GenericDataDescriptor. Type 0x0 (gap) produces no data — it is a label-only
separator.

---

