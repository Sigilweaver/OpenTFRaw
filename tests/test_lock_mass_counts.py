"""Distinct counts from published acquisitions, without vendor-derived output."""
import os
import xml.etree.ElementTree as ET

import opentfraw
import pytest


MATCHED = "opentfraw.number_of_matched_lock_masses"
CONFIGURED = "opentfraw.number_of_configured_lock_masses"
LEGACY = "opentfraw.number_of_lock_masses"


def test_lock_mass_extra_keys_are_registered():
    keys = set(opentfraw.extra_field_keys())
    assert {MATCHED, CONFIGURED} <= keys
    assert not any(k.startswith("opentfraw.status.") for k in keys)


@pytest.mark.parametrize(
    "variable,configured,zero_scan",
    [
        ("OPENTFRAW_LOCK_MASS_PLUS_RAW", 9, None),
        ("OPENTFRAW_LOCK_MASS_HFX_RAW", 1, 4034),
        ("OPENTFRAW_LOCK_MASS_EXPLORIS_RAW", 1, 23634),
    ],
)
def test_public_counts_and_zero_match_correction(variable, configured, zero_scan):
    path = os.environ.get(variable)
    if not path:
        pytest.skip(f"set {variable} to the public fixture listed in CORPUS.md")
    raw = opentfraw.RawFile(path)
    first = raw.scan(raw.first_scan)
    assert first["extra"][MATCHED] == "1"
    assert first["extra"][CONFIGURED] == str(configured)
    assert first["extra"][LEGACY] == "1"
    assert next(raw.iter_records())["extra"][MATCHED] == "1"
    if zero_scan is not None:
        scan = raw.scan(zero_scan)
        assert scan["ms_level"] == 1
        assert scan["extra"][MATCHED] == "0"
        assert scan["extra"][CONFIGURED] == "1"
        assert scan["extra"][LEGACY] == "0"
        assert float(scan["extra"]["opentfraw.lock_mass_correction_ppm"]) != 0
        assert scan["extra"]["opentfraw.lock_mass_correction_ppm"] == raw.scan(
            zero_scan - 1
        )["extra"]["opentfraw.lock_mass_correction_ppm"]


@pytest.mark.skipif(
    not os.environ.get("OPENTFRAW_LOCK_MASS_PLUS_RAW"),
    reason="requires public MTBLS5657 negative-mode Q Exactive Plus fixture",
)
def test_public_plus_counts_reach_canonical_records_and_mzml(tmp_path):
    raw = opentfraw.RawFile(os.environ["OPENTFRAW_LOCK_MASS_PLUS_RAW"])
    records = list(raw.iter_records())
    assert len(records) == 138
    for record in records:
        assert record["extra"][MATCHED] == "1"
        assert record["extra"][CONFIGURED] == "9"
        assert record["extra"][LEGACY] == "1"
    out = tmp_path / "lock-mass.mzML"
    raw.to_mzml(str(out), extra_fields=[MATCHED, CONFIGURED, LEGACY])
    ns = {"m": "http://psi.hupo.org/ms/mzml"}
    spectra = ET.parse(out).findall(".//m:spectrum", ns)
    assert len(spectra) == 138
    for spectrum in spectra:
        extra = {
            p.attrib["name"]: p.attrib["value"]
            for p in spectrum.findall("m:userParam", ns)
        }
        assert extra[MATCHED] == "1"
        assert extra[CONFIGURED] == "9"
        assert extra[LEGACY] == "1"
