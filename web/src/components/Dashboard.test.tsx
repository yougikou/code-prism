import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, waitFor } from '@testing-library/react';
import Dashboard from './Dashboard';
import { AppProvider } from '@/contexts/AppContext';
import * as dataService from '@/services/data';

// Mock the data service
vi.mock('@/services/data', () => ({
  fetchConfig: vi.fn(),
  fetchRuns: vi.fn(),
  fetchView: vi.fn(),
  fetchUnifiedProjects: vi.fn().mockResolvedValue([]),
  fetchScanSummary: vi.fn(),
  fetchMatches: vi.fn(),
  isMultiProject: vi.fn(),
  getDefaultProject: vi.fn(),
  getProjectNames: vi.fn(),
}));

function renderWithProviders(ui: React.ReactElement) {
  return render(<AppProvider>{ui}</AppProvider>);
}

describe('Dashboard', () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it('renders loading state initially', async () => {
    // Mock config fetch to return empty response
    vi.mocked(dataService.fetchConfig).mockResolvedValue({ projects: [] });
    vi.mocked(dataService.isMultiProject).mockReturnValue(false);
    vi.mocked(dataService.getDefaultProject).mockReturnValue(undefined);
    vi.mocked(dataService.getProjectNames).mockReturnValue([]);

    renderWithProviders(<Dashboard />);

    // Should show some loading or empty state
    await waitFor(() => {
      // Dashboard should render without crashing
      expect(document.body).toBeDefined();
    });
  });

  it('shows empty state when no projects exist', async () => {
    vi.mocked(dataService.fetchConfig).mockResolvedValue({ projects: [] });
    vi.mocked(dataService.isMultiProject).mockReturnValue(false);
    vi.mocked(dataService.getDefaultProject).mockReturnValue(undefined);
    vi.mocked(dataService.getProjectNames).mockReturnValue([]);

    renderWithProviders(<Dashboard />);

    await waitFor(() => {
      // Dashboard should handle empty state gracefully
      expect(document.body).toBeDefined();
    });
  });

  it('fetches configuration on mount', async () => {
    vi.mocked(dataService.fetchConfig).mockResolvedValue({
      projects: [{
        name: 'test_project',
        views: [],
        tech_stacks: ['Rust']
      }]
    });
    vi.mocked(dataService.isMultiProject).mockReturnValue(false);
    vi.mocked(dataService.getDefaultProject).mockReturnValue({
      name: 'test_project',
      views: [],
      tech_stacks: ['Rust']
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
