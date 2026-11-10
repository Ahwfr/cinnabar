[CmdletBinding()]
param(
    [string[]]$InputFiles,
    [string]$Output,
    [switch]$IncludeDemo
)

$ErrorActionPreference = 'Stop'
$interactive = !$InputFiles -and !$IncludeDemo
if ($interactive) {
    Add-Type -AssemblyName System.Windows.Forms
    $picker = New-Object System.Windows.Forms.OpenFileDialog
    $picker.Title = 'Select up to 8 schematics for one library'
    $picker.Filter = 'Minecraft schematics|*.schematic;*.schem;*.litematic;*.mcstructure'
    $picker.Multiselect = $true
    try {
        if ($picker.ShowDialog() -ne [System.Windows.Forms.DialogResult]::OK) { return }
        $InputFiles = $picker.FileNames
    } finally { $picker.Dispose() }
}

function Read-Uleb([byte[]]$Data, [ref]$Index) {
    $value = 0
    for ($shift=0; $shift -le 28; $shift+=7) {
        if ($Index.Value -ge $Data.Length) { throw 'Truncated WebAssembly section.' }
        $byte = $Data[$Index.Value]
        $Index.Value++
        $value = $value -bor (($byte -band 127) -shl $shift)
        if (($byte -band 128) -eq 0) { return $value }
    }
    throw 'Invalid WebAssembly section length.'
}

function Write-Uleb([IO.Stream]$Stream, [int]$Value) {
    while ($Value -ge 128) {
        $Stream.WriteByte([byte](($Value -band 127) -bor 128))
        $Value = $Value -shr 7
    }
    $Stream.WriteByte([byte]$Value)
}

function Set-ComponentGrants([byte[]]$Component, [IO.Stream]$OutputStream) {
    $encoding = [Text.Encoding]::UTF8
    $name = $encoding.GetBytes('cinnabar:grants')
    $request = $encoding.GetBytes('{"terrain":true,"controls":true,"world_draw":true,"render_material":true,"settings":true}')
    $newSection = [IO.MemoryStream]::new()
    try {
        Write-Uleb $newSection $name.Length
        $newSection.Write($name, 0, $name.Length)
        $newSection.Write($request, 0, $request.Length)
        $OutputStream.Write($Component, 0, 8)
        $cursor = 8
        $found = $false
        while ($cursor -lt $Component.Length) {
            $start = $cursor
            $id = $Component[$cursor]
            $cursor++
            $length = Read-Uleb $Component ([ref]$cursor)
            $end = $cursor + $length
            if ($length -lt 0 -or $end -gt $Component.Length) { throw 'Invalid component section.' }
            $custom = $cursor
            if ($id -eq 0) {
                $size = Read-Uleb $Component ([ref]$custom)
                if ($size -eq $name.Length -and $custom + $size -le $end -and
                    $encoding.GetString($Component, $custom, $size) -eq 'cinnabar:grants') {
                    if ($found) { throw 'The component has multiple grants sections.' }
                    $found = $true
                    $OutputStream.WriteByte(0)
                    Write-Uleb $OutputStream ([int]$newSection.Length)
                    $newSection.WriteTo($OutputStream)
                    $cursor = $end
                    continue
                }
            }
            $OutputStream.Write($Component, $start, $end - $start)
            $cursor = $end
        }
        if (!$found) { throw 'The codec did not include a grants section.' }
    } finally { $newSection.Dispose() }
}

$temp = Join-Path ([IO.Path]::GetTempPath()) "schematica-$([guid]::NewGuid())"
try {
    $paths = @($InputFiles | ForEach-Object { (Resolve-Path -LiteralPath $_).ProviderPath })
    $count = $paths.Count + [int]$IncludeDemo.IsPresent
    if ($count -lt 1 -or $count -gt 8) { throw 'Choose between 1 and 8 schematics.' }
    if (!$Output) {
        if ($paths.Count -eq 0) { throw 'Specify -Output when building the demo.' }
        $Output = if ($count -eq 1) {
            [IO.Path]::ChangeExtension($paths[0], '.cinnabar.wasm')
        } else {
            Join-Path (Split-Path -Parent $paths[0]) 'schematica-library.cinnabar.wasm'
        }
    }
    $Output = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Output)
    if ([IO.Path]::GetExtension($Output) -ne '.wasm') { throw 'Output must end in .wasm.' }
    if (Test-Path -LiteralPath $Output) { throw "Output already exists. Choose another name or move it first: $Output" }

    $codec = Join-Path $PSScriptRoot 'Schematica-Codec.exe'
    $template = Join-Path $PSScriptRoot 'schematica-guest-template.bin'
    New-Item -ItemType Directory -Path $temp | Out-Null
    $entries = @()
    if ($IncludeDemo) {
        $demo = Join-Path $temp 'demo.scb'
        & $codec demo $demo
        if ($LASTEXITCODE) { throw 'Demo encoding failed.' }
        $entries += $demo
    }
    foreach ($path in $paths) {
        $entry = Join-Path $temp "$($entries.Count).scb"
        & $codec import $path $entry
        if ($LASTEXITCODE) { throw "Could not import $path" }
        $entries += $entry
    }

    $library = [IO.MemoryStream]::new()
    $writer = [IO.BinaryWriter]::new($library)
    try {
        $writer.Write([Text.Encoding]::ASCII.GetBytes('SCL1'))
        $writer.Write([byte]$entries.Count)
        foreach ($entry in $entries) {
            $bytes = [IO.File]::ReadAllBytes($entry)
            if ($bytes.Length -gt 17408) { throw 'Blueprint exceeds the library entry limit.' }
            $writer.Write([int]$bytes.Length)
            $writer.Write($bytes)
        }
        $payload = $library.ToArray()
    } finally { $writer.Dispose() }

    $core = [IO.File]::ReadAllBytes($template)
    $marker = "schematica:embedded-library:v4" + [char]0 + ([string][char]0 * (4 + 5 + 8 * (4 + 17408)))
    $offset = [Text.Encoding]::ASCII.GetString($core).IndexOf($marker, [StringComparison]::Ordinal)
    if ($offset -lt 0) { throw 'The bundled guest template has no empty library slot.' }
    $lengthOffset = $offset + "schematica:embedded-library:v4".Length + 1
    [Array]::Copy([BitConverter]::GetBytes([int]$payload.Length), 0, $core, $lengthOffset, 4)
    [Array]::Copy($payload, 0, $core, $lengthOffset + 4, $payload.Length)
    $patched = Join-Path $temp 'guest.wasm'
    $packed = Join-Path $temp 'component.wasm'
    [IO.File]::WriteAllBytes($patched, $core)
    & $codec pack $patched $packed
    if ($LASTEXITCODE) { throw 'WebAssembly component validation failed.' }
    $converted = [IO.MemoryStream]::new()
    try {
        Set-ComponentGrants ([IO.File]::ReadAllBytes($packed)) $converted
        $file = [IO.File]::Open($Output, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write)
        try { $converted.WriteTo($file) } finally { $file.Dispose() }
    } finally { $converted.Dispose() }

    if ($interactive) {
        [void][System.Windows.Forms.MessageBox]::Show("Created $Output.`n`nImport this file in Cinnabar > Settings > Mods.", 'Schematica Import')
    } else { Write-Output "Wrote $Output" }
} catch {
    if ($interactive) { [void][System.Windows.Forms.MessageBox]::Show($_.Exception.Message, 'Schematica Import') }
    else { throw }
} finally {
    if (Test-Path -LiteralPath $temp) {
        Get-ChildItem -LiteralPath $temp -File | ForEach-Object { Remove-Item -LiteralPath $_.FullName }
        Remove-Item -LiteralPath $temp
    }
}
