"""``to_mzml(skip_unreadable=True)`` on a public file with unreadable scans."""
import os
import xml.etree.ElementTree as ET

import opentfraw
import pytest


@pytest.mark.skipif(
    not os.environ.get("OPENTFRAW_TSQ_VANTAGE_RAW"),
    reason="requires public PXD020246 TSQVantage.RAW fixture",
)
def test_skip_unreadable_reports_left_out_scans(tmp_path):
    # PXD020246 TSQVantage.RAW: 4 of 35,098 scans have a flat-peak record
    # that matches no known layout.
    raw = opentfraw.RawFile(os.environ["OPENTFRAW_TSQ_VANTAGE_RAW"])
    assert raw.num_scans == 35098

    with pytest.raises(OSError):
        raw.to_mzml(str(tmp_path / "fail.mzML"))

    out = tmp_path / "skip.mzML"
    with pytest.warns(UserWarning, match=r"left out 4 unreadable"):
        skipped = raw.to_mzml(str(out), skip_unreadable=True)
    assert skipped == 4
    ns = {"m": "http://psi.hupo.org/ms/mzml"}
    spectra = ET.parse(out).getroot().findall(".//m:spectrum", ns)
    assert len(spectra) == 35098 - 4
