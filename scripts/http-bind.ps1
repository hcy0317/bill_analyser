function Resolve-BillAnalyserHttpEndpoint {
    [CmdletBinding()]
    param(
        [AllowEmptyString()]
        [string]$Bind
    )

    $rawBind = if ([string]::IsNullOrWhiteSpace($Bind)) {
        "127.0.0.1:5000"
    } else {
        $Bind.Trim()
    }

    $hostText = $null
    $portText = $null
    if ($rawBind -match '^\[(?<host>[^\]]+)\]:(?<port>[0-9]+)$') {
        $hostText = $Matches.host
        $portText = $Matches.port
    } elseif ($rawBind -match '^(?<host>[^:]+):(?<port>[0-9]+)$') {
        $hostText = $Matches.host
        $portText = $Matches.port
    } else {
        throw "Invalid BILL_ANALYSER_HTTP_BIND '$rawBind'. Expected an IP address or localhost followed by a non-zero port."
    }

    $port = 0
    if (-not [int]::TryParse($portText, [ref]$port) -or $port -le 0 -or $port -gt 65535) {
        throw "Invalid BILL_ANALYSER_HTTP_BIND port in '$rawBind'."
    }

    [System.Net.IPAddress]$parsedAddress = $null
    $isLocalhost = $hostText.Equals("localhost", [System.StringComparison]::OrdinalIgnoreCase)
    if (-not $isLocalhost -and -not [System.Net.IPAddress]::TryParse($hostText, [ref]$parsedAddress)) {
        throw "Invalid BILL_ANALYSER_HTTP_BIND host in '$rawBind'. Only IP literals and localhost are supported."
    }

    if ($isLocalhost) {
        $normalizedHost = "localhost"
        $probeHost = "localhost"
    } else {
        $normalizedAddress = $parsedAddress.ToString()
        $isIpv6 = $parsedAddress.AddressFamily -eq [System.Net.Sockets.AddressFamily]::InterNetworkV6
        $normalizedHost = if ($isIpv6) { "[$normalizedAddress]" } else { $normalizedAddress }
        if ($parsedAddress.Equals([System.Net.IPAddress]::Any)) {
            $probeHost = "127.0.0.1"
        } elseif ($parsedAddress.Equals([System.Net.IPAddress]::IPv6Any)) {
            $probeHost = "[::1]"
        } else {
            $probeHost = $normalizedHost
        }
    }

    $normalizedBind = "${normalizedHost}:$port"
    $baseUrl = "http://${probeHost}:$port"
    [pscustomobject]@{
        Bind = $normalizedBind
        Host = $normalizedHost
        ProbeHost = $probeHost
        Port = $port
        BaseUrl = $baseUrl
        HealthUrl = "$baseUrl/api/health"
    }
}
