# ODR Oracle lab driver for Windows (PowerShell 5.1+ or 7). Same commands as lab.sh.
#   .\lab.ps1 up        start Oracle Free, wait until healthy (first run: 5-15 min)
#   .\lab.ps1 corpus    create tablespaces + golden corpus
#   .\lab.ps1 golden    export truth, clean shutdown, copy datafiles + SHA-256 -> out\golden\<run>\
#   .\lab.ps1 start     restart the stopped container
#   .\lab.ps1 down      stop container (DATA KEPT)
#   .\lab.ps1 destroy   remove container AND data volume (asks for confirmation)
# Requires Docker Desktop (WSL2 backend), >= 8 GB RAM, >= 30 GB free disk.
param([Parameter(Position = 0)][string]$Cmd = "")
$ErrorActionPreference = "Stop"
$Here = Split-Path -Parent $PSScriptRoot

# Load lab.env (KEY=VALUE, '#' comments).
$cfg = @{}
Get-Content (Join-Path $Here "lab.env") | ForEach-Object {
    if ($_ -match '^\s*([A-Z_]+)\s*=\s*(.*)\s*$') { $cfg[$Matches[1]] = $Matches[2] }
}
$C = $cfg.ODR_CONTAINER

function Invoke-Docker { & docker @args; if ($LASTEXITCODE -ne 0) { throw "docker $args failed ($LASTEXITCODE)" } }
function Invoke-Sql([string]$a) { Invoke-Docker exec -i $C bash -lc "cd /tmp/odr && sqlplus -s / as sysdba $a" }

function Wait-Healthy {
    Write-Host "waiting for database (first start creates it; can take 15 min)..."
    for ($i = 0; $i -lt 180; $i++) {
        $s = (& docker inspect -f '{{.State.Health.Status}}' $C 2>$null)
        if ($s -eq "healthy") { Write-Host "healthy"; return }
        Start-Sleep -Seconds 10
    }
    throw "timeout: run 'docker logs $C'"
}

function Copy-Sql {
    Invoke-Docker exec $C bash -lc "mkdir -p /tmp/odr /opt/oracle/oradata/odr"
    Invoke-Docker cp "$(Join-Path $Here 'sql')\." "${C}:/tmp/odr/"
    # Windows checkouts may carry CRLF; the shell helper must be LF inside Linux.
    Invoke-Docker exec $C bash -lc "sed -i 's/\r$//' /tmp/odr/*.sh /tmp/odr/*.sql"
}

switch ($Cmd) {
    "up" {
        & docker volume create odr-oradata | Out-Null
        & docker container inspect $C *> $null
        if ($LASTEXITCODE -ne 0) {
            Invoke-Docker run -d --name $C -p "$($cfg.ODR_PORT):1521" `
                -e "ORACLE_PWD=$($cfg.ODR_PWD)" -e "ORACLE_CHARACTERSET=AL32UTF8" `
                -v odr-oradata:/opt/oracle/oradata $cfg.ODR_IMAGE
        } else { Invoke-Docker start $C | Out-Null }
        Wait-Healthy
    }
    "start" { Invoke-Docker start $C | Out-Null; Wait-Healthy }
    "corpus" {
        Copy-Sql
        Invoke-Sql "@00_tablespaces.sql $($cfg.ODR_PDB) $($cfg.ODR_PWD)"
        Invoke-Docker exec -i $C bash -lc "cd /tmp/odr && sqlplus -s ODR/$($cfg.ODR_PWD)@//localhost:1521/$($cfg.ODR_PDB) @10_corpus.sql"
        Write-Host "corpus created"
    }
    "golden" {
        $run = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
        $out = Join-Path $Here "out\golden\$run"
        New-Item -ItemType Directory -Force -Path $out | Out-Null
        Copy-Sql
        Invoke-Docker exec $C bash -lc "rm -rf /tmp/odr/out && mkdir -p /tmp/odr/out/meta"
        Invoke-Sql "@20_export_golden.sql $($cfg.ODR_PDB) /tmp/odr/out"
        try { Invoke-Sql "@30_block_dumps.sql $($cfg.ODR_PDB)" } catch { Write-Warning "block dumps failed (non-fatal)" }
        & docker exec $C bash -lc 'cp $(ls -t $ORACLE_BASE/diag/rdbms/*/*/trace/*odr_dump*.trc 2>/dev/null) /tmp/odr/out/meta/ 2>/dev/null || true'
        "SHUTDOWN IMMEDIATE" | & docker exec -i $C bash -lc "sqlplus -s / as sysdba"
        Invoke-Docker exec $C bash -lc "bash /tmp/odr/collect.sh /tmp/odr/out"
        Invoke-Docker cp "${C}:/tmp/odr/out/." "$out\"
        (& docker image inspect --format '{{.Id}} {{index .RepoDigests 0}}' $cfg.ODR_IMAGE) | Set-Content (Join-Path $out "meta\image.txt")
        # SHA-256 of every artifact (same format as sha256sum: "<hash>  ./relative/path").
        $sums = Get-ChildItem -Recurse -File $out | Where-Object Name -ne "SHA256SUMS" | Sort-Object FullName | ForEach-Object {
            $rel = "./" + $_.FullName.Substring($out.Length + 1).Replace('\', '/')
            "{0}  {1}" -f (Get-FileHash -Algorithm SHA256 $_.FullName).Hash.ToLower(), $rel
        }
        [IO.File]::WriteAllLines((Join-Path $out "SHA256SUMS"), $sums)
        Invoke-Docker stop $C | Out-Null
        Write-Host "golden set: $out"
        Write-Host "next: .\lab.ps1 start   (to use the DB again)"
    }
    "down" { Invoke-Docker stop $C }
    "destroy" {
        $a = Read-Host "Delete container AND all lab data? type YES"
        if ($a -eq "YES") { & docker rm -f $C; & docker volume rm odr-oradata }
    }
    default { Get-Content $PSCommandPath -TotalCount 8 | Select-Object -Skip 1; exit 2 }
}
