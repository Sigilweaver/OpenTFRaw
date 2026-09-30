"""Source CID propagation from public-file method XML, with no vendor tools."""
import os
import xml.etree.ElementTree as ET

import opentfraw
import pytest


@pytest.mark.skipif(
    not os.environ.get("OPENTFRAW_SOURCE_CID_RAW"),
    reason="requires public PXD068962 insource-CID.raw fixture",
)
def test_source_cid_reaches_filters_and_canonical_records():
    raw = opentfraw.RawFile(os.environ["OPENTFRAW_SOURCE_CID_RAW"])
    method = ET.fromstring(raw.instrument_method_text())
    declared = float(method.find("./Segments/Segment/ScanEvent/Fragmentation_Source").text)
    assert declared > 0
    scan = raw.scan(raw.first_scan)
    assert f" NSI sid={declared:.2f} Full " in raw.scan_filter(raw.first_scan)
    assert scan["filter_string"] == raw.scan_filter(raw.first_scan)
    assert float(scan["extra"]["opentfraw.source_cid_energy_ev"]) == declared
    assert scan["extra"]["opentfraw.source_cid_energy_source"] == "instrument_method"
    assert next(iter(raw.iter_scans()))["filter_string"] == scan["filter_string"]
    record = next(raw.iter_records())
    assert record["extra"]["opentfraw.source_cid_energy_ev"] == scan["extra"]["opentfraw.source_cid_energy_ev"]
    assert record["extra"]["opentfraw.source_cid_energy_source"] == "instrument_method"
