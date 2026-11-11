$ErrorActionPreference = 'Stop'
$manifest = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'schematica/Cargo.toml') -Raw
$match = [regex]::Match($manifest, '(?s)\[workspace\.package\]\s*version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"')
if (!$match.Success) { throw 'Set the shared Builder release version in schematica/Cargo.toml.' }
$match.Groups[1].Value
