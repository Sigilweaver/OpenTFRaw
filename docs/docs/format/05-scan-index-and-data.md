# Scan Index and Data Packets

_Scan index, data packets, profile, peaks_

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

