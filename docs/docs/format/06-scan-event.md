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

The fixed body layout varies by instrument family. These observed offsets are
relative to the end of the 136-byte preamble:

| Layout | Body bytes | Acquisition window offset | Coefficient count offset |
|--------|------------|---------------------------|--------------------------|
| Fusion Lumos primary (PXD031322) | 96 | 8 | 24 |
| Q Exactive / Exploris uniform events | 136 / 144 | 64 | 80 |
| Tribrid dependent events | 208 | 120 | 144 |

For the 96-byte primary layout, using `body_size - 64` as the coefficient
count offset reads the wrong field. Short ion-trap bodies and other dependent
layouts retain their separate handling.

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

### 22.1 Common Fields (All Versions, bytes 0-40)

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
| 12-23 | unknown_byte[12-23] | |
| 24 | **activation** | 1=HCD, 4=CID |
| 25-31 | unknown_byte[25-31] | |
| 32 | wideband | 0=Off, 1=On |
| 33-39 | unknown_byte[33-39] | |
| 40 | **analyzer** | 0=ITMS, 1=TQMS, 2=SQMS, 3=TOFMS, 4=FTMS, 5=Sector |

### 22.2 Size by Version

| Version | Total Bytes |
|---------|-------------|
| v8 | 41 |
| v57, v60 | 80 |
| v62 | 120 |
| v63, v64 | 128 |
| v66 | 136 |

### 22.3 Source fragmentation energy

The preamble's source-fragmentation flag remains undecoded. Source CID energy
is obtained from `Source CID eV:` / `API Source CID Energy:` trailer fields.
If those fields are absent, a supported embedded `InstrumentSetupMethod` XML
can supply `Fragmentation_Source` with `unit="eV"`. The XML's one-based
`Segment id` / `ScanEvent id` must match explicitly typed per-scan
`Scan Segment:` / `Scan Event:` trailer IDs; the scan-index IDs are not used
as a guessed fallback. Invalid or ambiguous XML is ignored, and unsupported
XML namespaces are ignored.

Positive finite energy renders as `sid=<energy>` after ionization, with two
decimal places. A zero energy is preserved in metadata but does not establish
an off flag, so neither `sid=0` nor `!sid` is inferred. The
`opentfraw.source_cid_energy_ev` extra field carries the resolved value, and
`opentfraw.source_cid_energy_source` records `trailer` or `instrument_method`.
A method-derived value is the declared event setting, not an independently
decoded source-on flag. Other method schemas or scans without explicit trailer
IDs retain an unknown source energy.

Evidence: the public PRIDE PXD068962 `insource-CID.raw` acquisition contains
`Fragmentation_Source` 200 eV for segment 1 / event 1, matching all 3,047 scan
trailers, which omit a source-CID energy field. The ignored Rust regression
in `crates/opentfraw/tests/source_cid.rs` can be run with
`OPENTFRAW_SOURCE_CID_RAW` pointing to that acquisition. The Python regression
uses the same environment variable.

### 22.4 Filter Line Construction

OpenTFRaw renders a one-line scan filter from the preamble, the reactions,
the scan-index m/z range and the scan parameters:

```
{ANALYZER} {POLARITY} {SCAN_MODE} {IONIZATION} [sid={EV}] [d] {SCAN_TYPE} {MS_POWER} [{PRECURSOR}@{METHOD}{ENERGY} ...] [{LOW_MZ}-{HIGH_MZ}]
```

Examples:

- `FTMS + p NSI Full ms [350.0000-1500.0000]`
- `FTMS + c NSI d Full ms2 645.8311@hcd28.00 [150.0000-2000.0000]`
- `ITMS + c NSI d Full ms3 810.5000@cid35.00 265.2700@cid35.00 [100.0000-1000.0000]`
- `+ c SRM ms2 500.000@cid20.00 [100.450-100.550, 200.450-200.550]` (SRM)

**Token order.** The order follows the `stringify` methods of the Finnigan
Perl module (Gene Selkov, release 0.0206,
[metacpan.org/dist/Finnigan](https://metacpan.org/dist/Finnigan)):

| Part | Finnigan source |
|------|-----------------|
| analyzer, polarity, scan mode, ionization, `d`, scan type, `ms<n>` | [`lib/Finnigan/ScanEventPreamble.pm`, `stringify`](https://metacpan.org/release/SELKOVJR/Finnigan-0.0206/source/lib/Finnigan/ScanEventPreamble.pm#L558) |
| preamble, then precursors, then m/z range | [`lib/Finnigan/ScanEvent.pm`, `stringify`](https://metacpan.org/release/SELKOVJR/Finnigan-0.0206/source/lib/Finnigan/ScanEvent.pm#L197) |
| `{PRECURSOR}@{METHOD}{ENERGY}` | [`lib/Finnigan/Reaction.pm`, `stringify`](https://metacpan.org/release/SELKOVJR/Finnigan-0.0206/source/lib/Finnigan/Reaction.pm#L36) |
| `[{LOW_MZ}-{HIGH_MZ}]` | [`lib/Finnigan/FractionCollector.pm`, `stringify`](https://metacpan.org/release/SELKOVJR/Finnigan-0.0206/source/lib/Finnigan/FractionCollector.pm#L30) |

**Project conventions.** The following are OpenTFRaw's own conventions, not
taken from the Finnigan module:

- Numeric precision: precursor m/z and the m/z range use 4 decimals; energies
  use 2 decimals. SRM filters use 3 decimals for Q1 and the Q3 windows.
- Activation code 4 (section 31.7) renders as `hcd` on an FTMS analyzer and
  `cid` on any other analyzer. Code 1 renders as `hcd`.
- A tribrid FTMS MS2 event with two reactions, of which only one has a
  non-zero precursor m/z, renders a single `{PRECURSOR}@etd@hcd{ENERGY}`
  clause.
- SRM filters (flat-peak files) start with a fixed `+` polarity and have no
  analyzer or ionization token, because those files have no scan event to read
  them from. The `@cid{ENERGY}` clause is present only when a collision energy
  is decoded from the transition record.
- Multiple precursors are separated by a space.
- `sid={EV}` follows ionization when a positive source CID energy is known
  (section 22.3).
- The final precursor's energy is the scan parameters' activation energy
  (section 25.4) when present, otherwise the reaction's stored energy.
- Missing values fall back to `MS` for the analyzer, `+` for polarity and
  `Full` for the scan type. An unknown activation code omits the `@{METHOD}`
  clause.

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

Example: `542.3000@hcd35.00`, `480.2500@cid30.00`

The activation method name comes from the ScanEventPreamble `activation` field.
Precision and method tokens follow the conventions in section 22.4.

---

## 24. FractionCollector

M/z acquisition range for a scan event.

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| 0x00 | 8 | Float64 | low_mz | Lower M/z bound |
| 0x08 | 8 | Float64 | high_mz | Upper M/z bound |

**Total size**: 16 bytes

---

