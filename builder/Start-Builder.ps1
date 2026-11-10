[CmdletBinding()]
param(
    [ValidateSet('debug', 'release', 'play')][string]$Profile = 'release',
    [switch]$Import,
    [string[]]$InputFiles,
    [switch]$Demo,
    [switch]$ImportOnly,
    [switch]$Check
)

$ErrorActionPreference = 'Stop'
$checkout = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$profileDir = Join-Path $checkout "target/$Profile"
$client = Join-Path $profileDir 'cinnabar.exe'
$userRoot = Join-Path $checkout '.local/builder-user'
$stamp = Join-Path $profileDir 'builder-commit.txt'
$codec = Join-Path $checkout 'target/builder-importer/release/schematica-builder.exe'
$guest = Join-Path $checkout 'target/builder-importer/wasm32-unknown-unknown/release/schematica_guest.wasm'
$head = git -C $checkout rev-parse HEAD
if ($LASTEXITCODE) { throw 'Could not read the Cinnabar checkout revision.' }
$missing = @($client, (Join-Path $profileDir 'bedrock-core.exe'), $stamp | Where-Object {
    !(Test-Path -LiteralPath $_ -PathType Leaf)
})
$built = !$missing -and (Get-Content -LiteralPath $stamp -Raw).Trim() -eq $head
$importerReady = (Test-Path -LiteralPath $codec -PathType Leaf) -and (Test-Path -LiteralPath $guest -PathType Leaf)

if ($Check) {
    [pscustomobject]@{
        Checkout = $checkout
        Commit = $head
        Bedrock = (Get-Content -LiteralPath (Join-Path $checkout 'assets/bedrock-target.json') -Raw | ConvertFrom-Json).game_version
        ClientBuilt = [bool]$built
        ImporterBuilt = $importerReady
        UserRoot = $userRoot
        MissingTools = @('go', 'make' | Where-Object { !(Get-Command $_ -ErrorAction SilentlyContinue) })
    }
    return
}

if ($ImportOnly -and !$InputFiles -and !$Demo) { $Import = $true }
if (!$ImportOnly -and !$built) { throw 'Build this revision with builder/Build-Builder.ps1 before launching.' }

if ($Import -or $InputFiles -or $Demo) {
    if (!$importerReady) { throw 'Build the importer with builder/Build-Builder.ps1 -ImporterOnly first.' }
    if ($Import -and !$InputFiles) {
        Add-Type -AssemblyName System.Windows.Forms
        $picker = New-Object System.Windows.Forms.OpenFileDialog
        $picker.Title = 'Choose schematics for the Builder library'
        $picker.Filter = 'Minecraft schematics|*.schematic;*.schem;*.litematic;*.mcstructure'
        $picker.Multiselect = $true
        try {
            if ($picker.ShowDialog() -ne [System.Windows.Forms.DialogResult]::OK) { return }
            $InputFiles = $picker.FileNames
        } finally { $picker.Dispose() }
    }
    $inputs = @(foreach ($path in $InputFiles) { (Resolve-Path -LiteralPath $path).ProviderPath })
    $mode = if ($Demo) { 'demo-library' } else { 'library' }
    $mods = Join-Path $userRoot 'data/mods'
    $backups = Join-Path $userRoot 'library-backups'
    New-Item -ItemType Directory -Force -Path $mods, $backups | Out-Null
    $installed = Join-Path $mods 'schematica-builder.wasm'
    $id = [guid]::NewGuid()
    $staged = Join-Path $backups "$id.pending"
    $previousTemplate = [Environment]::GetEnvironmentVariable('SCHEMATICA_GUEST_TEMPLATE', 'Process')
    try {
        $env:SCHEMATICA_GUEST_TEMPLATE = $guest
        & $codec $mode $staged @inputs
        if ($LASTEXITCODE -or !(Test-Path -LiteralPath $staged -PathType Leaf)) {
            throw 'Import failed. The current schematic library is still available.'
        }
        if (Test-Path -LiteralPath $installed -PathType Leaf) {
            $backup = Join-Path $backups "$(Get-Date -Format yyyyMMdd-HHmmss)-$id.wasm"
            [IO.File]::Replace($staged, $installed, $backup)
        } else {
            [IO.File]::Move($staged, $installed)
        }
        Write-Output "Installed $installed. Enable it in Settings > Mods, then press Alt+J in a world."
    } finally {
        [Environment]::SetEnvironmentVariable('SCHEMATICA_GUEST_TEMPLATE', $previousTemplate, 'Process')
        if (Test-Path -LiteralPath $staged) { Remove-Item -LiteralPath $staged }
    }
}

if ($ImportOnly) { return }

$previousRoot = [Environment]::GetEnvironmentVariable('CINNABAR_USER_ROOT', 'Process')
Push-Location -LiteralPath $checkout
try {
    $env:CINNABAR_USER_ROOT = $userRoot
    & $client
    if ($LASTEXITCODE) { throw "Cinnabar exited with code $LASTEXITCODE." }
} finally {
    [Environment]::SetEnvironmentVariable('CINNABAR_USER_ROOT', $previousRoot, 'Process')
    Pop-Location
}
