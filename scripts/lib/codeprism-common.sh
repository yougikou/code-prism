#!/usr/bin/env bash
# CodePrism CLI shared library - API calls, polling, formatting
set -euo pipefail

# --- Dependencies ---
check_deps() {
    for cmd in curl jq; do
        if ! command -v "$cmd" &>/dev/null; then
            echo "Error: '$cmd' is required but not installed." >&2
            echo "  Install it with: brew install $cmd  (macOS)" >&2
            echo "  Or: sudo apt-get install $cmd       (Debian/Ubuntu)" >&2
            exit 1
        fi
    done
}

# --- Server URL resolution ---
# Priority: --server flag > CODEPRISM_SERVER env var > default
get_server_url() {
    if [[ -n "${flag_server:-}" ]]; then
        echo "$flag_server"
    elif [[ -n "${CODEPRISM_SERVER:-}" ]]; then
        echo "$CODEPRISM_SERVER"
    else
        echo "http://localhost:3000"
    fi
}

# --- Generic API call ---
# Usage: api_call <method> <endpoint> [json_body]
# Outputs raw JSON response to stdout, returns non-zero on HTTP errors
api_call() {
    local method="$1"
    local endpoint="$2"
    local data="${3:-}"
    local url endpoint_url

    endpoint_url="$(get_server_url)${endpoint}"

    local curl_args=(-s -S -X "$method")
    if [[ -n "$data" ]]; then
        curl_args+=(-H "Content-Type: application/json" -d "$data")
    fi
    curl_args+=("$endpoint_url")

    local http_code response
    response=$(curl "${curl_args[@]}" -w "\n%{http_code}" 2>/dev/null || true)
    http_code=$(echo "$response" | tail -1)
    response=$(echo "$response" | sed '$d')

    if [[ -z "$response" ]]; then
        echo "Error: Failed to connect to CodePrism server at $(get_server_url)" >&2
        echo "  Is the server running? (start it with 'codeprism serve' or 'cargo run -- serve')" >&2
        exit 1
    fi

    if [[ "$http_code" -ge 400 ]]; then
        local err_msg
        err_msg=$(echo "$response" | jq -r '.message // .error // "Unknown error"' 2>/dev/null || echo "$response")
        echo "Error [$http_code]: $err_msg" >&2
        return 1
    fi

    echo "$response"
}

# --- Scan job polling ---
# Usage: poll_job <job_id>
# Polls every 2s, shows progress, returns final JSON on completion
poll_job() {
    local job_id="$1"
    local update_interval=2
    local max_attempts=900  # 30 min timeout
    local attempt=0

    echo -n "Waiting for scan job #${job_id} to complete." >&2

    while [[ $attempt -lt $max_attempts ]]; do
        attempt=$((attempt + 1))

        local response status progress progress_msg error_msg scan_id
        response=$(api_call "GET" "/api/v1/scan-jobs/${job_id}") || return 1
        status=$(echo "$response" | jq -r '.status // "unknown"')
        progress=$(echo "$response" | jq -r '.progress // 0')
        progress_msg=$(echo "$response" | jq -r '.progress_message // ""')
        error_msg=$(echo "$response" | jq -r '.error_message // ""')
        scan_id=$(echo "$response" | jq -r '.scan_id // ""')

        if [[ -n "$progress_msg" ]]; then
            printf "\rProgress: %3d%% - %s" "$progress" "$progress_msg" >&2
        else
            printf "\rProgress: %3d%%" "$progress" >&2
        fi

        case "$status" in
            completed)
                echo >&2
                echo "$response"
                return 0
                ;;
            failed)
                echo >&2
                echo "Scan FAILED: ${error_msg}" >&2
                echo "$response"
                return 1
                ;;
            running|queued)
                sleep "$update_interval"
                ;;
            *)
                echo >&2
                echo "Warning: Unknown status '${status}'" >&2
                sleep "$update_interval"
                ;;
        esac
    done

    echo >&2
    echo "Error: Scan job #${job_id} timed out after 30 minutes." >&2
    return 1
}

# --- Output formatting ---
# Usage: format_output <json> <raw_json_flag>
format_output() {
    local json="$1"
    local raw_json="${2:-false}"
    if [[ "$raw_json" == "true" ]]; then
        echo "$json" | jq '.'
    else
        echo "$json"
    fi
}

# --- Repo helpers ---

# Resolve project name to repo_id via GET /api/v1/git/repos
# Usage: resolve_project_to_repo_id <project_name>
# Exits on 0 or multiple matches
resolve_project_to_repo_id() {
    local project_name="$1"
    local repos_response

    repos_response=$(api_call "GET" "/api/v1/git/repos") || exit 1

    # Filter repos by project_name match; output repo_id
    local matches
    matches=$(echo "$repos_response" | jq -r \
        --arg pn "$project_name" \
        '.repos // [] | map(select(.project_name == $pn)) |
         if length == 0 then "ZERO" else
           if length == 1 then .[0].repo_id else
             "MULTIPLE:" + ([.[].repo_id] | join(","))
           end
         end')

    if [[ "$matches" == "ZERO" ]]; then
        echo "Error: No cached repo found for project '${project_name}'." >&2
        echo "  Use --repo-id <uuid> to specify directly, or clone the repo first." >&2
        exit 1
    fi

    if [[ "$matches" == MULTIPLE:* ]]; then
        local uuids="${matches#MULTIPLE:}"
        echo "Error: Multiple cached repos found for project '${project_name}'." >&2
        echo "  Please use --repo-id to specify one of: ${uuids//,/, }" >&2
        exit 1
    fi

    echo "$matches"
}

# Get current branch for a cached repo
# Usage: get_current_branch <repo_id>
get_current_branch() {
    local repo_id="$1"
    local response
    response=$(api_call "GET" "/api/v1/git/${repo_id}/branches") || exit 1
    echo "$response" | jq -r '.current_branch // ""'
}

# Get latest commit hash for a branch
# Usage: get_latest_commit <repo_id> <branch>
get_latest_commit() {
    local repo_id="$1"
    local branch="$2"
    local response
    response=$(api_call "GET" "/api/v1/git/${repo_id}/commits?ref=${branch}&limit=1") || exit 1
    echo "$response" | jq -r '.commits[0].hash // ""'
}
