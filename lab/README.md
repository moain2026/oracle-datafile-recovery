# ODR Lab — golden corpus on Windows / Linux

> **Privacy:** never commit `*.dbf`, `*.dmp`, traces or `lab/out/`. Customer files stay off GitHub.

## Requirements
- Docker Desktop (WSL2 backend) on Windows, or Docker Engine on Linux.
- 8 GB RAM or more, and 30 GB of free disk space.
- Access to `container-registry.oracle.com`. Accept the licence for `database/free`, then run `docker login container-registry.oracle.com`.
- The image tag is set in `lab.env`. If the tag isn't available, run `docker pull container-registry.oracle.com/database/free:latest` and update `ODR_IMAGE`.

## Steps
| Step | Linux / WSL | PowerShell | Time |
|---|---|---|---|
| Pull and start the database | `lab/scripts/lab.sh up` | `Set-ExecutionPolicy -Scope Process Bypass; .\lab\scripts\lab.ps1 up` | 10–20 min the first time |
| Create the tablespaces and test tables | `lab.sh corpus` | `lab.ps1 corpus` | ~3 min |
| Golden export (truth + dumps + datafile copies after SHUTDOWN) | `lab.sh golden` | `lab.ps1 golden` | ~5 min |
| Stop / destroy | `lab.sh down` / `destroy` | same | — |

The output goes to `lab/out/golden/<UTC>/`:
- `meta/` — the version, the files, the segments and the rowids.
- `rows.psv` — `DUMP(col,1016)` for every value; this is the truth.
- `traces/` — block dumps.
- `datafiles/` — copies taken after `SHUTDOWN IMMEDIATE`, each checked with `dbv`.
- `SHA256SUMS`

## Making corrupted copies (on copies only)
```
cargo run -p odr-corrupt -- --input golden/datafiles/X.dbf --output cases/X_bitrot.dbf \
   --block-size 8192 --seed 1 --op bitrot:20
```
Available ops: `bitrot:N`, `torn:N`, `zero:N`, `ransom:LEAD[:CHUNK:EVERY]`, `wipe-header`, `truncate:BYTES`, `swap:N`, `holes:N:LEN`, `forge:N`.
Each output comes with `<out>.manifest.json`, which records every changed byte and the SHA-256 hashes.

Standard case matrix (fixed seed of 1 for each case):

`bitrot:20`, `torn:5`, `zero:10`, `ransom:1048576`, `ransom:0:8192:16`, `wipe-header`, `truncate:<half>`, `swap:4`, `holes:8:512`, `forge:5`, plus "lost SYSTEM" (omit the system01 file).

Manual corruption with HxD is allowed, but **always on a copy**, and you must write down the offset and length you changed.

## What to send back
Send `meta/`, `rows.psv`, `traces/` and `SHA256SUMS` (they're small). Only share the datafiles through a private channel.
