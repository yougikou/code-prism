
export interface AggregationResult {
  label: string;
  value: number;
  tech_stack?: string;
  category?: string;
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
  type: 'top_n' | 'sum';
  source?: {
    analyzer_id: string;
    metric_key: string;
  };
  params?: {
    limit: number;
  };
}

export interface AppConfig {
  views: ViewConfig[];
  tech_stacks: string[];
}

export async function fetchView(projectId: number | string, scanId: number | string, viewId: string): Promise<ViewResponse> {
  try {
    const res = await fetch(`/api/v1/projects/${projectId}/scans/${scanId}/views/${viewId}`);
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
    return { views: [], tech_stacks: [] };
  }
}
