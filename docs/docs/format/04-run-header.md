# Run Header

_RunHeader, SampleInfo_

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

