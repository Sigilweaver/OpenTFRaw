# RAW File Info

_RawFileInfo, RawFileInfoPreamble, InstID, MethodFile_

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
| 0x06 | 2 | UInt16 | month | Month (1-12) |
| 0x08 | 2 | UInt16 | day_of_week | Day of week (0=Sunday) |
| 0x0A | 2 | UInt16 | day | Day of month (1-31) |
| 0x0C | 2 | UInt16 | hour | Hour (0-23) |
| 0x0E | 2 | UInt16 | minute | Minute (0-59) |
| 0x10 | 2 | UInt16 | second | Second (0-59) |
| 0x12 | 2 | UInt16 | millisecond | Millisecond (0-999) |

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

### 10.3 Version 57-63 (32-bit addresses)

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

