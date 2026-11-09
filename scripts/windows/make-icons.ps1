# Genera los iconos de nüün para Windows (.ico multi-resolución + PNG del MSIX) desde vectores,
# con el mismo dibujo que scripts/media_icon.swift (placa ámbar, tarjeta SD con muesca, «n», contactos).
# Salida determinista: no hay arte binario que editar a mano.
# origen del escritor de .ico: nove scripts/windows/make-icons.ps1.
# Uso: powershell -NoProfile -File scripts\windows\make-icons.ps1 -Out native\windows\Nuun\Assets
param([Parameter(Mandatory = $true)][string]$Out)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
New-Item -ItemType Directory -Force -Path $Out | Out-Null

function New-Mark([int]$Size) {
    $bmp = New-Object System.Drawing.Bitmap $Size, $Size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'; $g.PixelOffsetMode = 'HighQuality'; $g.TextRenderingHint = 'AntiAliasGridFit'
    $g.Clear([System.Drawing.Color]::Transparent)
    $k = $Size / 1024.0
    function P([double]$x, [double]$y) { New-Object System.Drawing.PointF ([single]($x * $k)), ([single]($y * $k)) }
    # Plate (Mac: rounded 934 at 45, radius 212; gradient amber -> cream at 55 degrees).
    $r = 212 * $k; $o = 45 * $k; $w = 934 * $k
    $plate = New-Object System.Drawing.Drawing2D.GraphicsPath
    $plate.AddArc($o, $o, 2 * $r, 2 * $r, 180, 90); $plate.AddArc($o + $w - 2 * $r, $o, 2 * $r, 2 * $r, 270, 90)
    $plate.AddArc($o + $w - 2 * $r, $o + $w - 2 * $r, 2 * $r, 2 * $r, 0, 90); $plate.AddArc($o, $o + $w - 2 * $r, 2 * $r, 2 * $r, 90, 90)
    $plate.CloseFigure()
    $rect = New-Object System.Drawing.RectangleF ([single]$o), ([single]$o), ([single]$w), ([single]$w)
    $amber = [System.Drawing.Color]::FromArgb(255, 255, 187, 92); $cream = [System.Drawing.Color]::FromArgb(255, 255, 222, 173)
    $g.FillPath((New-Object System.Drawing.Drawing2D.LinearGradientBrush $rect, $cream, $amber, -55.0), $plate)
    # SD card with the notch at the top left (Mac y is flipped: y' = 1024 - y).
    $card = [System.Drawing.PointF[]]@((P 295 794), (P 295 344), (P 420 219), (P 730 219), (P 730 794))
    $brown = [System.Drawing.Color]::FromArgb(255, 153, 102, 51)
    $g.FillPolygon((New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(26, 153, 102, 51))), $card)
    $pen = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(166, 153, 102, 51)), ([single][Math]::Max(1, 18 * $k))
    $pen.LineJoin = 'Round'
    $g.DrawPolygon($pen, $card)
    if ($Size -ge 32) {
        $contact = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(179, 153, 102, 51))
        foreach ($x in 470, 540, 610) { $g.FillRectangle($contact, [single]($x * $k), [single](262 * $k), [single](35 * $k), [single](58 * $k)) }
    }
    $font = New-Object System.Drawing.Font 'Segoe UI Semibold', ([single](370 * $k)), ([System.Drawing.FontStyle]::Regular), ([System.Drawing.GraphicsUnit]::Pixel)
    $ink = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 61, 38, 20))
    $format = New-Object System.Drawing.StringFormat; $format.Alignment = 'Center'; $format.LineAlignment = 'Center'
    $g.DrawString('n', $font, $ink, (New-Object System.Drawing.RectangleF ([single](378 * $k)), ([single](330 * $k)), ([single](315 * $k)), ([single](410 * $k))), $format)
    $g.Dispose()
    return $bmp
}

function Save-Png($bmp, [string]$Path) { $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose() }

Save-Png (New-Mark 44) (Join-Path $Out 'Square44x44Logo.png')
Save-Png (New-Mark 88) (Join-Path $Out 'Square44x44Logo.scale-200.png')
Save-Png (New-Mark 150) (Join-Path $Out 'Square150x150Logo.png')
Save-Png (New-Mark 300) (Join-Path $Out 'Square150x150Logo.scale-200.png')
Save-Png (New-Mark 50) (Join-Path $Out 'StoreLogo.png')
Save-Png (New-Mark 256) (Join-Path $Out 'nuun-256.png')

$sizes = 16, 24, 32, 48, 64, 128, 256
$images = foreach ($size in $sizes) {
    $bmp = New-Mark $size; $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose(); , $ms.ToArray()
}
$ico = New-Object System.IO.MemoryStream; $w = New-Object System.IO.BinaryWriter $ico
$w.Write([uint16]0); $w.Write([uint16]1); $w.Write([uint16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($i = 0; $i -lt $sizes.Count; $i++) {
    $dim = if ($sizes[$i] -ge 256) { 0 } else { $sizes[$i] }
    $w.Write([byte]$dim); $w.Write([byte]$dim); $w.Write([byte]0); $w.Write([byte]0)
    $w.Write([uint16]1); $w.Write([uint16]32); $w.Write([uint32]$images[$i].Length); $w.Write([uint32]$offset)
    $offset += $images[$i].Length
}
foreach ($img in $images) { $w.Write($img) }
$w.Flush(); [System.IO.File]::WriteAllBytes((Join-Path $Out 'nuun.ico'), $ico.ToArray())
Write-Output "Icons written to $Out"
