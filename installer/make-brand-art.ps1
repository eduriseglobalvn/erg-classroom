# Builds the app icon, the title-bar wordmark and the installer bitmaps from the ERG logo PNG (2000x918, transparent).
# Run: make-brand-art.ps1 -Logo erg-logo.png -Out <folder>, then copy icon.ico/icon.png to apps\*\src-tauri\icons,
# erg.png to apps\tutor\public, and banner.bmp, dialog.bmp, icon.ico (as erg.ico) to this folder.
param([string]$Logo, [string]$Out)
Add-Type -AssemblyName System.Drawing
$src = New-Object Drawing.Bitmap $Logo
$full = New-Object Drawing.Rectangle 0, 0, 2000, 918     # ERG + EDURISE GLOBAL
$word = New-Object Drawing.Rectangle 0, 0, 2000, 700     # ERG
$mark = New-Object Drawing.Rectangle 1206, 0, 794, 700   # the red G with the arrows

function New-Canvas($w, $h, $bg) {
  $b = New-Object Drawing.Bitmap $w, $h, ([Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $g = [Drawing.Graphics]::FromImage($b)
  $g.InterpolationMode = 'HighQualityBicubic'; $g.SmoothingMode = 'AntiAlias'; $g.PixelOffsetMode = 'HighQuality'
  if ($bg) { $g.Clear($bg) } else { $g.Clear([Drawing.Color]::Transparent) }
  , @($b, $g)
}
# draw part $r of the logo into the box (x,y,w,h), keeping its proportions, centred
function Draw-Part($g, $r, $x, $y, $w, $h) {
  $k = [Math]::Min($w / $r.Width, $h / $r.Height)
  $dw = $r.Width * $k; $dh = $r.Height * $k
  $dst = New-Object Drawing.RectangleF ($x + ($w - $dw) / 2), ($y + ($h - $dh) / 2), $dw, $dh
  $g.DrawImage($src, $dst, (New-Object Drawing.RectangleF $r.X, $r.Y, $r.Width, $r.Height), 'Pixel')
}

# 1. app icon ("A · Screen + rise" on the design board): navy tile, white screen, the red ERG arrow rising out of it.
#    Drawn in a 256 x 256 space and scaled, same numbers as the SVG on the board.
function New-RoundRect($x, $y, $w, $h, $r) {
  $p = New-Object Drawing.Drawing2D.GraphicsPath; $d = $r * 2
  $p.AddArc($x, $y, $d, $d, 180, 90); $p.AddArc($x + $w - $d, $y, $d, $d, 270, 90); $p.AddArc($x + $w - $d, $y + $h - $d, $d, $d, 0, 90); $p.AddArc($x, $y + $h - $d, $d, $d, 90, 90)
  $p.CloseFigure(); $p
}
function New-Icon($n) {
  $c = New-Canvas $n $n $null; $b = $c[0]; $g = $c[1]
  $g.ScaleTransform($n / 256, $n / 256)
  $tile = New-RoundRect 4 4 248 248 56
  $grad = New-Object Drawing.Drawing2D.LinearGradientBrush (New-Object Drawing.PointF 0, 0), (New-Object Drawing.PointF 256, 256), ([Drawing.Color]::FromArgb(43, 43, 208)), ([Drawing.Color]::FromArgb(0, 0, 102))
  $g.FillPath($grad, $tile)
  $white = New-Object Drawing.Pen ([Drawing.Color]::White), 13
  $white.StartCap = 'Round'; $white.EndCap = 'Round'
  $g.DrawPath($white, (New-RoundRect 42 66 172 114 14))
  $g.DrawLine($white, 128, 182, 128, 208); $g.DrawLine($white, 98, 210, 158, 210)
  $a = New-Object Drawing.Drawing2D.GraphicsPath
  $a.AddBezier(52, 206, 78, 150, 128, 150, 168, 104)
  $a.AddLines([Drawing.PointF[]]@((New-Object Drawing.PointF 150, 92), (New-Object Drawing.PointF 214, 60), (New-Object Drawing.PointF 208, 132), (New-Object Drawing.PointF 190, 118)))
  $a.AddBezier(190, 118, 150, 170, 96, 170, 52, 206); $a.CloseFigure()
  $m = New-Object Drawing.Drawing2D.Matrix; $m.Translate(26, -22); $m.Scale(0.94, 0.94); $a.Transform($m)
  $edge = New-Object Drawing.Pen ([Drawing.Color]::FromArgb(10, 10, 140)), 8.5   # navy outline so the arrow reads where it crosses the screen frame
  $edge.LineJoin = 'Round'
  $g.DrawPath($edge, $a)
  $g.FillPath((New-Object Drawing.SolidBrush ([Drawing.Color]::FromArgb(255, 59, 54))), $a)
  $g.Dispose(); $b
}
$sizes = 16, 24, 32, 48, 64, 128, 256
$pngs = foreach ($n in $sizes) { $b = New-Icon $n; $m = New-Object IO.MemoryStream; $b.Save($m, [Drawing.Imaging.ImageFormat]::Png); $b.Dispose(); , $m.ToArray() }
# ICO container with PNG entries
$ico = New-Object IO.MemoryStream; $w = New-Object IO.BinaryWriter $ico
$w.Write([uint16]0); $w.Write([uint16]1); $w.Write([uint16]$sizes.Count)
$off = 6 + 16 * $sizes.Count
for ($i = 0; $i -lt $sizes.Count; $i++) {
  $n = $sizes[$i]; $w.Write([byte]($n % 256)); $w.Write([byte]($n % 256)); $w.Write([byte]0); $w.Write([byte]0)
  $w.Write([uint16]1); $w.Write([uint16]32); $w.Write([uint32]$pngs[$i].Length); $w.Write([uint32]$off); $off += $pngs[$i].Length
}
foreach ($p in $pngs) { $w.Write($p) }
[IO.File]::WriteAllBytes("$Out\icon.ico", $ico.ToArray())
(New-Icon 512).Save("$Out\icon.png", [Drawing.Imaging.ImageFormat]::Png)

# 2. wordmark for the Tutor title row (ERG only, transparent, 2x of a 16px line)
$c = New-Canvas 92 32 $null; Draw-Part $c[1] $word 0 0 92 32; $c[0].Save("$Out\erg.png", [Drawing.Imaging.ImageFormat]::Png)

# 3. installer art (24-bit BMP): banner 493x58 with the logo on the right; dialog 493x312 with the logo on a light left strip
function Save-Bmp($b, $path) {
  $o = New-Object Drawing.Bitmap $b.Width, $b.Height, ([Drawing.Imaging.PixelFormat]::Format24bppRgb)
  $g = [Drawing.Graphics]::FromImage($o); $g.DrawImage($b, 0, 0, $b.Width, $b.Height); $g.Dispose()
  $o.Save($path, [Drawing.Imaging.ImageFormat]::Bmp); $o.Dispose()
}
$c = New-Canvas 493 58 ([Drawing.Color]::White); Draw-Part $c[1] $full 393 7 90 44; Save-Bmp $c[0] "$Out\banner.bmp"
$c = New-Canvas 493 312 ([Drawing.Color]::White)
$c[1].FillRectangle((New-Object Drawing.SolidBrush ([Drawing.Color]::FromArgb(243, 243, 243))), 0, 0, 164, 312)
$c[1].FillRectangle((New-Object Drawing.SolidBrush ([Drawing.Color]::FromArgb(0, 0, 135))), 0, 306, 164, 6)
Draw-Part $c[1] $full 16 110 132 70; Save-Bmp $c[0] "$Out\dialog.bmp"
# preview sheet to look at
$c = New-Canvas 760 330 ([Drawing.Color]::FromArgb(230, 230, 230)); $x = 10
foreach ($n in 256, 128, 64, 48, 32, 24, 16) { $b = New-Icon $n; $c[1].DrawImageUnscaled($b, $x, 10); $x += $n + 10 }
$c[1].DrawImageUnscaled((New-Object Drawing.Bitmap "$Out\erg.png"), 290, 150)
$c[1].DrawImageUnscaled((New-Object Drawing.Bitmap "$Out\banner.bmp"), 10, 270)
$c[1].DrawImage((New-Object Drawing.Bitmap "$Out\dialog.bmp"), 520, 150, 230, 146)
$c[0].Save("$Out\preview.png", [Drawing.Imaging.ImageFormat]::Png)
'ok'
