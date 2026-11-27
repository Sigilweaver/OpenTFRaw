"""TSQ Vantage scans whose peak count is settled against the scan index."""
import os
import warnings
import xml.etree.ElementTree as ET

import opentfraw
import pytest

requires_vantage = pytest.mark.skipif(
    not os.environ.get("OPENTFRAW_TSQ_VANTAGE_RAW"),
    reason="requires public PXD020246 TSQVantage.RAW fixture",
)

# PXD020246 TSQVantage.RAW: 35,098 scans. The default peak count disagrees
# with the scan index on 1,969 of them (4 of which did not decode at all in
# 3.1.0); exactly one other count agrees on each.
NUM_SCANS = 35098
RECOVERED = 1969
# The first scan whose default count is wrong (m/z 0.0, then -3.5e14).
FIRST_RECOVERED_SCAN = 995


@requires_vantage
def test_to_mzml_recovers_the_peak_count_and_says_so(tmp_path):
    raw = opentfraw.RawFile(os.environ["OPENTFRAW_TSQ_VANTAGE_RAW"])
    assert raw.num_scans == NUM_SCANS

    out = tmp_path / "out.mzML"
    with pytest.warns(
        UserWarning, match=rf"wrote {RECOVERED} record\(s\) .* warning opentfraw\.peak_count_recovered"
    ):
        left_out = raw.to_mzml(str(out))
    assert left_out == 0
    root = ET.parse(out).getroot()
    ns = {"m": "http://psi.hupo.org/ms/mzml"}
    assert len(root.findall(".//m:spectrum", ns)) == NUM_SCANS
    flagged = [
        p
        for p in root.iter("{http://psi.hupo.org/ms/mzml}userParam")
        if p.get("name", "").endswith("warning opentfraw.peak_count_recovered")
    ]
    assert len(flagged) == RECOVERED


@requires_vantage
def test_peaks_warns_for_a_recovered_scan_only():
    raw = opentfraw.RawFile(os.environ["OPENTFRAW_TSQ_VANTAGE_RAW"])
    with pytest.warns(UserWarning, match="peak count was recovered"):
        mz, intensity = raw.peaks(FIRST_RECOVERED_SCAN)
    assert len(mz) == len(intensity) > 0
    assert (mz[1:] >= mz[:-1]).all()
    # Scan 1 agrees with the index with the default count.
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        raw.peaks(1)


@requires_vantage
def test_iter_records_flags_recovered_scans():
    raw = opentfraw.RawFile(os.environ["OPENTFRAW_TSQ_VANTAGE_RAW"])
    flagged = sum(
        1
        for record in raw.iter_records()
        if any(
            w["kind"] == "opentfraw.peak_count_recovered"
            for w in record.get("warnings", [])
        )
    )
    assert flagged == RECOVERED
