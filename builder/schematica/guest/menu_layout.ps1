# Regenerates the JSON-UI panels checked into guest/ui. Run with PowerShell 7.
param([string[]]$Screens = @('main','load','control','materials','settings','verifier','hotkeys'))
$ErrorActionPreference = 'Stop'
$ink = @(1,1,1,1)
$muted = @(0.70,0.70,0.72,1)
$accent = @(0.66,0.88,0.48,1)
$surface = @(0.19,0.19,0.19,1)
$buttonFill = @(0.29,0.29,0.30,1)
$activeFill = @(0.26,0.53,0.20,1)
$border = @(0.08,0.08,0.08,1)
$lightFill = @(0.82,0.83,0.85,1)
$dangerFill = @(0.73,0.17,0.19,1)
$origin = @{anchor_from='top_left';anchor_to='top_left'}

function New-Rect($x,$y,$w,$h,$color,$radius=0) {
    return $origin + @{type='custom';renderer='cinnabar_rounded_rectangle';size=@($w,$h);offset=@($x,$y);color=$color;radius=$radius}
}

function New-Label($text,$x,$y,$w,$color=$ink,$align='left') {
    $label = $origin + @{type='label';text=$text;size=@($w,12);offset=@($x,$y);color=$color;text_alignment=$align;shadow=$false;font_type='default'}
    if($text.StartsWith('#')) { $label.bindings=@(@{binding_name=$text}) }
    return $label
}

function Add-Node($node) {
    $nodes.Add(@{('node'+$nodes.Count)=$node})
}

function Button-Face($text,$w,$h,$fill,$pressed,$colorBinding,$subtitle) {
    $drop=if($pressed){2}else{0}
    $depth=if($pressed){1}else{3}
    $captionInk=if($fill[0] -gt 0.6 -and $fill[1] -gt 0.6){@(0.08,0.08,0.08,1)}else{$ink}
    $top=[double[]]@([Math]::Min(1.0,$fill[0]+0.13),[Math]::Min(1.0,$fill[1]+0.13),[Math]::Min(1.0,$fill[2]+0.13),1)
    $shade=[double[]]@(($fill[0]*0.5),($fill[1]*0.5),($fill[2]*0.5),1)
    if($pressed){$top=$shade}
    $face=New-Rect 1 (1+$drop) ($w-2) ($h-1-$depth-$drop) $fill
    if($colorBinding){$face.bindings=@(@{binding_name=$colorBinding;binding_name_override='#color'})}
    $align=if($subtitle){'left'}else{'center'}
    $textX=if($subtitle){7}else{0}
    $textY=if($subtitle){4+$drop}else{[int][Math]::Floor(($h-$depth-8)/2)+$drop}
    $caption=New-Label $text $textX $textY ($w-$textX*2) $captionInk $align
    $caption.size[1]=8
    $children=@(
        @{cover=(New-Rect 0 0 $w $h $border)},
        @{bottom=(New-Rect 1 ($h-$depth) ($w-2) $depth $shade)},
        @{face=$face}
    )
    if($pressed){$children+=@{shade=(New-Rect 1 (1+$drop) ($w-2) ($h-1-$depth-$drop) @(0,0,0,0.18))}}
    $children+=@(
        @{top=(New-Rect 1 (1+$drop) ($w-2) 1 $top)},
        @{left=(New-Rect 1 (1+$drop) 1 ($h-1-$depth-$drop) $top)},
        @{caption=$caption}
    )
    if($subtitle){
        $line=New-Label $subtitle 7 (14+$drop) ($w-14) $muted 'left'
        $line.size[1]=8
        $children+=@{subtitle=$line}
    }
    return $origin+@{type='panel';size=@($w,$h);offset=@(0,0);controls=$children}
}

function Add-Button($id,$text,$x,$y,$w,$h=20,$fill=$buttonFill,$field=$false,$colorBinding='',$visibleBinding='',$subtitle='',$enabledBinding='') {
    $index=$controls.Count
    $type=if($id){'button'}else{'panel'}
    $faceName='buttonFace'+$nodes.Count
    $definitions[$faceName]=Button-Face $text $w $h $fill $false $colorBinding $subtitle
    $node=$origin + @{type=$type;size=@($w,$h);offset=@($x,$y);controls=@(
        @{border=(New-Rect 0 0 $w $h $border)},
        @{("raised@schematica.$faceName")=@{}},
        @{("hover@schematica.$faceName")=@{}},
        @{pressed=(Button-Face $text $w $h $fill $true $colorBinding $subtitle)}
    )}
    if(!$id){$node.controls=@($node.controls[0],$node.controls[1])}
    if($id) {
        $node.default_control='raised'
        $node.hover_control='hover'
        $node.pressed_control='pressed'
        $route='control'
        if($field){
            $controls.Add(@{kind='text';id=$id;label=$id;value='';max_chars=10})
            $route='edit'
        }else{
            $controls.Add(@{kind='button';id=$id;label=$id})
        }
        $node.button_mappings=@(@{from_button_id='button.menu_select';to_button_id="mod.${route}:$index";mapping_type='pressed'})
    }
    if($id){
        $fade=@(@{binding_name="#row_${index}_hover";binding_name_override='#alpha'})
        $outline=New-Rect 0 0 $w $h @(0,0,0,0)
        $outline.border_color=@(1,1,1,1)
        $outline.border_width=1
        $outline.bindings=$fade
        $wash=New-Rect 1 1 ($w-2) ($h-4) @(1,1,1,0.06)
        $wash.bindings=$fade
        # Feedback belongs exclusively to hover; the native pressed state masks it.
        $node.controls[2]=@{hover=($origin+@{type='panel';size=@($w,$h);controls=@(
            @{("face@schematica.$faceName")=@{}},
            @{outline=$outline},
            @{wash=$wash}
        )})}
    }
    if($visibleBinding){ $node.bindings=@(@{binding_name=$visibleBinding;binding_name_override='#visible'}) }
    if($enabledBinding){ $node.bindings+=@(@{binding_name=$enabledBinding;binding_name_override='#enabled'}) }
    Add-Node $node
}

function Add-Label($text,$x,$y,$w,$color=$ink,$align='left') {
    Add-Node (New-Label $text $x $y $w $color $align)
}

function Add-Item($binding,$x,$y,$size) {
    Add-Node ($origin + @{type='custom';renderer='inventory_item_renderer';size=@($size,$size);offset=@($x,$y);bindings=@(@{binding_name=$binding;binding_name_override='#item_renderer_data'})})
}

function Add-Header($active) {
    Add-Node (New-Rect 0 0 396 284 $surface)
    Add-Node (New-Rect 0 0 396 27 @(0.90,0.90,0.91,1))
    Add-Button 'main' '<' 6 4 26 20 $lightFill
    Add-Label ($active.ToUpperInvariant()) 90 8 216 @(0.08,0.08,0.08,1) 'center'
    Add-Label '#build' 275 8 112 @(0.20,0.20,0.20,1) 'right'
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

function Add-Dashboard {
    Add-Node (New-Rect 0 0 640 360 @(0,0,0,1))
    Add-Node (New-Rect 0 0 640 24 @(0.90,0.90,0.91,1))
    Add-Button 'close' '<' 4 3 26 19 $lightFill
    Add-Label 'SCHEMATICS' 200 7 240 @(0.08,0.08,0.08,1) 'center'
    Add-Node (New-Rect 8 34 230 318 $border)
    Add-Node (New-Rect 9 35 228 316 $surface)
    Add-Node (New-Rect 246 34 386 318 $border)
    Add-Node (New-Rect 247 35 384 316 $surface)
    Add-Button 'import' 'Import schematic…' 16 43 214 21 $activeFill
    for($i=0;$i -lt 8;$i++){
        $y=70+31*$i
        Add-Button "select-$i" "#file$i" 16 $y 126 29 $buttonFill $false "#rowColor$i" "#visible$i" "#entryMeta$i"
        Add-Button "toggle-$i" "#entryStatus$i" 146 ($y+5) 40 19 $buttonFill $false "#statusColor$i" "#visible$i"
        Add-Button "delete-$i" 'Delete' 190 ($y+5) 40 19 $dangerFill $false '' "#visible$i"
    }
    Add-Label '#build' 16 319 214 $muted
    Add-Label '#libraryCount' 16 336 214 $muted
    Add-Label '#detailTitle' 254 43 370 $ink
    Add-Label '#detailSubtitle' 254 58 370 $muted
    foreach($row in @(
        @('Size','#detailSize','Volume','#detailVolume',78),
        @('Blocks','#detailBlocks','Palette states','#detailPalette',95),
        @('Guide','#detailState','Layers','#detailLayers',112)
    )){
        Add-Label $row[0] 254 $row[4] 55 $muted
        Add-Label $row[1] 311 $row[4] 116
        Add-Label $row[2] 439 $row[4] 77 $muted
        Add-Label $row[3] 518 $row[4] 106
    }
    Add-Label 'PLACEMENT' 254 133 370 $ink
    Add-Label '#detailPlacement' 254 148 370 $muted
    Add-Button 'place' 'Place here' 254 163 120 21 $activeFill $false '' '' '' '#hasBlueprint'
    Add-Button 'cw' 'Rotate 90°' 378 163 120 21 $lightFill $false '' '' '' '#hasBlueprint'
    Add-Button 'flip' 'Mirror' 502 163 122 21 $lightFill $false '' '' '' '#hasBlueprint'
    $i=0
    foreach($axis in @('x','y','z')){
        foreach($sign in @('minus','plus')){
            $text=$axis.ToUpperInvariant()+$(if($sign -eq 'minus'){'-'}else{'+'})
            Add-Button "$axis-$sign" $text (254+62*$i) 187 58 19 $buttonFill $false '' '' '' '#hasBlueprint'
            $i++
        }
    }
    Add-Label '#materialHeading' 254 215 370 $ink
    for($i=0;$i -lt 7;$i++){
        $y=234+14*$i
        Add-Item "#materialIcon$i" 254 ($y-1) 14
        Add-Label "#materialName$i" 273 $y 280
        Add-Label "#materialCount$i" 562 $y 62 $accent 'right'
        Add-Node (New-Rect 254 ($y+12) 370 0.5 @(0.27,0.27,0.27,1))
    }
    Add-Button 'settings' 'Settings' 254 332 120 19 $lightFill
    Add-Button 'materials' 'Materials' 378 332 120 19 $lightFill
    Add-Button 'verifier' 'Verifier' 502 332 122 19 $lightFill
}

function Stretch-Node($node,$parentSize) {
    $own=if($node.Contains('size')){@($node.size)}else{@($parentSize)}
    foreach($field in @('size','offset')){
        if($node.Contains($field)){
            for($axis=0;$axis -lt 2;$axis++){
                if($node[$field][$axis] -is [ValueType]){
                    $number=[double]$node[$field][$axis]*100/[double]$parentSize[$axis]
                    $node[$field][$axis]=$number.ToString('0.###',[Globalization.CultureInfo]::InvariantCulture)+'%'
                }
            }
        }
    }
    if($node.Contains('controls')){
        foreach($child in $node.controls){
            foreach($value in $child.Values){Stretch-Node $value $own}
        }
    }
}

foreach($screen in $Screens){
    $nodes=[Collections.Generic.List[object]]::new()
    $controls=[Collections.Generic.List[object]]::new()
    $definitions=@{}
    $dashboard=$screen -in @('main','load')
    if($dashboard){ Add-Dashboard }else{ Add-Header $screen }
    if(!$dashboard){switch($screen){
        'main' {
            Add-Label 'SCHEMATIC TOOLS' 16 80 364 $accent
            Add-Button 'library' 'Load schematic' 16 101 176 26
            Add-Button 'back' 'Placement controls' 204 101 176 26
            Add-Button 'materials' 'Material list' 16 136 176 26
            Add-Button 'verifier' 'Schematic verifier' 204 136 176 26
            Add-Button 'settings' 'Configuration' 16 171 176 26
            Add-Button 'hotkeys' 'Hotkeys' 204 171 176 26
            Add-Button 'import' 'Import schematics…' 16 207 176 26 $activeFill
            Add-Label 'Keybinds are in Keyboard & Mouse.' 204 216 176 $muted
            Add-Button 'close' 'Done' 296 252 84 22 $activeFill
        }
        'load' {
            Add-Label 'SELECT BLUEPRINT' 16 76 220 $muted
            Add-Button 'import' 'Import…' 296 72 84 19 $activeFill
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
                Add-Item "#materialIcon$i" 22 ($y-2) 16
                Add-Label "#row$i" 42 $y 130
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
            Add-Label 'Block preview' 24 85 175
            Add-Button 'look' '#look' 208 78 76 24
            Add-Button 'opacity' '#opacity' 290 78 77 24
            Add-Label 'Target info' 24 113 200
            Add-Button 'assist' '#assist' 246 106 121 24
            Add-Label 'Fast Place' 24 141 200
            Add-Button 'fast-place' '#fast-place' 246 134 121 24
            Add-Label 'Auto orientation (server aim)' 24 169 215
            Add-Button 'correct-aim' '#correct-aim' 246 162 121 24
            Add-Label 'Air placement (setblock cheats)' 24 197 215
            Add-Button 'air-place' '#air-place' 246 190 121 24
            Add-Label 'Menu layout' 24 225 200
            Add-Button 'ui-layout' '#ui-layout' 246 218 121 24
            Add-Button 'hotkeys' 'Keybinds' 16 252 84 22
            Add-Label 'Air commands use no items.' 108 258 180 $muted
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
    }
    $size=if($dashboard){@(640,360)}else{@(396,284)}
    $reference=if($dashboard){@(800,450)}else{@(408,296)}
    $panel=@{type='panel';size=$size.Clone();anchor_from='center';anchor_to='center';controls=$nodes.ToArray()}
    $document=@{namespace='schematica';menu=@{type='screen';size=@('100%','100%');controls=@(@{card=$panel})}}
    foreach($name in $definitions.Keys){$document[$name]=$definitions[$name]}
    $payload=@{reference_size=$reference;motion=$true;controls=$controls.ToArray();surface=@{
        screen='schematica.menu';document=(ConvertTo-Json $document -Depth 100 -Compress);bindings=@{}
    }}
    $payload | ConvertTo-Json -Depth 100 -Compress | Set-Content -LiteralPath (Join-Path $PSScriptRoot "ui/$screen.json")
    # Viewport fitting is authored once here, outside the WASM frame's fuel budget.
    Stretch-Node $panel $size
    foreach($definition in $definitions.Values){
        Stretch-Node $definition $definition.size.Clone()
    }
    $panel.size=@('100%','100%')
    $payload.reference_size=$size.Clone()
    $payload.surface.document=ConvertTo-Json $document -Depth 100 -Compress
    $payload | ConvertTo-Json -Depth 100 -Compress | Set-Content -LiteralPath (Join-Path $PSScriptRoot "ui/$screen-full.json")
}
