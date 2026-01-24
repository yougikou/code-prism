
export interface AggregationResult {
  label: string;
  value: number;
  tech_stack?: string;
  category?: string;
  change_type?: string;
  children?: AggregationResult[];
  group_key?: string;
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
  category?: string;
  include_children?: boolean;
  group_by?: string[];
  chart_type?: string;
  change_type_mode?: 'all' | 'switchable';
  type: 'top_n' | 'sum' | 'avg' | 'min' | 'max' | 'distribution';
  source?: {
    analyzer_id: string;
    metric_key: string;
  };
  params?: {
    limit: number;
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
