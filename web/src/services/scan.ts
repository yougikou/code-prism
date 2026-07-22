export interface ScanStartedResponse {
  job_id: number
  project_name: string
  status: string
  message: string
}

export interface ScanJobResponse {
  job_id: number
  project_name: string
  scan_mode: string
  status: 'queued' | 'running' | 'completed' | 'completed_with_errors' | 'failed'
  progress: number
  progress_message: string | null
  error_message: string | null
  scan_id: number | null
  created_at: string
  updated_at: string
}

export async function fetchScanJob(jobId: number): Promise<ScanJobResponse> {
  const response = await fetch(`/api/v1/scan-jobs/${jobId}`)
  if (!response.ok) {
    const error = await response.json().catch(() => ({ error: 'Failed to fetch scan job' }))
    throw new Error(error.error || 'Failed to fetch scan job')
  }
  return response.json()
}
