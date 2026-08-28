#!/usr/bin/env pwsh
# Clone a Git repository into the CodePrism server's cache.
param(
    [string]$Server,
    [string]$GitUrl,
    [string]$Project,
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
    param([string]$Method, [string]$Endpoint, [string]$Body)
    $url = "$(Get-ServerUrl)$Endpoint"
    $params = @{
        Uri = $url
        Method = $Method
        ContentType = 'application/json'
        UseBasicParsing = $true
    }
    if ($env:CODEPRISM_API_TOKEN) { $params.Headers = @{ Authorization = "Bearer $($env:CODEPRISM_API_TOKEN)" } }
    if ($Body) { $params.Body = $Body }
    try {
        return Invoke-RestMethod @params
    }
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
if ($Help) { Write-Host "Usage: codeprism-repo-clone.ps1 -GitUrl <url> [-Project <name>] [-Server <url>] [-Json]"; exit }

if (-not $GitUrl) { Write-Error "-GitUrl is required."; exit 1 }

$body = @{ git_url = $GitUrl } | ConvertTo-Json
if ($Project) {
    $body = @{ git_url = $GitUrl; project_name = $Project } | ConvertTo-Json
}

$response = Invoke-ApiCall -Method POST -Endpoint '/api/v1/git/clone' -Body $body

if ($Json) { return $response | ConvertTo-Json -Depth 10 }

$repoId = $response.repo_id
$branchCount = $response.branches.Count
$currentBranch = $response.current_branch

Write-Host "Repository cloned successfully."
Write-Host "  Repo ID: $repoId"
Write-Host "  Branches: $branchCount total"
Write-Host "  Current branch: $currentBranch"
