#!/usr/bin/env python3
"""Assert extension.toml satisfies Zed's published submission rules.

Zed's registry rejects on these at PR review, which is a slow way to find out.
Each rule below is one Zed states explicitly in its developing-extensions docs.

Deliberately not a `try: ... except: pass` around a TOML parse -- an unparseable
manifest must fail loudly, not be treated as "no violations found". A manifest
checker whose broken and clean states look the same is the exact bug shape this
project keeps paying for.
"""

from __future__ import annotations

import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

#: Zed rejects names containing any of these.
RESERVED = ("zed", "extension")

#: Fields Zed requires in every manifest.
REQUIRED = ("id", "name", "version", "schema_version", "authors",
            "description", "repository")

#: The nine licences Zed accepts, by the SPDX-ish identifiers it names.
APPROVED_LICENCES = (
    "MIT", "Apache-2.0", "GPL-3.0", "GPL-2.0", "LGPL-3.0",
    "BSD-3-Clause", "BSD-2-Clause", "MPL-2.0", "ISC",
)


def main() -> int:
    manifest_path = ROOT / "extension.toml"
    if not manifest_path.exists():
        print(f"error: no {manifest_path}", file=sys.stderr)
        return 1

    manifest = tomllib.loads(manifest_path.read_text())
    problems: list[str] = []

    for field in REQUIRED:
        if not manifest.get(field):
            problems.append(f"extension.toml has no `{field}` (Zed requires it)")

    for field in ("id", "name"):
        value = str(manifest.get(field, "")).casefold()
        for word in RESERVED:
            if word in value:
                problems.append(
                    f"`{field}` contains {word!r}; Zed rejects names containing "
                    f"{' or '.join(repr(w) for w in RESERVED)}")

    if not manifest.get("context_servers"):
        problems.append(
            "no [context_servers.*] table, so Zed would install this and expose "
            "no MCP server at all -- the one thing it exists to do")

    # The manifest declaring a context server and the Rust actually
    # implementing one are two separate claims. Checking only the first is a
    # check that passes on an extension that does nothing.
    lib = (ROOT / "src" / "lib.rs").read_text()
    if "context_server_command" not in lib:
        problems.append(
            "extension.toml declares a context server but src/lib.rs does not "
            "implement `context_server_command`")

    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text())
    licence = cargo.get("package", {}).get("license", "")
    if licence not in APPROVED_LICENCES:
        problems.append(
            f"Cargo.toml license is {licence!r}; Zed accepts one of "
            f"{', '.join(APPROVED_LICENCES)}")
    if not (ROOT / "LICENSE").exists():
        problems.append("no LICENSE file at the repo root")

    crate = cargo.get("package", {}).get("name", "")
    if cargo.get("lib", {}).get("crate-type") != ["cdylib"]:
        problems.append("Cargo.toml [lib] crate-type must be [\"cdylib\"] to build wasm")

    # The workflow asserts a specific artifact filename; cargo derives it from
    # the crate name. Renaming the crate without renaming there would leave CI
    # asserting the existence of a file that is no longer produced.
    workflow = (ROOT / ".github" / "workflows" / "build.yml").read_text()
    if crate and f"{crate}.wasm" not in workflow:
        problems.append(
            f"crate is {crate!r} so cargo emits {crate}.wasm, but the build "
            "workflow does not mention that filename")

    # Zed loads a wasm COMPONENT. `cargo build` emits a core module, and a
    # workflow that only checks the file is non-empty goes green on an artifact
    # Zed refuses with "attempted to parse a wasm module with a component
    # parser". That happened; this makes losing the componentize step loud.
    if "wasm-tools component new" not in workflow:
        problems.append(
            "the build workflow does not run `wasm-tools component new`, so it "
            "ships a core module. Zed only loads components")
    if "0061736d0d000100" not in workflow:
        problems.append(
            "the build workflow does not assert the component header "
            "(0061736d0d000100), so it cannot tell a component from the core "
            "module Zed rejects")

    if problems:
        print("extension.toml would be rejected:", file=sys.stderr)
        for problem in problems:
            print(f"  - {problem}", file=sys.stderr)
        return 1

    print(f"manifest OK: {manifest['id']} v{manifest['version']} ({licence})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
