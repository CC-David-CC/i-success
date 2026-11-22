"""Collect licence terms for the target-specific dependency graph and embedded fonts."""
import json
from pathlib import Path


def web_notices(root, run):
    target = "wasm32-unknown-unknown"
    tree = run("cargo", "tree", "--locked", "--target", target, "--edges", "normal", "--prefix", "none", "--format", "{p}", "-p", "i-success-web")
    selected = {tuple(row.split()[:2]) for row in tree.splitlines()}
    packages = json.loads(run("cargo", "metadata", "--locked", "--filter-platform", target, "--format-version", "1"))["packages"]
    sections = []
    for pkg in sorted(packages, key=lambda p: (p["name"], p["version"])):
        if not pkg["source"] or (pkg["name"], "v" + pkg["version"]) not in selected:
            continue
        directory = Path(pkg["manifest_path"]).parent
        docs = []
        for entry in directory.iterdir():
            if entry.name.lower().startswith(("license", "licence", "copying", "copyright", "notice")):
                docs.extend([entry] if entry.is_file() else [p for p in entry.rglob("*") if p.is_file() and p.suffix.lower() in ("", ".md", ".txt")])
        if pkg.get("license_file"):
            docs.append(directory / pkg["license_file"])
        if pkg["name"] == "epaint_default_fonts":
            docs.extend((directory / "fonts").glob("*.txt"))
        text = [f"{pkg['name']} {pkg['version']}", pkg["license"] or "", pkg["repository"] or "", ", ".join(pkg["authors"])]
        texts = [(p.relative_to(directory).as_posix(), p.read_text(encoding="utf-8",errors="replace")) for p in sorted(set(docs))]
        if not docs or pkg["name"] == "epaint_default_fonts":
            if "Apache-2.0" not in (pkg["license"] or ""):
                raise ValueError(f"Missing packaged licence for {pkg['name']}")
            texts.append(("Apache-2.0", (root / "LICENSE-APACHE").read_text()))
        for name, content in texts:
            text.extend(["", name, content.rstrip()])
        sections.append("\n".join(text))
    if not sections:
        raise ValueError("No WebAssembly dependencies found")
    return "\n\n".join(sections) + "\n"
