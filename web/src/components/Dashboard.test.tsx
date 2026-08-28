import { describe, it, expect, vi, beforeEach, beforeAll } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import Dashboard from './Dashboard';
import { AppProvider } from '@/contexts/AppContext';
import * as dataService from '@/services/data';
import i18n from '@/i18n';

// Mock the data service
vi.mock('@/services/data', () => ({
  fetchConfig: vi.fn(),
  fetchRuns: vi.fn(),
  fetchView: vi.fn(),
  fetchUnifiedProjects: vi.fn().mockResolvedValue([]),
  fetchScanSummary: vi.fn(),
  fetchExecutionOutcomes: vi.fn(),
  fetchMatches: vi.fn(),
  isMultiProject: vi.fn(),
  getDefaultProject: vi.fn(),
  getProjectNames: vi.fn(),
}));

function renderWithProviders(ui: React.ReactElement) {
  return render(<AppProvider>{ui}</AppProvider>);
}

describe('Dashboard', () => {
  beforeAll(async () => {
    await i18n.changeLanguage('en');
  });

  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(dataService.fetchUnifiedProjects).mockResolvedValue([]);
    vi.mocked(dataService.fetchRuns).mockResolvedValue([]);
    vi.mocked(dataService.fetchScanSummary).mockResolvedValue(null);
    vi.mocked(dataService.fetchExecutionOutcomes).mockResolvedValue([]);
  });

  it('renders a chart loading state while view data is pending', async () => {
    const project = {
      name: 'test_project',
      views: [{
        id: 'files',
        title: 'Files',
        tech_stacks: [],
        type: 'sum' as const,
        chart_type: 'bar_row',
      }],
      tech_stacks: [{ name: 'Rust' }],
      columns: 2,
    };
    vi.mocked(dataService.fetchConfig).mockResolvedValue({ projects: [project] });
    vi.mocked(dataService.isMultiProject).mockReturnValue(false);
    vi.mocked(dataService.getDefaultProject).mockReturnValue(project);
    vi.mocked(dataService.getProjectNames).mockReturnValue(['test_project']);
    vi.mocked(dataService.fetchUnifiedProjects).mockResolvedValue([{
      name: 'test_project',
      has_config: true,
      config_repo_path: null,
      has_cached_repo: false,
      cached_repo_id: null,
      cached_repo_branch: null,
      total_scans: 1,
      last_scan_time: null,
      scan_modes: ['SNAPSHOT'],
    }]);
    vi.mocked(dataService.fetchRuns).mockResolvedValue([{
      id: '1',
      commit_hash: 'abcdef123456',
      scan_time: '2026-07-22T00:00:00Z',
      scan_mode: 'SNAPSHOT',
    }]);
    vi.mocked(dataService.fetchView).mockReturnValue(new Promise(() => {}));

    const { container } = renderWithProviders(<Dashboard />);

    await waitFor(() => {
      expect(dataService.fetchView).toHaveBeenCalled();
      expect(container.querySelector('.animate-pulse')).not.toBeNull();
    });
  });

  it('shows empty state when no projects exist', async () => {
    vi.mocked(dataService.fetchConfig).mockResolvedValue({ projects: [] });
    vi.mocked(dataService.isMultiProject).mockReturnValue(false);
    vi.mocked(dataService.getDefaultProject).mockReturnValue(undefined);
    vi.mocked(dataService.getProjectNames).mockReturnValue([]);

    renderWithProviders(<Dashboard />);

    expect(await screen.findByText('No configured views found')).toBeDefined();
  });

  it('fetches configuration on mount', async () => {
    vi.mocked(dataService.fetchConfig).mockResolvedValue({
      projects: [{
        name: 'test_project',
        views: [],
        tech_stacks: [{ name: 'Rust' }],
        columns: 2,
      }]
    });
    vi.mocked(dataService.isMultiProject).mockReturnValue(false);
    vi.mocked(dataService.getDefaultProject).mockReturnValue({
      name: 'test_project',
      views: [],
      tech_stacks: [{ name: 'Rust' }],
      columns: 2,
    });
    vi.mocked(dataService.getProjectNames).mockReturnValue(['test_project']);
    vi.mocked(dataService.fetchRuns).mockResolvedValue([]);
    vi.mocked(dataService.fetchUnifiedProjects).mockResolvedValue([]);

    renderWithProviders(<Dashboard />);

    await waitFor(() => {
      expect(dataService.fetchConfig).toHaveBeenCalled();
    });
  });
});
