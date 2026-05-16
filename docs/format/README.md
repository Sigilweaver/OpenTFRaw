# OpenTFRaw binary format specification

This directory contains the reverse-engineered specification of the
Thermo Fisher RAW mass spectrometry file format as decoded by
OpenTFRaw.

| File | Topic |
| ---- | ----- |
| [00-overview.md](00-overview.md) | Format overview, conventions, version differences, instrument-specific notes |
| [01-file-layout.md](01-file-layout.md) | Top-level file layout, FileHeader, AuditTag |
| [02-sample-and-sequence.md](02-sample-and-sequence.md) | SeqRow, InjectionData, ASInfo |
| [03-raw-file-info.md](03-raw-file-info.md) | RawFileInfo, RawFileInfoPreamble, InstID, MethodFile |
| [04-run-header.md](04-run-header.md) | RunHeader, SampleInfo |
| [05-scan-index-and-data.md](05-scan-index-and-data.md) | Scan index, scan-data packets, packet headers, profile chunks, peaks |
| [06-scan-event.md](06-scan-event.md) | ScanEvent, ScanEventPreamble, Reaction, FractionCollector |
| [07-scan-parameters.md](07-scan-parameters.md) | ScanParameters, GenericDataHeader, GenericDataDescriptor, GenericRecord |
| [08-logs.md](08-logs.md) | Error Log, Instrument Log |
| [09-enumerations.md](09-enumerations.md) | Enumerations (instrument model IDs, scan flags, etc.) |
| [10-frequency-to-mz.md](10-frequency-to-mz.md) | Frequency-to-m/z conversion |
| [11-references.md](11-references.md) | References and prior art |

The specification was developed by binary analysis of a multi-instrument
PRIDE corpus and is validated against ProteoWizard `msconvert` output.
