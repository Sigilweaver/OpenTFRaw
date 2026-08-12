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
        "ms_level",
        "is_dia",
        "is_wideband",
        "polarity",
        "retention_time",
        "filter_string",
        "total_ion_current",
        "base_peak_mz",
        "base_peak_intensity",
        "low_mz",
        "high_mz",
        "ion_injection_time_ms",
        "charge",
        "precursor_mz",
        "isolation_width",
        "collision_energy",
        "mz",
        "intensity",
    }
    assert expected_keys <= scan.keys()
    assert scan["scan_number"] == raw_file.first_scan
    assert scan["ms_level"] >= 1
    assert isinstance(scan["is_dia"], bool)
    assert isinstance(scan["is_wideband"], bool)
    assert scan["polarity"] in ("+", "-", "")
    assert isinstance(scan["mz"], np.ndarray)
    assert isinstance(scan["intensity"], np.ndarray)
    assert scan["mz"].shape == scan["intensity"].shape


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
