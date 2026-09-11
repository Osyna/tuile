# Codebase Audit — tuiforge

Rust workspace (tuiforge library + showcase binary) on ratatui 0.30 · 53,387 code lines across 87 files · 18 commits · 1 contributor · audited 2026-09-11
Archetype: **library** (v0.1.0, unpublished, repository and keywords set for crates.io)

## Verdict

**64/100 — Strained**

The code itself is in good shape: zero `unsafe`, zero module cycles across 51 modules, clippy clean at `--all-targets`, 253 tests running in 0.01s, and every buffer write clipped through `area.intersection`. The problem is that none of it is enforced. There is no CI, no `deny.toml`, no advisory scanning, and no layering check, so the entire quality bar exists only because a human remembers to run the commands — and the repo is about to be published to crates.io, where that stops being a private matter. Fix the guardrails first (P0-1 through P0-3, roughly two hours total); they are what keeps the other eight dimensions from decaying silently.

## Scorecard

| # | Dimension | Grade | Weight | Evidence | Finding |
|---|-----------|-------|--------|----------|---------|
| 1 | Coupling & cycles | 3/5 | ×3 | strong | 0 module cycles, clean one-way foundation layer, but one large crate with no layering enforcement |
| 2 | Hotspot concentration | 3/5 | ×3 | strong | 2 severe hotspots (`charts.rs`, `toggle.rs`); both tested; churn signal weak on a 2-day history |
| 3 | Change locality & cohesion | 4/5 | ×2 | strong | Package-by-feature; exactly one cross-directory co-change pair |
| 4 | Cognitive load | 3/5 | ×2 | moderate | Narrow, consistent widget interface, but 5 files over 2,000 lines and a crate-wide clippy allow |
| 5 | Test design quality | 4/5 | ×2 | moderate | 208 lib + 45 doc tests, fast and deterministic; no integration tests against the public API |
| 6 | Boundary correctness | 4/5 | ×2 | moderate | No `unsafe`, no unguarded `unwrap`, all slicing char-safe; `forbid(unsafe_code)` not set |
| 7 | Duplication class | 2/5 | ×1 | moderate | `word_boundary` — one rule stated identically in two hotspot files |
| 8 | AI-risk signals | 2/5 | ×2 | emerging | No CI, no fitness functions, no duplication gate; 73,101 added vs 10,209 deleted lines |
| 9 | Supply chain | 3/5 | ×1 | strong | `Cargo.lock` committed, 3 direct deps current; no `cargo-deny`, `cargo-audit`, or SBOM |
| 10 | Docs & decisions | 4/5 | ×1 | expert opinion | README with gallery, `docs/WIDGET_CONTRACT.md`, 45 doc tests; no ADRs |

No dimensions marked n/a. Overall = 61/95 × 100 = **64**.

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
| 1 ★ | `tuiforge/src/widgets/charts.rs` | 9 | 1681 | 222.7 | 1 | 2004.3 |
| 2 ★ | `tuiforge/src/widgets/toggle.rs` | 8 | 1765 | 238.6 | 1 | 1908.8 |
| 3 | `tuiforge/src/widgets/textarea.rs` | 9 | 1116 | 204.8 | 1 | 1843.2 |
| 4 | `showcase/src/pages/navigation.rs` | 7 | 679 | 241.3 | 1 | 1689.1 |
| 5 | `tuiforge/src/widgets/ai.rs` | 7 | 2676 | 227.1 | 1 | 1589.7 |
| 6 | `showcase/src/pages/charts.rs` | 6 | 471 | 263.0 | 1 | 1578.0 |
| 7 | `showcase/src/pages/controls.rs` | 7 | 814 | 223.7 | 1 | 1565.9 |
| 8 | `tuiforge/src/widgets/select.rs` | 9 | 1260 | 171.8 | 1 | 1546.2 |
| 9 | `tuiforge/src/widgets/slider.rs` | 9 | 1100 | 170.1 | 1 | 1530.9 |
| 10 | `tuiforge/src/draw.rs` | 9 | 678 | 151.6 | 1 | 1364.4 |

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
**Where:** `tuiforge/src/widgets/input.rs:398-425`, `tuiforge/src/widgets/textarea.rs:450-477`
**Measured:** `duplication.py` flagged a 21-line drifted block; diffing the two functions with the parameter renamed shows the bodies are **byte-identical**

Ctrl+Left / Ctrl+Right word navigation is implemented twice, in the two text-entry widgets, in files ranked #3 and #12 by hotspot score. The rule (skip non-alphanumerics, then skip alphanumerics, saturating at the ends) is the same thing a user perceives in both widgets, so the copies must not diverge — and nothing stops them. This is the duplication class worth extracting: not shape-similar code, but one piece of knowledge with two homes.

### F-04 — One large crate, layering unenforced
**Severity:** medium
**Evidence:** [emerging]
**Where:** `tuiforge/src/` — 51 modules, 43 of them widgets
**Measured:** custom import graph: 0 cycles, 0 foundation→widget edges, fan-in `theme` 43, `draw` 42, `core` 36

The layering is currently correct: widgets depend on `theme`/`draw`/`core`/`anim`/`layout`, and no foundation module imports a widget. Rust does not check this — module cycles inside a crate are legal and invisible, and cargo only guarantees the *crate* graph is acyclic. So the property that makes this codebase navigable is held up by nothing but discipline. That is exactly the condition under which AI-assisted commits reintroduce boundary violations faster than review catches them.

### F-05 — Crate-wide clippy suppression
**Severity:** low
**Evidence:** [expert opinion]
**Where:** `tuiforge/src/lib.rs:65`
**Measured:** `#![allow(clippy::too_many_arguments, clippy::type_complexity)]`; removing it produces 16 warnings across 10 files

The allow is documented with a reason and was measured to be load-bearing, which is better than most. It is still a blanket crate-level waiver: any *new* nine-argument function or nested generic type is now silently accepted. Narrowing it to the specific items keeps the warning live for new code.

### F-06 — No integration tests against the public API
**Severity:** low
**Evidence:** [moderate]
**Where:** `tuiforge/tests/` does not exist
**Measured:** 208 unit tests, all inside `#[cfg(test)] mod tests`; 3 examples in `tuiforge/examples/`

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
| `tuiforge/src/widgets/spinner/spinners.rs` (1,437 lines) | Largest non-`ai` file in the crate | It is a data table of 102 spinner frame sets. Complexity is near zero and it has not changed since it was written. |
| 34 exact clone groups across widget builders | `duplication.py` flags them as identical | They are the per-widget `theme()` / `focused()` / `new()` setters that the widget contract requires. Collapsing them into a macro or trait would hurt rustdoc output and readability for a gain of nothing. |
| `ai_agents.rs:156-178` ↔ `ai_compose.rs:1100-1122` (largest drifted group) | 23 lines, flagged first by the tool | Two unrelated enums (`AgentStatus`, `HarnessMode`) that each happen to have a `glyph`/`label` match arm. Same shape, different rules — the textbook case for leaving duplication alone. |
| `ai.rs`, `ai_compose.rs`, `ai_agents.rs`, `ai_tools.rs` (2,400-3,200 lines each) | Large files | Each is one cohesive widget family with a narrow public interface. No independent change schedule has appeared to split along. |
| `spinners.rs:177` frame set containing `- – —` | Looks like the em-dash rule being violated | It is animation data: a spinner whose frames are a growing dash. |
| The 9 `self.expect(c)?` calls in `ai_tools.rs` | Read as panicking `Option::expect` | They are calls to the JSON parser's own `expect` method, which returns `Result` and propagates with `?`. |

## Change plan

Prioritised by `impact × (1 / effort)`, hotspots first. Total P0 effort is roughly two hours.

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
- **Where** `tuiforge/src/lib.rs`, top of file
- **How** Add the attribute
- **Effort** S (~2 min)
- **Risk** low — it either compiles or names the exception
- **Verify** `cargo build --workspace`
- **Rollback** remove the line

### P1 — do next

#### P1-1 Extract `word_boundary` to one home
- **Why** One user-visible rule with two identical implementations in two hotspot files (F-03) [contested in general, but this case is knowledge duplication under the standing ruling]
- **Where** `tuiforge/src/widgets/input.rs`, `tuiforge/src/widgets/textarea.rs`; new free function in `tuiforge/src/core.rs` or a small `text_nav` module
- **How** `pub(crate) fn word_boundary(graphemes: &[&str], pos: usize, forward: bool) -> usize`; both widgets call it with their own grapheme slice. The signatures differ only by how the slice is obtained, so the extraction is mechanical.
- **Effort** S (~30 min)
- **Risk** low — pure function, both call sites covered by existing tests
- **Verify** `cargo test -p tuiforge --lib input:: textarea::`
- **Rollback** single commit

#### P1-2 Declare and enforce the module layering
- **Why** The layering is correct today and held up by nothing (F-04). Fitness functions are the recommended counterweight when structural drift outpaces review [emerging, low risk]
- **Where** `tuiforge/src/`
- **How** Two options, in order of preference: (a) `cargo install layered-crate --locked`, declare `foundation = [theme, draw, core, anim, layout, fuzzy]` below `widgets`, run it in CI; (b) if that proves awkward, keep the custom import-graph check from this audit as a small script in CI asserting zero foundation→widget edges and zero module cycles.
- **Effort** M (~2 h including the CI wiring)
- **Risk** low — reports only
- **Verify** the check passes now and fails if a `use crate::widgets::` is added to `draw.rs`
- **Rollback** remove the CI step

#### P1-3 Add integration tests against the public API
- **Why** Nothing currently exercises the crate the way a consumer does; for a pre-publication library this doubles as API review (F-06) [moderate]
- **Where** new `tuiforge/tests/public_api.rs`
- **How** Build and render half a dozen representative widgets using only `tuiforge::prelude::*`, asserting on buffer contents. Anything that needs a private item is an API gap worth knowing about before 0.1.0 ships.
- **Effort** M (~half a day)
- **Risk** low
- **Verify** `cargo test -p tuiforge --test public_api`
- **Rollback** delete the file

#### P1-4 Narrow the crate-wide clippy allow
- **Why** A blanket waiver silently accepts new violations (F-05) [expert opinion]
- **Where** `tuiforge/src/lib.rs:65` and the 15 files that need it
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

## Guardrails to install

```yaml
# .github/workflows/ci.yml
name: ci
on:
  push: { branches: [main] }
  pull_request:
jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: rustfmt, clippy }
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all -- --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace
  deny:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: EmbarkStudios/cargo-deny-action@v2
```

```toml
# deny.toml
[advisories]
yanked = "deny"

[licenses]
allow = ["MIT", "Apache-2.0", "Unicode-3.0", "BSD-3-Clause"]

[bans]
multiple-versions = "warn"   # hashbrown is currently duplicated via kasuari

[sources]
unknown-registry = "deny"
unknown-git = "deny"
```

```toml
# Cargo.toml — replace the crate-level allow with versioned lint policy (P1-4)
[workspace.lints.clippy]
unwrap_used = "warn"
```

## Open questions

1. Is `showcase` intended to ship as a published binary, or is it a development gallery? If the latter, it should be excluded from the release profile and marked `publish = false`, which also removes it from the audit surface next run.
2. Is the AI harness family (`ai.rs`, `ai_compose.rs`, `ai_agents.rs`, `ai_tools.rs` — 11,000 lines, a third of the library) intended to stay in the core crate, or become a `tuiforge-ai` crate? A workspace split would make the layering compiler-enforced for free and parallelise compilation, which is the one structural change with a concrete argument behind it.

## Methodology and caveats

Audited with the evidence-audit skill on 2026-09-11. Grades are weighted by how strongly the research supports each dimension, not by preference.

Known limits of this audit:
- **The churn signal is young.** All 18 commits land within two days, so the hotspot ranking reflects authoring iterations, not maintenance history. Re-run after real change activity before trusting dimension 2.
- **Dimension 8 was graded on guardrails only**, because a trend needs a time series and this repository does not have one yet.
- **Six relevant tools were not installed** and nothing was installed without asking: `cargo-audit`, `cargo-deny`, `cargo-llvm-cov`, `cargo-mutants`, `layered-crate`, `lizard`. Advisory status, licence policy, coverage and machine-verified layering are therefore unverified; the module graph in F-04 comes from a custom `use crate::` parse, which resolves paths textually and could miss a re-export chain.
- **Complexity figures are an indentation proxy**, not cyclomatic complexity, and the literature is explicit that no metric captures understandability — dimension 4 was graded by reading `draw.rs`, `toggle.rs`, `charts.rs`, `ai.rs` and `input.rs`, using the numbers only to choose which files to read.
- **Coverage was not measured and no coverage target is recommended**; the evidence linking coverage to test effectiveness is weak once suite size is controlled.

Evidence tiers: **strong** = replicated across studies or large industry datasets · **moderate** = real support, limited samples · **emerging** = recent, not yet replicated · **expert opinion** = widely held, little measurement · **contested** = evidence conflicts.
