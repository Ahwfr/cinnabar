[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$PackageDirectory,
    [string]$Repository = 'Ahwfr/cinnabar',
    [string]$GitHubCli = 'gh',
    [switch]$Publish
)

$ErrorActionPreference = 'Stop'
$directory = (Resolve-Path -LiteralPath $PackageDirectory).Path
$version = & (Join-Path $PSScriptRoot 'Get-BuilderVersion.ps1')
$tag = "builder-v$version"
$notes = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'RELEASE-NOTES.md') -Raw
if (!$notes.StartsWith("# Cinnabar Builder $version")) {
    throw 'Update RELEASE-NOTES.md to match this release version before packaging.'
}
$commit = (git -C (Join-Path $PSScriptRoot '..') rev-parse HEAD).Trim()
if ($LASTEXITCODE) { throw 'Could not read the source revision.' }
$stamp = (Get-Content -LiteralPath (Join-Path $directory 'builder-commit.txt') -Raw).Trim()
if ($stamp -ne $commit) { throw 'Package this exact source revision before publishing.' }
Set-Content -LiteralPath (Join-Path $directory 'builder-version.txt') -Value $version -Encoding ASCII
foreach ($document in @('PORTABLE-README.md','RELEASE-NOTES.md','UPDATE.md')) {
    $name = if ($document -eq 'PORTABLE-README.md') { 'README.md' } else { $document }
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot $document) -Destination (Join-Path $directory $name) -Force
}

# The allowlist keeps private profiles, editable checkouts and compiler caches out of the ZIP.
$names = @(
    'runtime','Cinnabar Builder.exe','schematica-guest-template.bin',
    'builder-commit.txt','builder-version.txt','Builder-Source.zip',
    'README.md','RELEASE-NOTES.md','UPDATE.md',
    'LICENSE','THIRD_PARTY_NOTICES.md','SCHEMATICA-LICENSE.txt','licenses'
)
$paths = foreach ($name in $names) {
    $path = Join-Path $directory $name
    if (!(Test-Path -LiteralPath $path)) { throw "Release payload is missing $name." }
    $path
}
$zip = Join-Path $directory "Cinnabar-Builder-v$version-windows-x64.zip"
Compress-Archive -LiteralPath $paths -DestinationPath $zip -Force
$checksums = Join-Path $directory 'SHA256SUMS.txt'
@($zip, (Join-Path $directory 'Builder-Source.zip')) | ForEach-Object {
    $hash = Get-FileHash -LiteralPath $_ -Algorithm SHA256
    "$($hash.Hash.ToLowerInvariant())  $([IO.Path]::GetFileName($_))"
} | Set-Content -LiteralPath $checksums -Encoding ASCII

if ($Publish) {
    & $GitHubCli release view $tag --repo $Repository *> $null
    if ($LASTEXITCODE -eq 0) { throw "Release $tag already exists. Bump the shared version for this update." }
    & $GitHubCli release create $tag $zip (Join-Path $directory 'Builder-Source.zip') $checksums (Join-Path $directory 'README.md') (Join-Path $directory 'UPDATE.md') --repo $Repository --target $commit --title "Cinnabar Builder v$version" --notes-file (Join-Path $directory 'RELEASE-NOTES.md') --latest
    if ($LASTEXITCODE) { throw 'GitHub did not publish the release. The local package is still available.' }
}
Write-Output $zip
