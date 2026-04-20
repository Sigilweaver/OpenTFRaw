# Thermo Fisher RAW File Format Specification

**OpenTFRaw Project — Binary Format Reference**
**Version**: 0.1.0 (Draft)
**Scope**: File format versions 57, 60, 62, 63, 64, 66
**Primary target**: Version 66 (current, most widely deployed)

---

## Table of Contents

1. [Overview](#1-overview)
2. [Conventions](#2-conventions)
3. [File Layout](#3-file-layout)
4. [FileHeader](#4-fileheader)
5. [AuditTag](#5-audittag)
6. [SeqRow](#6-seqrow)
7. [InjectionData](#7-injectiondata)
8. [ASInfo](#8-asinfo)
9. [RawFileInfo](#9-rawfileinfo)
10. [RawFileInfoPreamble](#10-rawfileinfopreamble)
11. [InstID](#11-instid)
12. [MethodFile](#12-methodfile)
13. [RunHeader](#13-runheader)
14. [SampleInfo](#14-sampleinfo)
15. [ScanIndexEntry](#15-scanindexentry)
16. [ScanDataPacket](#16-scandatapacket)
17. [PacketHeader](#17-packetheader)
18. [Profile](#18-profile)
19. [ProfileChunk](#19-profilechunk)
20. [Peaks](#20-peaks)
21. [ScanEvent](#21-scanevent)
22. [ScanEventPreamble](#22-scaneventpreamble)
23. [Reaction](#23-reaction)
24. [FractionCollector](#24-fractioncollector)
25. [ScanParameters](#25-scanparameters)
26. [GenericDataHeader](#26-genericdataheader)
27. [GenericDataDescriptor](#27-genericdatadescriptor)
28. [GenericRecord](#28-genericrecord)
29. [Error Log](#29-error-log)
30. [Instrument Log](#30-instrument-log)
31. [Enumerations](#31-enumerations)
32. [Frequency-to-M/z Conversion](#32-frequency-to-mz-conversion)
33. [Version Differences](#33-version-differences)
34. [Instrument-Specific Notes](#34-instrument-specific-notes)
35. [References](#35-references)

---

## 1. Overview

Thermo Fisher RAW files (`.raw`) are proprietary binary files produced by
Thermo Fisher Scientific mass spectrometers running Xcalibur data acquisition
software. The format has its roots in the Finnigan Corporation instrumentation
line (acquired by Thermo in 1990) and retains the "Finnigan" magic signature.

The format stores:
- Complete mass spectra (profile and/or centroid mode)
- Chromatographic data (retention times, total ion current)
- Instrument configuration and method parameters
- Sample metadata and sequence table information
- Instrument log and error log streams
- Embedded OLE2 method file containers

All multi-byte values are **little-endian**. Text is encoded as either
**UTF-16-LE** (fixed-width fields, null-padded) or as **PascalStringWin32**
(length-prefixed UTF-16-LE, variable-width).

### 1.1 Known Format Versions

| Version | Era | Address Width | Notes |
|---------|-----|---------------|-------|
| 8 | Pre-2000 | 32-bit | Earliest known, minimal structure |
| 47 | ~2003 | 32-bit | |
| 57 | ~2006–2008 | 32-bit | LCQ, LTQ, LTQ-FT |
| 60 | ~2009 | 32-bit | |
| 62 | ~2010–2012 | 32-bit | Orbitrap (early), Q Exactive |
| 63 | ~2013 | 32-bit | |
| 64 | ~2014–2015 | 64-bit | Transition version, 64-bit addresses |
| 66 | ~2015–present | 64-bit | **Current version**, Orbitrap Fusion/Lumos/Eclipse/Exploris/Astral |

Version 66 is produced by all current instruments and is the primary target of
this specification. Earlier versions are documented where they differ.

---

## 2. Conventions

### 2.1 Primitive Types

| Notation | Size | Description |
|----------|------|-------------|
| `UInt8` | 1 | Unsigned 8-bit integer |
| `Int8` | 1 | Signed 8-bit integer |
| `UInt16` | 2 | Unsigned 16-bit integer, little-endian |
| `Int16` | 2 | Signed 16-bit integer, little-endian |
| `UInt32` | 4 | Unsigned 32-bit integer, little-endian |
| `Int32` | 4 | Signed 32-bit integer, little-endian |
| `UInt64` | 8 | Unsigned 64-bit integer, little-endian |
| `Float32` | 4 | IEEE 754 single-precision float, little-endian |
| `Float64` | 8 | IEEE 754 double-precision float, little-endian |
| `WindowsFileTime` | 8 | 100-nanosecond intervals since 1601-01-01 00:00:00 UTC |
| `UTF16LE(n)` | n bytes | Fixed-width null-padded UTF-16-LE string |
| `PascalStringWin32` | 4+2k | UInt32 char count `k`, then `k` UTF-16-LE code units |
| `RawBytes(n)` | n bytes | Opaque data |

### 2.2 WindowsFileTime Conversion

```
unix_timestamp = (filetime / 10_000_000) - 11_644_473_600
```

Where `filetime` is the 64-bit unsigned value read from the file.

### 2.3 PascalStringWin32 Encoding

```
[UInt32: char_count] [char_count × 2 bytes: UTF-16-LE data]
```

The char count includes the null terminator when present.  The actual string
is extracted by decoding the UTF-16-LE bytes and stripping trailing null
characters.

---

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
| 19–33 | PascalStringWin32 | unknown_text[e–s] | 15 additional text fields (usually empty) |

### 6.2 Version Differences

| Version | Extra fields after common 14 |
|---------|------|
| v8 | None |
| v47, v57 | vial + unk_c + unk_d + unknown_long |
| v60–v66 | vial + unk_c + unk_d + unknown_long + unk_e through unk_s (15 more strings) |

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

## 9. RawFileInfo

Primary index structure containing the date, controller configuration, and
pointers to the data and run header regions. Immediately follows ASInfo.

| Order | Type | Field | Description |
|-------|------|-------|-------------|
| 1 | RawFileInfoPreamble | preamble | Date, addresses, controller info |
| 2 | PascalStringWin32 | label_heading[1] | Label heading for user_label[1] (typically "Study") |
| 3 | PascalStringWin32 | label_heading[2] | Label heading for user_label[2] (typically "Client") |
| 4 | PascalStringWin32 | label_heading[3] | Label heading for user_label[3] (typically "Laboratory") |
| 5 | PascalStringWin32 | label_heading[4] | Label heading for user_label[4] (typically "Company") |
| 6 | PascalStringWin32 | label_heading[5] | Label heading for user_label[5] (typically "Phone") |
| 7 | PascalStringWin32 | unknown_text | Computer name / machine identifier |

### 9.1 Observed Values

The 6th string (unknown_text) consistently holds the **computer hostname** of
the acquisition system (e.g. "OEII", "FUSION-PC", "PCW-VBCF-28", "ADMIN-PC",
"THERMO-PC", "6RW3W52").

---

## 10. RawFileInfoPreamble

The binary-data portion of RawFileInfo containing the acquisition date and the
critical **data address** and **run header address** pointers that navigate to
the bulk of the file's content.

### 10.1 Common Fields (All Versions)

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| 0x00 | 4 | UInt32 | method_file_present | 1 if embedded method file exists |
| 0x04 | 2 | UInt16 | year | Acquisition year |
| 0x06 | 2 | UInt16 | month | Month (1–12) |
| 0x08 | 2 | UInt16 | day_of_week | Day of week (0=Sunday) |
| 0x0A | 2 | UInt16 | day | Day of month (1–31) |
| 0x0C | 2 | UInt16 | hour | Hour (0–23) |
| 0x0E | 2 | UInt16 | minute | Minute (0–59) |
| 0x10 | 2 | UInt16 | second | Second (0–59) |
| 0x12 | 2 | UInt16 | millisecond | Millisecond (0–999) |

**Common size**: 20 bytes

### 10.2 Version 66 Extended Fields

| Relative Offset | Size | Type | Field | Description |
|-----------------|------|------|-------|-------------|
| +0x00 | 4 | UInt32 | unknown_long[2] | |
| +0x04 | 4 | UInt32 | data_addr_32 | **Defunct** 32-bit data address (always 0 in v66) |
| +0x08 | 4 | UInt32 | controller_n[1] | Number of instrument controllers |
| +0x0C | 4 | UInt32 | controller_n[2] | Duplicate of controller_n[1] |
| +0x10 | 4 | UInt32 | unknown_long[5] | |
| +0x14 | 4 | UInt32 | unknown_long[6] | |
| +0x18 | 4 | UInt32 | run_header_addr_32 | **Defunct** 32-bit run header address (always 0 in v66) |
| +0x1C | 760 | RawBytes | unknown_area[1] | Reserved/padding |
| +0x314 | 8 | UInt64 | **data_addr** | **File offset to scan data stream** |
| +0x31C | 4 | UInt32 | unknown_long[7] | |
| +0x320 | 4 | UInt32 | unknown_long[8] | |
| +0x324 | 8 | UInt64 | **run_header_addr** | **File offset to RunHeader** |
| +0x32C | 4 | UInt32 | unknown_long[9] | |
| +0x330 | 4 | UInt32 | unknown_long[10] | |
| +0x334 | 8 | UInt64 | run_header_addr[2] | Second RunHeader address (0 if only one controller) |
| +0x33C | 1008 | RawBytes | unknown_area[2] | Reserved/padding |

**Total preamble size (v66)**: 20 + 4 + 4 + 8 + 8 + 4 + 760 + 8 + 8 + 8 + 8 + 8 + 1008 = **1856 bytes**

### 10.3 Version 57–63 (32-bit addresses)

In versions prior to 64, addresses are 32-bit and the preamble is smaller (804
bytes total including the 20-byte common part):

| Offset | Size | Type | Field |
|--------|------|------|-------|
| +0x00 | 4 | UInt32 | unknown_long[2] |
| +0x04 | 4 | UInt32 | **data_addr** |
| +0x08 | 4 | UInt32 | controller_n[1] |
| +0x0C | 4 | UInt32 | controller_n[2] |
| +0x10 | 4 | UInt32 | unknown_long[5] |
| +0x14 | 4 | UInt32 | unknown_long[6] |
| +0x18 | 4 | UInt32 | **run_header_addr** |
| +0x1C | 4 | UInt32 | unknown_long[7] |
| +0x20 | 4 | UInt32 | unknown_long[8] |
| +0x24 | 4 | UInt32 | run_header_addr[2] |
| +0x28 | 744 | RawBytes | unknown_area |

### 10.4 Controller Count

The `controller_n[1]` and `controller_n[2]` fields are equal and indicate how
many instrument data controllers stored data in this file. Observed values:

| Value | Meaning |
|-------|---------|
| 1 | Single controller (most common: one MS detector) |
| 2 | Two controllers (e.g. MS + UV detector, or MS + PDA) |
| 7 | Seven controllers (complex multi-detector setups) |

When `controller_n > 1`, `run_header_addr[2]` points to the second RunHeader
(for the second controller's data). Each controller has its own complete scan
data stream and run header.

---

## 11. InstID

Instrument identification. One or more InstID structures follow RawFileInfo,
one per controller.

| Order | Type | Field | Description |
|-------|------|-------|-------------|
| 1 | UInt32 | unknown_long[1] | |
| 2 | UInt32 | unknown_long[2] | |
| 3 | UInt32 | unknown_long[3] | If > 0, model[3] field is present |
| 4 | PascalStringWin32 | model[1] | Instrument model name (e.g. "LTQ Orbitrap Elite") |
| 5 | PascalStringWin32 | model[2] | Secondary model string |
| 6* | PascalStringWin32 | model[3] | **Conditional**: only present if unknown_long[3] > 0 |
| 7 | PascalStringWin32 | serial_number | Instrument serial number |
| 8 | PascalStringWin32 | software_version | Acquisition software version |
| 9 | PascalStringWin32 | tag[1] | Additional tag |
| 10 | PascalStringWin32 | tag[2] | Additional tag |
| 11 | PascalStringWin32 | tag[3] | Additional tag |
| 12 | PascalStringWin32 | tag[4] | Additional tag |

---

## 12. MethodFile

An embedded OLE2/Compound Document Format (CDF) container holding the
instrument method files used during acquisition. The OLE2 container may contain
sub-streams for each instrument module (e.g. MS method, LC method, autosampler
method).

Parsing the OLE2 container requires a separate OLE2/CDF decoder. The method
file begins after the InstID structure(s) and extends to the `data_addr`
offset.

---

## 13. RunHeader

The RunHeader is the **secondary index structure** containing pointers to all
major data streams. It is located at the file offset specified by
`RawFileInfoPreamble.run_header_addr`.

### 13.1 Common Fields

| Order | Type | Field | Description |
|-------|------|-------|-------------|
| 1 | SampleInfo | sample_info | Embedded sample information (see §14) |

### 13.2 Version 66 Fields (after SampleInfo)

| Order | Type | Field | Description |
|-------|------|-------|-------------|
| 2 | UTF16LE(520) | file_name[1] | Original file path (260 chars) |
| 3 | UTF16LE(520) | file_name[2] | |
| 4 | UTF16LE(520) | file_name[3] | |
| 5 | UTF16LE(520) | file_name[4] | |
| 6 | UTF16LE(520) | file_name[5] | |
| 7 | UTF16LE(520) | file_name[6] | |
| 8 | Float64 | unknown_double[1] | |
| 9 | Float64 | unknown_double[2] | |
| 10 | UTF16LE(520) | file_name[7] | |
| 11 | UTF16LE(520) | file_name[8] | |
| 12 | UTF16LE(520) | file_name[9] | |
| 13 | UTF16LE(520) | file_name[a] | |
| 14 | UTF16LE(520) | file_name[b] | |
| 15 | UTF16LE(520) | file_name[c] | |
| 16 | UTF16LE(520) | file_name[d] | |
| 17 | UInt32 | scan_trailer_addr_32 | **Defunct** (v64+) |
| 18 | UInt32 | scan_params_addr_32 | **Defunct** (v64+) |
| 19 | UInt32 | ntrailer | Number of scan event trailer entries |
| 20 | UInt32 | nparams | Number of scan parameter entries (should equal ntrailer) |
| 21 | UInt32 | nsegs | Number of scan segments |
| 22 | UInt32 | unknown_long[1] | |
| 23 | UInt32 | unknown_long[2] | |
| 24 | UInt32 | own_addr_32 | **Defunct** (v64+) |
| 25 | UInt32 | unknown_long[3] | |
| 26 | UInt32 | unknown_long[4] | |
| 27 | UInt64 | **scan_index_addr** | File offset to ScanIndexEntry array |
| 28 | UInt64 | **data_addr** | File offset to scan data stream |
| 29 | UInt64 | **inst_log_addr** | File offset to instrument log |
| 30 | UInt64 | **error_log_addr** | File offset to error log |
| 31 | UInt64 | unknown_addr[1] | |
| 32 | UInt64 | **scan_trailer_addr** | File offset to ScanEvent trailer stream |
| 33 | UInt64 | **scan_params_addr** | File offset to ScanParameters stream |
| 34 | UInt32 | unknown_long[5] | |
| 35 | UInt32 | unknown_long[6] | |
| 36 | UInt64 | own_addr | Self-address (file offset of this RunHeader) |
| 37–60 | UInt32 × 24 | unknown_long[7–30] | Reserved/unknown |

### 13.3 Version 57–63 (32-bit addresses)

In pre-v64 versions, the scan index, data, log, and trailer addresses are
obtained from the **SampleInfo** embedded structure rather than the RunHeader
proper. The RunHeader file name fields and ntrailer/nparams are the same, but
the 64-bit address block (fields 27–36) is absent.

---

## 14. SampleInfo

Embedded as the first field of RunHeader. Contains summary statistics about the
acquisition and (in pre-v64 versions) the stream address pointers.

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| 0x00 | 4 | UInt32 | unknown_long[1] | |
| 0x04 | 4 | UInt32 | unknown_long[2] | |
| 0x08 | 4 | UInt32 | first_scan_number | First scan number (usually 1) |
| 0x0C | 4 | UInt32 | last_scan_number | Last scan number |
| 0x10 | 4 | UInt32 | inst_log_length | Number of instrument log entries |
| 0x14 | 4 | UInt32 | error_log_length | Number of error log entries |
| 0x18 | 4 | UInt32 | unknown_long[4] | |
| 0x1C | 4 | UInt32 | scan_index_addr† | **Defunct in v64+** — use RunHeader |
| 0x20 | 4 | UInt32 | data_addr† | **Defunct in v64+** — use RunHeader |
| 0x24 | 4 | UInt32 | inst_log_addr† | **Defunct in v64+** — use RunHeader |
| 0x28 | 4 | UInt32 | error_log_addr† | **Defunct in v64+** — use RunHeader |
| 0x2C | 4 | UInt32 | unknown_long[5] | |
| 0x30 | 8 | Float64 | max_ion_current | Maximum total ion current across all scans |
| 0x38 | 8 | Float64 | low_mz | Lowest M/z across all scans |
| 0x40 | 8 | Float64 | high_mz | Highest M/z across all scans |
| 0x48 | 8 | Float64 | start_time | Retention time of first scan (minutes) |
| 0x50 | 8 | Float64 | end_time | Retention time of last scan (minutes) |
| 0x58 | 56 | RawBytes | unknown_area | |
| 0x90 | 88 | UTF16LE(88) | tag[1] | 44-char tag |
| 0xE8 | 40 | UTF16LE(40) | tag[2] | 20-char tag |
| 0x110 | 320 | UTF16LE(320) | tag[3] | 160-char tag |

**Total size**: 592 bytes

†In versions 64 and 66, these 32-bit address fields are no longer valid (the
file may exceed 4 GB). Use the 64-bit addresses from the RunHeader proper.

---

## 15. ScanIndexEntry

Array of scan index entries located at `RunHeader.scan_index_addr`. One entry
per scan. Provides the offset, size, and summary statistics for each scan's
data packet.

### 15.1 Version 66

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| 0x00 | 4 | UInt32 | offset_32 | **Defunct** 32-bit offset |
| 0x04 | 4 | UInt32 | index | Scan number (0-based) |
| 0x08 | 2 | UInt16 | scan_event | Scan event index |
| 0x0A | 2 | UInt16 | scan_segment | Scan segment index |
| 0x0C | 4 | UInt32 | next | Index of next scan (linked list) |
| 0x10 | 4 | UInt32 | unknown_long | |
| 0x14 | 4 | UInt32 | data_size | Size of scan data packet in bytes |
| 0x18 | 8 | Float64 | start_time | Retention time (minutes) |
| 0x20 | 8 | Float64 | total_current | Total ion current |
| 0x28 | 8 | Float64 | base_intensity | Base peak intensity |
| 0x30 | 8 | Float64 | base_mz | Base peak M/z |
| 0x38 | 8 | Float64 | low_mz | Lowest M/z in scan |
| 0x40 | 8 | Float64 | high_mz | Highest M/z in scan |
| 0x48 | 8 | UInt64 | **offset** | File offset to ScanDataPacket (relative to data stream start) |
| 0x50 | 4 | UInt32 | unknown_long[1] | |
| 0x54 | 4 | UInt32 | unknown_long[2] | |

**Total size**: 92 bytes per entry

### 15.2 Version 64

Same as v66 but without the trailing unknown_long[1] and [2]:

**Total size**: 84 bytes per entry

### 15.3 Pre-v64

No 64-bit offset field; `offset` at position 0x00 is the actual 32-bit offset:

**Total size**: 76 bytes per entry

### 15.4 Scan Data Location

The actual file position of scan `i`'s data:
```
absolute_offset = RunHeader.data_addr + ScanIndexEntry[i].offset
```

---

## 16. ScanDataPacket

Each scan's raw data packet, located at the offset indicated by its
ScanIndexEntry. The packet contains the mass spectrum in profile form,
centroid form, or both.

### 16.1 Structure

| Order | Size | Type | Field | Description |
|-------|------|------|-------|-------------|
| 1 | 40 | PacketHeader | header | Sizes and M/z range (see §17) |
| 2 | header.profile_size × 4 | bytes | profile_data | Raw profile data |
| 3 | header.peak_list_size × 4 | bytes | centroid_data | Centroid peak list |
| 4 | header.descriptor_list_size × 4 | bytes | descriptor_data | Additional descriptors |
| 5 | header.unknown_stream_size × 4 | bytes | unknown_data | Unknown stream |
| 6 | header.triplet_stream_size × 4 | bytes | triplet_data | Triplet data |

All size fields in the PacketHeader are in **4-byte words** (i.e., the actual
byte size is 4× the stored value).

---

## 17. PacketHeader

The 40-byte header at the start of each ScanDataPacket.

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| 0x00 | 4 | UInt32 | unknown_long[1] | |
| 0x04 | 4 | UInt32 | profile_size | Profile data size (in 4-byte words) |
| 0x08 | 4 | UInt32 | peak_list_size | Centroid data size (in 4-byte words) |
| 0x0C | 4 | UInt32 | layout | Profile layout flag: 0 or 128 |
| 0x10 | 4 | UInt32 | descriptor_list_size | Descriptor data size (in 4-byte words) |
| 0x14 | 4 | UInt32 | unknown_stream_size | Unknown data size (in 4-byte words) |
| 0x18 | 4 | UInt32 | triplet_stream_size | Triplet data size (in 4-byte words) |
| 0x1C | 4 | UInt32 | unknown_long[2] | |
| 0x20 | 4 | Float32 | low_mz | Lowest M/z in this scan |
| 0x24 | 4 | Float32 | high_mz | Highest M/z in this scan |

**Total size**: 40 bytes

### 17.1 Layout Flag

The `layout` field controls how ProfileChunk structures are decoded:

| Value | Meaning |
|-------|---------|
| 0 | No fudge factor in profile chunks (8-byte chunk preamble) |
| 128 (0x80) | Fudge factor present in profile chunks (12-byte chunk preamble) |

---

## 18. Profile

The profile data region within a ScanDataPacket. Represents the raw signal
intensity as a function of frequency bins (which map to M/z values via the
conversion function in the associated ScanEvent).

### 18.1 Structure

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| 0x00 | 8 | Float64 | first_value | First bin's frequency (or M/z in pre-FTMS) |
| 0x08 | 8 | Float64 | step | Bin width (negative for frequency domain) |
| 0x10 | 4 | UInt32 | peak_count | Number of ProfileChunks (non-zero signal regions) |
| 0x14 | 4 | UInt32 | nbins | Total number of bins across all chunks |

**Preamble size**: 24 bytes

After the preamble, `peak_count` ProfileChunk structures follow sequentially.

### 18.2 Interpretation

- If `step < 0`, data is in **frequency domain** and must be converted to M/z
  using the frequency-to-M/z conversion function (see §32).
- If `step > 0`, data is directly in **M/z domain** (older instruments).
- "Peaks" in the profile context refers to signal peaks (regions of non-zero
  signal), not mass spectral peaks.

---

## 19. ProfileChunk

A contiguous run of non-zero signal values within the profile.

### 19.1 Layout == 0 (No Fudge)

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0x00 | 4 | UInt32 | first_bin | Index of first bin in this chunk |
| 0x04 | 4 | UInt32 | nbins | Number of bins in this chunk |
| 0x08 | nbins × 4 | Float32[] | signal | Signal intensity values |

### 19.2 Layout > 0 (With Fudge)

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0x00 | 4 | UInt32 | first_bin | Index of first bin in this chunk |
| 0x04 | 4 | UInt32 | nbins | Number of bins in this chunk |
| 0x08 | 4 | Float32 | fudge | Instrument drift / conversion bias factor |
| 0x0C | nbins × 4 | Float32[] | signal | Signal intensity values |

### 19.3 M/z Calculation for a Bin

For bin index `i` within a chunk:
```
bin_index_global = chunk.first_bin + i
frequency = profile.first_value + bin_index_global * profile.step
mz = convert(frequency)    # See §32
```

If a fudge factor is present, it modifies the frequency before conversion:
```
frequency_adjusted = frequency + chunk.fudge
```

---

## 20. Peaks

The centroid peak list within a ScanDataPacket. Contains pre-picked peaks with
M/z and abundance values.

### 20.1 Structure

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0x00 | 4 | UInt32 | count | Number of peaks |
| 0x04 | count × 8 | Peak[] | peaks | Array of (mz, abundance) pairs |

### 20.2 Peak

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0x00 | 4 | Float32 | mz | Mass-to-charge ratio |
| 0x04 | 4 | Float32 | abundance | Peak intensity/abundance |

**Size per peak**: 8 bytes

---

## 21. ScanEvent

Describes the type and parameters of a scan. One ScanEvent per scan, stored in
the **scan event trailer stream** at `RunHeader.scan_trailer_addr`.

The scan event trailer stream begins with a `UInt32` count, followed by that
many ScanEvent structures.

### 21.1 Pre-v66 Structure

| Order | Type | Field | Description |
|-------|------|-------|-------------|
| 1 | ScanEventPreamble | preamble | Byte array of scan parameters (see §22) |
| 2 | UInt32 | np | Number of precursor ions (0 for MS1) |
| 3* | Reaction[np] | precursors | Precursor list (only if np > 0) |
| 4 | UInt32 | unknown_long[1] | |
| 5 | FractionCollector | fraction_collector | M/z acquisition range |
| 6 | UInt32 | nparam | Number of conversion coefficients |
| 7* | Float64[nparam] | coefficients | Frequency-to-M/z conversion (see §32) |
| 8 | UInt32 | unknown_long[2] | |
| 9 | UInt32 | unknown_long[3] | |

### 21.2 Version 66 Structure

Version 66 has a significantly restructured ScanEvent layout:

**Head:**
| Order | Type | Field |
|-------|------|-------|
| 1 | ScanEventPreamble | preamble |
| 2 | UInt32 | unknown_long[0] |
| 3 | UInt32 | n_reactions |

**If dependent scan (n_reactions > 0, i.e. MS2+):**
| Order | Type | Field |
|-------|------|-------|
| 4 | Reaction[n_reactions] | precursors |
| 5 | Float64 | unknown_double[0] |
| 6 | Float64 | unknown_double[1] |
| 7 | UInt32 | unknown_long[2] |
| 8 | UInt32 | unknown_long[3] |
| 9 | UInt32 | unknown_long[4] |
| 10 | FractionCollector | fraction_collector |
| 11 | UInt32 | nparam |
| 12 | Float64[nparam] | coefficients |

**If primary scan (n_reactions == 0, i.e. MS1):**
| Order | Type | Field |
|-------|------|-------|
| 4 | FractionCollector | fraction_collector[0] |
| 5 | UInt32 | unknown_long[2] |
| 6 | UInt32 | unknown_long[3] |
| 7 | UInt32 | unknown_long[4] |
| 8 | UInt32 | unknown_long[5] |
| 9 | FractionCollector | fraction_collector |
| 10 | UInt32 | unknown_long[6] |
| 11 | UInt32 | unknown_long[7] |
| 12 | UInt32 | unknown_long[8] |
| 13 | FractionCollector | fraction_collector[2] |
| 14 | UInt32 | nparam |
| 15 | Float64[nparam] | coefficients |

**Tail (both cases in v66):**
| Order | Type | Field |
|-------|------|-------|
| last-4 | UInt32 | unknown_long[a] |
| last-3 | UInt32 | unknown_long[b] |
| last-2 | UInt32 | unknown_long[c] |
| last-1 | UInt32 | unknown_long[d] |
| last | UInt32 | unknown_long[e] |

---

## 22. ScanEventPreamble

A byte array encoding the scan type, analyzer, polarity, ionization mode, and
other scan parameters. Version-dependent size.

### 22.1 Common Fields (All Versions, bytes 0–40)

| Byte | Field | Values |
|------|-------|--------|
| 0 | unknown_byte[0] | |
| 1 | unknown_byte[1] | |
| 2 | corona | 0=Off, 1=On |
| 3 | detector | 0=Valid, 1=Undefined |
| 4 | **polarity** | 0=Negative, 1=Positive, 2=Undefined |
| 5 | **scan_mode** | 0=Centroid, 1=Profile, 2=Undefined |
| 6 | **ms_power** | 0=Undefined, 1=MS1, 2=MS2, ... 8=MS8 |
| 7 | **scan_type** | 0=Full, 1=Zoom, 2=SIM, 3=SRM, 4=CRM, 5=Undefined, 6=Q1, 7=Q3 |
| 8 | unknown_byte[8] | |
| 9 | unknown_byte[9] | |
| 10 | **dependent** | 0=Primary (MS1), 1=Dependent (MS2+) |
| 11 | **ionization** | 0=EI, 1=CI, 2=FABI, 3=ESI, 4=APCI, 5=NSI, 6=TSI, 7=FDI, 8=MALDI, 9=GDI |
| 12–23 | unknown_byte[12–23] | |
| 24 | **activation** | 1=HCD, 4=CID |
| 25–31 | unknown_byte[25–31] | |
| 32 | wideband | 0=Off, 1=On |
| 33–39 | unknown_byte[33–39] | |
| 40 | **analyzer** | 0=ITMS, 1=TQMS, 2=SQMS, 3=TOFMS, 4=FTMS, 5=Sector |

### 22.2 Size by Version

| Version | Total Bytes |
|---------|-------------|
| v8 | 41 |
| v57, v60 | 80 |
| v62 | 120 |
| v63, v64 | 128 |
| v66 | 136 |

### 22.3 Filter Line Construction

A human-readable filter line can be constructed from the preamble:

```
{ANALYZER} {POLARITY} {SCAN_MODE} {IONIZATION}{DEPENDENT}{WIDEBAND} {SCAN_TYPE} {MS_POWER} [{LOW_MZ}-{HIGH_MZ}]
```

Example: `FTMS + p NSI Full ms [350.00-1500.00]`
Example: `FTMS + c NSI d Full ms2 542.30@hcd35.00 [100.00-1600.00]`

---

## 23. Reaction

Precursor ion information for MS2+ scans. Stored as an array within ScanEvent.

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| 0x00 | 8 | Float64 | precursor_mz | Precursor M/z selected for fragmentation |
| 0x08 | 8 | Float64 | unknown_double | Typically 1.0 |
| 0x10 | 8 | Float64 | energy | Collision/activation energy |
| 0x18 | 4 | UInt32 | unknown_long[1] | |
| 0x1C | 4 | UInt32 | unknown_long[2] | |

**Total size**: 32 bytes

### 23.1 String Representation

```
{precursor_mz}@{activation_method}{energy}
```

Example: `542.30@hcd35.00`, `480.25@cid30.00`

The activation method name comes from the ScanEventPreamble `activation` field.

---

## 24. FractionCollector

M/z acquisition range for a scan event.

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| 0x00 | 8 | Float64 | low_mz | Lower M/z bound |
| 0x08 | 8 | Float64 | high_mz | Upper M/z bound |

**Total size**: 16 bytes

---

## 25. ScanParameters

Per-scan metadata (also called "trailer extra" in Thermo parlance). Located at
`RunHeader.scan_params_addr`. This is a **self-describing** data stream: a
GenericDataHeader defines the field layout, followed by one GenericRecord per
scan.

### 25.1 Stream Layout

| Order | Type | Description |
|-------|------|-------------|
| 1 | GenericDataHeader | Describes the field types and labels |
| 2 | GenericRecord[n] | One record per scan, decoded using the header's templates |

### 25.2 Common Trailer Extra Fields

These are the field labels commonly found in the GenericDataHeader (the exact
set and order varies by instrument and software version):

| Label | Type | Description |
|-------|------|-------------|
| "Ion Injection Time (ms):" | Float64 | Time ions were accumulated |
| "Charge State:" | Int16 | Precursor charge state (for MS2+) |
| "Monoisotopic M/Z:" | Float64 | Monoisotopic precursor M/z |
| "Master Scan Number:" | Int32 | Scan number of the parent MS1 scan |
| "Micro Scan Count:" | Int16 | Number of micro-scans averaged |
| "Scan Segment:" | Int16 | Segment index |
| "Scan Event:" | Int16 | Event index |
| "HCD Energy:" | Float64 or string | Collision energy (may be "27.0 30.0 33.0" for stepped HCD) |
| "Elapsed Scan Time (sec):" | Float64 | Duration of the scan |
| "Master Index:" | Int32 | Alternative master scan reference |
| "AGC:" | Float64 | Automatic Gain Control target |
| "FT Resolution:" | Float64 | Orbitrap/FTMS resolution setting |

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

## 29. Error Log

Array of error entries located at `RunHeader.error_log_addr`. The number of
entries is given by `SampleInfo.error_log_length`.

### 29.1 Error Entry

| Order | Type | Field | Description |
|-------|------|-------|-------------|
| 1 | Float32 | time | Retention time (minutes) |
| 2 | PascalStringWin32 | message | Error message text |

---

## 30. Instrument Log

Instrument log stream located at `RunHeader.inst_log_addr`. The number of
entries is given by `SampleInfo.inst_log_length`.

### 30.1 Stream Layout

| Order | Type | Description |
|-------|------|-------------|
| 1 | GenericDataHeader | Schema for log entries |
| 2 | GenericRecord[n] | One record per log entry |

Log entries typically include temperatures, pressures, voltages, and other
instrument operating parameters recorded periodically during acquisition.

---

## 31. Enumerations

### 31.1 Analyzer Type

| Value | Name | Description |
|-------|------|-------------|
| 0 | ITMS | Ion Trap Mass Spectrometer |
| 1 | TQMS | Triple Quadrupole Mass Spectrometer |
| 2 | SQMS | Single Quadrupole Mass Spectrometer |
| 3 | TOFMS | Time-of-Flight Mass Spectrometer |
| 4 | FTMS | Fourier Transform Mass Spectrometer (Orbitrap) |
| 5 | Sector | Magnetic Sector |

### 31.2 Polarity

| Value | Name | Symbol |
|-------|------|--------|
| 0 | Negative | `-` |
| 1 | Positive | `+` |

### 31.3 Scan Mode

| Value | Name | Symbol |
|-------|------|--------|
| 0 | Centroid | `c` |
| 1 | Profile | `p` |

### 31.4 MS Power (MSn Order)

| Value | Name | Description |
|-------|------|-------------|
| 0 | Undefined | |
| 1 | MS1 | Full scan (no fragmentation) |
| 2 | MS2 | Tandem MS (one fragmentation) |
| 3 | MS3 | MS³ |
| ... | ... | ... |
| 8 | MS8 | MS⁸ |

### 31.5 Scan Type

| Value | Name |
|-------|------|
| 0 | Full |
| 1 | Zoom |
| 2 | SIM |
| 3 | SRM |
| 4 | CRM |
| 6 | Q1 |
| 7 | Q3 |

### 31.6 Ionization Mode

| Value | Name | Description |
|-------|------|-------------|
| 0 | EI | Electron Ionization |
| 1 | CI | Chemical Ionization |
| 2 | FABI | Fast Atom Bombardment |
| 3 | ESI | Electrospray Ionization |
| 4 | APCI | Atmospheric Pressure Chemical Ionization |
| 5 | NSI | Nanospray Ionization |
| 6 | TSI | Thermospray Ionization |
| 7 | FDI | Field Desorption Ionization |
| 8 | MALDI | Matrix-Assisted Laser Desorption/Ionization |
| 9 | GDI | Glow Discharge Ionization |

### 31.7 Activation Method

| Value | Name | Description |
|-------|------|-------------|
| 1 | HCD | Higher-energy Collisional Dissociation |
| 4 | CID | Collision-Induced Dissociation |

---

## 32. Frequency-to-M/z Conversion

FTMS analyzers (Orbitrap, LTQ-FT) store profile data in the **frequency
domain**. The ScanEvent contains conversion coefficients to translate
frequencies to M/z values.

### 32.1 LTQ-FT Conversion (nparam == 4)

Coefficients: `[unknown, A, B, C]`

```
M/z = A + B/f + C/f²
```

Where `f` is the frequency value.

### 32.2 Orbitrap Conversion (nparam == 5 or 7)

Coefficients: `[unknown, (I,) A, B, C (, D, E)]`

```
M/z = A + B/f² + C/f⁴
```

### 32.3 Inverse Conversion (M/z to Frequency)

Used when looking up a specific M/z in frequency-domain profile data:

**LTQ-FT**: Solve quadratic `C + Bf + (A - Mz)f² = 0`:
```
f = (-B - sqrt(B² - 4C(A - Mz))) / (2(A - Mz))
```

**Orbitrap**: Solve `(A - Mz) + B/f² + C/f⁴ = 0`:

Let `x = 1/f²`:
```
Cx² + Bx + (A - Mz) = 0
x = (-B - sqrt(B² - 4C(A - Mz))) / (2C)
f = 1/sqrt(x)
```

### 32.4 Direct M/z Data

When `profile.step > 0` (positive step), the data is directly in M/z domain
and no frequency conversion is needed. The M/z of bin `i` is:
```
mz = profile.first_value + i * profile.step
```

---

## 33. Version Differences

### 33.1 Address Width Transition (v64)

Version 64 is the transition version where 64-bit addresses were introduced.
Files prior to v64 use 32-bit addresses throughout. v64+ files contain **both**
the defunct 32-bit fields (for backward compatibility, always set to 0) and the
new 64-bit address fields.

| Feature | v57–v63 | v64–v66 |
|---------|---------|---------|
| RawFileInfoPreamble.data_addr | UInt32 | UInt64 (32-bit copy is 0) |
| RawFileInfoPreamble.run_header_addr | UInt32 | UInt64 (32-bit copy is 0) |
| RunHeader stream addresses | In SampleInfo (32-bit) | In RunHeader proper (64-bit) |
| ScanIndexEntry.offset | UInt32 at offset 0 | UInt64 at offset 0x48 |
| ScanIndexEntry size | 76 bytes | 84 bytes (v64), 92 bytes (v66) |

### 33.2 ScanEventPreamble Size Growth

| Version | Preamble Bytes |
|---------|---------------|
| v8 | 41 |
| v57–v60 | 80 |
| v62 | 120 |
| v63–v64 | 128 |
| v66 | 136 |

### 33.3 ScanEvent Restructuring (v66)

Version 66 fundamentally restructured the ScanEvent layout with different
field orderings for MS1 vs. dependent scans. Earlier versions use a uniform
layout for all scan types (see §21).

### 33.4 RawFileInfoPreamble Padding

| Version | unknown_area[2] size |
|---------|---------------------|
| v64 | 992 bytes |
| v66 | 1008 bytes |

---

## 34. Instrument-Specific Notes

### 34.1 Orbitrap Family (Orbitrap, Orbitrap Fusion, Lumos, Eclipse, Exploris, Astral)

- **Analyzer**: FTMS (code 4)
- **Profile data**: Frequency domain (negative step value)
- **Conversion**: nparam == 5 or 7 (Orbitrap formula: `A + B/f² + C/f⁴`)
- **Centroid data**: Usually available alongside profile data
- **Typical ionization**: NSI (nanospray) or ESI (electrospray)
- **Typical activation**: HCD (code 1)
- **Scan modes**: Profile and/or centroid; dependent scans typically centroid
- **Version**: Almost exclusively version 66

### 34.2 LTQ Family (LTQ, LTQ Orbitrap, LTQ Orbitrap Velos, LTQ Orbitrap Elite)

- **Analyzer**: ITMS (code 0) for ion trap scans, FTMS (code 4) for Orbitrap scans
- **Profile data**: ITMS scans are M/z domain (positive step); FTMS scans are frequency domain
- **Conversion**: nparam == 4 (LTQ-FT formula: `A + B/f + C/f²`) for LTQ-FT; nparam == 7 for LTQ Orbitrap
- **Typical activation**: CID (code 4) for ion trap, HCD for Orbitrap
- **Versions**: v57–v66 depending on era

### 34.3 Q Exactive Family (Q Exactive, Q Exactive HF, Q Exactive HF-X, Q Exactive Plus)

- **Analyzer**: FTMS (code 4)
- **Profile data**: Frequency domain
- **Conversion**: nparam == 5 (Orbitrap formula)
- **Typical ionization**: NSI or ESI
- **Typical activation**: HCD
- **Version**: v64–v66

### 34.4 LTQ (Ion Trap Only)

- **Analyzer**: ITMS (code 0)
- **Profile data**: M/z domain (positive step)
- **No frequency conversion needed**
- **Typical activation**: CID
- **Typical ionization**: ESI, NSI, or EI
- **Versions**: v57–v66

### 34.5 Controller Count by Instrument

| Controllers | Typical Setup |
|-------------|---------------|
| 1 | MS only (most common) |
| 2 | MS + UV/PDA detector, or MS + second MS controller |
| 7 | Complex LC-MS setup with multiple detectors (pump, autosampler, etc.) |

---

## 35. References

### 35.1 Prior Art

1. **unfinnigan** (Gene Selkov, 2010–2012): Perl/Python reverse-engineering
   project. The most comprehensive prior binary format analysis. Supports
   versions 57, 62, 63, 64, 66. Source: https://github.com/prvst/unfinnigan

2. **ThermoRawFileParser** (CompOmics, Ghent University): C#/.NET converter
   using Thermo's proprietary RawFileReader SDK. Does not parse the binary
   format directly. Source: https://github.com/compomics/ThermoRawFileParser

3. **ms_deisotope** (mobiusklein): Python wrapper around Thermo's .NET SDK via
   pythonnet. Source: https://github.com/mobiusklein/ms_deisotope

4. **ProteoWizard/msConvert**: C++ converter using Thermo's SDK.
   Source: https://proteowizard.sourceforge.io/

### 35.2 Key Observations

- All existing open-source readers (except unfinnigan) depend on Thermo's
  proprietary RawFileReader .NET DLL — this specification aims to enable
  truly independent implementations.
- The file format has remained stable at version 66 since ~2015, covering all
  current instruments from Orbitrap Fusion through Orbitrap Astral.
- The format is backwards-compatible: version 66 files retain defunct 32-bit
  address fields for tools that may not support 64-bit addressing.

### 35.3 Validation Corpus

This specification was validated against 6 real-world RAW files from PRIDE
Archive covering diverse instruments:

| Project | Instrument | Version |
|---------|-----------|---------|
| PXD000790 | LTQ Orbitrap Elite | 66 |
| PXD006060 | Orbitrap Fusion | 66 |
| PXD006062 | Orbitrap Fusion Lumos | 66 |
| PXD021648 | Q Exactive HF | 66 |
| PXD039587 | LTQ | 66 |
| PXD043983 | LTQ Orbitrap | 66 |

All header structures (FileHeader through RawFileInfo) parsed and validated
successfully with consistent field alignments and sensible values across all
samples.
