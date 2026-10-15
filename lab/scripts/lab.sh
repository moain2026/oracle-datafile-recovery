#!/usr/bin/env bash
# ODR Oracle lab driver (Linux / macOS / WSL / Git-Bash). Windows PowerShell: lab.ps1.
#   ./lab.sh up        start Oracle Free, wait until healthy (first run builds the DB: 5-15 min)
#   ./lab.sh corpus    create smallfile/bigfile tablespaces + golden corpus (idempotent: fails if exists)
#   ./lab.sh golden    export truth, shut down cleanly, copy datafiles + SHA-256 -> out/golden/<run>/
#   ./lab.sh start     restart the stopped container (after golden)
#   ./lab.sh down      stop container (DATA KEPT)
#   ./lab.sh destroy   remove container AND data volume (asks for confirmation)
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck disable=SC1091
set -a; . "$here/lab.env"; set +a
dk() { docker "$@"; }
sql() { dk exec -i "$ODR_CONTAINER" bash -lc "cd /tmp/odr && sqlplus -s / as sysdba $*"; }

wait_healthy() {
  echo "waiting for database (first start creates it; can take 15 min)..."
  for _ in $(seq 1 180); do
    s="$(dk inspect -f '{{.State.Health.Status}}' "$ODR_CONTAINER" 2>/dev/null || echo none)"
    [ "$s" = healthy ] && { echo "healthy"; return 0; }
    sleep 10
  done
  echo "timeout: check 'docker logs $ODR_CONTAINER'" >&2; return 1
}

case "${1:-}" in
  up)
    dk volume create odr-oradata >/dev/null
    if ! dk container inspect "$ODR_CONTAINER" >/dev/null 2>&1; then
      dk run -d --name "$ODR_CONTAINER" -p "$ODR_PORT:1521" \
        -e ORACLE_PWD="$ODR_PWD" -e ORACLE_CHARACTERSET=AL32UTF8 \
        -v odr-oradata:/opt/oracle/oradata "$ODR_IMAGE"
    else
      dk start "$ODR_CONTAINER" >/dev/null
    fi
    wait_healthy ;;
  start) dk start "$ODR_CONTAINER" >/dev/null; wait_healthy ;;
  corpus)
    dk exec "$ODR_CONTAINER" bash -lc "mkdir -p /tmp/odr /opt/oracle/oradata/odr"
    dk cp "$here/sql/." "$ODR_CONTAINER:/tmp/odr/"
    dk exec "$ODR_CONTAINER" bash -lc "sed -i 's/\r$//' /tmp/odr/*.sh /tmp/odr/*.sql"
    sql "@00_tablespaces.sql $ODR_PDB $ODR_PWD"
    dk exec -i "$ODR_CONTAINER" bash -lc "cd /tmp/odr && sqlplus -s ODR/$ODR_PWD@//localhost:1521/$ODR_PDB @10_corpus.sql"
    echo "corpus created" ;;
  golden)
    run="$(date -u +%Y%m%dT%H%M%SZ)"
    out="$here/out/golden/$run"; mkdir -p "$out/meta" "$out/datafiles"
    dk cp "$here/sql/." "$ODR_CONTAINER:/tmp/odr/"
    dk exec "$ODR_CONTAINER" bash -lc "sed -i 's/\r$//' /tmp/odr/*.sh /tmp/odr/*.sql"
    dk exec "$ODR_CONTAINER" bash -lc "rm -rf /tmp/odr/out && mkdir -p /tmp/odr/out/meta"
    sql "@20_export_golden.sql $ODR_PDB /tmp/odr/out"
    # Block dumps of the first block of every corpus segment (reference for lab-verify).
    sql "@30_block_dumps.sql $ODR_PDB" || echo "warning: block dumps failed (non-fatal)"
    dk exec "$ODR_CONTAINER" bash -lc 'cp $(ls -t $ORACLE_BASE/diag/rdbms/*/*/trace/*odr_dump*.trc 2>/dev/null) /tmp/odr/out/meta/ 2>/dev/null || true'
    # Consistent copy: clean shutdown first (ADR-0004, Oracle user-managed backup docs).
    echo "SHUTDOWN IMMEDIATE" | dk exec -i "$ODR_CONTAINER" bash -lc "sqlplus -s / as sysdba"
    dk exec "$ODR_CONTAINER" bash -lc "bash /tmp/odr/collect.sh /tmp/odr/out"
    dk cp "$ODR_CONTAINER:/tmp/odr/out/." "$out/"
    dk image inspect --format '{{.Id}} {{index .RepoDigests 0}}' "$ODR_IMAGE" > "$out/meta/image.txt" 2>/dev/null || echo "$ODR_IMAGE" > "$out/meta/image.txt"
    ( cd "$out" && find . -type f ! -name SHA256SUMS -print0 | sort -z | xargs -0 sha256sum > SHA256SUMS )
    dk stop "$ODR_CONTAINER" >/dev/null
    echo "golden set: $out"
    echo "next: ./lab.sh start   (to use the DB again)" ;;
  down) dk stop "$ODR_CONTAINER" ;;
  destroy)
    read -r -p "Delete container AND all lab data? type YES: " a
    [ "$a" = YES ] && { dk rm -f "$ODR_CONTAINER"; dk volume rm odr-oradata; } ;;
  *) sed -n '2,9p' "$0"; exit 2 ;;
esac
