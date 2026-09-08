# Select one IPv4 endpoint for both Vite and the Tauri WebView.
function Get-ArgusFlowDevPort {
    [OutputType([int])]
    param()

    for ($candidate = 5173; $candidate -le 5273; $candidate++) {
        $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, $candidate)
        $listener.Server.ExclusiveAddressUse = $true
        try {
            $listener.Start()
            return $candidate
        }
        catch [System.Net.Sockets.SocketException] {
            if ($_.Exception.SocketErrorCode -ne [System.Net.Sockets.SocketError]::AddressAlreadyInUse) {
                throw
            }
        }
        finally {
            $listener.Stop()
        }
    }
    throw 'No available development port between 5173 and 5273.'
}

# Use a temporary config file to avoid native Windows argument quoting of JSON.
function New-ArgusFlowDevConfig {
    [OutputType([string])]
    param(
        [Parameter(Mandatory)][string]$ProjectRoot,
        [Parameter(Mandatory)][int]$Port
    )

    $source = Get-Content -LiteralPath (Join-Path $ProjectRoot 'src-tauri/tauri.conf.json') -Raw | ConvertFrom-Json
    $policy = $source.app.security.csp.Replace('127.0.0.1:5173', "127.0.0.1:$Port")
    $config = @{
        build = @{ devUrl = "http://127.0.0.1:$Port" }
        app = @{ security = @{ csp = $policy } }
    }
    $path = Join-Path ([System.IO.Path]::GetTempPath()) ("argusflow-dev-{0}.json" -f [guid]::NewGuid())
    [System.IO.File]::WriteAllText($path, ($config | ConvertTo-Json -Depth 5), [System.Text.UTF8Encoding]::new($false))
    return $path
}
