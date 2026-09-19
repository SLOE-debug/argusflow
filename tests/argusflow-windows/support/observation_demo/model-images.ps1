param([Parameter(Mandatory=$true)][string]$Directory)
# 只编码录制窗口的裁剪图，不读取桌面或其他应用。
Add-Type -AssemblyName System.Drawing
$recordingRoot = (Resolve-Path -LiteralPath $Directory).Path
$frames = Get-Content -LiteralPath (Join-Path $recordingRoot 'model-input/frames.json') -Raw | ConvertFrom-Json
foreach ($frame in $frames) {
    $source = [System.Drawing.Bitmap]::FromFile((Join-Path $recordingRoot $frame.input))
    try {
        $rect = [System.Drawing.Rectangle]::new($frame.crop[0], $frame.crop[1], $frame.crop[2], $frame.crop[3])
        $cropped = $source.Clone($rect, [System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
        try { $cropped.Save((Join-Path $recordingRoot $frame.output), [System.Drawing.Imaging.ImageFormat]::Png) }
        finally { $cropped.Dispose() }
    } finally { $source.Dispose() }
}
Write-Output "Encoded $($frames.Count) cropped frames"
