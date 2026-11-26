param([string]$In, [string]$OutDir, [int]$Fit = 0)
# Coucou's montage of a print shop job in CorelDRAW (phase 2 of grafica.rs). Opens a COPY of the
# job's .cdr, puts as many pieces as fit on an A4 edge to edge (turned when more fit), finds where
# the number goes (the art's "Nº", or the shop's own print-merge fields <01>…), saves montagem.cdr
# and exports base.pdf (one sheet, no numbers). Prints one JSON line: the sheet, the slots where
# numbers go, and whether the art is only a picture. Never saves over the file it opened.
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$A4W = 210.0; $A4H = 297.0

function Out-Json($o) { $o | ConvertTo-Json -Compress -Depth 5 }

$app = $null; $doc = $null
try {
  $app = New-Object -ComObject CorelDRAW.Application
  try { $app.Visible = $false } catch {}
  $doc = $app.OpenDocument($In)
  $doc.Unit = 3                 # millimetres
  $doc.ReferencePoint = 9       # centre
  $page = $doc.Pages.Item(1)
  $page.Activate()
  $pw = [double]$page.SizeWidth; $ph = [double]$page.SizeHeight
  $all = $page.Shapes.All()
  if ($all.Count -eq 0) { Out-Json @{ ok = $false; error = 'empty page' }; exit 0 }

  # What the art is made of: only pictures means no vector to edit.
  $kinds = @{}
  foreach ($s in $page.Shapes.FindShapes()) { $kinds[[int]$s.Type] = 1 }
  $bitmapOnly = ($kinds.Keys | Where-Object { $_ -ne 5 -and $_ -ne 7 }).Count -eq 0   # 5 bitmap, 7 group

  # The shop's print-merge fields (<01>, <02>…) or the art's "Nº".
  $fields = @(); $anchors = @()
  foreach ($t in $page.Shapes.FindShapes($null, 6)) {
    $txt = ($t.Text.Story.Text -replace '\s+', ' ').Trim()
    $size = [double]$t.Text.Story.Size
    if ($txt -match '^<(\d{1,3})>$') {
      $fields += @{ index = [int]$Matches[1]; x = [double]$t.LeftX; y = [double]$t.BottomY; size = $size; shape = $t }
    } elseif ($txt -match '(?i)^(n\s*[º°o]\.?|nº|n°|número|numero)\s*:?$' -or $txt -match '(?i)\bn[º°]\s*$') {
      $anchors += @{ x = [double]$t.RightX + 2; y = [double]$t.BottomY; size = $size }
    }
  }

  if ($fields.Count -gt 0) {
    # Already the shop's montage: the fields are the slots; their text goes, the number comes later.
    $slots = @()
    foreach ($f in $fields) {
      $slots += @{ index = $f.index; x = [math]::Round($f.x, 2); y = [math]::Round($f.y, 2); size = $f.size; angle = 0 }
      $f.shape.Delete()
    }
    $per = ($fields | ForEach-Object { $_.index } | Sort-Object -Unique).Count
    $cells = @()
    $mode = 'campos'; $rot = $false; $sheetW = $pw; $sheetH = $ph
  } else {
    # Pieces of the file's page size, edge to edge on an A4, the way round that fits more.
    $cols = [math]::Floor(($A4W + 0.5) / $pw); $rows = [math]::Floor(($A4H + 0.5) / $ph)
    $colsR = [math]::Floor(($A4W + 0.5) / $ph); $rowsR = [math]::Floor(($A4H + 0.5) / $pw)
    $rot = ($colsR * $rowsR) -gt ($cols * $rows)
    if ($rot) { $cols = $colsR; $rows = $rowsR; $cw = $ph; $ch = $pw } else { $cw = $pw; $ch = $ph }
    $per = [int]($cols * $rows)
    if ($Fit -gt 0 -and $Fit -lt $per) { $per = $Fit }
    if ($per -lt 1) { Out-Json @{ ok = $false; error = "a peça de ${pw}x${ph} mm não cabe numa A4" }; exit 0 }
    $group = $all.Group()
    $gx = [double]$group.CenterX; $gy = [double]$group.CenterY
    # The art's centre against the page's, and each anchor against the art's centre.
    $offX = $gx - $pw / 2; $offY = $gy - $ph / 2
    $rel = @($anchors | ForEach-Object { @{ dx = $_.x - $gx; dy = $_.y - $gy; size = $_.size } })
    $page.SetSize($A4W, $A4H)
    if ($rot) { $group.Rotate(90) }
    $left = ($A4W - $cols * $cw) / 2; $bottom = ($A4H - $rows * $ch) / 2
    $slots = @(); $cells = @(); $n = 0
    for ($r = $rows - 1; $r -ge 0; $r--) {
      for ($c = 0; $c -lt $cols; $c++) {
        if ($n -ge $per) { break }
        $n++
        $shape = if ($n -eq 1) { $group } else { $group.Duplicate() }
        $cx = $left + $c * $cw + $cw / 2; $cy = $bottom + $r * $ch + $ch / 2
        if ($rot) { $ax = $cx - $offY; $ay = $cy + $offX } else { $ax = $cx + $offX; $ay = $cy + $offY }
        $shape.CenterX = $ax; $shape.CenterY = $ay
        $cells += @{ index = $n; x = [math]::Round($cx - $cw / 2, 2); y = [math]::Round($cy - $ch / 2, 2); w = [math]::Round($cw, 2); h = [math]::Round($ch, 2) }
        foreach ($a in $rel) {
          if ($rot) { $x = $ax - $a.dy; $y = $ay + $a.dx; $angle = 90 } else { $x = $ax + $a.dx; $y = $ay + $a.dy; $angle = 0 }
          $slots += @{ index = $n; x = [math]::Round($x, 2); y = [math]::Round($y, 2); size = $a.size; angle = $angle }
        }
      }
    }
    $mode = 'grade'; $sheetW = $A4W; $sheetH = $A4H
  }

  New-Item -ItemType Directory -Force $OutDir | Out-Null
  $cdr = Join-Path $OutDir 'montagem.cdr'
  $pdf = Join-Path $OutDir 'base.pdf'
  $opt = $app.CreateStructSaveAsOptions()
  $doc.SaveAs($cdr, $opt)
  $doc.PDFSettings.PublishRange = 0      # the whole document (one page)
  $doc.PublishToPDF($pdf)
  Out-Json @{
    ok = $true; mode = $mode; sheet_w = $sheetW; sheet_h = $sheetH; piece_w = $pw; piece_h = $ph
    per_sheet = [int]$per; rotated = [bool]$rot; bitmap_only = [bool]$bitmapOnly; slots = $slots; cells = $cells
  }
} catch {
  Out-Json @{ ok = $false; error = $_.Exception.Message }
} finally {
  if ($doc) { try { $doc.Close() } catch {} }
}
