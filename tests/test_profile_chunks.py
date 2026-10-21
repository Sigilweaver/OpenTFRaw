"""profile_chunks() on published Q Exactive Plus acquisitions that store a
per-chunk m/z correction, checked against profile() alone."""
import os

import numpy as np
import opentfraw
import pytest


@pytest.mark.parametrize(
    "variable,scans,chunks",
    [
        ("OPENTFRAW_LOCK_MASS_PLUS_RAW", 138, 97_664),
        ("OPENTFRAW_SOURCE_CID_PLUS_RAW", 6_674, 337_989),
    ],
)
def test_public_profile_chunks_rebuild_profile(variable, scans, chunks):
    path = os.environ.get(variable)
    if not path:
        pytest.skip(f"set {variable} to the public fixture listed in CORPUS.md")
    raw = opentfraw.RawFile(path)
    assert raw.num_scans == scans
    total = 0
    for n in range(raw.first_scan, raw.last_scan + 1):
        layout = raw.profile_chunks(n)
        fudge = layout["fudge"]
        # Every chunk stores a correction, and none is zero.
        assert np.isfinite(fudge).all()
        assert fudge.all()
        # One chunk per centroid (see the descriptor section of CORPUS.md).
        assert len(fudge) == len(raw.peaks(n)[0])
        total += len(fudge)

        # Each point's frequency, converted with the five coefficients
        # ([_, _, A, B, C]: A + B/f^2 + C/f^4), plus its chunk's fudge, is
        # profile()'s m/z.
        mz, _ = raw.profile(n)
        first_bin = layout["first_bin"].astype(np.int64)
        n_points = layout["n_points"].astype(np.int64)
        bins = np.concatenate([b + np.arange(m) for b, m in zip(first_bin, n_points)])
        f2 = (layout["first_value"] + bins * layout["step"]) ** 2
        assert len(layout["coefficients"]) == 5
        a, b, c = layout["coefficients"][2:]
        rebuilt = a + b / f2 + c / f2**2 + np.repeat(fudge.astype(np.float64), n_points)
        np.testing.assert_allclose(rebuilt, mz, rtol=1e-12)
    assert total == chunks
