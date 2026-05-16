---
sidebar_position: 3
---

# Instrument families

`DeviceFamily` (in `src/device.rs`) classifies the originating
instrument from a combination of the audit tag, the instrument-method
path, and the analyzer reported by the first scan event. The detection
is heuristic and best-effort.

Current taxonomy:

| Family             | Examples                                   |
| ------------------ | ------------------------------------------ |
| `IonTrap`          | LCQ, LTQ, LTQ Velos                        |
| `OrbitrapClassic`  | LTQ Orbitrap, Orbitrap XL, Velos Pro       |
| `OrbitrapQ`        | Q Exactive, Q Exactive HF, HF-X            |
| `OrbitrapTribrid`  | Fusion, Fusion Lumos, Eclipse, Ascend      |
| `OrbitrapExploris` | Exploris 240/480, Exploris GC             |
| `OrbitrapAstral`   | Orbitrap Astral                            |
| `TripleQuad`       | TSQ Quantiva, Altis, Endura                |
| `SingleQuad`       | ISQ, MSQ Plus                              |
| `Other`            | Anything that doesn't match a known family |

Family classification is informational only - the parser itself does
not branch on it. The scan-data dispatch is driven by `scan_format`.
