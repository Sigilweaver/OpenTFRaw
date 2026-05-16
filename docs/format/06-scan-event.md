# Scan Events

_ScanEvent, ScanEventPreamble, Reaction, FractionCollector_

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

