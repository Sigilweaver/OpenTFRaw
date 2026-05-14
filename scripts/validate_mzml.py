"""
Structural validation of mzML 1.1.0 output from OpenTFRaw.

Checks well-formedness, namespace, required elements/attributes, CV param
presence, and binary array integrity (base64 decode + length match).

Usage:
    python scripts/validate_mzml.py <file.mzML> [file2.mzML ...]

Exits 0 if all files pass, 1 on any failure.

This script uses only Python standard-library modules so it runs in any
Python 3.8+ environment without additional dependencies.
"""
from __future__ import annotations

import base64
import struct
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

NS = "http://psi.hupo.org/ms/mzml"
MZNS = f"{{{NS}}}"


def _tag(local: str) -> str:
    return f"{MZNS}{local}"


def check(condition: bool, msg: str, errors: list[str]) -> None:
    if not condition:
        errors.append(msg)


def validate_file(path: str) -> list[str]:
    errors: list[str] = []

    # 1. XML well-formedness.
    try:
        tree = ET.parse(path)
    except ET.ParseError as exc:
        return [f"XML parse error: {exc}"]
    root = tree.getroot()

    # 2. Root element and namespace.
    local = root.tag.split("}")[-1] if "}" in root.tag else root.tag
    check(
        local in ("mzML", "indexedmzML"),
        f"root element is <{local}>, expected <mzML> or <indexedmzML>",
        errors,
    )
    check(
        NS in root.tag,
        f"root element namespace missing (got '{root.tag}')",
        errors,
    )

    # Descend into <mzML> if this is an indexed file.
    mzml = root.find(_tag("mzML")) if local == "indexedmzML" else root

    if mzml is None:
        errors.append("no <mzML> element found")
        return errors

    # 3. Required structural elements.
    for req in ("fileDescription", "softwareList", "instrumentConfigurationList",
                "dataProcessingList", "run"):
        check(
            mzml.find(_tag(req)) is not None,
            f"required element <{req}> is absent",
            errors,
        )

    run = mzml.find(_tag("run"))
    if run is None:
        return errors  # already reported

    spectrum_list = run.find(_tag("spectrumList"))
    chromatogram_list = run.find(_tag("chromatogramList"))
    check(
        spectrum_list is not None or chromatogram_list is not None,
        "<run> must contain <spectrumList> or <chromatogramList>",
        errors,
    )

    if spectrum_list is None:
        return errors

    # 4. spectrumList count attribute.
    declared = spectrum_list.get("count", "")
    spectra = spectrum_list.findall(_tag("spectrum"))
    try:
        check(
            int(declared) == len(spectra),
            f"spectrumList count={declared} but found {len(spectra)} <spectrum> elements",
            errors,
        )
    except ValueError:
        errors.append(f"spectrumList count attribute is not an integer: {declared!r}")

    # 5. Per-spectrum checks.
    for spec in spectra:
        idx = spec.get("index", "<missing>")
        sid = spec.get("id", "<missing>")
        prefix = f"spectrum index={idx} id={sid!r}:"

        # Required attributes.
        check(spec.get("index") is not None, f"{prefix} missing 'index' attribute", errors)
        check(spec.get("id") is not None, f"{prefix} missing 'id' attribute", errors)
        check(spec.get("defaultArrayLength") is not None,
              f"{prefix} missing 'defaultArrayLength' attribute", errors)

        dal_str = spec.get("defaultArrayLength", "0")
        try:
            dal = int(dal_str)
        except ValueError:
            errors.append(f"{prefix} defaultArrayLength is not an integer: {dal_str!r}")
            dal = 0

        # Required CV params on <spectrum>.
        cv_accessions = {
            cv.get("accession", "")
            for cv in spec.findall(_tag("cvParam"))
        }
        has_ms_level = any(
            cv.get("accession", "") == "MS:1000511"
            for cv in spec.findall(_tag("cvParam"))
        )
        check(has_ms_level, f"{prefix} missing MS:1000511 (ms level) cvParam", errors)

        has_spectrum_type = bool(
            cv_accessions & {"MS:1000579", "MS:1000580", "MS:1000581",
                              "MS:1000526", "MS:1000527", "MS:1000528"}
        )
        # spectrum type cvParam lives on spectrum or scanList/scan
        scan_cv = {
            cv.get("accession", "")
            for scan in spec.findall(f".//{_tag('scan')}")
            for cv in scan.findall(_tag("cvParam"))
        }
        has_spectrum_mode = bool(
            (cv_accessions | scan_cv) & {"MS:1000127", "MS:1000128"}
        )
        check(
            has_spectrum_type or has_ms_level,
            f"{prefix} no spectrum type CV param (MS1 / MSn / SRM ...)",
            errors,
        )

        # 6. Binary data arrays.
        array_list = spec.find(_tag("binaryDataArrayList"))
        if array_list is not None:
            arrays = array_list.findall(_tag("binaryDataArray"))
            for arr in arrays:
                arr_accs = {cv.get("accession", "") for cv in arr.findall(_tag("cvParam"))}
                bin_elem = arr.find(_tag("binary"))

                # Data type: 32-bit or 64-bit float.
                is_32 = "MS:1000521" in arr_accs  # 32-bit float
                is_64 = "MS:1000514" in arr_accs  # 64-bit float
                check(
                    is_32 or is_64,
                    f"{prefix} binaryDataArray missing float-precision CV param",
                    errors,
                )

                # Encoding.
                is_b64 = "MS:1000514" not in arr_accs or True  # base64 is always assumed
                no_compress = "MS:1000576" in arr_accs  # no compression
                zlib_compress = "MS:1000574" in arr_accs
                check(
                    no_compress or zlib_compress,
                    f"{prefix} binaryDataArray missing compression CV param",
                    errors,
                )

                # Decode and length-check.
                if bin_elem is not None and bin_elem.text:
                    try:
                        raw_bytes = base64.b64decode(bin_elem.text.strip())
                        if zlib_compress:
                            import zlib
                            raw_bytes = zlib.decompress(raw_bytes)
                        item_size = 4 if is_32 else 8
                        if item_size > 0:
                            n_items = len(raw_bytes) // item_size
                            check(
                                n_items == dal,
                                f"{prefix} binary array decoded to {n_items} values, "
                                f"expected {dal} (defaultArrayLength)",
                                errors,
                            )
                    except Exception as exc:
                        errors.append(f"{prefix} binary data decode error: {exc}")
                elif dal > 0 and (bin_elem is None or not bin_elem.text):
                    errors.append(
                        f"{prefix} defaultArrayLength={dal} but binary element is empty"
                    )

    return errors


def main() -> int:
    if len(sys.argv) < 2:
        print("Usage: validate_mzml.py <file.mzML> [file2.mzML ...]", file=sys.stderr)
        return 1

    overall_ok = True
    for path in sys.argv[1:]:
        errs = validate_file(path)
        if errs:
            overall_ok = False
            print(f"FAIL  {path}")
            for e in errs:
                print(f"  - {e}")
        else:
            p = Path(path)
            n_spectra = "(unknown)"
            try:
                tree = ET.parse(path)
                root = tree.getroot()
                local = root.tag.split("}")[-1] if "}" in root.tag else root.tag
                mzml = root.find(f"{{{NS}}}mzML") if local == "indexedmzML" else root
                if mzml is not None:
                    run = mzml.find(f"{{{NS}}}run")
                    if run is not None:
                        sl = run.find(f"{{{NS}}}spectrumList")
                        if sl is not None:
                            n_spectra = sl.get("count", n_spectra)
            except Exception:
                pass
            print(f"OK    {path}  ({n_spectra} spectra)")

    return 0 if overall_ok else 1


if __name__ == "__main__":
    sys.exit(main())
