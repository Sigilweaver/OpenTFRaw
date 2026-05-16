# Overview

_Overview, Conventions, Version Differences, Instrument-Specific Notes_

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

