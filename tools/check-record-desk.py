#!/usr/bin/env python3
"""Canonical explicit checks for the excluded, independent consumer workspace."""
import json
from pathlib import Path
import subprocess
import tomllib
import importlib.util

ROOT = Path(__file__).resolve().parent.parent
CONSUMER = ROOT / "consumers/record-desk"
spec = importlib.util.spec_from_file_location("rust_tests", ROOT / "tools/test-rust.py")
rust_tests = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rust_tests)


def run(*args):
    print("Record Desk:", " ".join(args), flush=True)
    subprocess.run(args, cwd=CONSUMER, check=True)


def independence():
    manifest = tomllib.loads((CONSUMER / "Cargo.toml").read_text())
    if "workspace" not in manifest or manifest["workspace"].get("members"):
        raise ValueError("Record Desk must own a single-package workspace")
    def inherited(value):
        return isinstance(value, dict) and (value.get("workspace") is True or any(inherited(v) for v in value.values()))
    if inherited(manifest):
        raise ValueError("consumer declarations must not inherit workspace values")
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--format-version", "1"], cwd=CONSUMER))
    packages = {p["id"]: p for p in metadata["packages"]}
    members = [packages[key]["name"] for key in metadata["workspace_members"]]
    if Path(metadata["workspace_root"]).resolve() != CONSUMER or members != ["record-desk"]:
        raise ValueError("consumer is not its own workspace")
    framework = sorted(p["name"] for p in packages.values() if p["name"].startswith("polyorama-"))
    if framework != ["polyorama-core", "polyorama-ui-egui"]:
        raise ValueError(f"unnecessary framework dependencies: {framework}")
    forbidden = {"analytical-workspace-lab", "polyorama-gallery", "emuella-viewer", "emuella-viewer-source", "emuella-viewer-tools", "polyorama-tile-worker"}
    if forbidden.intersection(p["name"] for p in packages.values()):
        raise ValueError("consumer depends on an application package")
    for source in (CONSUMER / "src").rglob("*.rs"):
        if "include!(" in source.read_text():
            raise ValueError(f"source inclusion shortcut: {source}")
    evidence = ROOT / ".tools/runtime/verification-evidence/record-desk"
    evidence.mkdir(parents=True, exist_ok=True)
    (evidence / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    print(f"Independent workspace: {members}; framework crates: {framework}", flush=True)


if __name__ == "__main__":
    rust_tests.clear_reports(ROOT, ["record-desk"])
    independence()
    run("cargo", "fmt", "--all", "--check")
    run("cargo", "clippy", "--locked", "--all-targets", "--", "-D", "warnings")
    rust_tests.run("record-desk")
    run("cargo", "clippy", "--locked", "--target", "wasm32-unknown-unknown", "--lib", "--", "-D", "warnings")
    run("cargo", "build", "--locked", "--release")
    run("bash", "build-web.sh")
