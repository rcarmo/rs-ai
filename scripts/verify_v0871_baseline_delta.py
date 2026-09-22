#!/usr/bin/env python3
"""Verify exact full-record v0.87.0 -> v0.87.1 model deltas."""
from __future__ import annotations

import argparse
import sys
import tempfile
from pathlib import Path

import verify_v0851_baseline_delta as baseline

OLD_PACKAGE = "@earendil-works/pi-ai@0.87.0"
NEW_PACKAGE = "@earendil-works/pi-ai@0.87.1"
OLD_PACKAGE_SHA256 = "f2adf9de809d035f76f8dadf3d148720ebeef4606a848ab36ee834d895ae812f"
NEW_PACKAGE_SHA256 = "35b4432f27cc2665f86beebb9af6a39b1251970883c3044bd8be4f4e8c731ca0"
EXPECTED_TEXT = (62, 12, 35)
EXPECTED_IMAGE = (1, 0, 0)
EXPECTED_COUNTS = (1445, 1495, 54, 55)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--fault", choices=["", "baseline-record"], default="")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="rs-ai-v0871-delta-") as tmp:
        work = Path(tmp)
        old_pkg = baseline.extract_package(OLD_PACKAGE, OLD_PACKAGE_SHA256, work)
        new_pkg = baseline.extract_package(NEW_PACKAGE, NEW_PACKAGE_SHA256, work)
        old_text = baseline.provider_records(old_pkg)
        new_text = baseline.provider_records(new_pkg)
        old_image = baseline.image_records(old_pkg)
        new_image = baseline.image_records(new_pkg)
        counts = (len(old_text), len(new_text), len(old_image), len(new_image))
        if counts != EXPECTED_COUNTS:
            print(f"catalog count mismatch: got {counts}, expected {EXPECTED_COUNTS}", file=sys.stderr)
            return 1
        if args.fault:
            key = next(
                key
                for key in sorted(set(old_text) & set(new_text))
                if old_text[key] == new_text[key]
            )
            old_text[key] = {
                **old_text[key],
                "name": str(old_text[key].get("name", "")) + " FAULT",
            }
        text_delta = baseline.delta(old_text, new_text)
        image_delta = baseline.delta(old_image, new_image)
        failures = []
        failures.extend(baseline.check_delta("text", text_delta, EXPECTED_TEXT))
        failures.extend(baseline.check_delta("image", image_delta, EXPECTED_IMAGE))
        if failures:
            print("\n".join(failures), file=sys.stderr)
            return 1
        print(
            "v0.87.1 baseline delta verified: "
            f"text=+{text_delta[0]}/-{text_delta[1]}/{text_delta[2]} changed "
            f"image=+{image_delta[0]}/-{image_delta[1]}/{image_delta[2]} changed "
            f"counts={counts[0]}->{counts[1]}/{counts[2]}->{counts[3]}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
