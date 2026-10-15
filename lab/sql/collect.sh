#!/usr/bin/env bash
# Runs INSIDE the container after SHUTDOWN IMMEDIATE (database closed = consistent files).
# Copies every datafile of the CDB and PDBs (SYSTEM included: dictionary tests need it),
# runs dbv on each copy, writes a manifest. Usage: collect.sh <outdir>
set -euo pipefail
out="$1"; mkdir -p "$out/datafiles" "$out/dbv"
: > "$out/meta/datafiles_copied.txt"
find /opt/oracle/oradata -type f -name '*.dbf' | sort | while read -r f; do
  name="$(echo "${f#/opt/oracle/oradata/}" | tr '/' '__')"
  cp --sparse=never "$f" "$out/datafiles/$name"
  echo "$f|$name|$(stat -c %s "$f")" >> "$out/meta/datafiles_copied.txt"
  # dbv on the COPY (never on evidence). Temp files are skipped by dbv itself.
  dbv file="$out/datafiles/$name" blocksize=8192 > "$out/dbv/$name.txt" 2>&1 || true
done
