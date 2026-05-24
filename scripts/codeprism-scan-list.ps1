#!/usr/bin/env pwsh
# List completed scans for a project from the CodePrism server.
param(
    [string]$Server,
    [string]$Project,
    [ValidateSet('snapshot','diff')][string]$Mode,
    [switch]$Json,
    [switch]$Help
)

# --- Shared helpers ---
function Get-ServerUrl {
    if ($Server) { return $Server }
    if ($env:CODEPRISM_SERVER) { return $env:CODEPRISM_SERVER }
    return 'http://localhost:3000'
}

function Invoke-ApiCall {
    param([string]$Method, [string]$Endpoint)
    $url = "$(Get-ServerUrl)$Endpoint"
    $params = @{
        Uri = $url
        Method = $Method
        ContentType = 'application/json'
        UseBasicParsing = $true
    }
    try { return Invoke-RestMethod @params }
    catch {
        $statusCode = if ($_.Exception.Response.StatusCode) { [int]$_.Exception.Response.StatusCode } else { $null }
        if ($statusCode -and $statusCode -ge 400) {
            try {
                $reader = [System.IO.StreamReader]::new($_.Exception.Response.GetResponseStream())
                $errBody = $reader.ReadToEnd() | ConvertFrom-Json
                $errMsg = if ($errBody.message) { $errBody.message } elseif ($errBody.error) { $errBody.error } else { 'Unknown error' }
                Write-Error "[$statusCode] $errMsg"
            }
            catch { Write-Error "HTTP $statusCode - failed to read error body" }
        }
        elseif ($_.Exception.Status -eq [System.Net.WebExceptionStatus]::ConnectFailure) {
            Write-Error "Cannot connect to CodePrism server at $(Get-ServerUrl). Is the server running?"
        }
        else { Write-Error "API call failed: $_" }
        exit 1
    }
}

# --- Main ---
if ($Help) { Write-Host "Usage: codeprism-scan-list.ps1 -Project <name> [-Mode snapshot|diff] [-Server <url>] [-Json]"; exit }
if (-not $Project) { Write-Error "-Project is required."; exit 1 }

$endpoint = "/api/v1/projects/$Project/scans"
if ($Mode) { $endpoint += "?mode=$($Mode.ToUpper())" }

$response = Invoke-ApiCall -Method GET -Endpoint $endpoint

if ($Json) { return $response | ConvertTo-Json -Depth 10 }

if (-not $response -or $response.Count -eq 0) {
    Write-Host "No scans found for project '$Project'."
    exit
}

Write-Host "Scans for project `"$Project`":"
Write-Host ("  {0,-6} | {1,-40} | {2}" -f 'ID', 'Commit', 'Time')
Write-Host ("  {0}" -f ('-' * 72))

foreach ($scan in $response) {
    $id = $scan.id
    $commit = if ($scan.commit_hash) { $scan.commit_hash.Substring(0, [Math]::Min(12, $scan.commit_hash.Length)) + '...' } else { '-                    ' }
    $time = if ($scan.scan_time) { $scan.scan_time } else { '-' }
    Write-Host ("  {0,-6} | {1,-40} | {2}" -f $id, $commit, $time)
}
