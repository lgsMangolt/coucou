# Prints one PDF on one printer through its Windows driver, with nothing installed: Windows' own
# PDF reader (Windows.Data.Pdf) draws each page, and .NET's PrintDocument sends it with the
# copies, colour, sides and paper size asked. Prints "ok <pages>" or "erro: <why>".
# Coucou runs it with: -Pdf <file> -Printer <name> -Copies <n> [-Mono] [-Duplex] -Paper A4|A3 [-ToFile <pdf>]
param(
    [Parameter(Mandatory = $true)][string]$Pdf,
    [Parameter(Mandatory = $true)][string]$Printer,
    [int]$Copies = 1,
    [switch]$Mono,
    [switch]$Duplex,
    [string]$Paper = "A4",
    [string]$ToFile = ""
)
$ErrorActionPreference = "Stop"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
try {
    Add-Type -AssemblyName System.Drawing
    Add-Type -AssemblyName System.Runtime.WindowsRuntime
    $null = [Windows.Storage.StorageFile, Windows.Storage, ContentType = WindowsRuntime]
    $null = [Windows.Data.Pdf.PdfDocument, Windows.Data.Pdf, ContentType = WindowsRuntime]
    $null = [Windows.Data.Pdf.PdfPageRenderOptions, Windows.Data.Pdf, ContentType = WindowsRuntime]
    $null = [Windows.Storage.Streams.InMemoryRandomAccessStream, Windows.Storage.Streams, ContentType = WindowsRuntime]

    $methods = [System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object { $_.Name -eq "AsTask" -and $_.GetParameters().Count -eq 1 }
    $asTaskOf = $methods | Where-Object { $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1' } | Select-Object -First 1
    $asTaskAction = $methods | Where-Object { $_.GetParameters()[0].ParameterType.Name -eq "IAsyncAction" } | Select-Object -First 1
    function Wait-Result($op, [Type]$type) { $task = $asTaskOf.MakeGenericMethod($type).Invoke($null, @($op)); $task.Wait(); $task.Result }
    function Wait-Done($op) { $asTaskAction.Invoke($null, @($op)).Wait() }

    $file = Wait-Result ([Windows.Storage.StorageFile]::GetFileFromPathAsync((Resolve-Path -LiteralPath $Pdf).Path)) ([Windows.Storage.StorageFile])
    $doc = Wait-Result ([Windows.Data.Pdf.PdfDocument]::LoadFromFileAsync($file)) ([Windows.Data.Pdf.PdfDocument])
    if ($doc.IsPasswordProtected) { throw "o PDF tem senha" }
    $count = [int]$doc.PageCount
    if ($count -lt 1) { throw "o PDF não tem páginas" }

    $pd = New-Object System.Drawing.Printing.PrintDocument
    $pd.DocumentName = [System.IO.Path]::GetFileName($Pdf)
    $pd.PrinterSettings.PrinterName = $Printer
    if (-not $pd.PrinterSettings.IsValid) { throw "a impressora '$Printer' não existe neste computador" }
    $pd.PrinterSettings.Copies = [int16][Math]::Max(1, $Copies)
    $pd.PrinterSettings.Collate = $true
    if ($Duplex) {
        if (-not $pd.PrinterSettings.CanDuplex) { throw "a impressora '$Printer' não imprime frente e verso" }
        $pd.PrinterSettings.Duplex = [System.Drawing.Printing.Duplex]::Vertical
    }
    $pd.DefaultPageSettings.Color = -not $Mono
    $size = $pd.PrinterSettings.PaperSizes | Where-Object { $_.Kind.ToString() -eq $Paper } | Select-Object -First 1
    if (-not $size) { throw "a impressora '$Printer' não tem papel $Paper" }
    $pd.DefaultPageSettings.PaperSize = $size
    $pd.DefaultPageSettings.Margins = New-Object System.Drawing.Printing.Margins(0, 0, 0, 0)
    if ($ToFile) { $pd.PrinterSettings.PrintToFile = $true; $pd.PrinterSettings.PrintFileName = $ToFile }
    # No dialog, no "printing page n" window.
    $pd.PrintController = New-Object System.Drawing.Printing.StandardPrintController

    $script:index = 0
    $pd.add_QueryPageSettings({
        param($sender, $e)
        $page = $doc.GetPage([uint32]$script:index)
        $e.PageSettings.Landscape = $page.Size.Width -gt $page.Size.Height
        $page.Dispose()
    })
    $pd.add_PrintPage({
        param($sender, $e)
        $page = $doc.GetPage([uint32]$script:index)
        # 300 dpi: the page size is in 1/96 inch.
        $options = New-Object Windows.Data.Pdf.PdfPageRenderOptions
        $options.DestinationWidth = [uint32][Math]::Round($page.Size.Width * 300 / 96)
        $options.DestinationHeight = [uint32][Math]::Round($page.Size.Height * 300 / 96)
        $stream = New-Object Windows.Storage.Streams.InMemoryRandomAccessStream
        Wait-Done ($page.RenderToStreamAsync($stream, $options))
        $image = [System.Drawing.Image]::FromStream([System.IO.WindowsRuntimeStreamExtensions]::AsStream($stream))
        # The whole page in the printable area, centred, its proportions kept.
        $area = $e.Graphics.VisibleClipBounds
        $scale = [Math]::Min($area.Width / $image.Width, $area.Height / $image.Height)
        $w = $image.Width * $scale; $h = $image.Height * $scale
        $e.Graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $e.Graphics.DrawImage($image, [single]($area.X + ($area.Width - $w) / 2), [single]($area.Y + ($area.Height - $h) / 2), [single]$w, [single]$h)
        $image.Dispose(); $stream.Dispose(); $page.Dispose()
        $script:index++
        $e.HasMorePages = $script:index -lt $count
    })
    $pd.Print()
    "ok $count"
}
catch {
    "erro: $($_.Exception.GetBaseException().Message)"
    exit 1
}
