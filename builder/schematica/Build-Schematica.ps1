[CmdletBinding()]
param(
    [Parameter(Position=0)][string]$Schematic,
    [string]$Output
)

$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$manifest = Join-Path $root 'Cargo.toml'
if (!$Output) {
    $name = if ($Schematic) { [IO.Path]::GetFileNameWithoutExtension($Schematic) } else { 'schematica-demo' }
    $Output = Join-Path (Get-Location) "$name.wasm"
}
$Output = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Output)
if ($Schematic) {
    $Schematic = (Resolve-Path -LiteralPath $Schematic).ProviderPath
    if ($Schematic -eq $Output) { throw 'Choose an output path different from the input file.' }
}
if ([IO.Path]::GetExtension($Output) -ne '.wasm') { throw 'Output must have a .wasm extension.' }
$target = if ($env:CARGO_TARGET_DIR) {
    $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($env:CARGO_TARGET_DIR)
} else {
    Join-Path $root 'target'
}
$work = Join-Path $target 'schematica-blueprints'
New-Item -ItemType Directory -Path $work -Force | Out-Null
$blueprint = Join-Path $work "$([guid]::NewGuid()).scb"
$previousTarget = $env:CARGO_TARGET_DIR
$env:CARGO_TARGET_DIR = $target
try {
    [string[]]$mode = if ($Schematic) { @('import', $Schematic) } else { @('demo') }
    cargo run --locked --manifest-path $manifest -p schematica-builder -- @mode $blueprint
    if ($LASTEXITCODE) { throw 'Blueprint preparation failed.' }
    cargo build --locked --release --manifest-path $manifest -p schematica-guest --target wasm32-unknown-unknown
    if ($LASTEXITCODE) { throw 'Guest compilation failed.' }
    $core = Join-Path $target 'wasm32-unknown-unknown/release/schematica_guest.wasm'
    cargo run --locked --manifest-path $manifest -p schematica-builder -- pack-blueprint $core $blueprint $Output
    if ($LASTEXITCODE) { throw 'Component packaging failed.' }
} finally {
    $env:CARGO_TARGET_DIR = $previousTarget
    if (Test-Path -LiteralPath $blueprint) { Remove-Item -LiteralPath $blueprint }
}

