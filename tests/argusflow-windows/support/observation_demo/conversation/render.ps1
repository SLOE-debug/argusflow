param([Parameter(Mandatory=$true)][string]$Manifest)
Add-Type -AssemblyName System.Drawing
$jobs = Get-Content -LiteralPath $Manifest -Raw | ConvertFrom-Json
foreach ($job in $jobs) {
    $source = [System.Drawing.Bitmap]::FromFile($job.input)
    try {
        $crop = [System.Drawing.Rectangle]::new($job.crop[0], $job.crop[1], $job.crop[2], $job.crop[3])
        $output = [System.Drawing.Bitmap]::new($job.width, $job.height)
        try {
            $graphics = [System.Drawing.Graphics]::FromImage($output)
            try {
                $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
                $graphics.DrawImage($source, [System.Drawing.Rectangle]::new(0, 0, $job.width, $job.height), $crop, [System.Drawing.GraphicsUnit]::Pixel)
            } finally { $graphics.Dispose() }
            $output.Save($job.output, [System.Drawing.Imaging.ImageFormat]::Png)
        } finally { $output.Dispose() }
    } finally { $source.Dispose() }
}
