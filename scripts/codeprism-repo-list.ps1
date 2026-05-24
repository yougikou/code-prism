#!/usr/bin/env pwsh
# List all cached Git repositories from the CodePrism server.
param(
    [string]$Server,
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
if ($Help) { Write-Host "Usage: codeprism-repo-list.ps1 [-Server <url>] [-Json]"; exit }

$response = Invoke-ApiCall -Method GET -Endpoint '/api/v1/git/repos'

if ($Json) { return $response | ConvertTo-Json -Depth 10 }

if (-not $response.repos -or $response.repos.Count -eq 0) {
    Write-Host "No cached repositories."
    exit
}

Write-Host "Cached Repositories:"
Write-Host ("  {0,-36} | {1,-20} | {2,-22} | {3}" -f 'Repo ID', 'Project', 'Branch', 'Path')
Write-Host ("  {0}" -f ('-' * 108))

foreach ($repo in $response.repos) {
    $id = $repo.repo_id
    $project = if ($repo.project_name) { $repo.project_name } else { '-' }
    $branch = if ($repo.current_branch) { $repo.current_branch } else { '-' }
    $path = $repo.path
    Write-Host ("  {0,-36} | {1,-20} | {2,-22} | {3}" -f $id, $project, $branch, $path)
}
