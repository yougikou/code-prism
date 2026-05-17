import { useState, useEffect, useRef, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import i18n from '../i18n'
import { useApp } from '@/contexts/AppContext'
import { Card, CardContent, CardHeader, CardTitle } from './ui/card'
import {
  cloneRepo,
  addLocalProject,
  listBranches,
  checkoutBranch,
  listCommits,
  listRepos,
  deleteRepo,
  executeScanWithRepo,
  fetchScanJob,
  fetchFullProjectConfig,
  updateProjectConfig,
  fetchTemplates,
  type BranchInfo,
  type CommitInfo,
  type RepoInfo,
  type FullProjectConfig,
} from '@/services/data'
import {
  GitBranchIcon,
  GitForkIcon,
  LoaderIcon,
  CheckIcon,
  XIcon,
  SearchIcon,
  RefreshCwIcon,
  ChevronDownIcon,
  ChevronUpIcon,
  Code2Icon,
  ArrowRightIcon,
  InfoIcon,
  FolderGit2Icon,
  Trash2Icon,
  AlertTriangleIcon,
} from 'lucide-react'

// ─── Types ───────────────────────────────────────────────────────────────

interface RefSelection {
  type: 'branch' | 'commit'
  value: string
  label: string
  timestamp?: number
  short_hash?: string
}

interface ScanProgress {
  status: 'idle' | 'loading' | 'success' | 'error'
  message: string
  projectName?: string
  jobId?: number
  scanId?: number | null
  progress?: number
  progressMessage?: string
}

// ─── Helpers ─────────────────────────────────────────────────────────────

function formatDate(ts: number): string {
  const d = new Date(ts * 1000)
  return d.toLocaleDateString(i18n.language || 'zh-CN', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })
}

function classNames(...classes: (string | false | null | undefined)[]): string {
  return classes.filter(Boolean).join(' ')
}

function shortUrl(gitUrl: string): string {
  return gitUrl.split('/').pop()?.replace('.git', '') || gitUrl
}

// ─── Diff Comparison Detail ─────────────────────────────────────────────

function DiffComparison({ ref1, ref2, onSwap }: {
  ref1: RefSelection
  ref2: RefSelection
  onSwap: () => void
}) {
  const { t } = useTranslation()
  const hasTimestamps = ref1.timestamp != null && ref2.timestamp != null
  const targetOlder = hasTimestamps && ref1.timestamp! < ref2.timestamp!

  return (
    <div className={classNames(
      'p-4 rounded-lg border space-y-3',
      targetOlder
        ? 'bg-amber-500/5 border-amber-500/30'
        : 'bg-slate-100/30 dark:bg-slate-700/30 border-slate-200 dark:border-slate-600/50',
    )}>
      {/* Direction header */}
      <div className="flex items-center justify-center gap-3 sm:gap-6 text-center">
        <div className="flex-1 min-w-0">
          <div className="text-xs text-slate-500 mb-1 uppercase tracking-wider">{t('execute.diffBase')}</div>
          <div className="text-sm font-mono text-amber-300 truncate" title={ref2.value}>
            {ref2.short_hash ?? ref2.value.slice(0, 7)}
          </div>
          {ref2.timestamp != null && (
            <div className="text-xs text-slate-500 mt-0.5">{formatDate(ref2.timestamp)}</div>
          )}
          {!hasTimestamps && (
            <div className="text-xs text-slate-500 mt-0.5">{ref2.label}</div>
          )}
        </div>

        <div className="flex flex-col items-center gap-1 shrink-0">
          <ArrowRightIcon className={classNames(
            'w-5 h-5',
            targetOlder ? 'text-amber-400' : 'text-sky-400',
          )} />
          <span className={classNames(
            'text-xs font-medium',
            targetOlder ? 'text-amber-300' : 'text-sky-300',
          )}>
            {targetOlder ? t('execute.diffReversed') : t('execute.diffChanges')}
          </span>
        </div>

        <div className="flex-1 min-w-0">
          <div className="text-xs text-slate-500 mb-1 uppercase tracking-wider">{t('execute.diffTarget')}</div>
          <div className="text-sm font-mono text-sky-300 truncate" title={ref1.value}>
            {ref1.short_hash ?? ref1.value.slice(0, 7)}
          </div>
          {ref1.timestamp != null && (
            <div className="text-xs text-slate-500 mt-0.5">{formatDate(ref1.timestamp)}</div>
          )}
          {!hasTimestamps && (
            <div className="text-xs text-slate-500 mt-0.5">{ref1.label}</div>
          )}
        </div>
      </div>

      {/* Warnings */}
      {targetOlder && (
        <div className="space-y-2">
          <div className="p-3 bg-amber-500/10 border border-amber-500/20 rounded-lg text-xs text-amber-200 space-y-1">
            <p className="font-medium flex items-center gap-1.5">
              <AlertTriangleIcon className="w-3.5 h-3.5" /> {t('execute.diffOrderWarningTitle')}
            </p>
            <p>
              {t('execute.diffOrderWarningDesc')}
            </p>
            <p>{t('execute.diffOrderSuggestion')}</p>
          </div>
          <button
            onClick={onSwap}
            className="w-full py-2 bg-amber-600 hover:bg-amber-700 text-white text-sm font-medium rounded-lg transition-colors flex items-center justify-center gap-2"
          >
            {t('execute.diffSwapRefs')}
          </button>
        </div>
      )}

      {hasTimestamps && !targetOlder && (
        <div className="p-2 bg-emerald-500/10 border border-emerald-500/20 rounded-lg text-xs text-emerald-200 flex items-center gap-1.5">
          <span>{t('execute.diffOrderCorrect', { from: ref2.short_hash, to: ref1.short_hash })}</span>
        </div>
      )}
    </div>
  )
}

// ─── Component ───────────────────────────────────────────────────────────

export default function ExecutePage() {
  const { t } = useTranslation()
  const { currentProject, setProject, navigateTo, triggerConfigRefresh, projectList } = useApp()
  // ── Repo list state ──
  const [reposList, setReposList] = useState<RepoInfo[]>([])
  const [loadingRepos, setLoadingRepos] = useState(true)
  const [deletingRepoId, setDeletingRepoId] = useState<string | null>(null)

  // ── Source toggle ──
  const [repoSource, setRepoSource] = useState<'clone' | 'local'>('clone')

  // ── Clone state ──
  const [gitUrl, setGitUrl] = useState('')
  const [isCloning, setIsCloning] = useState(false)
  const [localPath, setLocalPath] = useState('')
  const [isAddingLocal, setIsAddingLocal] = useState(false)
  const [repoId, setRepoId] = useState<string | null>(null)
  const [branches, setBranches] = useState<BranchInfo[]>([])
  const [currentBranch, setCurrentBranch] = useState<string>('')
  const [cloneError, setCloneError] = useState<string | null>(null)
  const [cloneSuccess, setCloneSuccess] = useState<string | null>(null)

  // ── Branch / commit state ──
  const [selectedBranch, setSelectedBranch] = useState<string>('')
  const [commits, setCommits] = useState<CommitInfo[]>([])
  const [commitsLoading, setCommitsLoading] = useState(false)
  const [commitsHasMore, setCommitsHasMore] = useState(false)
  const [commitOffset, setCommitOffset] = useState(0)
  const [commitSearch, setCommitSearch] = useState('')
  const [commitError, setCommitError] = useState<string | null>(null)

  // ── Ref selection state ──
  const [ref1, setRef1] = useState<RefSelection | null>(null)
  const [ref2, setRef2] = useState<RefSelection | null>(null)
  // ── Scan state ──
  const [isScanning, setIsScanning] = useState(false)
  const [scanProgress, setScanProgress] = useState<ScanProgress>({ status: 'idle', message: '' })
  const progressHistory = useRef<Array<{ time: number; progress: number }>>([])
  const [eta, setEta] = useState<string | null>(null)

  // ── Project name: use shared context ──
  const projectName = currentProject
  const setProjectName = (name: string) => { if (name) setProject(name) }

  // ── Template state ──
  const [templates, setTemplates] = useState<Record<string, FullProjectConfig>>({})
  const [selectedTemplate, setSelectedTemplate] = useState('')

  // ── UI state ──
  const [isBranchListOpen, setIsBranchListOpen] = useState(true)
  const [checkoutLoading, setCheckoutLoading] = useState<string | null>(null)
  const [checkoutConfirm, setCheckoutConfirm] = useState<{ branch: string; is_remote: boolean } | null>(null)

  const commitSearchTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)

  // ── Determine scan mode ──
  const scanMode = ref1 && ref2 ? 'diff' : ref1 ? 'snapshot' : null

  // ── Load existing repos on mount ──
  const loadRepos = useCallback(async () => {
    setLoadingRepos(true)
    try {
      const repos = await listRepos()
      setReposList(repos)
    } catch {
      // ignore — server may not be ready
    } finally {
      setLoadingRepos(false)
    }
  }, [])

  useEffect(() => {
    loadRepos()
  }, [loadRepos])

  // ── Load templates ──
  useEffect(() => {
    fetchTemplates().then(setTemplates).catch(() => {})
  }, [])

  // ── Load commits for the selected branch ──
  const loadCommits = useCallback(async (repoId: string, branch: string, offset: number, search: string, append: boolean) => {
    if (!repoId) return
    setCommitsLoading(true)
    setCommitError(null)
    try {
      const res = await listCommits(repoId, {
        ref: branch,
        offset,
        limit: 50,
        search: search || undefined,
      })
      if (append) {
        setCommits(prev => [...prev, ...res.commits])
      } else {
        setCommits(res.commits)
      }
      setCommitsHasMore(res.has_more)
      setCommitOffset(offset + res.commits.length)
    } catch (err) {
      setCommitError(err instanceof Error ? err.message : 'Failed to load commits')
    } finally {
      setCommitsLoading(false)
    }
  }, [])

  // When branch selection changes, reload commits
  useEffect(() => {
    if (repoId && selectedBranch) {
      setCommits([])
      setCommitOffset(0)
      loadCommits(repoId, selectedBranch, 0, commitSearch, false)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repoId, selectedBranch])

  // Debounced search
  useEffect(() => {
    if (!repoId || !selectedBranch) return
    if (commitSearchTimer.current) clearTimeout(commitSearchTimer.current)
    commitSearchTimer.current = setTimeout(() => {
      setCommits([])
      setCommitOffset(0)
      loadCommits(repoId, selectedBranch, 0, commitSearch, false)
    }, 300)
    return () => {
      if (commitSearchTimer.current) clearTimeout(commitSearchTimer.current)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [commitSearch])

  // ── Scan job polling ──
  useEffect(() => {
    const jobId = scanProgress.jobId
    if (!jobId || scanProgress.status === 'success' || scanProgress.status === 'error') return

    const pollInterval = setInterval(async () => {
      try {
        const job = await fetchScanJob(jobId)
        if (job.status === 'completed') {
          setScanProgress({
            status: 'success',
            message: t('execute.scanCompleted'),
            projectName: job.project_name,
            scanId: job.scan_id,
            progress: 100,
          })
          setEta(null)
        } else if (job.status === 'failed') {
          setScanProgress({
            status: 'error',
            message: job.error_message || t('execute.scanFailed'),
          })
          setEta(null)
        } else {
          // Track progress history for ETA calculation
          const now = Date.now()
          progressHistory.current.push({ time: now, progress: job.progress })
          const cutoff = now - 60000
          progressHistory.current = progressHistory.current.filter(p => p.time >= cutoff)

          // Calculate ETA if enough samples and progress > 5
          let etaStr: string | null = null
          if (progressHistory.current.length >= 3 && job.progress > 5 && job.progress < 95) {
            const first = progressHistory.current[0]
            const last = progressHistory.current[progressHistory.current.length - 1]
            const elapsedSec = (last.time - first.time) / 1000
            const progressDelta = last.progress - first.progress
            if (progressDelta > 1 && elapsedSec > 2) {
              const ratePerSec = progressDelta / elapsedSec
              const remaining = (100 - last.progress) / ratePerSec
              if (remaining > 0 && remaining < 3600) {
                etaStr = remaining > 90
                  ? `${Math.round(remaining / 60)}m ${Math.round(remaining % 60)}s`
                  : `${Math.round(remaining)}s`
              }
            }
          }
          setEta(etaStr)

          setScanProgress(prev => ({
            ...prev,
            progress: job.progress,
            progressMessage: job.progress_message || undefined,
            message: job.progress_message || t('execute.scanningProgress', { progress: job.progress }),
          }))
        }
      } catch {
        // network error during poll — keep trying
      }
    }, 2000)

    return () => clearInterval(pollInterval)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scanProgress.jobId, scanProgress.status])

  // ── Add local repo handler ──
  const handleAddLocal = async () => {
    if (!localPath.trim() || !projectName) return
    setIsAddingLocal(true)
    setCloneError(null)
    setCloneSuccess(null)
    try {
      const res = await addLocalProject(projectName, localPath.trim())
      setRepoId(res.repo_id)
      setBranches(res.branches)
      setCurrentBranch(res.current_branch)
      const initialBranch = res.current_branch || (res.branches[0]?.name ?? '')
      setSelectedBranch(initialBranch)
      setRef1({ type: 'branch', value: initialBranch, label: `${initialBranch} (branch)` })
      setRef2(null)
      setCloneSuccess(t('execute.localRepoAdded'))

      // Create project config if it doesn't exist (with template support)
      if (projectName) {
        try {
          const existing = await fetchFullProjectConfig(projectName).catch(() => null)
          if (!existing) {
            if (selectedTemplate && templates[selectedTemplate]) {
              const appliedConfig: FullProjectConfig = {
                ...templates[selectedTemplate],
                name: projectName,
              }
              await updateProjectConfig(projectName, appliedConfig)
            } else {
              const defaultConfig: FullProjectConfig = {
                name: projectName,
                tech_stacks: [],
                global_excludes: [],
                custom_regex_analyzers: {},
                custom_impl_analyzers: {},
                external_analyzers: {},
                aggregation_views: {},
              }
              await updateProjectConfig(projectName, defaultConfig)
            }
            triggerConfigRefresh()
          }
        } catch (err) {
          console.error('Failed to create project config after adding local repo:', err)
        }
      }

      loadRepos()
    } catch (err) {
      setCloneError(err instanceof Error ? err.message : 'Failed to add local repo')
    } finally {
      setIsAddingLocal(false)
    }
  }

  // ── Clone handler ──
  const handleClone = async () => {
    if (!gitUrl.trim()) return
    setIsCloning(true)
    setCloneError(null)
    setCloneSuccess(null)
    try {
      const res = await cloneRepo(gitUrl.trim(), projectName)
      setRepoId(res.repo_id)
      setBranches(res.branches)
      setCurrentBranch(res.current_branch)
      const initialBranch = res.current_branch || (res.branches[0]?.name ?? '')
      setSelectedBranch(initialBranch)
      setRef1({ type: 'branch', value: initialBranch, label: `${initialBranch} (branch)` })
      setRef2(null)
      setCloneSuccess(t('execute.repoCloned', { branch: res.current_branch || 'unknown' }))
      // Auto-create or apply template to project config if it doesn't exist yet
      if (projectName) {
        try {
          const existing = await fetchFullProjectConfig(projectName).catch(() => null)
          if (!existing) {
            if (selectedTemplate && templates[selectedTemplate]) {
              const appliedConfig: FullProjectConfig = {
                ...templates[selectedTemplate],
                name: projectName,
              }
              await updateProjectConfig(projectName, appliedConfig)
            } else {
              const defaultConfig: FullProjectConfig = {
                name: projectName,
                tech_stacks: [],
                global_excludes: [],
                custom_regex_analyzers: {},
                custom_impl_analyzers: {},
                external_analyzers: {},
                aggregation_views: {},
              }
              await updateProjectConfig(projectName, defaultConfig)
            }
            triggerConfigRefresh()
          }
        } catch (err) {
          console.error('Failed to create project config after clone:', err)
        }
      }
      // Refresh the repo list
      loadRepos()
    } catch (err) {
      setCloneError(err instanceof Error ? err.message : 'Clone failed')
    } finally {
      setIsCloning(false)
    }
  }

  // ── Select existing repo ──
  const handleSelectRepo = async (repo: RepoInfo) => {
    setRepoId(repo.repo_id)
    setCloneError(null)
    setCloneSuccess(null)
    // Use the stored project name (shared context), so scans associate with the correct project
    if (repo.project_name) {
      setProject(repo.project_name)
    }
    try {
      const res = await listBranches(repo.repo_id)
      setBranches(res.branches)
      setCurrentBranch(res.current_branch)
      const initialBranch = res.current_branch || (res.branches[0]?.name ?? '')
      setSelectedBranch(initialBranch)
      setRef1({ type: 'branch', value: initialBranch, label: `${initialBranch} (branch)` })
      setRef2(null)
    } catch (err) {
      setCloneError(err instanceof Error ? err.message : t('execute.failedToLoadBranches'))
    }
  }

  // ── Delete repo ──
  const handleDeleteRepo = async (id: string) => {
    if (!confirm(t('execute.deleteRepoConfirm') || 'Remove this cached repository? The project config and scan data will be kept.')) return
    setDeletingRepoId(id)
    const prevList = reposList
    setReposList(prev => prev.filter(r => r.repo_id !== id))
    try {
      await deleteRepo(id)
      triggerConfigRefresh()
    } catch (err) {
      setReposList(prevList)
      setCloneError(err instanceof Error ? err.message : t('execute.failedToDeleteRepo'))
    } finally {
      setDeletingRepoId(null)
    }
  }

  // ── Checkout handlers ──
  const requestCheckout = (branch: string, isRemote: boolean) => {
    setCheckoutConfirm({ branch, is_remote: isRemote })
  }

  const confirmCheckout = async (branch: string) => {
    if (!repoId) return
    setCheckoutConfirm(null)
    setCheckoutLoading(branch)
    try {
      await checkoutBranch(repoId, branch)
      setCurrentBranch(branch)
      setSelectedBranch(branch)
      setRef1({ type: 'branch', value: branch, label: `${branch} (branch)` })
      setRef2(null)
      setCommits([])
      setCommitOffset(0)
      // Reload branches to update is_head
      const res = await listBranches(repoId)
      setBranches(res.branches)
      setCurrentBranch(res.current_branch)
    } catch (err) {
      setCloneError(err instanceof Error ? err.message : 'Checkout failed')
    } finally {
      setCheckoutLoading(null)
    }
  }

  // ── Ref selection handlers ──
  const selectRefFromBranch = (branch: string) => {
    if (!ref1 || (ref1.value !== branch && ref2?.value !== branch)) {
      if (ref1?.value === branch) return
      if (ref2?.value === branch) {
        setRef2(null)
        return
      }

      // Try to attach HEAD commit info when commits are loaded for this branch
      const headCommit = branch === selectedBranch ? commits[0] : undefined
      const item: RefSelection = {
        type: 'branch',
        value: branch,
        label: headCommit ? `${branch} (${headCommit.short_hash})` : `${branch} (branch)`,
        ...(headCommit ? { timestamp: headCommit.timestamp, short_hash: headCommit.short_hash } : {}),
      }

      if (!ref1) {
        setRef1(item)
      } else if (!ref2) {
        setRef2(item)
      } else {
        setRef2(item)
      }
    }
  }

  const selectRefFromCommit = (commit: CommitInfo) => {
    const label = `${commit.short_hash} - ${commit.message.slice(0, 40)}`
    const item: RefSelection = {
      type: 'commit',
      value: commit.hash,
      label,
      timestamp: commit.timestamp,
      short_hash: commit.short_hash,
    }

    if (!ref1 || ref1.type === 'branch') {
      setRef1(item)
    } else if (!ref2) {
      setRef2(item)
    } else {
      setRef2(item)
    }
  }

  const clearRef = (which: 1 | 2) => {
    if (which === 1) {
      if (ref2) {
        setRef1(ref2)
        setRef2(null)
      } else {
        setRef1(null)
      }
    } else {
      setRef2(null)
    }
  }

  // ── Scan handler ──
  const handleScan = async () => {
    if (!repoId || !ref1) return
    setIsScanning(true)
    setScanProgress({ status: 'loading', message: t('execute.startingScan') })
    const scanProjectName = projectName
      || reposList.find(r => r.repo_id === repoId)?.project_name
      || shortUrl(
        reposList.find(r => r.repo_id === repoId)?.git_url || ''
      ) || 'scanned_project'
    try {
      const response = await executeScanWithRepo({
        repo_id: repoId,
        ref_1: ref1.value,
        ref_2: ref2?.value,
        project_name: scanProjectName,
        scan_mode: scanMode!,
      })
      setScanProgress({
        status: 'loading',
        message: t('execute.scanQueued', { project: scanProjectName, mode: scanMode === 'diff' ? t('execute.diffScan') : t('execute.snapshotScan') }),
        projectName: scanProjectName,
        jobId: response.job_id,
        progress: 0,
      })
    } catch (err) {
      setScanProgress({
        status: 'error',
        message: err instanceof Error ? err.message : t('execute.scanFailed'),
      })
    } finally {
      setIsScanning(false)
    }
  }

  // Navigate to Dashboard with the scanned project selected
  const goToDashboard = (project: string) => {
    setProject(project)
    navigateTo('dashboard')
  }

  // ── Reset ──
  const handleReset = () => {
    setRepoId(null)
    setBranches([])
    setCurrentBranch('')
    setSelectedBranch('')
    setCommits([])
    setRef1(null)
    setRef2(null)
    setCloneError(null)
    setCloneSuccess(null)
    setScanProgress({ status: 'idle', message: '' })
    setIsBranchListOpen(true)
    loadRepos()
  }

  // ── Scan mode badge ──
  const renderScanModeBadge = () => {
    if (!scanMode) return null
    return (
      <span className={classNames(
        'inline-flex items-center gap-1 px-3 py-1 rounded-full text-xs font-medium',
        scanMode === 'snapshot'
          ? 'bg-blue-500/20 text-blue-300 border border-blue-500/30'
          : 'bg-amber-500/20 text-amber-300 border border-amber-500/30',
      )}>
        {scanMode === 'snapshot' ? (
          <><Code2Icon className="w-3 h-3" /> {t('execute.snapshotScan')}</>
        ) : (
          <><GitForkIcon className="w-3 h-3" /> {t('execute.diffScan')}</>
        )}
      </span>
    )
  }

  // ================================================================
  // RENDER
  // ================================================================

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-5xl mx-auto px-4 sm:px-6 lg:px-8 py-8 space-y-6">

        {/* ──────────── Repo Selection ──────────── */}
        {!repoId && (
          <>
            {/* Existing repos */}
            {loadingRepos ? (
              <Card className="bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700">
                <CardContent className="flex items-center justify-center py-8">
                  <LoaderIcon className="w-5 h-5 animate-spin text-slate-400" />
                </CardContent>
              </Card>
            ) : reposList.length > 0 ? (
              <Card className="bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700">
                <CardHeader>
                  <CardTitle className="flex items-center gap-2 text-lg">
                    <FolderGit2Icon className="w-5 h-5 text-sky-400" />
                    {t('execute.existingRepos')}
                  </CardTitle>
                </CardHeader>
                <CardContent className="space-y-3">
                  {reposList.map((repo) => (
                    <div
                      key={repo.repo_id}
                      className="flex items-center justify-between p-4 bg-slate-100/80 dark:bg-slate-700/40 rounded-lg border border-slate-200 dark:border-slate-600/50 hover:border-sky-500/40 transition-colors"
                    >
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2 mb-1">
                          {repo.git_url ? (
                            <GitBranchIcon className="w-4 h-4 text-slate-400 shrink-0" />
                          ) : (
                            <FolderGit2Icon className="w-4 h-4 text-slate-400 shrink-0" />
                          )}
                          <span className="text-sm font-medium text-slate-800 dark:text-slate-200 truncate">
                            {repo.git_url ? shortUrl(repo.git_url) : repo.path.split('\\').pop() || repo.path}
                          </span>
                        </div>
                        <div className="flex items-center gap-3 text-xs text-slate-500 ml-6">
                          {repo.git_url ? (
                            <span className="truncate">{repo.git_url}</span>
                          ) : (
                            <span className="truncate">{repo.path}</span>
                          )}
                          <span className="shrink-0">·</span>
                          <span className="shrink-0">{t('execute.branchLabel')} <span className="text-slate-400">{repo.current_branch || 'main'}</span></span>
                        </div>
                      </div>
                      <div className="flex items-center gap-2 shrink-0 ml-4">
                        <button
                          onClick={() => handleSelectRepo(repo)}
                          className="px-3 py-1.5 text-xs bg-sky-600 text-white rounded-md hover:bg-sky-700 transition-colors"
                        >
                          {t('execute.select')}
                        </button>
                        <button
                          onClick={() => handleDeleteRepo(repo.repo_id)}
                          disabled={deletingRepoId === repo.repo_id}
                          className="p-1.5 text-slate-500 hover:text-red-400 hover:bg-red-900/20 rounded-md transition-colors disabled:opacity-50"
                          title={t('execute.deleteRepo')}
                        >
                          {deletingRepoId === repo.repo_id ? (
                            <LoaderIcon className="w-4 h-4 animate-spin" />
                          ) : (
                            <Trash2Icon className="w-4 h-4" />
                          )}
                        </button>
                      </div>
                    </div>
                  ))}
                </CardContent>
              </Card>
            ) : null}

            {/* Clone new repo */}
            <Card className="bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700">
              <CardHeader>
                <CardTitle className="flex items-center gap-2 text-lg">
                  <span className="flex items-center justify-center w-6 h-6 rounded-full bg-sky-600 text-white text-xs font-bold shrink-0">
                    {reposList.length > 0 ? '+' : '1'}
                  </span>
                  {reposList.length > 0 ? t('execute.step1TitleNew') : t('execute.step1Title')}
                </CardTitle>
              </CardHeader>
              <CardContent>
                {/* Source type toggle */}
                <div className="flex gap-2 mb-4">
                  <button
                    onClick={() => setRepoSource('clone')}
                    className={`px-4 py-2 text-sm rounded-lg transition-colors ${
                      repoSource === 'clone'
                        ? 'bg-sky-600 text-white'
                        : 'bg-slate-100 dark:bg-slate-700 text-slate-600 dark:text-slate-400'
                    }`}
                  >
                    <GitBranchIcon className="w-4 h-4 inline mr-1.5" />
                    {t('execute.cloneTab')}
                  </button>
                  <button
                    onClick={() => setRepoSource('local')}
                    className={`px-4 py-2 text-sm rounded-lg transition-colors ${
                      repoSource === 'local'
                        ? 'bg-sky-600 text-white'
                        : 'bg-slate-100 dark:bg-slate-700 text-slate-600 dark:text-slate-400'
                    }`}
                  >
                    <FolderGit2Icon className="w-4 h-4 inline mr-1.5" />
                    {t('execute.localRepoTab')}
                  </button>
                </div>

                {/* Clone remote input */}
                {repoSource === 'clone' && (
                  <div className="flex gap-3">
                    <input
                      type="text"
                      placeholder={t('execute.clonePlaceholder')}
                      value={gitUrl}
                      onChange={(e) => {
                        setGitUrl(e.target.value)
                        const name = shortUrl(e.target.value)
                        if (name) setProject(name)
                      }}
                      onKeyDown={(e) => e.key === 'Enter' && handleClone()}
                      className="flex-1 px-4 py-2 bg-slate-100 dark:bg-slate-700 border border-slate-300 dark:border-slate-600 rounded-lg text-slate-900 dark:text-slate-100 placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-sky-500 disabled:opacity-50"
                      disabled={isCloning}
                    />
                    <button
                      onClick={handleClone}
                      disabled={isCloning || !gitUrl.trim() || !projectName}
                      className="px-5 py-2 bg-sky-600 text-white rounded-lg font-medium hover:bg-sky-700 disabled:bg-slate-600 disabled:cursor-not-allowed transition-colors flex items-center gap-2 shrink-0"
                    >
                      {isCloning ? (
                        <><LoaderIcon className="w-4 h-4 animate-spin" /> {t('execute.cloning')}</>
                      ) : (
                        <><GitBranchIcon className="w-4 h-4" /> {t('execute.clone')}</>
                      )}
                    </button>
                  </div>
                )}

                {/* Local repo input */}
                {repoSource === 'local' && (
                  <div className="flex gap-3">
                    <input
                      type="text"
                      placeholder={t('execute.localPathPlaceholder')}
                      value={localPath}
                      onChange={e => setLocalPath(e.target.value)}
                      onKeyDown={e => e.key === 'Enter' && handleAddLocal()}
                      className="flex-1 px-4 py-2 bg-slate-100 dark:bg-slate-700 border border-slate-300 dark:border-slate-600 rounded-lg text-slate-900 dark:text-slate-100 placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-sky-500 disabled:opacity-50"
                      disabled={isAddingLocal}
                    />
                    <button
                      onClick={handleAddLocal}
                      disabled={isAddingLocal || !localPath.trim() || !projectName}
                      className="px-5 py-2 bg-sky-600 text-white rounded-lg font-medium hover:bg-sky-700 disabled:bg-slate-600 disabled:cursor-not-allowed transition-colors flex items-center gap-2 shrink-0"
                    >
                      {isAddingLocal ? (
                        <><LoaderIcon className="w-4 h-4 animate-spin" /> {t('execute.adding')}</>
                      ) : (
                        <><FolderGit2Icon className="w-4 h-4" /> {t('execute.addLocalRepo')}</>
                      )}
                    </button>
                  </div>
                )}

                {/* Project alias — uses shared project context */}
                <div className="mt-3">
                  <label className="block text-sm font-medium text-slate-400 mb-1.5">
                    {t('execute.projectAlias')}
                  </label>
                  <input
                    type="text"
                    value={projectName}
                    onChange={(e) => {
                      const val = e.target.value.trim()
                      if (val) setProject(val)
                    }}
                    placeholder={t('execute.projectPlaceholder')}
                    className="w-full px-4 py-2 bg-slate-100 dark:bg-slate-700 border border-slate-300 dark:border-slate-600 rounded-lg text-slate-900 dark:text-slate-100 placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-sky-500 text-sm"
                  />
                  {projectName && projectList.some(p => p.name === projectName) && (
                    <div className="mt-1.5 flex items-center gap-1.5 text-amber-400 text-xs">
                      <InfoIcon className="w-3.5 h-3.5" />
                      {t('execute.projectAliasExists') || 'Project already exists — new scan data will be added to the existing project'}
                    </div>
                  )}
                </div>

                {/* Template selection */}
                {Object.keys(templates).length > 0 && (
                  <div className="mt-3">
                    <label className="block text-sm font-medium text-slate-400 mb-1.5">
                      {t('templates.selectTemplate')}
                    </label>
                    <select
                      value={selectedTemplate}
                      onChange={(e) => setSelectedTemplate(e.target.value)}
                      className="w-full px-3 py-2 text-sm border rounded-lg bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 text-slate-700 dark:text-slate-300 outline-none focus:ring-2 focus:ring-sky-500"
                    >
                      <option value="">{t('templates.noTemplate')}</option>
                      {Object.entries(templates).map(([name]) => (
                        <option key={name} value={name}>{name}</option>
                      ))}
                    </select>
                  </div>
                )}

                {cloneError && (
                  <div className="mt-3 p-3 bg-red-900/30 border border-red-500/30 rounded-lg flex items-center gap-2 text-red-200 text-sm">
                    <XIcon className="w-4 h-4 shrink-0" />
                    {cloneError}
                  </div>
                )}
              </CardContent>
            </Card>
          </>
        )}

        {/* ──────────── After repo selected / cloned ──────────── */}
        {repoId && (
          <>
            {/* Repo context bar */}
            <div className="flex items-center justify-between p-3 bg-white/80 dark:bg-slate-800/80 border border-slate-200 dark:border-slate-700 rounded-lg">
              <div className="flex items-center gap-3 text-sm text-slate-700 dark:text-slate-300 min-w-0">
                <FolderGit2Icon className="w-4 h-4 text-sky-400 shrink-0" />
                <span className="truncate font-medium">
                  {(() => {
                    const repo = reposList.find(r => r.repo_id === repoId)
                    if (!repo) return ''
                    return repo.git_url ? shortUrl(repo.git_url) : repo.path.split('\\').pop() || repo.path
                  })()}
                </span>
                <span className="text-slate-500">·</span>
                <GitBranchIcon className="w-4 h-4 text-slate-500 shrink-0" />
                <span className="text-sky-300">{currentBranch}</span>
              </div>
              <button
                onClick={handleReset}
                className="px-3 py-1.5 text-xs bg-slate-100 dark:bg-slate-700 text-slate-700 dark:text-slate-300 rounded-md hover:bg-slate-200 dark:hover:bg-slate-600 transition-colors flex items-center gap-1.5 shrink-0 ml-4"
              >
                <RefreshCwIcon className="w-3.5 h-3.5" /> {t('execute.switchRepo')}
              </button>
            </div>

            {/* ──────────── Step: Branches ──────────── */}
            <Card className="bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700">
              <CardHeader>
                <CardTitle className="flex items-center gap-2 text-lg">
                  <span className="flex items-center justify-center w-6 h-6 rounded-full bg-sky-600 text-white text-xs font-bold shrink-0">2</span>
                  {t('execute.step2Title')}
                </CardTitle>
              </CardHeader>
              <CardContent>
                {cloneSuccess && (
                  <div className="mb-3 p-3 bg-green-900/30 border border-green-500/30 rounded-lg flex items-center gap-2 text-green-200 text-sm">
                    <CheckIcon className="w-4 h-4 shrink-0" />
                    {cloneSuccess}
                  </div>
                )}
                <div className="flex items-center justify-between mb-3">
                  <span className="text-sm text-slate-400">
                    {t('execute.branchesFound', { count: branches.length })}
                  </span>
                  <button
                    onClick={() => setIsBranchListOpen(!isBranchListOpen)}
                    className="text-slate-400 hover:text-slate-700 dark:hover:text-slate-200 transition-colors"
                  >
                    {isBranchListOpen ? <ChevronUpIcon className="w-4 h-4" /> : <ChevronDownIcon className="w-4 h-4" />}
                  </button>
                </div>

                {isBranchListOpen && (
                  <div className="space-y-1 max-h-80 overflow-y-auto pr-1">
                    {/* Local branches */}
                    {branches.filter(b => !b.is_remote).length > 0 && (
                      <div className="text-xs text-slate-500 font-medium px-3 py-1.5 uppercase tracking-wider">
                      {t('execute.localBranches')}
                      </div>
                    )}
                    {branches.filter(b => !b.is_remote).map((branch) => {
                      const isCurrent = branch.name === currentBranch
                      const isRef1 = ref1?.value === branch.name && ref1?.type === 'branch'
                      const isRef2 = ref2?.value === branch.name && ref2?.type === 'branch'
                      const isLoading = checkoutLoading === branch.name

                      return (
                        <div
                          key={branch.name}
                          className={classNames(
                            'flex items-center justify-between px-3 py-2 rounded-lg transition-colors cursor-pointer',
                            isCurrent
                              ? 'bg-sky-600/10 border border-sky-500/30'
                              : 'hover:bg-slate-100/50 dark:hover:bg-slate-700/50 border border-transparent',
                          )}
                          onClick={() => {
                            if (!isCurrent) requestCheckout(branch.name, false)
                          }}
                        >
                          <div className="flex items-center gap-3 min-w-0">
                            {isCurrent && (
                              <span className="w-2 h-2 rounded-full bg-sky-400 shrink-0" />
                            )}
                            {!isCurrent && (
                              <span className="w-2 h-2 rounded-full bg-slate-300 dark:bg-slate-600 shrink-0" />
                            )}
                            <span className={classNames(
                              'text-sm font-medium truncate',
                              isCurrent ? 'text-sky-200' : 'text-slate-700 dark:text-slate-300',
                            )}>
                              {branch.name}
                            </span>
                            {isCurrent && (
                              <span className="text-xs text-sky-400 bg-sky-500/10 px-2 py-0.5 rounded-full shrink-0">{t('execute.current')}</span>
                            )}
                            {isRef1 && (
                              <span className="text-xs text-blue-300 bg-blue-500/10 px-2 py-0.5 rounded-full shrink-0">{t('execute.ref1')}</span>
                            )}
                            {isRef2 && (
                              <span className="text-xs text-amber-300 bg-amber-500/10 px-2 py-0.5 rounded-full shrink-0">{t('execute.ref2')}</span>
                            )}
                          </div>

                          <div className="flex items-center gap-2 shrink-0" onClick={(e) => e.stopPropagation()}>
                            {!isCurrent && (
                              <button
                                onClick={() => requestCheckout(branch.name, false)}
                                disabled={isLoading}
                                className="px-2.5 py-1 text-xs bg-slate-100 dark:bg-slate-700 text-slate-700 dark:text-slate-300 rounded-md hover:bg-slate-200 dark:hover:bg-slate-600 disabled:opacity-50 transition-colors"
                              >
                                {isLoading ? <LoaderIcon className="w-3 h-3 animate-spin" /> : t('execute.checkout')}
                              </button>
                            )}
                            <button
                              onClick={() => selectRefFromBranch(branch.name)}
                              disabled={isRef1 && !ref2}
                              className={classNames(
                                'px-2.5 py-1 text-xs rounded-md transition-colors',
                                isRef1 || isRef2
                                  ? 'bg-sky-600/20 text-sky-300 hover:bg-sky-600/30'
                                  : 'bg-slate-100 dark:bg-slate-700 text-slate-500 dark:text-slate-400 hover:bg-slate-200 dark:hover:bg-slate-600',
                              )}
                            >
                              {isRef1 || isRef2 ? t('execute.selected') : t('execute.select')}
                            </button>
                          </div>
                        </div>
                      )
                    })}

                    {/* Remote branches */}
                    {branches.filter(b => b.is_remote).length > 0 && (
                      <div className="text-xs text-slate-500 font-medium px-3 py-1.5 mt-2 uppercase tracking-wider border-t border-slate-200 dark:border-slate-700/50 pt-3">
                  {t('execute.remoteBranches')}
                      </div>
                    )}
                    {branches.filter(b => b.is_remote).map((branch) => {
                      const isRef1 = ref1?.value === branch.name && ref1?.type === 'branch'
                      const isRef2 = ref2?.value === branch.name && ref2?.type === 'branch'
                      const isLoading = checkoutLoading === branch.name

                      return (
                        <div
                          key={branch.name}
                          className={classNames(
                            'flex items-center justify-between px-3 py-2 rounded-lg transition-colors cursor-pointer',
                            isRef1 || isRef2
                              ? 'bg-sky-600/10 border border-sky-500/30'
                              : 'hover:bg-slate-100/50 dark:hover:bg-slate-700/50 border border-transparent',
                          )}
                          onClick={() => {
                            requestCheckout(branch.name, true)
                          }}
                        >
                          <div className="flex items-center gap-3 min-w-0">
                            <GitForkIcon className="w-3.5 h-3.5 text-slate-500 shrink-0" />
                            <span className="text-sm text-slate-400 truncate">
                              {branch.name}
                            </span>
                            <span className="text-xs text-slate-500 bg-slate-100/50 dark:bg-slate-700/50 px-1.5 py-0.5 rounded shrink-0">{t('execute.remote')}</span>
                            {isRef1 && (
                              <span className="text-xs text-blue-300 bg-blue-500/10 px-2 py-0.5 rounded-full shrink-0">{t('execute.ref1')}</span>
                            )}
                            {isRef2 && (
                              <span className="text-xs text-amber-300 bg-amber-500/10 px-2 py-0.5 rounded-full shrink-0">{t('execute.ref2')}</span>
                            )}
                          </div>

                          <div className="flex items-center gap-2 shrink-0" onClick={(e) => e.stopPropagation()}>
                            <button
                              onClick={() => requestCheckout(branch.name, true)}
                              disabled={isLoading}
                              className="px-2.5 py-1 text-xs bg-slate-100 dark:bg-slate-700 text-slate-700 dark:text-slate-300 rounded-md hover:bg-slate-200 dark:hover:bg-slate-600 disabled:opacity-50 transition-colors"
                            >
                              {isLoading ? <LoaderIcon className="w-3 h-3 animate-spin" /> : 'Checkout'}
                            </button>
                            <button
                              onClick={() => selectRefFromBranch(branch.name)}
                              disabled={isRef1 && !ref2}
                              className={classNames(
                                'px-2.5 py-1 text-xs rounded-md transition-colors',
                                isRef1 || isRef2
                                  ? 'bg-sky-600/20 text-sky-300 hover:bg-sky-600/30'
                                  : 'bg-slate-100 dark:bg-slate-700 text-slate-500 dark:text-slate-400 hover:bg-slate-200 dark:hover:bg-slate-600',
                              )}
                            >
                              {isRef1 || isRef2 ? t('execute.selected') : t('execute.select')}
                            </button>
                          </div>
                        </div>
                      )
                    })}
                  </div>
                )}
              </CardContent>
            </Card>

            {/* ──────────── Step: Commits ──────────── */}
            {selectedBranch && (
              <Card className="bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700">
                <CardHeader>
                  <CardTitle className="flex items-center gap-2 text-lg">
                    <span className="flex items-center justify-center w-6 h-6 rounded-full bg-sky-600 text-white text-xs font-bold shrink-0">3</span>
                    {t('execute.step3Title')}
                    <span className="text-sm font-normal text-slate-400 ml-1">— {selectedBranch}</span>
                  </CardTitle>
                </CardHeader>
                <CardContent>
                  {/* Search */}
                  <div className="relative mb-3">
                    <SearchIcon className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-500" />
                    <input
                      type="text"
                      placeholder={t('execute.searchCommits')}
                      value={commitSearch}
                      onChange={(e) => setCommitSearch(e.target.value)}
                      className="w-full pl-10 pr-4 py-2 bg-slate-100 dark:bg-slate-700 border border-slate-300 dark:border-slate-600 rounded-lg text-slate-900 dark:text-slate-100 placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-sky-500 text-sm"
                    />
                  </div>

                  {/* Commit list */}
                  <div className="space-y-1 max-h-80 overflow-y-auto pr-1">
                    {commits.length === 0 && !commitsLoading && !commitError && (
                      <div className="text-center py-8 text-slate-500 text-sm">
                        {commitSearch ? t('execute.noCommitsMatch') : t('execute.noCommitsFound')}
                      </div>
                    )}

                    {commitError && (
                      <div className="p-3 bg-red-900/30 border border-red-500/30 rounded-lg text-red-200 text-sm">
                        {commitError}
                      </div>
                    )}

                    {commits.map((commit) => {
                      const isRef1 = ref1?.value === commit.hash && ref1?.type === 'commit'
                      const isRef2 = ref2?.value === commit.hash && ref2?.type === 'commit'
                      const isSelected = isRef1 || isRef2
                      const hasEmptySlot = !ref1 || !ref2
                      const slotHint = !ref1 ? t('execute.ref1') : !ref2 ? t('execute.ref2') : ''

                      return (
                        <div
                          key={commit.hash}
                          onClick={() => hasEmptySlot && selectRefFromCommit(commit)}
                          className={classNames(
                            'flex items-center justify-between px-3 py-2 rounded-lg transition-colors',
                            isSelected
                              ? 'bg-sky-600/10 border border-sky-500/30 cursor-default'
                              : hasEmptySlot
                                ? 'hover:bg-slate-100/50 dark:hover:bg-slate-700/50 border border-transparent cursor-pointer'
                                : 'border border-transparent cursor-default opacity-60',
                          )}
                          title={!hasEmptySlot && !isSelected ? t('execute.bothRefsFilled') : slotHint ? t('execute.clickToSelectAs', { slot: slotHint }) : ''}
                        >
                          <div className="flex items-center gap-3 min-w-0">
                            <div className={classNames(
                              'w-4 h-4 rounded-full border-2 shrink-0 flex items-center justify-center transition-colors',
                              isSelected
                                ? 'border-sky-400 bg-sky-400/20'
                                : hasEmptySlot
                                  ? 'border-slate-500 group-hover:border-slate-400'
                                  : 'border-slate-300 dark:border-slate-600',
                            )}>
                              {isSelected && <div className="w-2 h-2 rounded-full bg-sky-400" />}
                            </div>
                            <code className="text-xs font-mono text-sky-300 shrink-0">{commit.short_hash}</code>
                            <span className="text-sm text-slate-700 dark:text-slate-300 truncate">{commit.message}</span>
                          </div>
                          <div className="flex items-center gap-2 shrink-0 ml-2">
                            <span className="text-xs text-slate-500 hidden sm:inline">{commit.author}</span>
                            <span className="text-xs text-slate-500 hidden sm:inline">{formatDate(commit.timestamp)}</span>
                            {isRef1 && (
                              <span className="text-xs px-2 py-0.5 rounded-full bg-blue-500/10 text-blue-300">Ref 1</span>
                            )}
                            {isRef2 && (
                              <span className="text-xs px-2 py-0.5 rounded-full bg-amber-500/10 text-amber-300">Ref 2</span>
                            )}
                            {!isSelected && hasEmptySlot && (
                              <span className="text-xs text-slate-500 hidden sm:inline">{slotHint}</span>
                            )}
                          </div>
                        </div>
                      )
                    })}

                    {commitsLoading && (
                      <div className="flex items-center justify-center py-4 text-slate-400 text-sm gap-2">
                        <LoaderIcon className="w-4 h-4 animate-spin" />
                        {t('execute.loadingCommits')}
                      </div>
                    )}
                  </div>

                  {/* Load more */}
                  {commitsHasMore && !commitsLoading && (
                    <button
                      onClick={() => loadCommits(repoId!, selectedBranch, commitOffset, commitSearch, true)}
                      className="w-full mt-2 py-2 bg-slate-100 dark:bg-slate-700 text-slate-700 dark:text-slate-300 rounded-lg text-sm hover:bg-slate-200 dark:hover:bg-slate-600 transition-colors"
                    >
                      {t('execute.loadMore')}
                    </button>
                  )}
                </CardContent>
              </Card>
            )}

            {/* ──────────── Step: Scan Configuration ──────────── */}
            <Card className="bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700">
              <CardHeader>
                <CardTitle className="flex items-center gap-2 text-lg">
                  <span className="flex items-center justify-center w-6 h-6 rounded-full bg-sky-600 text-white text-xs font-bold shrink-0">4</span>
                  {t('execute.step4Title')}
                  <div className="ml-auto">{renderScanModeBadge()}</div>
                </CardTitle>
              </CardHeader>
              <CardContent>
                {/* Selected Refs */}
                <div className="space-y-3 mb-4">
                  {/* Ref 1 — Target */}
                  <div className="flex items-center gap-3 p-3 bg-slate-100/50 dark:bg-slate-700/50 rounded-lg border border-slate-200 dark:border-slate-600/50">
                    <span className="text-xs font-medium text-blue-300 w-12 shrink-0">{t('execute.ref1')}</span>
                    {ref1 ? (
                      <>
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center gap-2">
                            <span className="text-sm text-slate-800 dark:text-slate-200 truncate">{ref1.label}</span>
                            {scanMode === 'diff' && ref1.timestamp && (
                              <span className="text-xs text-blue-400/70 bg-blue-500/10 px-1.5 py-0.5 rounded shrink-0">{t('execute.target')}</span>
                            )}
                          </div>
                          {ref1.timestamp && (
                            <span className="text-xs text-slate-500">{formatDate(ref1.timestamp)}</span>
                          )}
                        </div>
                        <button
                          onClick={() => clearRef(1)}
                          className="p-1 hover:bg-slate-200 dark:hover:bg-slate-600 rounded transition-colors"
                          title={t('execute.clear')}
                        >
                          <XIcon className="w-3.5 h-3.5 text-slate-400" />
                        </button>
                      </>
                    ) : (
                      <span className="flex-1 text-sm text-slate-500">{t('execute.selectBranchOrCommit')}</span>
                    )}
                  </div>

                  {/* Ref 2 — Base */}
                  <div className="flex items-center gap-3 p-3 bg-slate-100/50 dark:bg-slate-700/50 rounded-lg border border-slate-200 dark:border-slate-600/50">
                    <span className="text-xs font-medium text-amber-300 w-12 shrink-0">{t('execute.ref2')}</span>
                    {ref2 ? (
                      <>
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center gap-2">
                            <span className="text-sm text-slate-800 dark:text-slate-200 truncate">{ref2.label}</span>
                            {scanMode === 'diff' && ref2.timestamp && (
                              <span className="text-xs text-amber-400/70 bg-amber-500/10 px-1.5 py-0.5 rounded shrink-0">{t('execute.base')}</span>
                            )}
                          </div>
                          {ref2.timestamp && (
                            <span className="text-xs text-slate-500">{formatDate(ref2.timestamp)}</span>
                          )}
                        </div>
                        <button
                          onClick={() => clearRef(2)}
                          className="p-1 hover:bg-slate-200 dark:hover:bg-slate-600 rounded transition-colors"
                          title={t('execute.clear')}
                        >
                          <XIcon className="w-3.5 h-3.5 text-slate-400" />
                        </button>
                      </>
                    ) : (
                      <span className="flex-1 text-sm text-slate-500 italic">
                        {t('execute.optionalDiffRef')}
                      </span>
                    )}
                  </div>

                  {/* Diff comparison detail (only when both refs are set) */}
                  {ref1 && ref2 && scanMode === 'diff' && (
                    <DiffComparison ref1={ref1} ref2={ref2} onSwap={() => {
                      setRef1(ref2)
                      setRef2(ref1)
                    }} />
                  )}
                </div>

                {/* Project name */}
                <div className="mb-4">
                  <label className="block text-sm text-slate-400 mb-1.5">{t('execute.projectName')}</label>
                  <input
                    type="text"
                    placeholder={t('execute.projectPlaceholder')}
                    value={projectName}
                    onChange={(e) => setProjectName(e.target.value)}
                    className="w-full px-4 py-2 bg-slate-100 dark:bg-slate-700 border border-slate-300 dark:border-slate-600 rounded-lg text-slate-900 dark:text-slate-100 placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-sky-500 text-sm"
                    disabled={isScanning}
                  />
                </div>

                {/* Info */}
                <div className="mb-4 p-3 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-500/30 rounded-lg flex items-start gap-2">
                  <InfoIcon className="w-4 h-4 text-blue-300 mt-0.5 shrink-0" />
                  <div className="text-xs text-blue-200 space-y-1">
                    <p>{t('execute.infoIntro')}</p>
                    <p>{t('execute.infoSnapshot')}</p>
                    <p>{t('execute.infoDiff')}</p>
                    <p>{t('execute.infoOrder')}</p>
                  </div>
                </div>

                {/* Progress */}
                {scanProgress.status !== 'idle' && (
                  <div className="mb-4">
                    {/* Status message */}
                    <div
                      className={classNames(
                        'p-3 rounded-lg text-sm flex items-center gap-2',
                        scanProgress.status === 'success'
                          ? 'bg-green-900/30 border border-green-500/30 text-green-200'
                          : scanProgress.status === 'error'
                          ? 'bg-red-900/30 border border-red-500/30 text-red-200'
                          : 'bg-blue-900/30 border border-blue-500/30 text-blue-200',
                      )}
                    >
                      {scanProgress.status === 'loading' && <LoaderIcon className="w-4 h-4 animate-spin shrink-0" />}
                      {scanProgress.status === 'success' && <CheckIcon className="w-4 h-4 shrink-0" />}
                      {scanProgress.status === 'error' && <XIcon className="w-4 h-4 shrink-0" />}
                      <span className="flex-1">{scanProgress.message}</span>
                      {scanProgress.status === 'success' && scanProgress.projectName && (
                        <button
                          onClick={() => goToDashboard(scanProgress.projectName!)}
                          className="px-3 py-1.5 text-xs bg-green-600 hover:bg-green-700 text-white rounded-md transition-colors flex items-center gap-1.5 shrink-0"
                        >
                          <ArrowRightIcon className="w-3.5 h-3.5" />
                          {t('execute.viewDashboard')}
                        </button>
                      )}
                    </div>
                    {/* Progress bar (only during loading/running) */}
                    {scanProgress.status === 'loading' && scanProgress.progress != null && (
                      <div className="mt-2">
                        <div className="flex justify-between text-xs text-slate-400 mb-1">
                          <span>{t('execute.scanProgress')}</span>
                          <span>{scanProgress.progress}%</span>
                        </div>
                        <div className="w-full bg-slate-700 rounded-full h-2">
                          <div
                            className="bg-sky-500 h-2 rounded-full transition-all duration-500 ease-out"
                            style={{ width: `${scanProgress.progress}%` }}
                          />
                        </div>
                        {/* Current phase description */}
                        {scanProgress.progressMessage && (
                          <div className="mt-1.5 text-xs text-slate-400 truncate">
                            {scanProgress.progressMessage}
                          </div>
                        )}
                        {/* ETA */}
                        {eta && (
                          <div className="mt-0.5 text-xs text-slate-500">
                            {t('execute.eta', { time: eta })}
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                )}

                {/* Start Scan */}
                <button
                  onClick={handleScan}
                  disabled={!ref1 || isScanning || scanMode === null}
                  className="w-full px-6 py-2.5 bg-sky-600 text-white rounded-lg font-medium hover:bg-sky-700 disabled:bg-slate-600 disabled:cursor-not-allowed transition-colors flex items-center justify-center gap-2"
                >
                  {isScanning ? (
                    <><LoaderIcon className="w-4 h-4 animate-spin" /> {t('execute.scanning')}</>
                  ) : !ref1 ? (
                    t('execute.selectToStart')
                  ) : scanMode === 'snapshot' ? (
                    <><Code2Icon className="w-4 h-4" /> {t('execute.startSnapshot')}</>
                  ) : (
                    <><GitForkIcon className="w-4 h-4" /> {t('execute.startDiff')}</>
                  )}
                </button>
              </CardContent>
            </Card>
          </>
        )}
      </div>

      {/* ──────────── Checkout Confirmation Modal ──────────── */}
      {checkoutConfirm && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
          <div className="bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-600 rounded-xl shadow-2xl max-w-md w-full mx-4 overflow-hidden">
            <div className="p-5">
              <div className="flex items-start gap-4">
                <div className="w-10 h-10 rounded-full bg-amber-500/20 flex items-center justify-center shrink-0">
                  <AlertTriangleIcon className="w-5 h-5 text-amber-400" />
                </div>
                <div className="min-w-0 flex-1">
                  <h3 className="text-base font-semibold text-slate-900 dark:text-slate-100 mb-1">{t('execute.checkoutTitle')}</h3>
                  <p className="text-sm text-slate-400 break-words mb-2">
                    {t('execute.checkoutSwitchTo', { branch: checkoutConfirm.branch })}
                  </p>
                  <div className="p-3 bg-amber-500/10 border border-amber-500/20 rounded-lg text-xs text-amber-200 space-y-1.5">
                    <p className="font-medium">{t('execute.checkoutCaution')}</p>
                    <p>{t('execute.checkoutWarning')}</p>
                    <p>{t('execute.checkoutConfirm')}</p>
                  </div>
                </div>
              </div>
            </div>
            <div className="flex items-center justify-end gap-2 px-5 py-3 bg-slate-850 border-t border-slate-200 dark:border-slate-700">
              <button
                onClick={() => setCheckoutConfirm(null)}
                className="px-4 py-2 text-sm text-slate-700 dark:text-slate-300 bg-slate-100 dark:bg-slate-700 rounded-lg hover:bg-slate-200 dark:hover:bg-slate-600 transition-colors"
              >
                {t('execute.cancel')}
              </button>
              <button
                onClick={() => confirmCheckout(checkoutConfirm.branch)}
                className="px-4 py-2 text-sm text-white bg-amber-600 rounded-lg hover:bg-amber-700 transition-colors"
              >
                {t('execute.confirmCheckout')}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
