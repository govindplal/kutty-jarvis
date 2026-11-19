# Draws a synthetic UI with labelled buttons at known positions and writes
# the image plus ground-truth centres (pixels) as JSON.
param([string]$OutDir = ".", [int]$Width = 1280, [int]$Height = 720)
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap $Width, $Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = 'AntiAlias'; $g.TextRenderingHint = 'AntiAlias'
$g.Clear([System.Drawing.Color]::FromArgb(245, 245, 247))
$font = New-Object System.Drawing.Font 'Segoe UI', 16
$labels = 'Save','Open','Export','Settings','Share','Delete','Undo','Help'
$cols = 4; $bw = 220; $bh = 70
$targets = @()
for ($i = 0; $i -lt $labels.Count; $i++) {
  $col = $i % $cols; $row = [math]::Floor($i / $cols)
  # irregular offsets so positions are not a perfect grid
  $x = 90 + $col * 290 + (($i * 37) % 40)
  $y = 140 + $row * 260 + (($i * 53) % 60)
  $color = if ($labels[$i] -eq 'Delete') { 'IndianRed' } else { 'SteelBlue' }
  $g.FillRectangle((New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromName($color))), $x, $y, $bw, $bh)
  $sf = New-Object System.Drawing.StringFormat; $sf.Alignment = 'Center'; $sf.LineAlignment = 'Center'
  $rect = New-Object System.Drawing.RectangleF $x, $y, $bw, $bh
  $g.DrawString($labels[$i], $font, [System.Drawing.Brushes]::White, $rect, $sf)
  $targets += [pscustomobject]@{ label = $labels[$i]; x = $x + $bw/2; y = $y + $bh/2; w = $bw; h = $bh }
}
$g.DrawString('Untitled document - Demo App', $font, [System.Drawing.Brushes]::Black, 20, 20)
$bmp.Save((Join-Path $OutDir 'synthetic-ui.png'), [System.Drawing.Imaging.ImageFormat]::Png)
@{ width = $Width; height = $Height; targets = $targets } | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $OutDir 'synthetic-ui.json') -Encoding utf8
$g.Dispose(); $bmp.Dispose()
