
export interface AggregationResult {
  label: string;
  value: number;
  tech_stack?: string;
  category?: string;
  change_type?: string;
  metric_key?: string;
  analyzer_id?: string;
  children?: AggregationResult[];
  group_key?: string;
  tags?: Record<string, string>;
}


export interface ViewResponse {
  view_id: string;
  items: AggregationResult[];
}

// View Configuration Types from Backend
export interface ViewConfig {
  id: string;
  title: string;
  tech_stacks: string[];
  include_children?: boolean;
  group_by?: string[];
  chart_type?: string;
  change_type_mode?: 'all' | 'switchable';
  width?: number;
  type: 'top_n' | 'sum' | 'avg' | 'min' | 'max' | 'distribution';
  source?: {
    analyzer_id: string[];
    tag_filters?: Record<string, string>;
  };
  params?: {
    limit: number;
    order?: string;
  };
}

// Project-specific configuration
export interface ProjectConfig {
  name: string;
  views: ViewConfig[];
  tech_stacks: string[];
}

// Root application config with multiple projects
export interface AppConfig {
  projects: ProjectConfig[];
}

// Helper: Check if multi-project mode
export function isMultiProject(config: AppConfig): boolean {
  return config.projects.length > 1;
}

// Helper: Get project names
export function getProjectNames(config: AppConfig): string[] {
  return config.projects.map(p => p.name);
}

// Helper: Get project config by name
export function getProjectConfig(config: AppConfig, name: string): ProjectConfig | undefined {
  return config.projects.find(p => p.name === name);
}

// Helper: Get first/default project
export function getDefaultProject(config: AppConfig): ProjectConfig | undefined {
  return config.projects[0];
}

export interface FetchViewOptions {
  techStack?: string;
  changeType?: string;
  groupBy?: string;
}

export async function fetchView(
  projectId: number | string,
  scanId: number | string,
  viewId: string,
  options?: FetchViewOptions | string // backwards compat: string = techStack
): Promise<ViewResponse> {
  try {
    const params = new URLSearchParams();

    // Handle backwards compatibility
    if (typeof options === 'string') {
      if (options) params.append('tech_stack', options);
    } else if (options) {
      if (options.techStack) params.append('tech_stack', options.techStack);
      if (options.changeType) params.append('change_type', options.changeType);
      if (options.groupBy) params.append('group_by', options.groupBy);
    }

    const queryString = params.toString();
    const url = `/api/v1/projects/${projectId}/scans/${scanId}/views/${viewId}${queryString ? `?${queryString}` : ''}`;

    const res = await fetch(url);
    if (!res.ok) {
      throw new Error(`Failed to fetch view ${viewId}`);
    }
    return await res.json();
  } catch (error) {
    console.warn(`Error fetching view ${viewId}, falling back to empty`, error);
    return { view_id: viewId, items: [] };
  }
}

export interface Run {
  id: string;
  commit_hash: string;
  scan_time: string;
  scan_mode?: 'SNAPSHOT' | 'DIFF';
}

export async function fetchRuns(projectId: number | string, mode: 'SNAPSHOT' | 'DIFF'): Promise<Run[]> {
  try {
    const res = await fetch(`/api/v1/projects/${projectId}/scans?mode=${mode}`);
    if (!res.ok) {
      throw new Error(`Failed to fetch runs: ${res.statusText}`);
    }
    const data = await res.json();
    return data;
  } catch (error) {
    console.error("Error fetching runs:", error);
    return [];
  }
}

export async function fetchConfig(): Promise<AppConfig> {
  try {
    const res = await fetch('/api/v1/config');
    if (!res.ok) throw new Error('Failed to fetch config');
    return await res.json();
  } catch (error) {
    console.error("Error fetching config:", error);
    return { projects: [] };
  }
}

// ─── DB Projects (projects that have scan data in the database) ─────────────

export interface ProjectInfo {
  id: number;
  name: string;
  repo_path: string;
  created_at: string;
  scan_modes: string[];
  total_scans: number;
  last_scan_time: string | null;
}

export async function fetchProjects(): Promise<ProjectInfo[]> {
  try {
    const res = await fetch('/api/v1/projects');
    if (!res.ok) throw new Error('Failed to fetch projects');
    return await res.json();
  } catch (error) {
    console.error("Error fetching projects:", error);
    return [];
  }
}

// ─── Full Config Types (for config editor) ─────────────────────────────────

export interface FullTechStack {
  name: string;
  extensions: string[];
  analyzers: string[];
  paths: string[];
  excludes: string[];
}

export interface CustomAnalyzerDef {
  pattern: string;
  metric_key: string;
  category?: string;
  tags?: Record<string, string>;
  scan_mode?: 'all' | 'snapshot' | 'diff';
  change_type?: 'all' | 'A' | 'M' | 'D';
}

export interface ImplAnalyzerConfig {
  metric_key?: string;
  category?: string;
  tags?: Record<string, string>;
  scan_mode?: 'all' | 'snapshot' | 'diff';
  change_type?: 'all' | 'A' | 'M' | 'D';
}

export interface AggregationFunc {
  type: 'top_n' | 'sum' | 'avg' | 'min' | 'max' | 'distribution';
  analyzer_id?: string[];
  tag_filters?: Record<string, string>;
  limit?: number;
  order?: string;
  buckets?: number[];
}

export interface AggregationView {
  title: string;
  tech_stacks?: string[];
  include_children?: boolean;
  group_by?: string[];
  chart_type?: string;
  change_type_mode?: string;
  func: AggregationFunc;
}

export interface FullProjectConfig {
  name: string;
  repo_path?: string;
  tech_stacks: FullTechStack[];
  global_excludes: string[];
  custom_regex_analyzers: Record<string, CustomAnalyzerDef>;
  custom_impl_analyzers: Record<string, ImplAnalyzerConfig>;
  external_analyzers: Record<string, string>;
  aggregation_views: Record<string, AggregationView>;
}

export async function fetchFullProjectConfig(projectName: string): Promise<FullProjectConfig> {
  const res = await fetch(`/api/v1/config/projects/${encodeURIComponent(projectName)}`);
  if (!res.ok) throw new Error(`Failed to fetch project config: ${res.statusText}`);
  return await res.json();
}

export async function updateProjectConfig(
  projectName: string,
  config: FullProjectConfig
): Promise<{ status: string; message: string }> {
  const res = await fetch(`/api/v1/config/projects/${encodeURIComponent(projectName)}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(config),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: 'Failed to update config' }));
    throw new Error(err.error || 'Failed to update config');
  }
  return await res.json();
}

/** Get project names that exist in the database (have scan data) */
export function getDbProjectNames(projects: ProjectInfo[]): string[] {
  return projects.map(p => p.name);
}

/** Merge config project names with DB project names, preserving order: config first, then DB-only */
export function mergeProjectNames(configNames: string[], dbProjectInfos: ProjectInfo[]): string[] {
  const configSet = new Set(configNames);
  const dbNames = dbProjectInfos.map(p => p.name);
  const dbOnly = dbNames.filter(n => !configSet.has(n));
  return [...configNames, ...dbOnly];
}

// ─── Project Template API ─────────────────────────────────────────────

export async function fetchTemplates(): Promise<Record<string, FullProjectConfig>> {
  const res = await fetch('/api/v1/config/templates');
  if (!res.ok) throw new Error('Failed to fetch templates');
  return await res.json();
}

export async function fetchTemplate(name: string): Promise<FullProjectConfig> {
  const res = await fetch(`/api/v1/config/templates/${encodeURIComponent(name)}`);
  if (!res.ok) throw new Error(`Template '${name}' not found`);
  return await res.json();
}

export async function saveTemplate(
  name: string,
  config: FullProjectConfig
): Promise<{ status: string; message: string }> {
  const templateConfig: FullProjectConfig = {
    ...config,
    name,
  };
  const res = await fetch(`/api/v1/config/templates/${encodeURIComponent(name)}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(templateConfig),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: 'Failed to save template' }));
    throw new Error(err.error || 'Failed to save template');
  }
  return await res.json();
}

export async function deleteTemplate(name: string): Promise<void> {
  const res = await fetch(`/api/v1/config/templates/${encodeURIComponent(name)}`, {
    method: 'DELETE',
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: 'Failed to delete template' }));
    throw new Error(err.error || 'Failed to delete template');
  }
}

// ─── Config Reload ──────────────────────────────────────────────────

export async function reloadConfig(): Promise<{ status: string; message: string }> {
  const res = await fetch('/api/v1/config/reload', { method: 'POST' });
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: 'Failed to reload config' }));
    throw new Error(err.error || 'Failed to reload config');
  }
  return await res.json();
}

// ─── Git Repo Management API ─────────────────────────────────────────────

export interface BranchInfo {
  name: string;
  is_head: boolean;
  is_remote: boolean;
}

export interface CloneResponse {
  repo_id: string;
  branches: BranchInfo[];
  current_branch: string;
}

export interface CommitInfo {
  hash: string;
  short_hash: string;
  message: string;
  author: string;
  timestamp: number;
}

export interface CommitsResponse {
  commits: CommitInfo[];
  has_more: boolean;
}

export async function cloneRepo(gitUrl: string, projectName?: string): Promise<CloneResponse> {
  const res = await fetch('/api/v1/git/clone', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ git_url: gitUrl, project_name: projectName || null }),
  });
  if (!res.ok) {
    const err = await res.json();
    throw new Error(err.error || 'Clone failed');
  }
  return res.json();
}

export async function addLocalProject(name: string, path: string): Promise<CloneResponse> {
  const res = await fetch('/api/v1/projects/add-local', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name, path }),
  });
  if (!res.ok) {
    const err = await res.json();
    throw new Error(err.error || 'Failed to add local project');
  }
  return res.json();
}

export async function listBranches(repoId: string): Promise<{ branches: BranchInfo[]; current_branch: string }> {
  const res = await fetch(`/api/v1/git/${repoId}/branches`);
  if (!res.ok) {
    const err = await res.json();
    throw new Error(err.error || 'Failed to list branches');
  }
  return res.json();
}

export async function checkoutBranch(repoId: string, branch: string): Promise<{ branch: string; message: string }> {
  const res = await fetch(`/api/v1/git/${repoId}/checkout`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ branch }),
  });
  if (!res.ok) {
    const err = await res.json();
    throw new Error(err.error || 'Checkout failed');
  }
  return res.json();
}

export async function listCommits(
  repoId: string,
  options?: { ref?: string; offset?: number; limit?: number; search?: string }
): Promise<CommitsResponse> {
  const params = new URLSearchParams();
  if (options?.ref) params.set('ref', options.ref);
  if (options?.offset !== undefined) params.set('offset', String(options.offset));
  if (options?.limit !== undefined) params.set('limit', String(options.limit));
  if (options?.search) params.set('search', options.search);

  const qs = params.toString();
  const res = await fetch(`/api/v1/git/${repoId}/commits${qs ? `?${qs}` : ''}`);
  if (!res.ok) {
    const err = await res.json();
    throw new Error(err.error || 'Failed to list commits');
  }
  return res.json();
}

export interface ScanWithRepoRequest {
  repo_id: string;
  ref_1: string;
  ref_2?: string;
  project_name?: string;
  scan_mode: 'snapshot' | 'diff';
}

export async function executeScanWithRepo(req: ScanWithRepoRequest): Promise<ScanStartedResponse> {
  const res = await fetch('/api/v1/scan', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      git_url: '',
      repo_id: req.repo_id,
      ref_1: req.ref_1,
      ref_2: req.ref_2 || null,
      scan_mode: req.scan_mode,
      project_name: req.project_name || 'scanned_project',
    }),
  });
  if (!res.ok) {
    const err = await res.json();
    throw new Error(err.message || err.error || 'Scan failed');
  }
  return res.json();
}

// ─── Scan Job Tracking ──────────────────────────────────────────────

export interface ScanStartedResponse {
  job_id: number;
  project_name: string;
  status: string;
  message: string;
}

export interface ScanJobResponse {
  job_id: number;
  project_name: string;
  scan_mode: string;
  status: 'queued' | 'running' | 'completed' | 'failed';
  progress: number;
  error_message: string | null;
  scan_id: number | null;
  created_at: string;
  updated_at: string;
}

export async function fetchScanJob(jobId: number): Promise<ScanJobResponse> {
  const res = await fetch(`/api/v1/scan-jobs/${jobId}`);
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: 'Failed to fetch scan job' }));
    throw new Error(err.error || 'Failed to fetch scan job');
  }
  return res.json();
}

// ─── Scan Execution Summary ─────────────────────────────────────────

export interface AnalyzerStatItem {
  analyzer_id: string;
  files_analyzed: number;
  execution_errors: number;
  error_details: string[];
}

export interface ScanSummary {
  scan_id: number;
  total_files_scanned: number;
  total_analyzers_loaded: number;
  total_analyzers_executed: number;
  total_analyzer_executions: number;
  total_errors: number;
  load_errors: string[];
  analyzer_stats: AnalyzerStatItem[];
}

export async function fetchScanSummary(projectName: string, scanId: string | number): Promise<ScanSummary | null> {
  try {
    const res = await fetch(`/api/v1/projects/${encodeURIComponent(projectName)}/scans/${scanId}/summary`);
    if (!res.ok) {
      if (res.status === 404) return null;
      throw new Error(`Failed to fetch scan summary: ${res.statusText}`);
    }
    return await res.json();
  } catch (error) {
    console.error("Error fetching scan summary:", error);
    return null;
  }
}

// ─── Repo Listing & Management ──────────────────────────────────────────────

export interface RepoInfo {
  repo_id: string;
  git_url: string;
  current_branch: string;
  path: string;
  project_name?: string | null;
}

export interface ListReposResponse {
  repos: RepoInfo[];
}

export async function listRepos(): Promise<RepoInfo[]> {
  const res = await fetch('/api/v1/git/repos');
  if (!res.ok) {
    throw new Error('Failed to list repos');
  }
  const data: ListReposResponse = await res.json();
  return data.repos;
}

export async function deleteRepo(repoId: string): Promise<void> {
  const res = await fetch(`/api/v1/git/${repoId}`, {
    method: 'DELETE',
  });
  if (!res.ok) {
    const err = await res.json();
    throw new Error(err.error || 'Failed to delete repo');
  }
}

// ─── Unified Project API ─────────────────────────────────────────────

export interface UnifiedProjectInfo {
  name: string;
  has_config: boolean;
  config_repo_path: string | null;
  has_cached_repo: boolean;
  cached_repo_id: string | null;
  cached_repo_branch: string | null;
  total_scans: number;
  last_scan_time: string | null;
  scan_modes: string[];
}

export async function fetchUnifiedProjects(): Promise<UnifiedProjectInfo[]> {
  try {
    const res = await fetch('/api/v1/projects/unified');
    if (!res.ok) throw new Error('Failed to fetch unified project list');
    return await res.json();
  } catch (error) {
    console.error("Error fetching unified projects:", error);
    return [];
  }
}

export async function createProject(name: string): Promise<{ status: string; message: string }> {
  const res = await fetch('/api/v1/projects', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name }),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: 'Failed to create project' }));
    throw new Error(err.error || 'Failed to create project');
  }
  return await res.json();
}

export async function deleteProject(projectName: string): Promise<{ status: string; message: string }> {
  const res = await fetch(`/api/v1/projects/${encodeURIComponent(projectName)}`, {
    method: 'DELETE',
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: 'Failed to delete project' }));
    throw new Error(err.error || 'Failed to delete project');
  }
  return await res.json();
}
