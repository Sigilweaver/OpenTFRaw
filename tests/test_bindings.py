"""Smoke tests for the opentfraw Python bindings (crates/opentfraw-py).

These exercise every public method/property on `opentfraw.RawFile` by
actually importing and calling the compiled extension from Python (the
Rust core already has its own `cargo test` coverage; this suite is only
about the PyO3 boundary: does each binding return the right shape/type
and not crash).

Per CONTRIBUTING.md's clean-room policy, nothing here asserts against
exact values that could only be known from vendor software - only
shape, type, and non-emptiness where that's a structural guarantee.
"""

from __future__ import annotations

import re
import xml.etree.ElementTree as ET

import numpy as np
import pytest

import opentfraw


def test_module_version():
    assert isinstance(opentfraw.__version__, str)
    assert opentfraw.__version__


def test_open_and_repr(raw_file):
    assert isinstance(raw_file, opentfraw.RawFile)
    r = repr(raw_file)
    assert isinstance(r, str)
    assert "RawFile" in r


def test_path(raw_file, raw_file_path):
    assert raw_file.path == str(raw_file_path)


def test_num_scans_and_len(raw_file):
    assert isinstance(raw_file.num_scans, int)
    assert raw_file.num_scans > 0
    assert len(raw_file) == raw_file.num_scans


def test_first_last_scan(raw_file):
    assert isinstance(raw_file.first_scan, int)
    assert isinstance(raw_file.last_scan, int)
    assert raw_file.first_scan >= 1
    assert raw_file.last_scan >= raw_file.first_scan
    assert raw_file.last_scan - raw_file.first_scan + 1 == raw_file.num_scans


def test_instrument_model(raw_file):
    assert raw_file.instrument_model is None or isinstance(
        raw_file.instrument_model, str
    )


def test_created(raw_file):
    assert raw_file.created is None or isinstance(raw_file.created, float)


def test_ended(raw_file):
    assert raw_file.ended is None or isinstance(raw_file.ended, float)


def test_acquisition_date(raw_file):
    assert raw_file.acquisition_date is None or isinstance(
        raw_file.acquisition_date, float
    )


def test_sample_info(raw_file):
    info = raw_file.sample_info
    assert isinstance(info, dict)
    expected_keys = {
        "id",
        "comment",
        "vial",
        "injection_volume",
        "sample_weight",
        "sample_volume",
        "istd_amount",
        "dilution_factor",
        "user_labels",
        "label_headings",
        "user_labels_by_heading",
        "inst_method",
        "proc_method",
        "file_name",
        "path",
    }
    assert expected_keys <= info.keys()
    assert isinstance(info["user_labels"], list)
    assert all(isinstance(x, str) for x in info["user_labels"])
    assert isinstance(info["label_headings"], list)
    assert len(info["label_headings"]) == len(info["user_labels"]) == 5
    assert all(isinstance(x, str) for x in info["label_headings"])
    assert info["user_labels_by_heading"] == dict(
        zip(info["label_headings"], info["user_labels"])
    )


def test_computer_name(raw_file):
    assert isinstance(raw_file.computer_name, str)


def test_controller_count(raw_file):
    assert isinstance(raw_file.controller_count, int)
    assert raw_file.controller_count >= 1


def test_scan_filter(raw_file):
    result = raw_file.scan_filter(raw_file.first_scan)
    assert result is None or isinstance(result, str)
    # Out-of-range scan numbers should not raise; they return None.
    assert raw_file.scan_filter(raw_file.last_scan + 1_000_000) is None


def test_error_log(raw_file):
    log = raw_file.error_log()
    assert isinstance(log, list)
    for entry in log:
        assert isinstance(entry, dict)
        assert isinstance(entry["time"], float)
        assert isinstance(entry["message"], str)


def test_scan_parameters(raw_file):
    params = raw_file.scan_parameters(raw_file.first_scan)
    assert params is None or isinstance(params, dict)
    if params:
        assert all(isinstance(k, str) for k in params)


def test_status_log(raw_file):
    log = raw_file.status_log(raw_file.first_scan)
    assert log is None or isinstance(log, dict)
    if log:
        assert all(isinstance(k, str) for k in log)


def test_peaks(raw_file):
    mz, intensity = raw_file.peaks(raw_file.first_scan)
    assert isinstance(mz, np.ndarray)
    assert isinstance(intensity, np.ndarray)
    assert mz.dtype == np.float64
    assert intensity.dtype == np.float32
    assert mz.shape == intensity.shape


def test_peaks_out_of_range_raises(raw_file):
    with pytest.raises(Exception):
        raw_file.peaks(raw_file.last_scan + 1_000_000)


def test_profile(raw_file):
    try:
        mz, intensity = raw_file.profile(raw_file.first_scan)
    except ValueError as e:
        pytest.skip(f"profile() unsupported for this file's scan format: {e}")
        return
    assert isinstance(mz, np.ndarray)
    assert isinstance(intensity, np.ndarray)
    assert mz.dtype == np.float64
    assert intensity.dtype == np.float64
    assert mz.shape == intensity.shape


def test_centroid_labels(raw_file):
    labels = raw_file.centroid_labels(raw_file.first_scan)
    assert isinstance(labels, dict)
    expected_keys = {
        "mz",
        "intensity",
        "resolution",
        "noise",
        "baseline",
        "signal_to_noise",
    }
    assert expected_keys <= labels.keys()
    n = len(labels["mz"])
    for key in expected_keys:
        assert isinstance(labels[key], np.ndarray)
        assert len(labels[key]) == n
    assert labels["mz"].dtype == np.float64
    assert labels["intensity"].dtype == np.float32


def test_scan(raw_file):
    scan = raw_file.scan(raw_file.first_scan)
    assert isinstance(scan, dict)
    expected_keys = {
        "scan_number",
        "scan_event",
        "scan_segment",
        "data_size",
        "ms_level",
        "is_dia",
        "is_wideband",
        "polarity",
        "scan_mode",
        "analyzer",
        "retention_time",
        "filter_string",
        "total_ion_current",
        "base_peak_mz",
        "base_peak_intensity",
        "low_mz",
        "high_mz",
        "ion_injection_time_ms",
        "faims_cv",
        "charge",
        "precursor_mz",
        "isolation_target_mz",
        "isolation_width",
        "collision_energy",
        "collision_energy_is_nce",
        "activation",
        "master_scan_number",
        "extra",
        "mz",
        "intensity",
    }
    assert expected_keys <= scan.keys()
    assert scan["scan_number"] == raw_file.first_scan
    assert scan["ms_level"] >= 1
    assert isinstance(scan["is_dia"], bool)
    assert isinstance(scan["is_wideband"], bool)
    assert scan["polarity"] in ("+", "-", "")
    assert scan["scan_mode"] in ("centroid", "profile", None)
    for key in ("scan_event", "scan_segment"):
        assert isinstance(scan[key], int)
        assert 0 <= scan[key] <= 0xFFFF
    assert isinstance(scan["data_size"], int)
    assert scan["data_size"] >= 0
    assert isinstance(scan["collision_energy_is_nce"], bool)
    assert isinstance(scan["extra"], dict)
    assert set(scan["extra"]) <= set(opentfraw.extra_field_keys())
    assert isinstance(scan["mz"], np.ndarray)
    assert isinstance(scan["intensity"], np.ndarray)
    assert scan["mz"].shape == scan["intensity"].shape


def test_precursor_mz_consistent_with_filter(raw_file):
    """MS2 scans whose filter string carries a precursor also expose it
    as ``precursor_mz``, and the two roughly agree.

    Both values are derived purely from the file (trailer / scan event),
    so this is a self-consistency check, not a vendor-value assertion.
    The trailer value is monoisotopic-corrected while the filter shows
    the isolation target, so they can differ by isotope spacings (n/z);
    the tolerance only guards against wiring in an unrelated field.
    """
    checked = 0
    for scan in raw_file.iter_scans():
        if scan["ms_level"] < 2 or not scan["filter_string"]:
            continue
        match = re.search(r"ms2 (\d+\.\d+)@", scan["filter_string"])
        if match is None:
            continue
        assert scan["precursor_mz"] is not None
        assert scan["precursor_mz"] == pytest.approx(float(match.group(1)), abs=5.0)
        checked += 1
    if checked == 0:
        pytest.skip("fixture file has no MS2 scans with a filter precursor")


def test_scan_mode_consistent_with_filter(raw_file):
    """``scan_mode`` matches the scan-data token (``c`` / ``p``) of the
    filter string, which is rendered from the same scan event."""
    token = {"centroid": "c", "profile": "p"}
    checked = 0
    for scan in raw_file.iter_scans():
        if scan["scan_mode"] is None or not scan["filter_string"]:
            continue
        assert scan["filter_string"].split()[2] == token[scan["scan_mode"]]
        checked += 1
    assert checked > 0


def test_ms1_scans_have_no_precursor(raw_file):
    """MS1 scans carry no precursor keys, whatever the trailer holds."""
    precursor_keys = (
        "charge",
        "precursor_mz",
        "isolation_target_mz",
        "isolation_width",
        "collision_energy",
        "activation",
        "master_scan_number",
    )
    for scan in raw_file.iter_scans():
        if scan["ms_level"] != 1:
            continue
        for key in precursor_keys:
            assert scan[key] is None, (scan["scan_number"], key)
        assert scan["collision_energy_is_nce"] is False


def test_scan_precursor_matches_mzml(raw_file, tmp_path):
    """``scan()`` and ``to_mzml()`` share one derivation, so the precursor
    m/z and collision energy they report for each scan must agree."""
    out_path = tmp_path / "out.mzML"
    raw_file.to_mzml(str(out_path))
    ns = {"m": "http://psi.hupo.org/ms/mzml"}
    from_mzml = {}
    for spectrum in ET.parse(out_path).getroot().iterfind(".//m:spectrum", ns):
        scan_number = int(spectrum.get("id").rsplit("scan=", 1)[1])
        values = {}
        for param in spectrum.iterfind("./m:precursorList//m:cvParam", ns):
            if param.get("accession") == "MS:1000744":
                values["precursor_mz"] = float(param.get("value"))
            elif param.get("accession") == "MS:1000045":
                values["collision_energy"] = float(param.get("value"))
        from_mzml[scan_number] = values
    # mzML writes m/z with 6 decimals and energies with 2.
    tolerance = {"precursor_mz": 1e-6, "collision_energy": 1e-2}
    checked = 0
    for scan in raw_file.iter_scans():
        if scan["ms_level"] < 2:
            continue
        expected = from_mzml[scan["scan_number"]]
        for key, abs_tol in tolerance.items():
            if key in expected:
                assert scan[key] == pytest.approx(expected[key], abs=abs_tol), (
                    scan["scan_number"],
                    key,
                )
                checked += 1
    if checked == 0:
        pytest.skip("fixture file has no MS2 precursors")


def _opentfraw_user_params(path):
    ns = {"m": "http://psi.hupo.org/ms/mzml"}
    root = ET.parse(path).getroot()
    return {
        p.get("name")
        for p in root.iterfind(".//m:spectrum/m:userParam", ns)
        if p.get("name").startswith("opentfraw.")
    }


def test_extra_field_keys():
    keys = opentfraw.extra_field_keys()
    assert keys
    assert len(keys) == len(set(keys))
    assert all(k.startswith("opentfraw.") for k in keys)


def test_to_mzml_extra_field_selection(raw_file, tmp_path):
    carried = set()
    for scan in raw_file.iter_scans():
        carried |= set(scan["extra"])
    assert carried, "fixture scans carry no extra fields"
    some = sorted(carried)[0]

    out = tmp_path / "all.mzML"
    raw_file.to_mzml(str(out))
    assert _opentfraw_user_params(out) == carried

    out = tmp_path / "none.mzML"
    raw_file.to_mzml(str(out), extra_fields=[])
    assert _opentfraw_user_params(out) == set()

    out = tmp_path / "only.mzML"
    raw_file.to_mzml(str(out), extra_fields=[some])
    assert _opentfraw_user_params(out) == {some}

    out = tmp_path / "except.mzML"
    raw_file.to_mzml(str(out), exclude_extra_fields=[some])
    assert _opentfraw_user_params(out) == carried - {some}


def test_to_mzml_rejects_bad_extra_field_arguments(raw_file, tmp_path):
    out = str(tmp_path / "out.mzML")
    with pytest.raises(ValueError, match="unknown extra field"):
        raw_file.to_mzml(out, extra_fields=["opentfraw.no_such_field"])
    with pytest.raises(ValueError, match="not both"):
        raw_file.to_mzml(out, extra_fields=[], exclude_extra_fields=[])


def test_to_mzml_references_scan_analyzers(raw_file, tmp_path):
    """Each spectrum points at the instrument configuration of its analyzer."""
    out = tmp_path / "out.mzML"
    raw_file.to_mzml(str(out))
    ns = {"m": "http://psi.hupo.org/ms/mzml"}
    root = ET.parse(out).getroot()
    configs = {c.get("id") for c in root.iterfind(".//m:instrumentConfiguration", ns)}
    analyzers = {s["analyzer"] for s in raw_file.iter_scans() if s["analyzer"]}
    assert len(configs) == 1 + len(analyzers)
    refs = {
        scan.get("instrumentConfigurationRef")
        for scan in root.iterfind(".//m:spectrum//m:scan", ns)
    }
    assert refs <= configs


def test_iter_scans(raw_file):
    scans = raw_file.iter_scans()
    assert isinstance(scans, list)
    assert len(scans) == raw_file.num_scans
    assert scans, "expected at least one scan"
    for scan in scans:
        assert isinstance(scan, dict)
        assert "mz" in scan and "intensity" in scan


def test_controllers(raw_file):
    controllers = raw_file.controllers()
    assert isinstance(controllers, list)
    assert len(controllers) >= 1
    expected_keys = {
        "index",
        "is_ms_controller",
        "controller_type",
        "first_scan",
        "last_scan",
        "start_time",
        "end_time",
    }
    for c in controllers:
        assert isinstance(c, dict)
        assert expected_keys <= c.keys()
        assert isinstance(c["is_ms_controller"], bool)
    assert any(c["is_ms_controller"] for c in controllers)


def test_instrument_method_text(raw_file):
    text = raw_file.instrument_method_text()
    assert text is None or isinstance(text, str)


def test_to_mzml(raw_file, tmp_path):
    out_path = tmp_path / "out.mzML"
    raw_file.to_mzml(str(out_path))
    assert out_path.exists()
    assert out_path.stat().st_size > 0
    # Structural check only (well-formed XML, root element name) - no
    # comparison against vendor-derived expected values.
    tree = ET.parse(out_path)
    root = tree.getroot()
    assert root.tag.endswith("mzML")
