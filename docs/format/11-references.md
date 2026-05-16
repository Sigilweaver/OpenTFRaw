# References

_References_

## 35. References

### 35.1 Prior Art

1. **unfinnigan** (Gene Selkov, 2010–2012): Perl/Python reverse-engineering
   project. The most comprehensive prior binary format analysis. Supports
   versions 57, 62, 63, 64, 66. Source: https://github.com/prvst/unfinnigan

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
