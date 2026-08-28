#!/usr/bin/env pwsh
# Trigger a scan on the CodePrism server via API.
param(
    [string]$Server,
    [string]$RepoId,
    [string]$Project,
    [string]$GitUrl,
    [string]$Ref,
    [string]$Branch,
    [string]$BaseRef,
    [ValidateSet('snapshot','diff')][string]$Mode = 'snapshot',
    [switch]$Wait,
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

function Resolve-ProjectToRepoId {
    param([string]$ProjectName)
    $repos = Invoke-ApiCall -Method GET -Endpoint '/api/v1/git/repos'
    $matches = $repos.repos | Where-Object { $_.project_name -eq $ProjectName } | ForEach-Object { $_.repo_id }

    if (-not $matches) {
        Write-Error "No cached repo found for project '$ProjectName'. Use -RepoId or clone it first."
        exit 1
    }
    if ($matches.Count -gt 1) {
        $uuids = $matches -join ', '
        Write-Error "Multiple cached repos found for project '$ProjectName'. Use -RepoId to specify one of: $uuids"
        exit 1
    }
    return $matches[0]
}

function Get-CurrentBranch {
    param([string]$RepoId)
    $response = Invoke-ApiCall -Method GET -Endpoint "/api/v1/git/$RepoId/branches"
    return $response.current_branch
}

function Get-LatestCommit {
    param([string]$RepoId, [string]$Branch)
    $response = Invoke-ApiCall -Method GET -Endpoint "/api/v1/git/$RepoId/commits?ref=$Branch&limit=1"
    return $response.commits[0].hash
}

function Wait-ScanJob {
    param([int64]$JobId)
    $pollInterval = 2
    $maxAttempts = 900  # 30 min timeout
    $attempt = 0

    Write-Host "Waiting for scan job #$JobId to complete." -NoNewline

    while ($attempt -lt $maxAttempts) {
        $attempt++
        $job = Invoke-ApiCall -Method GET -Endpoint "/api/v1/scan-jobs/$JobId"
        $msg = if ($job.progress_message) { " - $($job.progress_message)" } else { '' }
        Write-Host "`rProgress: $($job.progress)%$msg" -NoNewline

        if ($job.status -eq 'completed') {
            Write-Host "`nScan completed!"
            return $job
        }
        if ($job.status -eq 'failed') {
            Write-Host "`nScan FAILED: $($job.error_message)"
            exit 1
        }
        Start-Sleep -Seconds $pollInterval
    }

    Write-Host "`nError: Scan job #$JobId timed out after 30 minutes."
    exit 1
}

# --- Main ---
if ($Help) {
    Write-Host "Usage: codeprism-scan.ps1 (-RepoId <uuid> | -Project <name> | -GitUrl <url>) [options]"
    Write-Host ""
    Write-Host "Required (choose one):"
    Write-Host "  -RepoId <uuid>      Cached repository ID"
    Write-Host "  -Project <name>     Project name (resolves to repo-id)"
    Write-Host "  -GitUrl <url>       Clone a fresh repository"
    Write-Host ""
    Write-Host "Options:"
    Write-Host "  -Ref <commit>       Target commit (default: latest on current branch)"
    Write-Host "  -Branch <name>      Branch name (default: current checked-out branch)"
    Write-Host "  -BaseRef <commit>   Base commit for diff mode (required for diff)"
    Write-Host "  -Mode <mode>        Scan mode: snapshot (default) or diff"
    Write-Host "  -Wait               Wait for scan to complete"
    Write-Host "  -Json               Output raw JSON"
    Write-Host "  -Server <url>       Server URL (default: http://localhost:3000)"
    exit
}

# --- Determine mode: cached repo vs fresh clone ---
$cachedRepo = $false
if ($RepoId) {
    $cachedRepo = $true
}
elseif ($Project) {
    $cachedRepo = $true
    $RepoId = Resolve-ProjectToRepoId -ProjectName $Project
}
elseif (-not $GitUrl) {
    Write-Error "One of -RepoId, -Project, or -GitUrl is required."
    exit 1
}

# --- Validation ---
if ($cachedRepo -and $Mode -eq 'diff' -and -not $BaseRef) {
    Write-Error "-BaseRef is required for diff mode."
    exit 1
}
if (-not $cachedRepo -and $Mode -eq 'diff' -and -not $BaseRef) {
    Write-Error "-BaseRef is required for diff mode."
    exit 1
}

# --- Resolve defaults for cached repo mode ---
if ($cachedRepo) {
    if (-not $Branch) {
        $Branch = Get-CurrentBranch -RepoId $RepoId
        if (-not $Branch) {
            Write-Error "Could not determine current branch for repo $RepoId."
            exit 1
        }
    }
    if (-not $Ref) {
        $Ref = Get-LatestCommit -RepoId $RepoId -Branch $Branch
        if (-not $Ref) {
            Write-Error "Could not determine latest commit for branch '$Branch'."
            exit 1
        }
    }
}

# --- Build request body ---
$body = @{ scan_mode = $Mode }
if ($Project) { $body.project_name = $Project }

if ($cachedRepo) {
    $body.git_url = ''
    $body.repo_id = $RepoId
    $body.ref_1 = $Ref
    if ($BaseRef) { $body.ref_2 = $BaseRef }
}
else {
    $body.git_url = $GitUrl
    if ($Ref) { $body.commit = $Ref }
    if ($BaseRef) { $body.base_commit = $BaseRef }
    if ($Branch) { $body.branch = $Branch }
}

$jsonBody = $body | ConvertTo-Json

# --- Execute scan ---
$response = Invoke-ApiCall -Method POST -Endpoint '/api/v1/scan' -Body $jsonBody

$jobId = $response.job_id
Write-Host "Scan started. Job ID: $jobId"

if ($Wait) {
    $final = Wait-ScanJob -JobId $jobId
    if ($Json) { $final | ConvertTo-Json -Depth 10 }
    else { $final }
}
elseif ($Json) {
    $response | ConvertTo-Json -Depth 10
}
