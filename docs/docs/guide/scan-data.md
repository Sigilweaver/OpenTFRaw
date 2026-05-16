---
sidebar_position: 2
---

# Scan data

Scan peak data is laid out one of three ways. `RawFileReader::open`
inspects the file version and first scan to pick a decoder, stored as
`scan_format: ScanDataFormat`.

| Variant        | When it applies                              | Instruments              |
| -------------- | -------------------------------------------- | ------------------------ |
| `PacketHeader` | Profile + centroid scans with a header packet| Orbitrap, ion trap       |
| `FlatV63`      | Variable-size flat peaks, version `<= 63`    | Older TSQ / SRM          |
| `FlatV66`      | Fixed 12-byte peak triplets, version `>= 64` | TSQ Quantiva, Altis SRM  |

`read_scan_peaks(file, scan_num)` returns:

```rust
pub struct ScanPeaks {
    pub mz: Vec<f64>,
    pub intensity: Vec<f32>,
}
```

For full layouts of each packet variant see
[Scan index and data](../format/scan-index-and-data).
