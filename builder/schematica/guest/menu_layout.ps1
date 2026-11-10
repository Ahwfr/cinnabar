# Regenerates the four JSON-UI panels checked into guest/ui. Run with PowerShell 7.
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
    Add-Label 'SCHEMATICA' 16 14 118 $accent
    Add-Label '#blueprint' 130 14 250 $ink 'right'
    $i=0
    foreach($tab in @(@('load','library','Library'),@('control','back','Controls'),@('materials','materials','Materials'),@('settings','settings','Settings'))){
        $isActive=$active -eq $tab[0]
        $tabFill=if($isActive){$activeFill}else{$buttonFill}
        $id=if($isActive){''}else{$tab[1]}
        Add-Button $id $tab[2] (16+92*$i) 38 88 21 $tabFill
        $i++
    }
    Add-Node (New-Rect 16 67 364 1 $border)
    Add-Node (New-Rect 16 245 364 1 $border)
}

foreach($screen in @('load','control','materials','settings')){
    $nodes=[Collections.Generic.List[object]]::new()
    $controls=[Collections.Generic.List[object]]::new()
    Add-Header $screen
    switch($screen){
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
            Add-Button 'unload' 'Unload' 104 194 76 22
            Add-Label 'TRANSFORM & LAYERS' 212 90 154 $accent
            Add-Button 'rotate-axis' '#rotate_axis' 212 111 76
            Add-Button 'rotate' 'Rotate' 294 111 76
            Add-Button 'flip-axis' '#flip_axis' 212 140 76
            Add-Button 'mirror' 'Flip' 294 140 76
            Add-Label 'Layer mode' 212 175 102 $muted
            Add-Label 'Layer' 320 175 50 $muted 'center'
            Add-Button 'mode' '#mode' 212 193 102 22
            Add-Button 'layer' '#layer' 320 193 50 22 $buttonFill $true
            Add-Label 'Click a field to edit. Enter applies it.' 16 258 264 $muted
            Add-Button 'close' 'Done' 296 252 84 22 $activeFill
        }
        'materials' {
            Add-Node (New-Rect 16 79 364 21 $surface 3)
            Add-Label 'BLOCK' 24 84 211 $muted
            Add-Label 'TOTAL' 245 84 53 $muted 'center'
            Add-Label 'NEEDED' 306 84 62 $muted 'center'
            for($i=0;$i -lt 7;$i++){
                $y=104+17*$i
                if($i%2 -eq 0){ Add-Node (New-Rect 16 ($y-2) 364 16 $surface 2) }
                Add-Label "#row$i" 24 $y 211
                Add-Label "#total$i" 245 $y 53 $ink 'center'
                Add-Label "#needed$i" 306 $y 62 $accent 'center'
            }
            Add-Label '#scan' 16 229 364 $muted
            Add-Button 'sort' '#sort' 16 252 125 22
            Add-Button 'previous-page' '<' 156 252 28 22
            Add-Label '#page' 187 258 56 $muted 'center'
            Add-Button 'next-page' '>' 246 252 28 22
            Add-Button 'close' 'Done' 296 252 84 22 $activeFill
        }
        'settings' {
            Add-Node (New-Rect 16 80 364 46 $surface 5)
            Add-Label 'Block preview' 27 86 200
            Add-Label 'Uses the native block textures.' 27 104 210 $muted
            Add-Button 'look' '#look' 246 91 121 24
            Add-Node (New-Rect 16 132 364 46 $surface 5)
            Add-Label 'Target info' 27 138 200
            Add-Label 'Shows the planned block and position.' 27 156 210 $muted
            Add-Button 'assist' '#assist' 246 143 121 24
            Add-Node (New-Rect 16 184 364 49 $surface 5)
            Add-Label 'Fast Place' 27 190 200
            Add-Label 'Creative only. Right-click the guide.' 27 210 215 $muted
            Add-Button 'fast-place' '#fast-place' 246 196 121 24
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
