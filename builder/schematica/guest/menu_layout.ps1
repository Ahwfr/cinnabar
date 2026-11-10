# Regenerates the JSON-UI panels checked into guest/ui. Run with PowerShell 7.
param([string[]]$Screens = @('main','load','control','materials','settings','verifier','hotkeys'))
$ErrorActionPreference = 'Stop'
$ink = @(0.90,0.94,0.96,1)
$muted = @(0.59,0.68,0.74,1)
$accent = @(0.38,0.88,0.82,1)
$surface = @(0.085,0.12,0.15,0.98)
$buttonFill = @(0.14,0.20,0.24,1)
$activeFill = @(0.10,0.30,0.31,1)
$border = @(0.24,0.33,0.38,1)
$origin = @{anchor_from='top_left';anchor_to='top_left'}

function New-Rect($x,$y,$w,$h,$color,$radius=0) {
    return $origin + @{type='custom';renderer='cinnabar_rounded_rectangle';size=@($w,$h);offset=@($x,$y);color=$color;radius=$radius}
}

function New-Label($text,$x,$y,$w,$color=$ink,$align='left') {
    $label = $origin + @{type='label';text=$text;size=@($w,12);offset=@($x,$y);color=$color;text_alignment=$align;shadow=$true}
    if($text.StartsWith('#')) { $label.bindings=@(@{binding_name=$text}) }
    return $label
}

function Add-Node($node) {
    $nodes.Add(@{('node'+$nodes.Count)=$node})
}

function Add-Button($id,$text,$x,$y,$w,$h=20,$fill=$buttonFill,$field=$false,$colorBinding='',$visibleBinding='') {
    $index=$controls.Count
    $type=if($id){'button'}else{'panel'}
    $line=New-Rect 1 1 ($w-2) ($h-2) $fill 3
    if($colorBinding){ $line.bindings=@(@{binding_name=$colorBinding;binding_name_override='#color'}) }
    $node=$origin + @{type=$type;size=@($w,$h);offset=@($x,$y);controls=@(
        @{border=(New-Rect 0 0 $w $h $border 4)},
        @{fill=$line},
        @{caption=(New-Label $text 0 ([int][Math]::Floor(($h-10)/2)) $w $ink 'center')}
    )}
    if($id) {
        $route='control'
        if($field){
            $controls.Add(@{kind='text';id=$id;label=$id;value='';max_chars=10})
            $route='edit'
        }else{
            $controls.Add(@{kind='button';id=$id;label=$id})
        }
        $node.button_mappings=@(@{from_button_id='button.menu_select';to_button_id="mod.${route}:$index";mapping_type='pressed'})
    }
    if($visibleBinding){ $node.bindings=@(@{binding_name=$visibleBinding;binding_name_override='#visible'}) }
    Add-Node $node
}

function Add-Label($text,$x,$y,$w,$color=$ink,$align='left') {
    Add-Node (New-Label $text $x $y $w $color $align)
}

function Add-Header($active) {
    Add-Node (New-Rect 0 0 396 284 @(0.042,0.062,0.081,0.98) 8)
    Add-Node (New-Rect 0 0 396 3 $accent)
    Add-Button 'main' 'Schematica' 16 7 104 21 $activeFill
    Add-Label '#blueprint' 130 14 250 $ink 'right'
    $i=0
    $tabs = if ($active -ne 'main') {
        @(@('load','library','Library'),@('control','back','Controls'),@('materials','materials','Materials'),@('settings','settings','Settings'))
    }
    foreach($tab in $tabs){
        $isActive=$active -eq $tab[0]
        $tabFill=if($isActive){$activeFill}else{$buttonFill}
        $id=if($isActive){''}else{$tab[1]}
        Add-Button $id $tab[2] (16+92*$i) 38 88 21 $tabFill
        $i++
    }
    Add-Node (New-Rect 16 67 364 1 $border)
    Add-Node (New-Rect 16 245 364 1 $border)
}

foreach($screen in $Screens){
    $nodes=[Collections.Generic.List[object]]::new()
    $controls=[Collections.Generic.List[object]]::new()
    Add-Header $screen
    switch($screen){
        'main' {
            Add-Label 'SCHEMATIC TOOLS' 16 80 364 $accent
            Add-Button 'library' 'Load schematic' 16 101 176 26
            Add-Button 'back' 'Placement controls' 204 101 176 26
            Add-Button 'materials' 'Material list' 16 136 176 26
            Add-Button 'verifier' 'Schematic verifier' 204 136 176 26
            Add-Button 'settings' 'Configuration' 16 171 176 26
            Add-Button 'hotkeys' 'Hotkeys' 204 171 176 26
            Add-Label 'Open Hotkeys to review or change bindings.' 16 220 364 $muted
            Add-Button 'close' 'Done' 296 252 84 22 $activeFill
        }
        'load' {
            Add-Label 'SELECT BLUEPRINT' 16 76 364 $muted
            for($i=0;$i -lt 8;$i++){
                Add-Button "select-$i" "#file$i" 16 (93+18*$i) 364 17 $buttonFill $false "#rowColor$i" "#visible$i"
            }
            Add-Label '#selected' 16 258 220 $muted
            Add-Button 'load' '#loadAction' 236 252 144 22 $activeFill
        }
        'control' {
            Add-Node (New-Rect 16 79 174 151 $surface 5)
            Add-Node (New-Rect 202 79 178 151 $surface 5)
            Add-Label 'POSITION' 26 90 152 $accent
            $i=0
            foreach($axis in @('x','y','z')){
                $x=26+52*$i
                Add-Label $axis.ToUpperInvariant() $x 113 48 $muted 'center'
                Add-Button $axis "#$axis" $x 129 48 22 $buttonFill $true
                $i++
            }
            Add-Button 'place' 'Move here' 26 161 154 22
            Add-Button 'visibility' '#visibility' 26 194 72 22
            Add-Button 'lock' '#lock' 104 194 76 22
            Add-Label 'TRANSFORM & LAYERS' 212 90 154 $accent
            Add-Button 'rotate-axis' '#rotate_axis' 212 111 76
            Add-Button 'rotate' 'Rotate' 294 111 76
            Add-Button 'flip-axis' '#flip_axis' 212 140 76
            Add-Button 'mirror' 'Flip' 294 140 76
            Add-Label 'Layer mode' 212 175 102 $muted
            Add-Label 'Layer' 320 175 50 $muted 'center'
            Add-Button 'mode' '#mode' 212 193 102 22
            Add-Button 'layer' '#layer' 320 193 50 22 $buttonFill $true
            Add-Button 'unload' 'Unload' 16 252 84 22
            Add-Button 'close' 'Done' 296 252 84 22 $activeFill
        }
        'materials' {
            Add-Node (New-Rect 16 79 364 21 $surface 3)
            Add-Label 'BLOCK' 24 84 151 $muted
            Add-Label 'TOTAL' 174 84 43 $muted 'center'
            Add-Label 'NEEDED' 220 84 48 $muted 'center'
            Add-Label 'PLACED' 270 84 48 $muted 'center'
            Add-Label 'UNKNOWN' 320 84 55 $muted 'center'
            for($i=0;$i -lt 7;$i++){
                $y=104+17*$i
                if($i%2 -eq 0){ Add-Node (New-Rect 16 ($y-2) 364 16 $surface 2) }
                Add-Label "#row$i" 24 $y 151
                Add-Label "#total$i" 174 $y 43 $ink 'center'
                Add-Label "#needed$i" 220 $y 48 $accent 'center'
                Add-Label "#placed$i" 270 $y 48 $ink 'center'
                Add-Label "#unknown$i" 320 $y 55 $muted 'center'
            }
            Add-Label '#scan' 16 229 364 $muted
            Add-Button 'sort' '#sort' 16 252 125 22
            Add-Button 'previous-page' '<' 156 252 28 22
            Add-Label '#page' 187 258 56 $muted 'center'
            Add-Button 'next-page' '>' 246 252 28 22
            Add-Button 'close' 'Done' 296 252 84 22 $activeFill
        }
        'settings' {
            Add-Node (New-Rect 16 80 364 35 $surface 5)
            Add-Label 'Block preview' 27 84 175
            Add-Label 'Native block textures.' 27 100 175 $muted
            Add-Button 'look' '#look' 208 86 76 24
            Add-Button 'opacity' '#opacity' 290 86 77 24
            Add-Node (New-Rect 16 119 364 35 $surface 5)
            Add-Label 'Target info' 27 123 200
            Add-Label 'Shows the planned block and position.' 27 139 210 $muted
            Add-Button 'assist' '#assist' 246 125 121 24
            Add-Node (New-Rect 16 158 364 35 $surface 5)
            Add-Label 'Fast Place' 27 162 200
            Add-Label 'Uses inventory or Creative catalog.' 27 178 210 $muted
            Add-Button 'fast-place' '#fast-place' 246 164 121 24
            Add-Node (New-Rect 16 197 364 35 $surface 5)
            Add-Label 'Auto orientation' 27 201 200
            Add-Label 'Server aim only. Camera stays still.' 27 217 210 $muted
            Add-Button 'correct-aim' '#correct-aim' 246 203 121 24
            Add-Button 'hotkeys' 'Hotkeys' 16 252 84 22
            Add-Button 'close' 'Done' 296 252 84 22 $activeFill
        }
        'verifier' {
            Add-Node (New-Rect 16 79 364 21 $surface 3)
            Add-Label 'COORDINATES' 24 84 188 $muted
            Add-Label 'RESULT' 224 84 150 $muted
            for($i=0;$i -lt 7;$i++){
                $y=104+17*$i
                if($i%2 -eq 0){ Add-Node (New-Rect 16 ($y-2) 364 16 $surface 2) }
                Add-Button "verify-row-$i" "#pos$i" 24 ($y-3) 182 16 $buttonFill $false '' "#verify-visible$i"
                Add-Label "#kind$i" 224 $y 150 $accent
            }
            Add-Label '#scan' 16 229 364 $muted
            Add-Button 'verify-clear' 'Clear marker' 16 252 118 22
            Add-Button 'verify-previous' '<' 156 252 28 22
            Add-Label '#page' 187 258 56 $muted 'center'
            Add-Button 'verify-next' '>' 246 252 28 22
            Add-Button 'close' 'Done' 296 252 84 22 $activeFill
        }
        'hotkeys' {
            for($i=0;$i -lt 12;$i++){
                $x=16+188*[Math]::Floor($i/6)
                $y=78+24*($i%6)
                Add-Label "#title$i" $x ($y+5) 108 $muted
                Add-Button "bind-$i" "#bind$i" ($x+112) $y 64 20 $buttonFill $true
            }
            Add-Label 'Example M+V. Enter applies. Blank unbinds.' 16 229 364 $muted
            Add-Button 'reset-hotkeys' 'Reset defaults' 16 252 118 22
            Add-Button 'close' 'Done' 296 252 84 22 $activeFill
        }
    }
    $panel=@{type='panel';size=@(396,284);anchor_from='center';anchor_to='center';controls=$nodes.ToArray()}
    $document=@{namespace='schematica';menu=@{type='screen';size=@('100%','100%');controls=@(@{card=$panel})}}
    $payload=@{reference_size=@(408,296);controls=$controls.ToArray();surface=@{
        screen='schematica.menu';document=(ConvertTo-Json $document -Depth 100 -Compress);bindings=@{}
    }}
    $payload | ConvertTo-Json -Depth 100 -Compress | Set-Content -LiteralPath (Join-Path $PSScriptRoot "ui/$screen.json")
}
