#!/usr/bin/env python3
"""Build the hosted matrix and fail closed when aggregating qualification."""
import argparse
import json
import os
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def matrix():
    configuration = json.loads((ROOT / "tools/verification-stages.json").read_text())
    if configuration.get("schema_version") != 1:
        raise ValueError("unsupported verification stage inventory")
    stages = configuration["stages"]
    names = [stage["name"] for stage in stages]
    if not names or len(set(names)) != len(names):
        raise ValueError("verification stages must be non-empty and unique")
    for stage in stages:
        if set(stage) != {"name", "ui", "browser", "consumer"} or any(
                not isinstance(stage[key], bool) for key in ("ui", "browser", "consumer")):
            raise ValueError("invalid verification stage setup")
    return {"include": stages}


def gate(needs, evidence=None, revision=None):
    if needs["scope"]["result"] != "success":
        raise ValueError("verification classification did not succeed")
    mode = needs["scope"]["outputs"]["mode"]
    results = {job: needs[job]["result"] for job in ("qualification", "documentation")}
    expected = {"full": {"qualification": "success", "documentation": "skipped"},
                "docs": {"qualification": "skipped", "documentation": "success"}}
    if mode not in expected or results != expected[mode]:
        raise ValueError(f"incomplete {mode!r} verification: {results}")
    if mode == "full":
        if evidence is None or not revision:
            raise ValueError("full verification requires attributed interface evidence")
        observed = []
        selections = {
            "native-lab": ({"lab"}, {"native"}),
            "browser": ({"lab"}, {"browser"}),
            "record-desk": ({"record-desk"}, {"browser", "native"}),
        }
        for stage, (apps, hosts) in selections.items():
            summary = json.loads((evidence / f"interface-{stage}/application-interface/summary.json").read_text())
            if summary["revision"] != revision or summary["dirty"] is not False:
                raise ValueError(f"unattributed interface qualification: {stage}")
            if summary.get("slice_only") is not False:
                raise ValueError(f"partial interface qualification: {stage}")
            selection = summary["selection"]
            if set(selection["apps"]) != apps or set(selection["hosts"]) != hosts:
                raise ValueError(f"incorrect interface selection: {stage}")
            pairs = [(report["app"], report["host"]) for report in summary["reports"]]
            if sorted(pairs) != sorted((app, host) for app in apps for host in hosts):
                raise ValueError(f"incomplete interface qualification: {stage}")
            observed.extend(pairs)
        if sorted(observed) != sorted((app, host) for app in ("lab", "record-desk") for host in ("native", "browser")):
            raise ValueError("interface journeys do not cover the complete surface")
    return mode


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("matrix", "gate"))
    args = parser.parse_args()
    if args.command == "matrix":
        value = json.dumps(matrix(), separators=(",", ":"))
        print(value)
        if os.environ.get("GITHUB_OUTPUT"):
            with open(os.environ["GITHUB_OUTPUT"], "a") as output:
                output.write(f"matrix={value}\n")
    else:
        mode = gate(json.loads(os.environ["VERIFICATION_NEEDS"]),
                    ROOT / ".tools/runtime/verification-evidence", os.environ.get("GITHUB_SHA"))
        print(f"Complete {mode} verification passed")


if __name__ == "__main__":
    main()
