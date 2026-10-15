# Brief — Agent "Astra": Platform & Evidence (scanner, performance, CLI, Oracle lab, golden corpus, benchmarks, fuzzing)

You are one of two engineers on **Oracle Datafile Recovery (ODR)**: a Rust tool that extracts data from corrupted Oracle `.dbf` files without an Oracle instance, delivered first to YemenSoft (Windows Server shops, Arabic data, accounting systems). The other engineer is the **Lead**, who also reviews and merges your PRs. Quality bar: the highest — this tool will touch customers' real financial data.

**Repo:** https://github.com/moain2026/oracle-datafile-recovery · **Your branch:** `agent/astra` (sub-branches `agent/astra/<feature>` welcome) · **PR target:** `main`

## Read first (in order)
`AGENTS.md`, `STATUS.md`, `docs/adr/0002` (read-only evidence), `0004` (golden corpus), `0005` (quality bar — enforced by CI), `0006` (workflow), `docs/agents/README.md`, `docs/design/architecture.md`, `docs/research/01_foundational_research_ar.md` §3.1 and §10, then the code: `crates/odr-core/src/{bytes,error,block,source,scan}.rs`, `crates/odr-core/src/row/ktb.rs`, `crates/odr-testkit/`, `fuzz/`, `ci/`.

## You own (write only here)
`crates/odr-core/src/scan*`, `crates/odr-core/src/header*` (new), `crates/odr-core/benches/`, `crates/odr-cli/`, `crates/odr-testkit/`, `lab/`, `bench/`, `fuzz/`, `ci/`.
**Do not edit** `bytes.rs` `error.rs` `block.rs` `source.rs` `row/` `types/` `charset/` (Lead) — open an issue `contract: …` instead and work around it inside your area meanwhile.

## Step 0 — environment check
Run `docker info`. **If Docker works**, do L1 first (the lab is the critical path for the whole project). **If not**, start with S1 and write the lab so the owner can run it on Windows with Docker Desktop (`lab/up.ps1`), and say so in your first PR.

## Track L — Lab & ground truth (critical path)
- **L1 Lab** (`lab/`): `docker compose` with Oracle Database Free 23ai (`gvenzl/oracle-free`); idempotent `up` / `build-corpus` / `export-golden` / `down` in **both `.ps1` and `.sh`**. Document a manual path for 19c and 11gR2 (YemenSoft's likely versions).
- **L2 Corpus** (`lab/sql/`, deterministic): ≥ 20 tables — NUMBER edge cases (0, negatives, 38 digits, 1e-130, 9.99…e125, ±∞ via BINARY types where applicable), DATE incl. BC and 1582-10, all TIMESTAMP/INTERVAL kinds, CHAR/VARCHAR2/NCHAR/NVARCHAR2, **Arabic text in an AR8MSWIN1256 DB and an AL32UTF8 DB**, RAW, **a `ROWDEPENDENCIES` table (incl. a chained and a migrated row — LV-019)**, **an indexed cluster with a 2-column key whose 2nd column is NULL for some keys, plus a key spanning > 1 block (LV-020/021)**, **`ALTER TABLE … ADD c DEFAULT x [NOT NULL]` on a populated table, then update some rows (LV-024)**, **a PDB in a CDB (LV-023)**, rows > block size (chained), updated rows (migrated), NULL patterns incl. trailing NULLs, deleted rows, BASIC + OLTP compression, BasicFile/SecureFile LOB inline + out-of-line, partitioned table, IOT, **indexed cluster (incl. a NULL trailing key column, a key spanning two blocks, a deleted member)**, **a `ROWDEPENDENCIES` table with chained and migrated rows** (LV-019 is still unknown), **`ADD COLUMN … DEFAULT` on a populated table (11g NOT NULL and 12c nullable)**, **a CDB with two PDBs**; 8K and 16K block sizes; MSSM and ASSM tablespaces; one tablespace per feature.
- **L3 Golden export** → `lab/golden/<db>/`: per table a canonical CSV (stable order by key; numbers/dates in the exact text form of `OraNumber`/`OraDate` `Display`), per-row SHA-256, `DUMP(col,16)` samples for every column, `dbv` output per datafile, and **`ALTER SYSTEM DUMP DATAFILE … BLOCK …` trace files** for a sample of blocks (these are the reference for every `lab-verify` offset). Large binaries go to a GitHub Release asset with SHA-256 manifest in git — never commit big `.dbf`.
- **L4 lab-verify**: for every entry in `docs/design/lab-verify.md`, add a test in `crates/odr-core/tests/golden_*.rs` (behind a `golden` cargo feature or env var so CI without the corpus still passes) that confirms or refutes it. Refutation ⇒ issue to the Lead immediately with the block trace.

## Track S — Scanner, performance, CLI (REQ-001)
- **S1 Header probe** (`header.rs`): `kcvfh` fields (block size, byte order, DB name/id, file#, checkpoint/resetlogs SCN, fuzzy flag, version) — offsets `lab-verify`. Published 11g BBED map (block 1, see `docs/research/03_deep_research_round2_ar.md` §1): swv @20, cvn @24, dbid @28, dbname[8] @32, fsz @44, bsz @48, fno u16 @52, typ @54, **`kcvfhrdb` @96 (bootstrap$ root dba — expose it, L-6 needs it; LV-023)**, crs @100, rlc @112, rls @116, sta u16 @138, cpc @140, tsn @332, tln @336, tnm[30] @338, rfn @368, ckp scn @484. Struct size differs by version (10g 676, 11g 860, 12c 1112) — validate, don't assume. When the header is destroyed (ransomware), **infer** block size × byte order from the population of self-consistent `kcbh` blocks (format, tail, checksum, rdba continuity); return candidates with a confidence score; ties ⇒ ambiguous, never a silent guess.
- **S2 Streaming parallel scan**: replace the whole-file read in the CLI. Large aligned chunks, `rayon`, ordered results, **SHA-256 computed in the same single pass** (hardware SHA where available; must build on Windows MSVC), memory bounded (≤ 256 MiB at any file size), files > 4 GiB.
- **S3 Index**: add `rdba` and `objd` to the per-block record (use `odr_core::row::ktb::data_object_id`); ≤ 32 bytes/block. `BlockRecord` is a contract — propose the change in your PR description, the Lead approves.
- **S4 Report**: per-objd summary (REQ-001.5), text + `--json` with a versioned schema documented in `docs/design/scan-report.md`; exit codes 0/1/2; Arabic and long Windows paths; clear message when Oracle still holds the file open.
- **S5 Benchmarks** (`criterion`): `xor_fold`, `classify`, end-to-end MB/s on 1 GiB. **Baseline to beat:** 14.0 s / 1 GiB (≈ 77 MB/s) on the Lead's 4-core sandbox. **Target:** classification ≥ 300 MB/s/core, near-linear to 8 cores, end-to-end disk-bound. Publish CPU model + numbers in each perf PR.

## Track Q — Quality harness
- **Q1 Corruption generator** (`odr-testkit`, `bench/corrupt`): random block overwrite at a rate, destroy blocks 0–1, lose SYSTEM file, ransomware simulation (encrypt first N MiB + every k-th 64 KiB), truncation, 512-byte torn writes, block swap (wrong rdba). Deterministic from seed; JSON manifest of damage.
- **Q2 Accuracy harness** (`bench/`): {corpus × pattern × ≥ 5 seeds} → row precision/recall, field bit-exactness, agreement with `dbv`, MB/s, peak RSS; Markdown + JSON report; keep a **hold-out** schema set that is never used while tuning (for M4).
- **Q3 Fuzzing**: seed corpora from golden blocks; a target for every parser (Lead's included — `fuzz/` is yours); nightly long run + 60 s PR smoke; crashing inputs become regression tests.
- **Q4 CI**: keep `ci/github-ci.yml` green on Windows + Linux; produce `odr.exe` artifact.

## Suggested order
Docker available: L1 → L2 → L3 → S1 → S2 → S5 → Q1 → S3 → S4 → Q2 → L4 → Q3.
No Docker: S1 → S2 → S5 → Q1 → L1/L2/L3 scripts (owner runs) → S3 → S4 → Q2 → Q3 → L4.

## Definition of done (every PR)
The five gates green (`docs/agents/README.md` §3) on Windows + Linux; every new parser has proptest "never panics" + fuzz target; no `#[allow]` without a justification comment; no evidence opened for write; no whole-file loads in production paths; numbers only from `bench/`; `STATUS.md` line + `CHANGELOG.md` updated.

## Communication
Your `STATUS.md` line ("قيد العمل") always current. Questions about contracts or formats ⇒ GitHub issue mentioning the Lead. Lessons > 30 min ⇒ `LESSONS.md`.
