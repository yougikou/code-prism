import { useEffect, useRef, useState, type Dispatch, type SetStateAction } from 'react'
import { useTranslation } from 'react-i18next'
import { fetchScanJob } from '@/services/scan'

export interface ScanProgress {
  status: 'idle' | 'loading' | 'success' | 'error'
  message: string
  projectName?: string
  jobId?: number
  scanId?: number | null
  progress?: number
  progressMessage?: string
}

export function useScanJob(
  scanProgress: ScanProgress,
  setScanProgress: Dispatch<SetStateAction<ScanProgress>>,
) {
  const { t } = useTranslation()
  const history = useRef<Array<{ time: number; progress: number }>>([])
  const [eta, setEta] = useState<string | null>(null)

  useEffect(() => {
    const jobId = scanProgress.jobId
    if (!jobId || scanProgress.status === 'success' || scanProgress.status === 'error') return
    history.current = []

    const poll = async () => {
      try {
        const job = await fetchScanJob(jobId)
        if (job.status === 'completed' || job.status === 'completed_with_errors') {
          setScanProgress({
            status: 'success',
            message: job.status === 'completed_with_errors'
              ? (job.progress_message || 'Scan completed with analyzer errors')
              : t('execute.scanCompleted'),
            projectName: job.project_name,
            scanId: job.scan_id,
            progress: 100,
          })
          setEta(null)
          return
        }
        if (job.status === 'failed') {
          setScanProgress({ status: 'error', message: job.error_message || t('execute.scanFailed') })
          setEta(null)
          return
        }

        const now = Date.now()
        history.current.push({ time: now, progress: job.progress })
        history.current = history.current.filter(item => item.time >= now - 60_000)
        const first = history.current[0]
        const last = history.current.at(-1)
        if (history.current.length >= 3 && first && last && job.progress > 5 && job.progress < 95) {
          const elapsed = (last.time - first.time) / 1000
          const delta = last.progress - first.progress
          if (delta > 1 && elapsed > 2) {
            const remaining = (100 - last.progress) / (delta / elapsed)
            setEta(remaining > 0 && remaining < 3600
              ? (remaining > 90 ? `${Math.round(remaining / 60)}m ${Math.round(remaining % 60)}s` : `${Math.round(remaining)}s`)
              : null)
          }
        }
        setScanProgress(previous => ({
          ...previous,
          progress: job.progress,
          progressMessage: job.progress_message || undefined,
          message: job.progress_message || t('execute.scanningProgress', { progress: job.progress }),
        }))
      } catch {
        // A transient polling failure should not terminate the scan UI.
      }
    }

    void poll()
    const interval = window.setInterval(poll, 2_000)
    return () => window.clearInterval(interval)
  }, [scanProgress.jobId, scanProgress.status, setScanProgress, t])

  return eta
}
