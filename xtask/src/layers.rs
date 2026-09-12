//! Assert tuile's internal module layering.
//!
//! Rust permits module cycles inside a crate and cargo only guarantees the *crate* graph is
//! acyclic, so nothing in the toolchain checks the shape this library relies on: widgets
//! build on the foundation modules, never the other way round.
//!
//! Two rules, both failing loudly:
//!   1. no foundation module may import from `widgets`
//!   2. no cycles between modules
//!
//! This is a textual check of `use crate::` and `use super::` paths. It catches the realistic
//! regression, a new import pointing the wrong way, and does not resolve re-export chains.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const FOUNDATION: [&str; 8] = ["term", "theme", "draw", "core", "anim", "layout", "fuzzy", "runtime"];

pub fn run() -> Result<(), String> {
    let src = crate::root().join("tuile/src");
    if !src.is_dir() {
        return Err(format!("layers: {} not found", src.display()));
    }

    let edges = build_graph(&src)?;
    let mut failures: Vec<String> = Vec::new();

    for (source, targets) in &edges {
        if FOUNDATION.contains(&top(source)) {
            for target in targets {
                if top(target) == "widgets" {
                    failures.push(format!("  {source} imports {target} (foundation -> widgets)"));
                }
            }
        }
    }
    for cycle in find_cycles(&edges) {
        failures.push(format!("  cycle: {}", cycle.join(" -> ")));
    }

    if !failures.is_empty() {
        return Err(format!("layers: FAILED\n{}", failures.join("\n")));
    }
    let edge_count: usize = edges.values().map(BTreeSet::len).sum();
    println!("layers: ok - {} modules, {edge_count} edges, no cycles", edges.len());
    Ok(())
}

fn top(module: &str) -> &str {
    module.split("::").next().unwrap_or(module)
}

/// `tuile/src/widgets/spinner/mod.rs` becomes `widgets::spinner`.
fn module_name(src: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(src).unwrap_or(path).with_extension("");
    rel.to_string_lossy().replace("/mod", "").replace('/', "::")
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            rust_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

fn build_graph(src: &Path) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let mut files = Vec::new();
    rust_files(src, &mut files).map_err(|e| format!("layers: reading {}: {e}", src.display()))?;

    let modules: BTreeMap<String, PathBuf> =
        files.into_iter().map(|p| (module_name(src, &p), p)).collect();
    let names: Vec<&String> = modules.keys().collect();
    let mut edges: BTreeMap<String, BTreeSet<String>> =
        modules.keys().map(|n| (n.clone(), BTreeSet::new())).collect();

    for (name, path) in &modules {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("layers: reading {}: {e}", path.display()))?;
        // Test modules may import anything; only production code is checked.
        let code = match text.find("#[cfg(test)]") {
            Some(cut) => &text[..cut],
            None => &text[..],
        };

        for target in use_paths(code, "crate::") {
            // Longest matching module wins: `widgets::spinner::spinners` over `widgets`.
            let resolved = names
                .iter()
                .filter(|m| target == ***m || target.starts_with(&format!("{m}::")))
                .max_by_key(|m| m.len());
            if let Some(resolved) = resolved
                && *resolved != name
            {
                edges.get_mut(name).unwrap().insert((*resolved).clone());
            }
        }
        for sibling in use_paths(code, "super::") {
            let leaf = sibling.split("::").next().unwrap_or(&sibling).to_string();
            let resolved = match name.rsplit_once("::") {
                Some((parent, _)) => format!("{parent}::{leaf}"),
                None => leaf,
            };
            if modules.contains_key(&resolved) && resolved != *name {
                edges.get_mut(name).unwrap().insert(resolved);
            }
        }
    }
    Ok(edges)
}

/// Every `use <prefix>PATH` in the source, returning the part after the prefix.
fn use_paths(code: &str, prefix: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in code.lines() {
        let trimmed = line.trim_start();
        let rest = trimmed
            .strip_prefix("pub use ")
            .or_else(|| trimmed.strip_prefix("use "));
        let Some(rest) = rest else { continue };
        let Some(path) = rest.strip_prefix(prefix) else { continue };
        let end = path
            .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == ':'))
            .unwrap_or(path.len());
        let path = path[..end].trim_end_matches(':');
        if !path.is_empty() {
            found.push(path.to_string());
        }
    }
    found
}

fn find_cycles(edges: &BTreeMap<String, BTreeSet<String>>) -> Vec<Vec<String>> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Unseen,
        OnStack,
        Done,
    }
    let mut state: BTreeMap<&str, Mark> = edges.keys().map(|k| (k.as_str(), Mark::Unseen)).collect();
    let mut cycles = Vec::new();
    let mut stack: Vec<&str> = Vec::new();

    fn visit<'a>(
        node: &'a str,
        edges: &'a BTreeMap<String, BTreeSet<String>>,
        state: &mut BTreeMap<&'a str, Mark>,
        stack: &mut Vec<&'a str>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        state.insert(node, Mark::OnStack);
        stack.push(node);
        if let Some(targets) = edges.get(node) {
            for next in targets {
                match state.get(next.as_str()).copied().unwrap_or(Mark::Unseen) {
                    Mark::OnStack => {
                        let start = stack.iter().position(|s| *s == next.as_str()).unwrap_or(0);
                        let mut cycle: Vec<String> =
                            stack[start..].iter().map(|s| s.to_string()).collect();
                        cycle.push(next.clone());
                        cycles.push(cycle);
                    }
                    Mark::Unseen => visit(next, edges, state, stack, cycles),
                    Mark::Done => {}
                }
            }
        }
        stack.pop();
        state.insert(node, Mark::Done);
    }

    for node in edges.keys() {
        if state.get(node.as_str()).copied().unwrap_or(Mark::Unseen) == Mark::Unseen {
            visit(node, edges, &mut state, &mut stack, &mut cycles);
        }
    }
    cycles
}
