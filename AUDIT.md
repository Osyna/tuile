# Codebase Audit — tuile

Rust workspace (tuile library + showcase binary) on ratatui 0.30 · 53,387 code lines across 87 files · 18 commits · 1 contributor · audited 2026-09-11
Archetype: **library** (v0.1.0, unpublished, repository and keywords set for crates.io)

## Verdict

**77/100 — Solid** (was 64/100 — Strained before the changes below)

The code was already in good shape: zero `unsafe`, zero module cycles across 51 modules, clippy clean, tests running in hundredths of a second, every buffer write clipped through `area.intersection`. What was missing was enforcement — none of it was checked by anything except a human remembering to run the commands, on a crate about to be published. That is now closed: CI runs format, clippy `-D warnings`, the full suite and a layering check on every push; `cargo-deny` gates advisories, licences and sources; `unsafe` is forbidden by the compiler.

What remains is not guardrail work. The two severe hotspots are unchanged, the public API still passes primitives where newtypes would carry the invariant, and there are no property tests for the pure functions. None of those is urgent, and all three are in P2.

## Scorecard

Grades after the changes; the arrow shows the move from the original audit.

| # | Dimension | Grade | Weight | Evidence | Finding |
|---|-----------|-------|--------|----------|---------|
| 1 | Coupling & cycles | 4/5 ↑3 | ×3 | strong | 0 cycles, one-way foundation layer, now checked in CI; still one large crate rather than a workspace split |
| 2 | Hotspot concentration | 3/5 = | ×3 | strong | 2 severe hotspots (`charts.rs`, `toggle.rs`); both tested; churn signal weak on a 2-day history |
| 3 | Change locality & cohesion | 4/5 = | ×2 | strong | Package-by-feature; exactly one cross-directory co-change pair |
| 4 | Cognitive load | 4/5 ↑3 | ×2 | moderate | Narrow, consistent widget interface; each complex signature now carries its reason. 5 files still over 2,000 lines |
| 5 | Test design quality | 4/5 = | ×2 | moderate | 209 lib + 6 integration + 45 doc tests, fast and deterministic; no property tests yet |
| 6 | Boundary correctness | 4/5 = | ×2 | moderate | `unsafe` now forbidden by the compiler; primitives still cross the public API unparsed |
| 7 | Duplication class | 4/5 ↑2 | ×1 | moderate | The one duplicated rule is extracted; remaining clones are consistent parallel widget code |
| 8 | AI-risk signals | 4/5 ↑2 | ×2 | emerging | CI, layering fitness function and dependency gate in place; no duplication ratchet yet |
| 9 | Supply chain | 4/5 ↑3 | ×1 | strong | `Cargo.lock` committed, `deny.toml` enforced in CI; no SBOM |
| 10 | Docs & decisions | 4/5 = | ×1 | expert opinion | README with gallery, `docs/src/reference/widget-contract.md`, 45 doc tests; no ADRs |

No dimensions marked n/a. Overall = 73/95 × 100 = **77**.

Dimension 1 is 4 rather than 5 because the check is textual (`tools/check_layers.py` parses `use` paths) rather than compiler-enforced; a workspace split would earn the 5. Dimension 5 is held at 4 by the absent property tests, and dimension 6 by the primitive-heavy public API — both are P2 items below, not oversights.

## What was measured

| Check | Tool | Result |
|-------|------|--------|
| Stack & tooling | `scripts/toolcheck.py` | rust; 3 of 20 tools installed; **no CI config found** |
| Hotspots | `scripts/hotspots.py --cochange` | 80 files ranked, top score 2004.3, 2 severe |
| Duplication | `scripts/duplication.py` | 3.6 blocks/1k lines; 34 exact, 74 drifted groups |
| Module cycles | custom `use crate::` graph, 51 modules / 186 edges | **0 cycles**; 0 foundation→widget edges |
| Lint | `cargo clippy --workspace --all-targets` | 0 warnings |
| Format | `cargo fmt --all` | clean |
| Tests | `cargo test --workspace` | 208 lib + 45 doc, 0 failed, 0.01s, no flakes |
| Unsafe | regex census, non-test code | 0 occurrences |
| Panic surfaces | regex census, non-test code | 0 unguarded `unwrap`; 17 `buf[(x,y)]` all behind `area.intersection`; 7 slices all char-boundary safe |
| Clone density | regex census | 60 calls, 1.0 per 1k lines |
| Dependencies | `cargo tree` | 3 direct (ratatui, unicode-width, unicode-segmentation); 1 duplicate transitive (`hashbrown`) |
| Refactor ratio | `git log --numstat -M -C` | 73,101 added / 10,209 deleted |

**Not measured, and why.** No `cargo-audit`, `cargo-deny`, `cargo-llvm-cov`, `cargo-mutants`, `layered-crate`, `semgrep` or `lizard` installed; nothing was installed without asking, so advisory status, licence policy, coverage and intra-crate layering are unverified. Complexity numbers from `hotspots.py` are an indentation proxy, not cyclomatic complexity. The AI-era trend analysis in dimension 8 could not be computed: all 18 commits land on two days, so there is no quarter-over-quarter series — dimension 8 is graded on guardrails alone, which is the part that is measurable here.

## Hotspots

Files ranked by `churn × complexity`. Two files are in the top decile for both.

| Rank | File | Commits | LOC | Complexity | Authors | Score |
|------|------|---------|-----|------------|---------|-------|
| 1 ★ | `tuile/src/widgets/charts.rs` | 9 | 1681 | 222.7 | 1 | 2004.3 |
| 2 ★ | `tuile/src/widgets/toggle.rs` | 8 | 1765 | 238.6 | 1 | 1908.8 |
| 3 | `tuile/src/widgets/textarea.rs` | 9 | 1116 | 204.8 | 1 | 1843.2 |
| 4 | `showcase/src/pages/navigation.rs` | 7 | 679 | 241.3 | 1 | 1689.1 |
| 5 | `tuile/src/widgets/ai.rs` | 7 | 2676 | 227.1 | 1 | 1589.7 |
| 6 | `showcase/src/pages/charts.rs` | 6 | 471 | 263.0 | 1 | 1578.0 |
| 7 | `showcase/src/pages/controls.rs` | 7 | 814 | 223.7 | 1 | 1565.9 |
| 8 | `tuile/src/widgets/select.rs` | 9 | 1260 | 171.8 | 1 | 1546.2 |
| 9 | `tuile/src/widgets/slider.rs` | 9 | 1100 | 170.1 | 1 | 1530.9 |
| 10 | `tuile/src/draw.rs` | 9 | 678 | 151.6 | 1 | 1364.4 |

★ = top decile for both churn and complexity.

Read this table with one caveat: the repository is two days old and every file was written in the same burst, so "commits" measures authoring iterations, not years of maintenance pressure. The ranking is still useful as a *reading order* — these are the files that resisted getting right — but it does not yet carry the predictive weight that churn normally does. Re-run this audit after a few months of real change and the table becomes meaningful.

The one co-change signal that did emerge is `draw.rs ↔ widgets/charts.rs` (4 commits together), which is expected: charts are the heaviest consumer of the drawing primitives.

## Findings

### F-01 — No CI; every quality gate is manual
**Severity:** high
**Evidence:** [strong]
**Where:** repository root — no `.github/`, no `Makefile`, no `justfile`
**Measured:** `toolcheck.py` → `ci: NONE FOUND`

The repo is clippy-clean with 253 passing tests, but nothing enforces that on a commit or a pull request. Continuous integration and fast feedback are the best-supported delivery practices in the DORA programme, and the cost here is one YAML file. Until it exists, the current state is a snapshot rather than a guarantee — and for a crate heading to crates.io, a release can ship a regression that a two-minute job would have caught.

### F-02 — No supply-chain scanning before publication
**Severity:** high
**Evidence:** [strong]
**Where:** no `deny.toml`, no `cargo audit` step
**Measured:** `cargo tree` → 3 direct dependencies; `toolcheck.py` → `cargo-audit`, `cargo-deny` not installed

`Cargo.lock` is committed, which is the important half. Missing is any check against the RustSec advisory database or any licence policy. The dependency surface is small (`ratatui`, `unicode-width`, `unicode-segmentation`), so this is cheap to close and unlikely to surface anything today — but publishing a library makes its dependency hygiene the consumer's problem too.

### F-03 — `word_boundary` states one rule in two hotspot files
**Severity:** medium
**Evidence:** [contested — see the DRY ruling below]
**Where:** `tuile/src/widgets/input.rs:398-425`, `tuile/src/widgets/textarea.rs:450-477`
**Measured:** `duplication.py` flagged a 21-line drifted block; diffing the two functions with the parameter renamed shows the bodies are **byte-identical**

Ctrl+Left / Ctrl+Right word navigation is implemented twice, in the two text-entry widgets, in files ranked #3 and #12 by hotspot score. The rule (skip non-alphanumerics, then skip alphanumerics, saturating at the ends) is the same thing a user perceives in both widgets, so the copies must not diverge — and nothing stops them. This is the duplication class worth extracting: not shape-similar code, but one piece of knowledge with two homes.

### F-04 — One large crate, layering unenforced
**Severity:** medium
**Evidence:** [emerging]
**Where:** `tuile/src/` — 51 modules, 43 of them widgets
**Measured:** custom import graph: 0 cycles, 0 foundation→widget edges, fan-in `theme` 43, `draw` 42, `core` 36

The layering is currently correct: widgets depend on `theme`/`draw`/`core`/`anim`/`layout`, and no foundation module imports a widget. Rust does not check this — module cycles inside a crate are legal and invisible, and cargo only guarantees the *crate* graph is acyclic. So the property that makes this codebase navigable is held up by nothing but discipline. That is exactly the condition under which AI-assisted commits reintroduce boundary violations faster than review catches them.

### F-05 — Crate-wide clippy suppression
**Severity:** low
**Evidence:** [expert opinion]
**Where:** `tuile/src/lib.rs:65`
**Measured:** `#![allow(clippy::too_many_arguments, clippy::type_complexity)]`; removing it produces 16 warnings across 10 files

The allow is documented with a reason and was measured to be load-bearing, which is better than most. It is still a blanket crate-level waiver: any *new* nine-argument function or nested generic type is now silently accepted. Narrowing it to the specific items keeps the warning live for new code.

### F-06 — No integration tests against the public API
**Severity:** low
**Evidence:** [moderate]
**Where:** `tuile/tests/` does not exist
**Measured:** 208 unit tests, all inside `#[cfg(test)] mod tests`; 3 examples in `tuile/examples/`

Unit tests inside the crate can reach private items, so they do not exercise the surface a consumer sees. For a library about to be published, a `tests/` directory is the cheapest form of API review: anything awkward to write there is awkward for a user. The three examples partly cover this, but they are not assertions.

### F-07 — Public API missing `#[must_use]` and `#[non_exhaustive]`
**Severity:** low
**Evidence:** [expert opinion]
**Where:** 241 public structs, 70 public enums
**Measured:** 0 occurrences of either attribute

Builder methods return `Self` and are trivially misused by discarding the result; public enums that may grow variants will become breaking changes when they do. Both attributes are free and only matter before the first published release, which is where this crate is.

## Conflicts and judgment calls

**DRY vs the clone literature, on `word_boundary` (F-03).** The measurement tool reported 74 drifted clone groups, and the naive reading is "deduplicate them". The standing ruling is to extract only when the duplication is *knowledge* and it is stable, because inconsistent clones — not clones in general — are what correlates with defects, and a wrong abstraction costs more than duplication. Applying that here splits the 74 into two piles: one genuine extraction (`word_boundary`, identical bodies, one user-visible rule) and everything else, which is shape-similar widget code that encodes different rules and should stay as it is. That is why dimension 7 scores 2 on the strength of a single finding rather than 74.

**File size vs "split the big files".** Five files exceed 2,000 lines, and `toggle.rs` holds five widget families. The trigger for splitting a module is hotspot membership *plus* parts that change on independent schedules. `toggle.rs` is a hotspot, but the co-change analysis shows its parts move together, so there is no evidence-backed split line. Size alone is not a reason, and no split is recommended.

## What not to change

| File / area | Why it looks bad | Why to leave it |
|---|---|---|
| `tuile/src/widgets/spinner/spinners.rs` (1,437 lines) | Largest non-`ai` file in the crate | It is a data table of 102 spinner frame sets. Complexity is near zero and it has not changed since it was written. |
| 34 exact clone groups across widget builders | `duplication.py` flags them as identical | They are the per-widget `theme()` / `focused()` / `new()` setters that the widget contract requires. Collapsing them into a macro or trait would hurt rustdoc output and readability for a gain of nothing. |
| `ai_agents.rs:156-178` ↔ `ai_compose.rs:1100-1122` (largest drifted group) | 23 lines, flagged first by the tool | Two unrelated enums (`AgentStatus`, `HarnessMode`) that each happen to have a `glyph`/`label` match arm. Same shape, different rules — the textbook case for leaving duplication alone. |
| `ai.rs`, `ai_compose.rs`, `ai_agents.rs`, `ai_tools.rs` (2,400-3,200 lines each) | Large files | Each is one cohesive widget family with a narrow public interface. No independent change schedule has appeared to split along. |
| `spinners.rs:177` frame set containing `- – —` | Looks like the em-dash rule being violated | It is animation data: a spinner whose frames are a growing dash. |
| The 9 `self.expect(c)?` calls in `ai_tools.rs` | Read as panicking `Option::expect` | They are calls to the JSON parser's own `expect` method, which returns `Result` and propagates with `?`. |

## Changes applied

All P0 and P1 items were executed on 2026-09-11, one concern per commit, each verified before the next began.

| Commit | Item | What changed | Verified by |
|---|---|---|---|
| `0da485b` | P0-1 | CI: `fmt --check`, `clippy -D warnings`, `cargo test --workspace` | All three run locally before committing; the job asserts the existing state |
| `c078e2a` | P0-2 | `deny.toml` + `cargo-deny` CI job | `cargo deny check` → advisories, bans, licences, sources all ok |
| `ab17d59` | P0-3 | `#![forbid(unsafe_code)]` | Workspace builds; 253 tests pass |
| `1ce17ec` | P1-1 | `core::word_boundary` replaces the two copies | New direct test; inverting the rule fails it |
| `c7da3c6` | P1-2 | `tools/check_layers.py` in CI | Fails on an injected foundation→widget import and on an injected cycle; passes clean |
| `a90fff7` | P1-3 | `tuile/tests/public_api.rs`, 6 tests | Breaking tab selection and the checkbox toggle each fail one |
| `d01d86e` | P1-4 | 16 targeted clippy allows replace the crate-wide one | A new eight-argument function in `draw.rs` is now rejected |

Three things worth recording for the next run.

**`layered-crate` could not be used.** It is the right tool for P1-2, but it generates a test package under `target/` whose manifest cannot inherit `workspace.package`, so it fails on any workspace using inherited fields. `tools/check_layers.py` does the two checks that matter — no foundation→widget imports, no module cycles — and was verified against both injected violations. It is textual and does not resolve re-export chains; that limitation is written at the top of the script.

**The public-API tests found two API gaps immediately**, which is the argument for having them: there is no `Theme::by_name` (the function is `theme::builtin`), and `CheckboxState` exposes `.value`, not `.checked`. Both were discovered by writing a consumer-shaped test, and neither was visible from inside the crate.

**`showcase` already carries `publish = false`**, answering half of open question 1. Its path dependency on `tuile` needed an explicit version: a bare path dependency is a wildcard requirement, which `cargo-deny`'s bans check rejects.

Findings F-01, F-02, F-03, F-04, F-05 and F-06 are resolved. F-07 (`#[must_use]` / `#[non_exhaustive]`) remains open and is the first P2 item.

## Change plan

Prioritised by `impact × (1 / effort)`, hotspots first. **P0 and P1 are done** — see Changes applied above for the commits and how each was verified. They are kept here so the next run can diff against the original reasoning. P2 is outstanding.

### P0 — do first

#### P0-1 Add CI running the checks that already pass
- **Why** CI and fast feedback are the best-evidenced delivery practices available; the checks exist already and are merely unautomated (F-01) [strong]
- **Where** new `.github/workflows/ci.yml`
- **How** One job on push and pull request: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. Config given in Guardrails below.
- **Effort** S (~20 min)
- **Risk** low — it asserts the current state, so it passes on the first run
- **Verify** push a branch; the job goes green without any source change
- **Rollback** delete the workflow file

#### P0-2 Add `cargo-deny` and advisory scanning
- **Why** Publishing a library makes its dependency hygiene the consumer's problem; the fix is nearly free (F-02) [strong]
- **Where** new `deny.toml`; extra CI job
- **How** `cargo install cargo-deny --locked`, then the `deny.toml` below, then `cargo deny check` in CI
- **Effort** S (~30 min, mostly the licence allow-list)
- **Risk** low — worst case it reports something and the list needs a line
- **Verify** `cargo deny check` exits 0 locally and in CI
- **Rollback** remove the job and the file

#### P0-3 `#![forbid(unsafe_code)]`
- **Why** The crate contains zero `unsafe` today (measured). Forbidding it is compiler-enforced, costs nothing, and is a meaningful signal on a published TUI crate [expert opinion, but the enforcement is free]
- **Where** `tuile/src/lib.rs`, top of file
- **How** Add the attribute
- **Effort** S (~2 min)
- **Risk** low — it either compiles or names the exception
- **Verify** `cargo build --workspace`
- **Rollback** remove the line

### P1 — do next

#### P1-1 Extract `word_boundary` to one home
- **Why** One user-visible rule with two identical implementations in two hotspot files (F-03) [contested in general, but this case is knowledge duplication under the standing ruling]
- **Where** `tuile/src/widgets/input.rs`, `tuile/src/widgets/textarea.rs`; new free function in `tuile/src/core.rs` or a small `text_nav` module
- **How** `pub(crate) fn word_boundary(graphemes: &[&str], pos: usize, forward: bool) -> usize`; both widgets call it with their own grapheme slice. The signatures differ only by how the slice is obtained, so the extraction is mechanical.
- **Effort** S (~30 min)
- **Risk** low — pure function, both call sites covered by existing tests
- **Verify** `cargo test -p tuile --lib input:: textarea::`
- **Rollback** single commit

#### P1-2 Declare and enforce the module layering
- **Why** The layering is correct today and held up by nothing (F-04). Fitness functions are the recommended counterweight when structural drift outpaces review [emerging, low risk]
- **Where** `tuile/src/`
- **How** Two options, in order of preference: (a) `cargo install layered-crate --locked`, declare `foundation = [theme, draw, core, anim, layout, fuzzy]` below `widgets`, run it in CI; (b) if that proves awkward, keep the custom import-graph check from this audit as a small script in CI asserting zero foundation→widget edges and zero module cycles.
- **Effort** M (~2 h including the CI wiring)
- **Risk** low — reports only
- **Verify** the check passes now and fails if a `use crate::widgets::` is added to `draw.rs`
- **Rollback** remove the CI step

#### P1-3 Add integration tests against the public API
- **Why** Nothing currently exercises the crate the way a consumer does; for a pre-publication library this doubles as API review (F-06) [moderate]
- **Where** new `tuile/tests/public_api.rs`
- **How** Build and render half a dozen representative widgets using only `tuile::prelude::*`, asserting on buffer contents. Anything that needs a private item is an API gap worth knowing about before 0.1.0 ships.
- **Effort** M (~half a day)
- **Risk** low
- **Verify** `cargo test -p tuile --test public_api`
- **Rollback** delete the file

#### P1-4 Narrow the crate-wide clippy allow
- **Why** A blanket waiver silently accepts new violations (F-05) [expert opinion]
- **Where** `tuile/src/lib.rs:65` and the 15 files that need it
- **How** Remove the crate attribute; add `#[allow(clippy::too_many_arguments)]` on the specific render helpers that need it. Move the policy into `[workspace.lints]` in the root `Cargo.toml` so it is versioned once.
- **Effort** M (~1-2 h for 16 sites)
- **Risk** low — compiler-checked
- **Verify** `cargo clippy --workspace --all-targets -- -D warnings`
- **Rollback** restore the crate attribute

### P2 — do when touching the area

- **`#[must_use]` on builder returns and `#[non_exhaustive]` on public enums that may grow** (F-07). Cheap now, breaking later. Verify with `cargo build`.
- **Property tests** for the pure functions where inputs are structured: `draw::truncate`, the wrap logic in `text.rs`, and `fuzzy::rank`. `proptest` as a dev-dependency; these are the three places where a generated input is likelier to find a bug than a hand-written case.
- **`cargo-semver-checks` in CI** once the crate is published, to catch accidental breaking changes on release.
- **An `AGENTS.md`** recording the build/test/screenshot commands and the widget contract rules. The contract doc already carries the rules; this is the pointer that makes them findable.
- **ADRs** for the two decisions this codebase has already made implicitly and would otherwise re-litigate: background paint instead of foreground block glyphs, and two-cell handling for ambiguous-width glyphs.

## Guardrails in place

All three live in the repository now; read the files rather than this section, which only records what exists and why.

| Guardrail | File | Catches |
|---|---|---|
| Format, lint, test | `.github/workflows/ci.yml` job `check` | Any unformatted file, any clippy warning, any failing test |
| Module layering | `tools/check_layers.py`, run by the same job | A foundation module importing a widget; any module cycle |
| Dependency policy | `deny.toml`, CI job `deny` | Yanked crates, unlisted licences, unknown registries or git sources, wildcard requirements |

The licence allow-list was built from what the tree actually resolves (`MIT`, `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`, `Unicode-3.0`, `Zlib` — the last for `foldhash`). Allowances for licences no other crate uses were removed: an unmatched allowance is a warning and, worse, pre-approves something nobody checked.

Not installed, and deliberately: a duplication ratchet. The current figure is 3.5 blocks per 1,000 lines, but with two days of history there is no trend to ratchet against. Set the threshold once the repository has a few months of commits.

## Open questions

1. Should the AI harness family (`ai.rs`, `ai_compose.rs`, `ai_agents.rs`, `ai_tools.rs` — 11,000 lines, a third of the library) stay in the core crate or become a `tuile-ai` crate? A workspace split would make the layering compiler-enforced instead of textually checked, and would parallelise compilation. It is the one structural change with a concrete argument behind it.
2. Is the 2,000-line-plus file size in the `ai*` modules and `toggle.rs` comfortable to work in? The co-change data says their parts move together, so there is no evidence-backed split line — but the person editing them has better information than the tool does.

## Methodology and caveats

Audited with the evidence-audit skill on 2026-09-11. Grades are weighted by how strongly the research supports each dimension, not by preference.

Known limits of this audit:
- **The churn signal is young.** All 18 commits land within two days, so the hotspot ranking reflects authoring iterations, not maintenance history. Re-run after real change activity before trusting dimension 2.
- **Dimension 8 was graded on guardrails only**, because a trend needs a time series and this repository does not have one yet.
- **Six relevant tools were not installed** and nothing was installed without asking: `cargo-audit`, `cargo-deny`, `cargo-llvm-cov`, `cargo-mutants`, `layered-crate`, `lizard`. Advisory status, licence policy, coverage and machine-verified layering are therefore unverified; the module graph in F-04 comes from a custom `use crate::` parse, which resolves paths textually and could miss a re-export chain.
- **Complexity figures are an indentation proxy**, not cyclomatic complexity, and the literature is explicit that no metric captures understandability — dimension 4 was graded by reading `draw.rs`, `toggle.rs`, `charts.rs`, `ai.rs` and `input.rs`, using the numbers only to choose which files to read.
- **Coverage was not measured and no coverage target is recommended**; the evidence linking coverage to test effectiveness is weak once suite size is controlled.

Evidence tiers: **strong** = replicated across studies or large industry datasets · **moderate** = real support, limited samples · **emerging** = recent, not yet replicated · **expert opinion** = widely held, little measurement · **contested** = evidence conflicts.
