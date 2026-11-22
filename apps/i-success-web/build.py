"""Build the WebOS browser package from an exact Git revision."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import sys
sys.dont_write_bytecode = True
from notices import web_notices

root = Path(__file__).resolve().parents[2]
run = lambda *args: subprocess.check_output(args, cwd=root, text=True).strip()
parser = argparse.ArgumentParser()
parser.add_argument("output", type=Path, help="new directory outside the source checkout")
output = parser.parse_args().output.resolve()
if output.exists() or output.is_relative_to(root):
    parser.error("select a new package directory outside the source checkout")
if run("git", "status", "--porcelain", "--untracked-files=normal"):
    parser.error("the source checkout has uncommitted changes")
bindgen = os.environ.get("WASM_BINDGEN", "wasm-bindgen")
if run(bindgen, "--version") != "wasm-bindgen 0.2.130":
    parser.error("wasm-bindgen 0.2.130 is required")
target = Path(json.loads(run("cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"))["target_directory"])
subprocess.run(["cargo", "build", "--locked", "--release", "--target", "wasm32-unknown-unknown", "-p", "i-success-web"], cwd=root, check=True)
output.parent.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(dir=output.parent) as temp_dir:
    package = Path(temp_dir) / "site"
    subprocess.run([bindgen, str(target / "wasm32-unknown-unknown/release/i_success_web.wasm"), "--target", "web", "--no-typescript", "--out-name", "i_success", "--out-dir", str(package)], cwd=root, check=True)
    (package / "THIRD-PARTY-NOTICES.txt").write_text(web_notices(root, run))
    files = []
    for name in ["i_success.js", "i_success_bg.wasm", "THIRD-PARTY-NOTICES.txt"]:
        data = (package / name).read_bytes()
        files.append(dict(path=name, bytes=len(data), sha256=hashlib.sha256(data).hexdigest()))
    receipt = dict(schema="webos-component-build/1", component="i-success", sourceCommit=run("git","rev-parse","HEAD"), sourceTree=run("git","rev-parse","HEAD^{tree}"), cargoLockSHA256=hashlib.sha256((root/"Cargo.lock").read_bytes()).hexdigest(), rustc=run("rustc","--version"), wasmBindgen=run(bindgen,"--version"), files=files)
    (package / "component-build.json").write_text(json.dumps(receipt, indent=2) + "\n")
    package.rename(output)
