"""Descriptor bit 21 on published acquisitions, without vendor-derived output."""
import os

import numpy as np
import opentfraw
import pytest

LOCK_MASS_BIT = 1 << 21
LOCK_MASS_LABELS = [f"Lock Mass #{n} (m/z):" for n in (1, 2, 3)]


@pytest.mark.parametrize(
    "variable,scans,by_label",
    [
        ("OPENTFRAW_LOCK_MASS_PLUS_RAW", 138, [0, 3, 135]),
        ("OPENTFRAW_LOCK_MASS_HFX_RAW", 27832, [3559, 0, 0]),
        ("OPENTFRAW_LOCK_MASS_EXPLORIS_RAW", 51254, [16033, 0, 0]),
    ],
)
def test_public_lock_mass_centroids_carry_bit_21(variable, scans, by_label):
    path = os.environ.get(variable)
    if not path:
        pytest.skip(f"set {variable} to the public fixture listed in CORPUS.md")
    raw = opentfraw.RawFile(path)
    assert raw.num_scans == scans
    counts = dict.fromkeys(LOCK_MASS_LABELS, 0)
    for n in range(raw.first_scan, raw.last_scan + 1):
        params = raw.scan_parameters(n)
        labels = raw.centroid_labels(n)
        descriptor = labels["descriptor"]
        assert descriptor.dtype == np.uint32
        assert len(descriptor) == len(labels["mz"])
        # As many centroids flagged as lock masses found: one or none.
        flagged = np.flatnonzero(descriptor & LOCK_MASS_BIT)
        assert len(flagged) == params["Number of LM Found:"] <= 1
        # Centroid m/z is stored as float32 in these files, and the flagged
        # one is the float32 nearest a lock mass of the trailer.
        for k in flagged:
            mz = np.float32(labels["mz"][k])
            matches = [m for m in LOCK_MASS_LABELS if np.float32(params[m]) == mz]
            assert len(matches) == 1
            counts[matches[0]] += 1
    assert list(counts.values()) == by_label
