# Renders assets\icon\quick-video-transcoder.svg into the files the build uses: the .ico compiled into the
# executable (build.rs) and the 256 px .png the window shows (main.rs).
#
#   .\scripts\render-icon.ps1
#
# Run it after editing the SVG and commit the three files together. Microsoft Edge renders the SVG once at
# 1024 px (so the result matches a browser's rendering exactly); each size is then downsampled from that
# render, never from another size, and the .ico stores every size as a PNG frame.

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$dir = Join-Path $root 'assets\icon'
$svg = Join-Path $dir 'quick-video-transcoder.svg'
$ico = Join-Path $dir 'quick-video-transcoder.ico'
$png = Join-Path $dir 'quick-video-transcoder-256.png'
if (-not (Test-Path $svg)) { throw "no SVG at $svg" }

$edge = @(
    (Join-Path ${env:ProgramFiles(x86)} 'Microsoft\Edge\Application\msedge.exe'),
    (Join-Path $env:ProgramFiles 'Microsoft\Edge\Application\msedge.exe')
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $edge) { throw 'Microsoft Edge is needed to render the SVG' }

# The render happens in a fresh temporary folder, deleted at the end: nothing else is written.
$work = Join-Path ([System.IO.Path]::GetTempPath()) ("qvt-icon-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
try {
    $page = Join-Path $work 'icon.html'
    $render = Join-Path $work 'render.png'
    $svgUrl = ([System.Uri]$svg).AbsoluteUri
    Set-Content -Path $page -Encoding utf8 -Value @"
<!doctype html><html><body style="margin:0;background:transparent">
<img src="$svgUrl" style="display:block;width:1024px;height:1024px">
</body></html>
"@
    $userData = Join-Path $work 'profile'
    & $edge --headless --disable-gpu --hide-scrollbars --no-first-run "--user-data-dir=$userData" `
        --default-background-color=00000000 --window-size=1024,1024 "--screenshot=$render" `
        ([System.Uri]$page).AbsoluteUri 2>$null | Out-Null
    for ($i = 0; $i -lt 50 -and -not (Test-Path $render); $i++) { Start-Sleep -Milliseconds 200 }
    if (-not (Test-Path $render)) { throw 'Edge produced no render' }

    Add-Type -AssemblyName System.Drawing
    $source = [System.Drawing.Bitmap]::FromFile($render)
    function Resize([int] $size) {
        $bitmap = New-Object System.Drawing.Bitmap $size, $size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
        $graphics.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
        $graphics.DrawImage($source, 0, 0, $size, $size)
        $graphics.Dispose()
        $stream = New-Object System.IO.MemoryStream
        $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
        $bitmap.Dispose()
        , $stream.ToArray()
    }

    # The sizes Windows asks for between 100 % and 300 % scaling.
    $sizes = 16, 20, 24, 32, 40, 48, 64, 256
    $frames = foreach ($size in $sizes) { , (Resize $size) }
    [System.IO.File]::WriteAllBytes($png, $frames[-1])

    # ICO: a 6-byte header, a 16-byte entry per frame, then the PNG frames.
    $out = New-Object System.IO.MemoryStream
    $writer = New-Object System.IO.BinaryWriter $out
    $writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]$sizes.Count)
    $offset = 6 + 16 * $sizes.Count
    for ($i = 0; $i -lt $sizes.Count; $i++) {
        $side = if ($sizes[$i] -ge 256) { 0 } else { $sizes[$i] }
        $writer.Write([byte]$side); $writer.Write([byte]$side)
        $writer.Write([byte]0); $writer.Write([byte]0)
        $writer.Write([uint16]1); $writer.Write([uint16]32)
        $writer.Write([uint32]$frames[$i].Length); $writer.Write([uint32]$offset)
        $offset += $frames[$i].Length
    }
    foreach ($frame in $frames) { $writer.Write($frame) }
    $writer.Flush()
    [System.IO.File]::WriteAllBytes($ico, $out.ToArray())
    $source.Dispose()
    Write-Host "wrote $ico ($($out.Length) bytes) and $png"
}
finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
