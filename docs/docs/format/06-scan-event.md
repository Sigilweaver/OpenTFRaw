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

### 21.3 Version 66 Record Sizes

The v66 records lie back to back from `scan_trailer_addr + 4` to
`RunHeader.scan_params_addr`, and no record stores its own size. The reader
tries these layouts in order and uses the first that fits:

1. Tribrid sizes: 232-byte primary records and 288- or 344-byte dependent
   records. Used only if walking every record finds plausible preamble fields
   (section 22.1) and ends exactly at `scan_params_addr`.
2. Named events (below), walked and checked the same way.
3. One size for every record, when the stream length divides evenly by the
   number of events.
4. The 232/344 tribrid sizes by stream length alone, and otherwise the floor of
   the average record size.

**Named events.** In the 136-byte Exploris body, the last UInt32 (body offset
132) counts the UTF-16LE characters stored right after the body. The count is
0 on most events. Targeted-MS2 events from a method with a named inclusion list
store the name of the event's inclusion-list entry there, so their records are
`272 + 2 * count` bytes, and one stream mixes record sizes. The fixed part of
the body keeps the uniform layout above (acquisition window at offset 64,
coefficient count at 80). The reader walks the records by the count and uses
the walk only if at least one record carries a name and the walk ends exactly
at `scan_params_addr`. A stream of named records can happen to divide evenly by
the number of events, so the walk is tried before the uniform size.

Evidence: an Orbitrap Exploris 120 targeted-MS2 acquisition with a 9-entry
named inclusion list (not public; 462 records of 272, 286, 288 and 290 bytes).
The walk ends exactly at `scan_params_addr`, and every name reads as text.
Synthetic streams in the `reader.rs` unit tests cover mixed record sizes, a
stream that divides evenly, and walks that do not end at the stream end.

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

Byte 24 does not carry ETD: on ETD scans it holds 4, the same value as on
the MS1 scans of the same file. Electron-transfer scans are marked by
byte 120 (section 22.5).

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
can supply `Fragmentation_Source` with `unit="eV"`. The XML is read as the
`Data` stream of the embedded method file, by its sector chain and recorded
length (see MethodFile on the RawFileInfo page). The longest block of text in
the metadata region is used only when the region holds no such stream. The
XML's one-based
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

The stream read is covered by synthetic method files in the unit tests: one
whose `Data` stream has its last sector after a directory sector, so that a
text scan loses the closing `>` of the document, and one with text left after
the stream's recorded end.

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
- An EThcD event (section 22.5) renders a single
  `{PRECURSOR}@etd@hcd{ENERGY}` clause, where the energy is the scan
  parameters' activation energy (the supplemental HCD energy).
- An ETD event renders `@etd` followed by the reaction's stored value,
  which on the corpus ETD files is the reaction time in ms (section 22.5).
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

### 22.5 Electron-transfer flag (byte 120, v66)

In a 136-byte (v66) preamble, byte 120 is 1 on electron-transfer MSn
events. OpenTFRaw reads it only on MSn events whose byte 24 is a named code
(1 or 4), and decodes such an event as ETD. An ETD MS2 event read out in
the Orbitrap (FTMS) whose body has two or more reactions, exactly one of
them with a precursor m/z, is decoded as EThcD; the precursor-less reaction
is the supplemental step.

| Value | Seen on |
|-------|---------|
| 0 | MS1 and CID/HCD MSn events of Velos-family and tribrid instruments |
| 1 | ETD and EThcD MSn events |
| 2 | Every event of Q Exactive, Exploris and Astral files |

Evidence (PRIDE public files, every scan event of every corpus file read):

- PXD006767 `EFM4_AspN_ETD_22032016.raw`,
  `EFM4_lysargiNase_ETD_22032016.raw` and
  `EFM7_lysargiNase_ETD_22032016.raw` (Orbitrap Velos Pro, v66). The
  embedded instrument method says `Activation Type: ETD` with supplemental
  activation enabled. Byte 120 is 1 on all 10,836 MS2 events (3,415, 3,266
  and 4,155) and 0 on all 6,939 MS1 events; byte 24 is 4 on both. Each
  event stores one reaction whose energy field is 130, equal to the method's
  `Activation Time: 130.000`. In CID files the same field equals the
  method's normalized collision energy (PXD051336 Velos Pro: 35 and
  `Normalized Coll. Energy: 35.0`). So on ETD events this value is a
  reaction time, and the mzML writer does not report it as a collision
  energy.
- PXD077396 `EC-OT30-25-190-EThcD-01.raw`, `-03.raw` and `-09.raw`
  (Orbitrap Eclipse) and PXD070648 `..._Trypsin_Chymotrypsin_1019_a.raw` and
  `..._991_a.raw` (Orbitrap Fusion Lumos). The method text says
  `Is EThcD Active = True`. Byte 120 is 1 on all 59,034 MS2 events, each
  with two reactions of which only the first has a precursor m/z. The first
  reaction's energy field scales with 1/z^2 of the scan's `Charge State:`
  (PXD077396 `-01.raw`: 95.834 at z=2, 42.593 at z=3, 23.959 at z=4), as a
  charge-dependent ETD reaction time would, so it is not reported as a
  collision energy either.
- Files whose method declares only CID or HCD, for example PXD051336
  (Velos Pro), PXD000790 and PXD044191 (Elite), PXD032800 (Eclipse) and
  PXD006062 (Fusion Lumos), have 0 on every event.

The corpus has no ETD file in format versions 63 or 64, so byte 120 is not
read there. The 38 corpus files whose scan events are misread (see the
README's known issues) also show 1 at byte 120, mostly with byte 24 zero or
out of range. Requiring a named byte-24 code leaves 132 of their MSn scans
decoded as ETD: 131 in PXD064117 and PXD067311, whose methods enable EThcD,
and 1 in PXD039587, whose method declares only CID. The corpus files carry
no stored scan filter text, so filters could not be used as evidence.


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

The activation method name comes from the ScanEventPreamble `activation`
field (byte 24) and the electron-transfer flag (byte 120, section 22.5).
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

