# Error and Instrument Logs

_Error Log, Instrument Log_

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

The instrument status log: temperatures, pressures, voltages, and other
instrument operating values, written every few seconds during acquisition.
It is a time series, not one record per scan. The number of records is
`SampleInfo.inst_log_length`.

### 30.1 Stream Layout

The status log belongs to the MS controller's RunHeader and is laid out
directly after it:

| Order | Type | Description |
|-------|------|-------------|
| 1 | InstID | Instrument identification (section 11), starting at the end of the RunHeader |
| 2 | GenericDataHeader | Field layout of the records; ends exactly at `RunHeader.inst_log_addr` |
| 3 | StatusLogRecord[n] | Starts at `RunHeader.inst_log_addr`; ends exactly at `RunHeader.error_log_addr` |

`RunHeader.inst_log_addr` addresses the first record, not the header. The
header is reached by reading the InstID block that follows the RunHeader.
In v57-v63 files the addresses come from the 32-bit `SampleInfo` fields; in
v64+ from the 64-bit `RunHeader` fields. On all 291 corpus files (v63, v64,
v66) the header ends exactly at `inst_log_addr` and the records end exactly
at `error_log_addr`. No v57, v60 or v62 file is in the corpus.

### 30.2 StatusLogRecord

| Order | Type | Field | Description |
|-------|------|-------|-------------|
| 1 | Float32 | time | Time the record was written, in minutes (the scan start-time scale) |
| 2 | GenericRecord | values | One value per GenericDataHeader field |

### 30.3 Time Axis

Within one acquisition the record times never decrease. Some files also
hold records on another clock, where the time drops back towards zero
partway through the log:

- Orbitrap Fusion and Fusion Lumos (21 corpus files): a block of records at
  the start of the log, with times from an earlier clock (for example 60-72
  minutes), followed by the acquisition's records starting near 0. The
  block's values match an idle instrument.
- TSQ Quantum (PXD020246): one trailing record with time 0.

OpenTFRaw takes the longest run of records whose times never decrease as
the acquisition's time axis (`RawFileReader::inst_log_time_axis`). A scan
sees the last record on that axis written at or before its start time;
scans before the first record have no status-log values. The other records
stay in `inst_log`.

### 30.4 Labels

Labels and field counts depend on the instrument family and its software
version: from 50 fields (Q Exactive) to 251 (Orbitrap Eclipse). Newer
Tribrid and TSQ Altis software appends the unit in braces
(`Spray Voltage {V}`) where older software has no unit (`Spray Voltage`).
Some labels name a section rather than a value (type Gap, for example
`====  FAIMS Device:  ====:`) and have no value in the records.

### 30.5 Decode Failures

If the header is not found after the InstID block, or either boundary check
fails, the status log is not decoded. The file still opens; the reason is
in `RawFileReader::status_log_error`, Python `RawFile.status_log_error`, and
the mzML run-level `opentfraw.status_log_error` userParam, and Python
`status_log()` raises `ValueError`.

Source: the record layout (a Float32 time followed by the header's fields)
and the InstID-then-header order follow the Finnigan Perl module by Gene
Selkov (CPAN, Finnigan-0.0206d: `lib/Finnigan/InstrumentLogRecord.pm`,
`bin/uf-log`). The time unit, boundaries and time-axis behaviour were
measured on the corpus.

---
