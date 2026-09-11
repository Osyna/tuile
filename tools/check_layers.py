#!/usr/bin/env python3
"""Assert tuile's internal module layering.

Rust permits module cycles inside a crate and cargo only guarantees the *crate*
graph is acyclic, so nothing in the toolchain checks the shape this library
relies on: widgets build on the foundation modules, never the other way round.

Two rules, both failing loudly:
  1. no foundation module may import from `widgets`
  2. no cycles between modules

`layered-crate` is the off-the-shelf tool for this, but it generates a package
under `target/` that cannot inherit `workspace.package`, so it cannot run on a
workspace laid out like this one. This is a textual check of `use crate::` and
`use super::` paths: it catches the realistic regression (a new import pointing
the wrong way) and does not attempt to resolve re-export chains.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

FOUNDATION = {"theme", "draw", "core", "anim", "layout", "fuzzy", "runtime"}
SRC = Path(__file__).resolve().parent.parent / "tuile" / "src"

USE_CRATE = re.compile(r"(?:^|\s)(?:pub\s+)?use\s+crate::([A-Za-z0-9_:]+)", re.M)
USE_SUPER = re.compile(r"^\s*(?:pub\s+)?use\s+super::([A-Za-z0-9_]+)", re.M)


def module_name(path: Path) -> str:
    rel = path.relative_to(SRC).with_suffix("")
    return str(rel).replace("/mod", "").replace("/", "::")


def production_code(path: Path) -> str:
    """Everything above `#[cfg(test)]`; test modules may import freely."""
    text = path.read_text(encoding="utf-8", errors="replace")
    cut = text.find("#[cfg(test)]")
    return text if cut < 0 else text[:cut]


def build_graph() -> dict[str, set[str]]:
    modules = {module_name(p): p for p in SRC.rglob("*.rs")}
    edges: dict[str, set[str]] = {name: set() for name in modules}
    for name, path in modules.items():
        code = production_code(path)
        for target in USE_CRATE.findall(code):
            matches = [m for m in modules if target == m or target.startswith(m + "::")]
            if matches:
                resolved = max(matches, key=len)
                if resolved != name:
                    edges[name].add(resolved)
        for sibling in USE_SUPER.findall(code):
            parent = name.rsplit("::", 1)[0] if "::" in name else ""
            resolved = f"{parent}::{sibling}" if parent else sibling
            if resolved in modules and resolved != name:
                edges[name].add(resolved)
    return edges


def find_cycles(edges: dict[str, set[str]]) -> list[list[str]]:
    state: dict[str, int] = {}
    cycles: list[list[str]] = []

    def visit(node: str, stack: list[str]) -> None:
        state[node] = 1
        stack.append(node)
        for nxt in sorted(edges.get(node, ())):
            if state.get(nxt, 0) == 1:
                cycles.append(stack[stack.index(nxt) :] + [nxt])
            elif state.get(nxt, 0) == 0:
                visit(nxt, stack)
        stack.pop()
        state[node] = 2

    for node in sorted(edges):
        if state.get(node, 0) == 0:
            visit(node, [])
    return cycles


def main() -> int:
    if not SRC.is_dir():
        print(f"layers: {SRC} not found", file=sys.stderr)
        return 2

    edges = build_graph()
    failures: list[str] = []

    for source, targets in sorted(edges.items()):
        if source.split("::")[0] in FOUNDATION:
            for target in sorted(targets):
                if target.split("::")[0] == "widgets":
                    failures.append(f"  {source} imports {target} (foundation -> widgets)")

    for cycle in find_cycles(edges):
        failures.append("  cycle: " + " -> ".join(cycle))

    if failures:
        print("layers: FAILED", file=sys.stderr)
        print("\n".join(failures), file=sys.stderr)
        return 1

    edge_count = sum(len(t) for t in edges.values())
    print(f"layers: ok — {len(edges)} modules, {edge_count} edges, no cycles")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
