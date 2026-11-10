[CmdletBinding()]
param(
    [ValidateSet('dev', 'release', 'play')][string]$Profile = 'release',
    [switch]$ImporterOnly
)

$ErrorActionPreference = 'Stop'
$checkout = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$missing = @('cargo', 'rustup' | Where-Object { !(Get-Command $_ -ErrorAction SilentlyContinue) })
if (!$ImporterOnly) {
    $missing += @('go', 'make' | Where-Object { !(Get-Command $_ -ErrorAction SilentlyContinue) })
}
if ($missing) { throw "Install the required build tools and reopen PowerShell: $($missing -join ', ')." }

$manifest = Join-Path $PSScriptRoot 'schematica/Cargo.toml'
$target = Join-Path $checkout 'target/builder-importer'
$profileDir = if ($Profile -eq 'dev') { 'debug' } else { $Profile }
$stamp = Join-Path $checkout "target/$profileDir/builder-commit.txt"
Push-Location -LiteralPath $checkout
try {
    cargo build --locked --release --manifest-path $manifest --target-dir $target -p schematica-builder
    if ($LASTEXITCODE) { throw 'Could not build the schematic importer.' }
    cargo build --locked --release --manifest-path $manifest --target-dir $target -p schematica-guest --target wasm32-unknown-unknown
    if ($LASTEXITCODE) { throw 'Could not build the schematic guest.' }
    if ($ImporterOnly) { return }

    $commit = git rev-parse HEAD
    if ($LASTEXITCODE -or (git status --porcelain)) { throw 'Commit or stash changes before building the launcher version.' }
    if (Test-Path -LiteralPath $stamp) { Remove-Item -LiteralPath $stamp }
    make play-build "PROFILE=$Profile"
    if ($LASTEXITCODE) { throw 'Cinnabar build failed. Check the build output above.' }
    if ((git rev-parse HEAD) -ne $commit -or (git status --porcelain)) {
        throw 'The source changed during the build. Rebuild before launching.'
    }
    foreach ($file in 'cinnabar.exe', 'bedrock-core.exe') {
        if (!(Test-Path -LiteralPath (Join-Path (Split-Path -Parent $stamp) $file) -PathType Leaf)) {
            throw "The upstream build did not produce $file."
        }
    }
    Set-Content -LiteralPath $stamp -Value $commit -Encoding ASCII
} finally { Pop-Location }
