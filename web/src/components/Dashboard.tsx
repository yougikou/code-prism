import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useApp } from '@/contexts/AppContext';
import { Sidebar } from './layout/Sidebar';
import { TechStackTabs } from './dashboard/TechStackTabs';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { MetricCard } from './widgets/MetricCard';

import ChartRenderer from './ChartRenderer';
import { ChildrenViewer } from './dashboard/ChildrenViewer';
import { MatchDetailView } from './dashboard/MatchDetailView';
import { fetchView, fetchScanSummary, fetchMatches, fetchTrend, type AggregationResult, type AppConfig, type ScanSummary, type MatchDetail, type TrendSeries, getDefaultProject } from '@/services/data';
import TrendRenderer from './widgets/TrendRenderer';
import { TrendScanSelector } from './dashboard/TrendScanSelector';
import { ChartSkeleton } from '@/components/ui/skeleton';
import { BarChart3, FileText, Maximize2, Minimize2, SlidersHorizontal, TrendingUp } from 'lucide-react';

const TIME_RANGE_PRESETS: Record<string, number> = {
  '1m': 30,
  '2m': 60,
  '3m': 90,
  '6m': 180,
  '9m': 270,
  '12m': 365,
  '2y': 730,
  '3y': 1095,
  '5y': 1825,
};

const TIME_RANGE_KEYS = ['1m', '2m', '3m', '6m', '9m', '12m', '2y', '3y', '5y'] as const;

const Dashboard = () => {
  const { t } = useTranslation();
  const {
    currentProject, setProject,
    viewMode,
    selectedTechStack, setSelectedTechStack,
    selectedRunId, setSelectedRunId,
    availableTechStacks, setAvailableTechStacks,
    theme, navigateTo, configVersion,
    projectList
  } = useApp();

  const [appConfig, setAppConfig] = useState<AppConfig | null>(null);
  // Derive all project names from the unified project list

  const [viewsConfig, setViewsConfig] = useState<any[]>([]);
  const [activeViews, setActiveViews] = useState<any[]>([]);
  const [viewDataMap, setViewDataMap] = useState<Record<string, AggregationResult[]>>({});
  const [loading, setLoading] = useState(true);
  const [runs, setRuns] = useState<any[]>([]);
  const [scanSummary, setScanSummary] = useState<ScanSummary | null>(null);
  // Track change_type filter per view (for switchable mode)
  const [changeTypeFilters, setChangeTypeFilters] = useState<Record<string, string>>({});
  // Trend state
  const [trendViews, setTrendViews] = useState<any[]>([]);
  const [trendDataMap, setTrendDataMap] = useState<Record<string, TrendSeries[]>>({});
  // Ad-hoc trend mode: each view can be toggled to show trend instead of normal chart: each view can be toggled to show trend instead of normal chart
  const [trendActive, setTrendActive] = useState<Record<string, boolean>>({});
  const [trendCustomScanIds, setTrendCustomScanIds] = useState<Record<string, number[]>>({});
  // Trend time range preset state
  const [trendTimeRange, setTrendTimeRange] = useState<string>('3m');
  // TopN limit selector state
  const [topnLimits, setTopnLimits] = useState<Record<string, number>>({});
  const [trendLoadingMap, setTrendLoadingMap] = useState<Record<string, boolean>>({});
  const [scanSelectorView, setScanSelectorView] = useState<{
    open: boolean;
    projectName: string;
    viewId: string;
    scanMode: 'snapshot' | 'diff';
    baseCommit?: string;
    initialSelectedScanIds?: number[];
  } | null>(null);

  // Children viewer modal state
  interface LeafItem {
    label: string;
    value: number;
    group?: string;
    analyzerId?: string;
  }
  const [childrenView, setChildrenView] = useState<{
    open: boolean;
    title: string;
    items: LeafItem[];
  }>({ open: false, title: '', items: [] });

  // Match detail modal state
  const [matchDetailView, setMatchDetailView] = useState<{
    open: boolean;
    title: string;
    filePath: string;
    analyzerId?: string;
    matches: MatchDetail[];
    total: number;
    loading: boolean;
    side?: boolean;
  }>({ open: false, title: '', filePath: '', matches: [], total: 0, loading: false });

  // Fullscreen chart modal state
  const [fullscreenView, setFullscreenView] = useState<{
    open: boolean;
    title: string;
    type: 'chart' | 'card' | 'table' | 'trend';
    options?: any;
    value?: string;
    data?: AggregationResult[];
    trendSeries?: TrendSeries[];
    viewId?: string;
    isTopN?: boolean;
    chartType?: string;
    rawData?: AggregationResult[];
    rawTrendSeries?: TrendSeries[];
  }>({ open: false, title: '', type: 'chart' });

  // Sidebar State
  const [sidebarWidth, setSidebarWidth] = useState(280);
  const [isSidebarCollapsed, setIsSidebarCollapsed] = useState(false);

  // Fetch Config
  useEffect(() => {
    const loadConfig = async () => {
      const config = await import('@/services/data').then(m => m.fetchConfig());
      setAppConfig(config);

      // Use unified project list from context for determining available names
      const allProjectNames = projectList.map(p => p.name);
      const defaultProject = getDefaultProject(config);
      if (defaultProject) {
        // If currentProject is not in available projects, use the first project
        if (!allProjectNames.includes(currentProject)) {
          setProject(allProjectNames[0] || defaultProject.name);
        }

        const projectConfig = config.projects.find(p => p.name === currentProject) || defaultProject;
        setViewsConfig(projectConfig.views);
        setAvailableTechStacks(projectConfig.tech_stacks);

        // Ensure "Summary" is selected if selection invalid
        const stackNames = projectConfig.tech_stacks.map(s => s.name);
        if (!['Summary', ...stackNames].includes(selectedTechStack)) {
          setSelectedTechStack('Summary');
        }
      } else if (projectList.length > 0) {
        // No config but we have projects — use the first one
        setProject(projectList[0].name);
        setViewsConfig([]);
        setAvailableTechStacks([]);
      }
    };
    loadConfig();
  }, [currentProject, configVersion]);

  // Update config when currentProject changes
  useEffect(() => {
    if (appConfig) {
      const projectConfig = appConfig.projects.find(p => p.name === currentProject);
      if (projectConfig) {
        setViewsConfig(projectConfig.views);
        setAvailableTechStacks(projectConfig.tech_stacks);
        setSelectedTechStack('Summary'); // Reset to Summary on project change
        setViewDataMap({}); // Clear old data
      } else if (projectList.some(p => p.name === currentProject && !p.has_config)) {
        // DB-only project (no config) — show empty views
        setViewsConfig([]);
        setAvailableTechStacks([]);
        setSelectedTechStack('Summary');
        setViewDataMap({});
      }
      // Note: selectedRunId is handled by the fetchRuns effect
    }
  }, [currentProject, appConfig, projectList]);

  // Filter Active Views based on selection
  useEffect(() => {
    // Helper: Check if a view is a "global" view (should appear on Summary tab)
    const isGlobalView = (v: any) =>
      !v.tech_stacks || v.tech_stacks.length === 0 || v.tech_stacks.includes('All');

    const filtered = selectedTechStack === 'Summary'
      ? viewsConfig.filter(isGlobalView)
      : viewsConfig.filter(v => v.tech_stacks && v.tech_stacks.includes(selectedTechStack));

    // All views stay in activeViews; trend mode is controlled by trendActive state
    setActiveViews(filtered);
    setTrendViews(filtered.filter((v: any) => v.trend));
    // Auto-activate trend mode for views configured with trend: true
    setTrendActive(prev => {
      const next = { ...prev };
      filtered.filter((v: any) => v.trend).forEach((v: any) => { next[v.id] = true; });
      return next;
    });
  }, [selectedTechStack, viewsConfig]);

  // Clean up changeTypeFilters for views that are no longer active
  useEffect(() => {
    setChangeTypeFilters(prev => {
      const activeIds = new Set(activeViews.map(v => v.id));
      const next = Object.fromEntries(
        Object.entries(prev).filter(([id]) => activeIds.has(id))
      );
      // Avoid re-render if nothing changed
      if (Object.keys(next).length === Object.keys(prev).length) return prev;
      return next;
    });
  }, [activeViews]);

  // Fetch Runs - depends on selected project and view mode
  useEffect(() => {
    if (!currentProject) return;

    let isActive = true;

    const loadRuns = async () => {
      const data = await import('@/services/data').then(m => m.fetchRuns(currentProject, viewMode === 'snapshot' ? 'SNAPSHOT' : 'DIFF'));

      if (!isActive) return;

      setRuns(data.map(r => ({
        id: r.id,
        hash: r.commit_hash.substring(0, 7),
        date: r.scan_time,
        scan_mode: r.scan_mode,
        commit_timestamp: r.commit_timestamp,
        base_commit_hash: r.base_commit_hash,
      })));

      // Always select the first (latest) run when mode changes or on initial load
      if (data.length > 0) {
        setSelectedRunId(data[0].id);
      } else {
        setSelectedRunId(null);
      }
    };
    loadRuns();

    return () => {
      isActive = false;
    };
  }, [viewMode, currentProject]);

  // Fetch View Data - Initial load and global changes (run, tech stack, active views)
  useEffect(() => {
    if (!selectedRunId || activeViews.length === 0) return;

    let isActive = true;

    const load = async () => {
      setLoading(true);
      try {
        // Pass tech_stack filter when not on Summary
        const techStackFilter = selectedTechStack !== 'Summary' ? selectedTechStack : undefined;

        const promises = activeViews.map(view => {
          const options: { techStack?: string; changeType?: string; groupBy?: string } = {};
          if (techStackFilter) options.techStack = techStackFilter;

          // For "all" mode: add change_type to group_by
          if (view.change_type_mode === 'all') {
            const existingGroupBy = view.group_by?.join(',') || '';
            options.groupBy = existingGroupBy ? `${existingGroupBy},change_type` : 'change_type';
          }

          // For "switchable" mode: pass the selected change_type filter
          if (view.change_type_mode === 'switchable') {
            const currentFilter = changeTypeFilters[view.id] || 'A';
            options.changeType = currentFilter;
          }

          return fetchView(currentProject, selectedRunId, view.id, options);
        });

        const results = await Promise.all(promises);

        if (!isActive) return;

        const newMap: Record<string, AggregationResult[]> = {};
        results.forEach((res, index) => {
          newMap[activeViews[index].id] = res.items;
        });
        setViewDataMap(newMap);
      } catch (e) {
        console.error(e);
      } finally {
        if (isActive) {
          setLoading(false);
        }
      }
    };
    load();

    return () => {
      isActive = false;
    };
    // Note: changeTypeFilters is intentionally excluded from dependencies
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentProject, selectedRunId, activeViews, selectedTechStack]);

  // Fetch scan summary when a run is selected
  useEffect(() => {
    if (!selectedRunId || !currentProject) {
      setScanSummary(null);
      return;
    }

    let isActive = true;

    const loadSummary = async () => {
      const summary = await fetchScanSummary(currentProject, selectedRunId);
      if (isActive) {
        setScanSummary(summary);
      }
    };
    loadSummary();

    return () => { isActive = false; };
  }, [currentProject, selectedRunId]);

  // Track per-view trend fetch params to avoid re-fetching unchanged views
  const prevTrendParamsRef = useRef<Record<string, string>>({});

  // Fetch Trend Data for all views in trend mode
  useEffect(() => {
    const activeTrendViewIds = Object.entries(trendActive)
      .filter(([, isActive]) => isActive)
      .map(([id]) => id);

    if (activeTrendViewIds.length === 0) return;

    let isActive = true;

    const loadTrends = async () => {
      const newMap: Record<string, TrendSeries[]> = { ...trendDataMap };

      for (const viewId of activeTrendViewIds) {
        const view = [...activeViews, ...trendViews].find(v => v.id === viewId);
        if (!view) continue;

        // Compute effective params for this view to detect real changes
        const paramStr = [
          currentProject,
          viewMode,
          selectedRunId,
          selectedTechStack,
          trendTimeRange,
          changeTypeFilters[viewId] || '',
          trendCustomScanIds[viewId]?.join(',') || '',
        ].join('::');

        if (prevTrendParamsRef.current[viewId] === paramStr) {
          continue; // Skip fetch — this view's params haven't changed
        }

        setTrendLoadingMap(prev => ({ ...prev, [viewId]: true }));

        try {
          const options: any = {};
          if (trendCustomScanIds[viewId] && trendCustomScanIds[viewId].length > 0) {
            options.scanIds = trendCustomScanIds[viewId];
          } else {
            // Time-range based auto-selection: fetch scans and filter by time
            const now = Math.floor(Date.now() / 1000);
            const days = TIME_RANGE_PRESETS[trendTimeRange] || 90;
            const fromTs = now - days * 86400;

            const { fetchRuns } = await import('@/services/data');
            const scanMode = viewMode === 'snapshot' ? 'SNAPSHOT' : 'DIFF';
            const allScans = await fetchRuns(currentProject, scanMode);

            // Filter scans within the time range
            const rangeScans = allScans.filter(s => {
              const ts = s.commit_timestamp || 0;
              return ts >= fromTs && ts <= now;
            });

            // Baseline: scan immediately before fromTs
            const beforeScans = allScans.filter(s => {
              const ts = s.commit_timestamp || 0;
              return ts < fromTs;
            });
            beforeScans.sort((a, b) => (b.commit_timestamp || 0) - (a.commit_timestamp || 0));

            const scanIds = rangeScans.map(s => Number(s.id));
            if (beforeScans.length > 0) {
              const baselineId = Number(beforeScans[0].id);
              if (!scanIds.includes(baselineId)) {
                scanIds.unshift(baselineId);
              }
            }

            if (scanIds.length > 0) {
              options.scanIds = scanIds;
            }

            // For diff mode, also filter by base commit
            if (viewMode === 'diff') {
              const currentRun = runs.find(r => r.id === selectedRunId);
              if (currentRun?.base_commit_hash) {
                options.baseCommit = currentRun.base_commit_hash;
              }
            }
          }
          // Pass tech_stack filter so trend data respects current tab context
          if (selectedTechStack !== 'Summary') {
            options.techStack = selectedTechStack;
          }
          // Pass change_type filter for switchable mode
          if (view.change_type_mode === 'switchable') {
            options.changeType = changeTypeFilters[viewId] || 'A';
          }
          const result = await fetchTrend(currentProject, viewId, options);
          if (!isActive) return;
          newMap[viewId] = result.series;
          prevTrendParamsRef.current[viewId] = paramStr; // Record successful fetch params
        } catch (e) {
          console.error(`Error fetching trend for ${viewId}:`, e);
        } finally {
          if (isActive) setTrendLoadingMap(prev => ({ ...prev, [viewId]: false }));
        }
      }

      if (isActive) setTrendDataMap(newMap);
    };

    loadTrends();

    return () => { isActive = false; };
  }, [currentProject, viewMode, selectedRunId, trendActive, trendCustomScanIds, activeViews, trendViews, runs, changeTypeFilters, trendTimeRange]);

  // Track previous change_type filters using ref to avoid re-render cycles
  const prevChangeTypeFilters = useRef<Record<string, string>>({});

  useEffect(() => {
    if (!selectedRunId || activeViews.length === 0) return;

    // Find which view's filter changed
    const changedViewId = Object.keys(changeTypeFilters).find(
      viewId => changeTypeFilters[viewId] !== prevChangeTypeFilters.current[viewId]
    );

    if (!changedViewId) return;

    // Update ref immediately — no re-render triggered
    prevChangeTypeFilters.current = changeTypeFilters;

    const view = activeViews.find(v => v.id === changedViewId);
    if (!view || view.change_type_mode !== 'switchable') return;

    let isActive = true;

    // Fetch only the changed view
    const fetchSingleView = async () => {
      try {
        const techStackFilter = selectedTechStack !== 'Summary' ? selectedTechStack : undefined;
        const options: { techStack?: string; changeType?: string } = {};
        if (techStackFilter) options.techStack = techStackFilter;
        options.changeType = changeTypeFilters[changedViewId] || 'A';

        const result = await fetchView(currentProject, selectedRunId, changedViewId, options);

        if (!isActive) return;

        setViewDataMap(prev => ({
          ...prev,
          [changedViewId]: result.items
        }));
      } catch (e) {
        console.error(e);
      }
    };
    fetchSingleView();

    return () => {
      isActive = false;
    };
  }, [changeTypeFilters, selectedRunId, activeViews, selectedTechStack]);

  // --- Chart Option Generators ---

  const textColor = theme === 'dark' ? '#94a3b8' : '#64748b';
  const splitLineColor = theme === 'dark' ? '#334155' : '#e2e8f0';
  const labelColor = theme === 'dark' ? '#f8fafc' : '#1e293b';

  // 1. Horizontal Bar (Row)
  const getBarRowOption = (_title: string, data: AggregationResult[], color: string) => ({
    tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
    xAxis: {
      type: 'value',
      splitLine: { show: true, lineStyle: { color: splitLineColor } },
      axisLabel: { color: textColor }
    },
    yAxis: {
      type: 'category',
      data: data.map(d => d.label.split('/').pop()),
      axisLabel: {
        width: 120,
        overflow: 'truncate',
        interval: 0,
        color: textColor
      },
      inverse: true
    },
    series: [{
      data: data.map(d => Math.round(d.value)),
      type: 'bar',
      itemStyle: { color: color, borderRadius: [0, 4, 4, 0] },
      label: { show: true, position: 'right', color: labelColor, formatter: (params: { value: number }) => Math.round(params.value).toLocaleString() }
    }],
    grid: { left: 10, right: 40, top: 10, bottom: 20, containLabel: true }
  });

  // 2. Vertical Bar (Col)
  const getBarColOption = (_title: string, data: AggregationResult[], color: string) => ({
    tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
    xAxis: {
      type: 'category',
      data: data.map(d => d.label.split('/').pop()),
      axisLabel: {
        color: textColor,
        rotate: 30, // Rotate labels if many
        interval: 0
      }
    },
    yAxis: {
      type: 'value',
      splitLine: { show: true, lineStyle: { color: splitLineColor } },
      axisLabel: { color: textColor }
    },
    series: [{
      data: data.map(d => Math.round(d.value)),
      type: 'bar',
      itemStyle: { color: color, borderRadius: [4, 4, 0, 0] },
      label: { show: true, position: 'top', color: labelColor, formatter: (params: { value: number }) => Math.round(params.value).toLocaleString() }
    }],
    grid: { left: 10, right: 10, top: 30, bottom: 10, containLabel: true }
  });

  // 3. Pie Chart - optimized layout with minimal whitespace
  const getPieOption = (title: string, data: AggregationResult[]) => {
    const itemCount = data.length;

    return {
      tooltip: {
        trigger: 'item',
        formatter: (params: { name: string; value: number; percent: number }) =>
          `${params.name}: ${Math.round(params.value).toLocaleString()} (${params.percent.toFixed(1)}%)`
      },
      legend: {
        type: itemCount > 6 ? 'scroll' : 'plain',
        orient: 'horizontal',
        bottom: 0,
        left: 'center',
        textStyle: { color: textColor, fontSize: 11 },
        itemWidth: 12,
        itemHeight: 12,
        itemGap: 8,
        formatter: (name: string) => name.length > 24 ? name.substring(0, 24) + '..' : name
      },
      series: [
        {
          name: title,
          type: 'pie',
          radius: ['30%', '70%'],  // Larger pie, less whitespace
          center: ['50%', '45%'],  // Slightly above center to make room for legend
          avoidLabelOverlap: true,
          itemStyle: {
            borderRadius: 6,
            borderColor: theme === 'dark' ? '#1e293b' : '#fff',
            borderWidth: 2
          },
          label: {
            show: true,
            position: 'inside',  // Labels inside the slices
            formatter: (params: { value: number; percent: number }) =>
              params.percent > 5 ? Math.round(params.value).toLocaleString() : '',  // Only show if slice is big enough
            color: '#fff',
            fontSize: 11,
            fontWeight: 'bold',
            textShadowColor: 'rgba(0,0,0,0.5)',
            textShadowBlur: 2
          },
          emphasis: {
            label: {
              show: true,
              fontSize: 13,
              fontWeight: 'bold',
              formatter: (params: { name: string; value: number; percent: number }) =>
                `${Math.round(params.value).toLocaleString()}`
            },
            itemStyle: {
              shadowBlur: 10,
              shadowOffsetX: 0,
              shadowColor: 'rgba(0, 0, 0, 0.5)'
            }
          },
          labelLine: { show: false },  // No label lines needed for inside labels
          data: [...data]
            .sort((a, b) => a.label.localeCompare(b.label))
            .map(d => ({ value: Math.round(d.value), name: d.label }))
        }
      ]
    };
  };

  // 4. Line Chart
  const getLineOption = (_title: string, data: AggregationResult[], color: string) => ({
    tooltip: { trigger: 'axis' },
    xAxis: {
      type: 'category',
      data: data.map(d => d.label),
      axisLabel: { color: textColor }
    },
    yAxis: {
      type: 'value',
      splitLine: { show: true, lineStyle: { color: splitLineColor } },
      axisLabel: { color: textColor }
    },
    series: [{
      data: data.map(d => Math.round(d.value)),
      type: 'line',
      smooth: false, // Point to point
      symbol: 'circle',
      symbolSize: 8,
      itemStyle: { color: color },
      lineStyle: { width: 3 },
      label: { show: true, position: 'top', color: labelColor, formatter: (params: { value: number }) => Math.round(params.value).toLocaleString() }
    }],
    grid: { left: 10, right: 20, top: 30, bottom: 20, containLabel: true }
  });

  // 5. Stacked Bar Chart
  const getStackedBarOption = (_title: string, data: AggregationResult[]) => {
    // Group data by category for stacking
    const categories = [...new Set(data.map(d => d.category || 'Unknown'))];
    const labels = [...new Set(data.map(d => d.label))];
    const colors = ['#38bdf8', '#22c55e', '#f59e0b', '#ef4444', '#a855f7', '#06b6d4'];

    return {
      tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
      legend: {
        data: categories,
        textStyle: { color: textColor },
        top: 0
      },
      xAxis: {
        type: 'category',
        data: labels,
        axisLabel: { color: textColor, rotate: 30 }
      },
      yAxis: {
        type: 'value',
        splitLine: { show: true, lineStyle: { color: splitLineColor } },
        axisLabel: { color: textColor }
      },
      series: categories.map((cat, i) => ({
        name: cat,
        type: 'bar',
        stack: 'total',
        emphasis: { focus: 'series' },
        itemStyle: { color: colors[i % colors.length] },
        data: labels.map(label => {
          const item = data.find(d => d.label === label && (d.category || 'Unknown') === cat);
          return item ? Math.round(item.value) : 0;
        })
      })),
      grid: { left: 10, right: 10, top: 40, bottom: 10, containLabel: true }
    };
  };

  // 6. Heatmap Chart
  const getHeatmapOption = (_title: string, data: AggregationResult[]) => {
    const xLabels = [...new Set(data.map(d => d.tech_stack || 'Unknown'))];
    const yLabels = [...new Set(data.map(d => d.category || 'Unknown'))];
    const maxVal = Math.max(...data.map(d => d.value), 1);

    const heatmapData = data.map(d => {
      const x = xLabels.indexOf(d.tech_stack || 'Unknown');
      const y = yLabels.indexOf(d.category || 'Unknown');
      return [x, y, Math.round(d.value)];
    });

    return {
      tooltip: {
        position: 'top',
        formatter: (params: { data: number[] }) => {
          const [x, y, val] = params.data;
          return `${xLabels[x]} / ${yLabels[y]}: ${Math.round(val).toLocaleString()}`;
        }
      },
      xAxis: {
        type: 'category',
        data: xLabels,
        axisLabel: { color: textColor },
        splitArea: { show: true }
      },
      yAxis: {
        type: 'category',
        data: yLabels,
        axisLabel: { color: textColor },
        splitArea: { show: true }
      },
      visualMap: {
        min: 0,
        max: maxVal,
        calculable: true,
        orient: 'horizontal',
        left: 'center',
        bottom: 0,
        inRange: {
          color: ['#1e3a5f', '#38bdf8', '#22c55e', '#f59e0b', '#ef4444']
        },
        textStyle: { color: textColor }
      },
      series: [{
        type: 'heatmap',
        data: heatmapData,
        label: { show: true, color: '#f8fafc' }, // Keep logic contrast label usually needed on heatmap
        emphasis: {
          itemStyle: { shadowBlur: 10, shadowColor: 'rgba(0, 0, 0, 0.5)' }
        }
      }],
      grid: { left: 10, right: 10, top: 10, bottom: 60, containLabel: true }
    };
  };

  // 7. Radar Chart
  const getRadarOption = (title: string, data: AggregationResult[]) => {
    const maxVal = Math.max(...data.map(d => d.value), 1);
    const indicators = data.map(d => ({ name: d.label, max: maxVal * 1.2 }));

    return {
      tooltip: { trigger: 'item' },
      legend: {
        data: [title],
        bottom: 0,
        textStyle: { color: textColor }
      },
      radar: {
        indicator: indicators,
        shape: 'polygon',
        splitNumber: 5,
        axisName: { color: textColor },
        splitLine: { lineStyle: { color: splitLineColor } },
        splitArea: { areaStyle: { color: theme === 'dark' ? ['rgba(56, 189, 248, 0.05)', 'rgba(56, 189, 248, 0.1)'] : ['rgba(56, 189, 248, 0.05)', 'rgba(56, 189, 248, 0.1)'] } }, // Light tint is fine for both
        axisLine: { lineStyle: { color: theme === 'dark' ? '#475569' : '#cbd5e1' } }
      },
      series: [{
        name: title,
        type: 'radar',
        data: [{
          value: data.map(d => Math.round(d.value)),
          name: title,
          areaStyle: { color: 'rgba(56, 189, 248, 0.3)' },
          lineStyle: { color: '#38bdf8', width: 2 },
          itemStyle: { color: '#38bdf8' }
        }]
      }]
    };
  };

  // 8. Gauge Chart
  const getGaugeOption = (title: string, data: AggregationResult[]) => {
    const value = data.length > 0 ? data[0].value : 0;
    const maxVal = Math.max(value * 1.5, 100);

    return {
      tooltip: { formatter: `${title}: {c}` },
      series: [{
        name: title,
        type: 'gauge',
        min: 0,
        max: maxVal,
        splitNumber: 10,
        radius: '85%',
        axisLine: {
          lineStyle: {
            width: 20,
            color: [
              [0.3, '#22c55e'],
              [0.7, '#f59e0b'],
              [1, '#ef4444']
            ]
          }
        },
        pointer: {
          itemStyle: { color: 'auto' },
          width: 5
        },
        axisTick: { distance: -20, length: 8, lineStyle: { color: '#fff', width: 2 } },
        splitLine: { distance: -25, length: 20, lineStyle: { color: '#fff', width: 3 } },
        axisLabel: { color: textColor, distance: 30, fontSize: 12 },
        detail: {
          valueAnimation: true,
          formatter: '{value}',
          color: labelColor,
          fontSize: 24,
          offsetCenter: [0, '70%']
        },
        title: {
          offsetCenter: [0, '90%'],
          color: textColor,
          fontSize: 14
        },
        data: [{ value: Math.round(value), name: title }]
      }]
    };
  };

  // ─── Children Viewer Helpers ──────────────────────────────────────

  const getItemField = (item: AggregationResult, field: string): string | undefined => {
    switch (field) {
      case 'tech_stack': return item.tech_stack;
      case 'category': return item.category;
      case 'metric_key': return item.metric_key;
      case 'analyzer_id': return item.analyzer_id;
      case 'change_type': return item.change_type;
      case 'extension': {
        const dotIdx = item.label.lastIndexOf('.');
        return dotIdx > 0 ? item.label.substring(dotIdx + 1) : '';
      }
      default: return undefined;
    }
  };

  const collectLeafItems = (items: AggregationResult[], groupByFields: string[]): LeafItem[] => {
    const result: LeafItem[] = [];
    for (const item of items) {
      if (item.children && item.children.length > 0) {
        result.push(...collectLeafItems(item.children, groupByFields));
      } else {
        // Build group label from effective group_by fields
        const groupParts: string[] = [];
        let changeType: string | undefined;
        for (const field of groupByFields) {
          if (field === 'change_type') {
            changeType = item.change_type;
          } else {
            const val = getItemField(item, field);
            if (val) groupParts.push(val);
          }
        }
        const groupLabel = groupParts.join(':');
        const group = changeType && groupLabel
          ? `${groupLabel}(${changeType})`
          : changeType
            ? `(${changeType})`
            : groupLabel || item.group_key || undefined;

        result.push({ label: item.label, value: Math.round(item.value), group, analyzerId: item.analyzer_id });
      }
    }
    return result;
  };

  const openChildrenView = (view: any, viewTitle: string) => {
    const rawData = viewDataMap[view.id] || [];
    const groupByFields: string[] = [];
    if (view.group_by) {
      groupByFields.push(...view.group_by);
    }
    if (view.change_type_mode === 'all') {
      groupByFields.push('change_type');
    }
    const items = collectLeafItems(rawData, groupByFields);
    if (items.length > 0) {
      setChildrenView({ open: true, title: viewTitle, items });
    }
  };

  const handleFileClick = async (filePath: string, viewTitle: string, analyzerId?: string, side?: boolean) => {
    if (!selectedRunId) return;
    setMatchDetailView({ open: true, title: viewTitle, filePath, analyzerId, matches: [], total: 0, loading: true, side });
    try {
      const params: { file_path: string; analyzer_id?: string; side?: number } = { file_path: filePath };
      if (analyzerId) params.analyzer_id = analyzerId;
      if (side !== undefined) params.side = side ? 1 : 0;
      const res = await fetchMatches(currentProject, selectedRunId, params);
      setMatchDetailView(prev => ({ ...prev, matches: res.matches, total: res.total, loading: false }));
    } catch (err) {
      console.error('Failed to fetch matches:', err);
      setMatchDetailView(prev => ({ ...prev, loading: false }));
    }
  };

  const closeMatchDetail = () => {
    setMatchDetailView(prev => ({ ...prev, open: false }));
  };

  const backToFileList = () => {
    setMatchDetailView(prev => ({ ...prev, open: false }));
  };

  // Close fullscreen on Escape key
  useEffect(() => {
    if (!fullscreenView.open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setFullscreenView({ open: false, title: '', type: 'chart' });
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [fullscreenView.open]);


  return (
    <div className="flex h-full overflow-hidden bg-slate-50 dark:bg-slate-900 text-slate-900 dark:text-slate-100 transition-colors">
      <div className="flex flex-1 overflow-hidden">
        <Sidebar
          runs={runs}
          selectedRunId={selectedRunId}
          onRunSelect={setSelectedRunId}
          width={sidebarWidth}
          isCollapsed={isSidebarCollapsed}
          onWidthChange={setSidebarWidth}
          onToggleCollapse={() => setIsSidebarCollapsed(!isSidebarCollapsed)}
        />

        <main className="flex-1 overflow-y-scroll relative scrollbar-stable">
          {/* Background gradient effect */}
          <div className="absolute inset-0 bg-gradient-to-br from-white via-slate-50 to-blue-50/50 dark:from-slate-900 dark:via-slate-900 dark:to-blue-950/20 pointer-events-none -z-10" />

          <div className="max-w-7xl mx-auto p-3 sm:p-4 lg:p-5 space-y-4 min-h-full">

            <TechStackTabs
              techStacks={availableTechStacks}
              selectedStack={selectedTechStack}
              onSelect={setSelectedTechStack}
            />

            {/* Dynamic Widgets Grid */}
            <div key={`${selectedTechStack}-${theme}`} className="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-4 gap-6">
              {activeViews.length === 0 ? (
                <div style={{ gridColumn: '1 / -1' }} className="flex flex-col items-center justify-center py-16 text-slate-400">
                  <div className="text-6xl mb-4 opacity-30">📊</div>
                  <p className="text-lg font-medium text-slate-500 dark:text-slate-400 mb-2">
                    {t('dashboard.noViewsTitle')}
                  </p>
                  <p className="text-sm text-slate-400 dark:text-slate-500 mb-6 max-w-md text-center">
                    {t('dashboard.noViewsDesc')}
                  </p>
                  <button
                    onClick={() => navigateTo('execute')}
                    className="px-4 py-2 bg-sky-600 text-white rounded-lg text-sm font-medium hover:bg-sky-700 transition-colors"
                  >
                    {t('dashboard.goToExecute')}
                  </button>
                </div>
              ) : activeViews.map(view => {
                let data = viewDataMap[view.id] || [];
                const title = view.title || view.id.replace(/_/g, ' ').toUpperCase();

                // For "all" mode: recursively flatten hierarchical data
                // (e.g., tech_stack -> change_type -> individual entries) into flat labels
                // like "Rust(A)", "Rust:compiler(A)", etc.
                // We know effective group_by = view.group_by + ["change_type"],
                // so the meaningful grouping depth = (group_by.length || 0) + 1.
                // Individual entries below the change_type level are summed up.
                if (view.change_type_mode === 'all' && data.length > 0 && data[0].children) {
                  const groupDepth = (view.group_by?.length || 0) + 1; // +1 for change_type
                  const flattenByDepth = (items: AggregationResult[], prefix = '', depth = 0): AggregationResult[] => {
                    const result: AggregationResult[] = [];
                    for (const item of items) {
                      if (item.children && item.children.length > 0 && depth < groupDepth - 1) {
                        // Intermediate group level — recurse deeper, accumulate path
                        const newPrefix = prefix ? `${prefix}:${item.label}` : item.label;
                        result.push(...flattenByDepth(item.children, newPrefix, depth + 1));
                      } else {
                        // Change_type level (or below) — aggregate values into a flat result
                        const totalValue = item.children
                          ? item.children.reduce((sum, c) => sum + (c.value || 0), 0)
                          : (item.value || 0);
                        result.push({
                          ...item,
                          label: prefix ? `${prefix}(${item.label})` : item.label,
                          value: totalValue,
                          children: undefined,
                        });
                      }
                    }
                    return result;
                  };
                  data = flattenByDepth(data);
                } else if (view.group_by && view.group_by.length > 1 && data.length > 0 && data[0].children) {
                  // Multi-level grouping without "all" mode (e.g., tech_stack -> extension):
                  // flatten intermediate group levels so all grouping layers are visible in the chart.
                  const groupDepth = view.group_by.length;
                  const flattenMultiLevel = (items: AggregationResult[], prefix = '', depth = 0): AggregationResult[] => {
                    const result: AggregationResult[] = [];
                    for (const item of items) {
                      if (item.children && item.children.length > 0 && depth < groupDepth - 1) {
                        const newPrefix = prefix ? `${prefix}/${item.label}` : item.label;
                        result.push(...flattenMultiLevel(item.children, newPrefix, depth + 1));
                      } else {
                        const totalValue = item.children
                          ? item.children.reduce((sum, c) => sum + (c.value || 0), 0)
                          : (item.value || 0);
                        result.push({
                          ...item,
                          label: prefix ? `${prefix}/${item.label}` : item.label,
                          value: totalValue,
                          children: undefined,
                        });
                      }
                    }
                    return result;
                  };
                  data = flattenMultiLevel(data);
                }

                // Save unlimited data for fullscreen modal TopN
                const preLimitData = data;

                // Apply frontend TopN limit (client-side slicing)
                if (view.type === 'top_n') {
                  const limit = topnLimits[view.id] || 10;
                  if (limit > 0 && data.length > limit) {
                    data = data.slice(0, limit);
                  }
                }

                // Apply TopN limit to trend data too, consistent with snapshot behavior
                const rawTrendSeries = trendDataMap[view.id] || [];
                const trendLimit = view.type === 'top_n' ? (topnLimits[view.id] || 10) : 0;
                const displayedTrendSeries = trendLimit > 0 && rawTrendSeries.length > trendLimit
                  ? rawTrendSeries.slice(0, trendLimit)
                  : rawTrendSeries;

                // Compute fixed x-axis bounds for trend chart, extending to cover earliest data
                const nowMs = Date.now();
                const trendDays = TIME_RANGE_PRESETS[trendTimeRange] || 90;
                let trendXAxisMin = nowMs - trendDays * 86400 * 1000;
                const trendXAxisMax = nowMs;
                if (displayedTrendSeries.length > 0) {
                  const allTs = displayedTrendSeries.flatMap(s => s.data.map(d => d.timestamp * 1000));
                  if (allTs.length > 0) {
                    const earliest = Math.min(...allTs);
                    if (earliest < trendXAxisMin) {
                      trendXAxisMin = earliest;
                    }
                  }
                }

                // Determine Chart Type
                let content;
                const chartType = view.chart_type || (view.type === 'top_n' ? 'bar_row' : 'card');

                // Special handling for legacy Sum fallback logic (grouped Sum -> Pie)
                let actualChartType = chartType;
                if (view.type === 'sum' && !view.chart_type) {
                  if (view.group_by && view.group_by.length > 0) {
                    actualChartType = 'pie';
                  } else {
                    actualChartType = 'card';
                  }
                }

                // Setup expand handler for fullscreen
                const expandView = () => {
                  if (actualChartType === 'card') {
                    const totalValue = data.reduce((acc, curr) => acc + curr.value, 0);
                    setFullscreenView({ open: true, title, type: 'card', value: totalValue.toLocaleString() });
                  } else if (actualChartType === 'table') {
                    setFullscreenView({ open: true, title, type: 'table', data });
                  } else {
                    const baseColor = view.id.includes('complexity') ? '#ef4444' : '#38bdf8';
                    let opt;
                    switch (actualChartType) {
                      case 'bar_row':
                      case 'bar_horizontal':
                        opt = getBarRowOption(title, data, baseColor);
                        break;
                      case 'bar_col':
                      case 'bar_vertical':
                        opt = getBarColOption(title, data, baseColor);
                        break;
                      case 'pie':
                        opt = getPieOption(title, data);
                        break;
                      case 'line':
                        opt = getLineOption(title, data, baseColor);
                        break;
                      case 'stacked_bar':
                        opt = getStackedBarOption(title, data);
                        break;
                      case 'heatmap':
                        opt = getHeatmapOption(title, data);
                        break;
                      case 'radar':
                        opt = getRadarOption(title, data);
                        break;
                      case 'gauge':
                        opt = getGaugeOption(title, data);
                        break;
                      default:
                        opt = getBarRowOption(title, data, baseColor);
                    }
                    setFullscreenView({ open: true, title, type: 'chart', options: opt,
                      viewId: view.id, isTopN: view.type === 'top_n',
                      chartType: actualChartType,
                      rawData: view.type === 'top_n' ? preLimitData : undefined });
                  }
                };

                // Trend expand handler
                const trendExpandView = () => {
                  if (displayedTrendSeries.length > 0) {
                    setFullscreenView({ open: true, title, type: 'trend', trendSeries: displayedTrendSeries,
                      viewId: view.id, isTopN: view.type === 'top_n',
                      rawTrendSeries: view.type === 'top_n' ? rawTrendSeries : undefined });
                  }
                };

                // Show empty state when data is empty and not loading
                if (data.length === 0 && !loading) {
                  content = (
                    <div className="flex flex-col items-center justify-center h-[300px] text-slate-400 dark:text-slate-500">
                      <p className="text-sm">{t('dashboard.noData') || 'No data available'}</p>
                    </div>
                  );
                } else {
                  if (actualChartType === 'card') {
                    // Metric Card
                    const totalValue = data.reduce((acc, curr) => acc + curr.value, 0);
                    content = (
                      <MetricCard
                        key={view.id}
                        title={title}
                        value={totalValue.toLocaleString()}
                        subValue={t('dashboard.totalValue')}
                        loading={loading}
                      />
                    );
                  } else if (actualChartType === 'table') {
                    // Simple Table — sort by label alphabetically
                    const sortedData = [...data].sort((a, b) => a.label.localeCompare(b.label));
                    content = (
                      <div className="overflow-x-auto">
                        <table className="w-full text-sm text-left text-slate-600 dark:text-slate-300">
                          <thead className="text-xs text-slate-500 dark:text-slate-400 uppercase bg-slate-100 dark:bg-slate-800/50">
                            <tr>
                              <th className="px-4 py-2">{t('table.label')}</th>
                              <th className="px-4 py-2 text-right">{t('table.value')}</th>
                            </tr>
                          </thead>
                          <tbody>
                            {sortedData.map((d, i) => (
                              <tr key={i} className="border-b border-slate-200 dark:border-slate-700/50 hover:bg-slate-50 dark:hover:bg-slate-700/20">
                                <td className="px-4 py-2 font-medium">{d.label}</td>
                                <td className="px-4 py-2 text-right">{Math.round(d.value).toLocaleString()}</td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    );
                  } else {
                    // ECharts based types
                    let options;
                    const baseColor = view.id.includes('complexity') ? '#ef4444' : '#38bdf8'; // Red for complexity, Blue default

                    switch (actualChartType) {
                      case 'bar_row':
                      case 'bar_horizontal': // Alias
                        options = getBarRowOption(title, data, baseColor);
                        break;
                      case 'bar_col':
                      case 'bar_vertical': // Alias
                        options = getBarColOption(title, data, baseColor);
                        break;
                      case 'pie':
                        options = getPieOption(title, data);
                        break;
                      case 'line':
                        options = getLineOption(title, data, baseColor);
                        break;
                      case 'stacked_bar':
                        options = getStackedBarOption(title, data);
                        break;
                      case 'heatmap':
                        options = getHeatmapOption(title, data);
                        break;
                      case 'radar':
                        options = getRadarOption(title, data);
                        break;
                      case 'gauge':
                        options = getGaugeOption(title, data);
                        break;
                      default:
                        options = getBarRowOption(title, data, baseColor);
                    }
                    content = <ChartRenderer options={options} theme={theme} />;
                  }
                }
                const changeTypeMode = view.change_type_mode;
                const currentFilter = changeTypeFilters[view.id] || 'A'; // Default to Add

                return (
                  <Card key={view.id} className="border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800/50 backdrop-blur shadow-sm dark:shadow-xl transition-colors duration-200" style={{ gridColumn: `span ${Math.min(view.width || 1, 4)}` }}>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2 border-b border-slate-100 dark:border-slate-700/50">
                      <CardTitle className="text-xl font-semibold text-slate-800 dark:text-slate-200">
                        {title}
                      </CardTitle>
                      <div className="flex items-center gap-2">
                        {/* A/M/D Toggle for switchable mode */}
                        {changeTypeMode === 'switchable' && (
                          <div className="flex gap-1 mr-2">
                            {['A', 'M', 'D'].map(type => (
                              <button
                                key={type}
                                onClick={() => setChangeTypeFilters(prev => ({ ...prev, [view.id]: type }))}
                                className={`px-2 py-1 text-xs font-bold rounded transition-all ${currentFilter === type
                                  ? type === 'A' ? 'bg-green-500/80 text-white'
                                    : type === 'M' ? 'bg-blue-500/80 text-white'
                                      : 'bg-red-500/80 text-white'
                                  : 'bg-slate-100 dark:bg-slate-700 text-slate-500 dark:text-slate-400 hover:bg-slate-200 dark:hover:bg-slate-600'
                                  }`}
                              >
                                {type}
                              </button>
                            ))}
                          </div>
                        )}
                        {trendActive[view.id] ? (
                          <>
                            {displayedTrendSeries.length > 0 && (
                              <button
                                onClick={trendExpandView}
                                className="p-1.5 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-700 text-slate-400 hover:text-sky-500 transition-colors"
                                title={t('dashboard.expand')}
                              >
                                <Maximize2 className="h-5 w-5" />
                              </button>
                            )}
                            <button
                              onClick={() => setTrendActive(prev => ({ ...prev, [view.id]: false }))}
                              className="p-1.5 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-700 text-slate-400 hover:text-sky-500 transition-colors"
                              title={t('trend.viewCurrent')}
                            >
                              <BarChart3 className="h-5 w-5" />
                            </button>
                            <button
                              onClick={() => {
                                const currentRun = runs.find(r => r.id === selectedRunId);
                                setScanSelectorView({
                                  open: true,
                                  projectName: currentProject,
                                  viewId: view.id,
                                  scanMode: viewMode,
                                  baseCommit: currentRun?.base_commit_hash,
                                  initialSelectedScanIds: trendCustomScanIds[view.id],
                                });
                              }}
                              className="p-1.5 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-700 text-slate-400 hover:text-sky-500 transition-colors"
                              title={t('trend.adjustScans')}
                            >
                              <SlidersHorizontal className="h-5 w-5" />
                            </button>
                          </>
                        ) : (
                          <>
                            {data.length > 0 && (
                              <button
                                onClick={expandView}
                                className="p-1.5 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-700 text-slate-400 hover:text-sky-500 transition-colors"
                                title={t('dashboard.expand')}
                              >
                                <Maximize2 className="h-5 w-5" />
                              </button>
                            )}
                            <button
                              onClick={() => {
                                setTrendActive(prev => ({ ...prev, [view.id]: true }));
                              }}
                              className="p-1.5 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-700 text-slate-400 hover:text-sky-500 transition-colors"
                              title={t('trend.viewTrend')}
                            >
                              <TrendingUp className="h-5 w-5" />
                            </button>
                            {view.include_children && (
                              <button
                                onClick={() => openChildrenView(view, title)}
                                className="p-1.5 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-700 text-slate-400 hover:text-sky-500 transition-colors"
                                title={t('dashboard.viewRawResults')}
                              >
                                <FileText className="h-5 w-5" />
                              </button>
                            )}
                          </>
                        )}
                      </div>
                    </CardHeader>
                    <CardContent className="pt-6">
                      <div className="flip-card">
                        <div className={`flip-card-inner ${trendActive[view.id] ? 'flipped' : ''}`}>
                          {/* Front: Normal chart */}
                          <div className="flip-card-front">
                            {loading ? <ChartSkeleton type="bar" height="300px" /> : content}
                          </div>
                          {/* Back: Trend chart */}
                          <div className="flip-card-back">
                            {trendLoadingMap[view.id] ? (
                              <ChartSkeleton type="bar" height="300px" />
                            ) : displayedTrendSeries.length === 0 ? (
                              <div className="flex items-center justify-center h-[300px] text-slate-400 dark:text-slate-500">
                                <p className="text-sm">{t('dashboard.noData') || 'No trend data available'}</p>
                              </div>
                            ) : (
                              <TrendRenderer series={displayedTrendSeries} theme={theme}
                                xAxisMin={trendXAxisMin} xAxisMax={trendXAxisMax} />
                            )}
                          </div>
                        </div>
                      </div>
                      {/* Time range preset buttons for trend charts */}
                      {trendActive[view.id] && (
                        <div className="flex items-center justify-center gap-1 mt-3 pt-3 border-t border-slate-100 dark:border-slate-700/50">
                          <span className="text-xs text-slate-400 mr-2">{t('trend.timeRange')}</span>
                          {TIME_RANGE_KEYS.map(range => {
                            const isActive = trendTimeRange === range;
                            return (
                              <button
                                key={range}
                                onClick={() => {
                                  setTrendTimeRange(range);
                                  // Clear manual scan IDs to switch to auto time-range mode
                                  setTrendCustomScanIds(prev => ({ ...prev, [view.id]: [] }));
                                }}
                                className={`px-2 py-0.5 text-xs font-medium rounded transition-all ${
                                  isActive
                                    ? 'bg-sky-500 text-white'
                                    : 'bg-slate-100 dark:bg-slate-700 text-slate-500 dark:text-slate-400 hover:bg-slate-200 dark:hover:bg-slate-600'
                                }`}
                              >
                                {range}
                              </button>
                            );
                          })}
                        </div>
                      )}
                      {/* Limit selector for TopN charts */}
                      {view.type === 'top_n' && (
                        <div className="flex items-center justify-center gap-1 mt-3 pt-3 border-t border-slate-100 dark:border-slate-700/50">
                          <span className="text-xs text-slate-400 mr-2">{t('dashboard.showTop')}</span>
                          {[3, 5, 10, 30, 50, 80, 100, 0].map(n => {
                            const isActive = (topnLimits[view.id] || 10) === n;
                            return (
                              <button
                                key={n}
                                onClick={() => setTopnLimits(prev => ({ ...prev, [view.id]: n }))}
                                className={`px-2 py-0.5 text-xs font-medium rounded transition-all ${
                                  isActive
                                    ? 'bg-sky-500 text-white'
                                    : 'bg-slate-100 dark:bg-slate-700 text-slate-500 dark:text-slate-400 hover:bg-slate-200 dark:hover:bg-slate-600'
                                }`}
                              >
                                {n === 0 ? t('dashboard.unlimited') : n}
                              </button>
                            );
                          })}
                        </div>
                      )}
                    </CardContent>
                  </Card>
                );
              })}
            </div>

            {/* Scan Execution Summary — only show on Summary tab */}
            {selectedRunId && selectedTechStack === 'Summary' && (
              <Card className="border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800/50 backdrop-blur shadow-sm dark:shadow-xl transition-colors duration-200">
                <CardHeader className="border-b border-slate-100 dark:border-slate-700/50">
                  <CardTitle className="text-lg font-semibold text-slate-800 dark:text-slate-200">
                    {t('dashboard.scanSummary')}
                  </CardTitle>
                </CardHeader>
                <CardContent className="pt-6">
                  {scanSummary ? (
                    <div className="space-y-4">
                      {/* Overview stats row */}
                      <div className="grid grid-cols-2 md:grid-cols-6 gap-4 text-sm">
                        <div className="bg-slate-50 dark:bg-slate-700/30 rounded-lg p-3 text-center">
                          <div className="text-2xl font-bold text-sky-600 dark:text-sky-400">{scanSummary.total_files_scanned.toLocaleString()}</div>
                          <div className="text-slate-500 dark:text-slate-400 mt-1">{t('dashboard.totalFiles')}</div>
                        </div>
                        <div className="bg-slate-50 dark:bg-slate-700/30 rounded-lg p-3 text-center">
                          <div className="text-2xl font-bold text-indigo-600 dark:text-indigo-400">{scanSummary.total_analyzers_loaded.toLocaleString()}</div>
                          <div className="text-slate-500 dark:text-slate-400 mt-1">{t('dashboard.analyzersLoaded')}</div>
                        </div>
                        <div className="bg-slate-50 dark:bg-slate-700/30 rounded-lg p-3 text-center">
                          <div className="text-2xl font-bold text-indigo-600 dark:text-indigo-400">{scanSummary.total_analyzers_executed.toLocaleString()}</div>
                          <div className="text-slate-500 dark:text-slate-400 mt-1">{t('dashboard.analyzersExecuted')}</div>
                        </div>
                        <div className="bg-slate-50 dark:bg-slate-700/30 rounded-lg p-3 text-center">
                          <div className="text-2xl font-bold text-indigo-600 dark:text-indigo-400">{scanSummary.total_analyzer_executions.toLocaleString()}</div>
                          <div className="text-slate-500 dark:text-slate-400 mt-1">{t('dashboard.totalExecutions')}</div>
                        </div>
                        <div className="bg-slate-50 dark:bg-slate-700/30 rounded-lg p-3 text-center">
                          <div className={`text-2xl font-bold ${(scanSummary.load_errors.length > 0 || scanSummary.total_errors > 0) ? 'text-red-500' : 'text-green-500'}`}>
                            {scanSummary.total_errors.toLocaleString()}
                          </div>
                          <div className="text-slate-500 dark:text-slate-400 mt-1">{t('dashboard.totalErrors')}</div>
                        </div>
                        <div className="bg-slate-50 dark:bg-slate-700/30 rounded-lg p-3 text-center">
                          <div className={`text-2xl font-bold ${(scanSummary.load_errors.length > 0 || scanSummary.total_errors > 0) ? 'text-amber-500' : 'text-green-500'}`}>
                            {scanSummary.load_errors.length.toLocaleString()}
                          </div>
                          <div className="text-slate-500 dark:text-slate-400 mt-1">{t('dashboard.loadErrors')}</div>
                        </div>
                      </div>

                      {/* Load errors detail */}
                      {scanSummary.load_errors.length > 0 && (
                        <div className="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-3 text-sm">
                          <div className="font-medium text-red-700 dark:text-red-400 mb-1">{t('dashboard.loadErrors')}:</div>
                          <ul className="list-disc list-inside text-red-600 dark:text-red-300 space-y-1">
                            {scanSummary.load_errors.map((err, i) => (
                              <li key={i} className="text-xs break-all">{err}</li>
                            ))}
                          </ul>
                        </div>
                      )}

                      {/* Per-analyzer table */}
                      {scanSummary.analyzer_stats.length > 0 && (
                        <div className="overflow-x-auto">
                          <table className="w-full text-sm text-left text-slate-600 dark:text-slate-300">
                            <thead className="text-xs text-slate-500 dark:text-slate-400 uppercase bg-slate-100 dark:bg-slate-800/50">
                              <tr>
                                <th className="px-3 py-2">{t('dashboard.analyzerId')}</th>
                                <th className="px-3 py-2 text-right">{t('dashboard.filesAnalyzed')}</th>
                                <th className="px-3 py-2 text-right">{t('dashboard.executionErrors')}</th>
                              </tr>
                            </thead>
                            <tbody>
                              {[...scanSummary.analyzer_stats]
                                .sort((a, b) => a.analyzer_id.localeCompare(b.analyzer_id))
                                .map((stat) => (
                                <tr key={stat.analyzer_id} className="border-b border-slate-200 dark:border-slate-700/50 hover:bg-slate-50 dark:hover:bg-slate-700/20">
                                  <td className="px-3 py-2 font-mono text-xs font-medium">{stat.analyzer_id}</td>
                                  <td className="px-3 py-2 text-right">{stat.files_analyzed.toLocaleString()}</td>
                                  <td className="px-3 py-2 text-right">
                                    {stat.execution_errors > 0 ? (
                                      <span className="text-red-500 font-medium">{stat.execution_errors}</span>
                                    ) : (
                                      <span className="text-green-500">0</span>
                                    )}
                                  </td>
                                </tr>
                              ))}
                            </tbody>
                          </table>
                        </div>
                      )}
                    </div>
                  ) : (
                    <p className="text-sm text-slate-400 dark:text-slate-500 text-center py-4">
                      {t('dashboard.noSummary')}
                    </p>
                  )}
                </CardContent>
              </Card>
            )}

          </div>
        </main>
      </div>

      {/* ─── Children Viewer Modal ──────────────────────────────── */}
      <ChildrenViewer
        open={childrenView.open}
        title={childrenView.title}
        items={childrenView.items}
        onClose={() => setChildrenView({ open: false, title: '', items: [] })}
        onFileClick={(filePath, analyzerId) => handleFileClick(filePath, childrenView.title, analyzerId)}
      />

      {/* ─── Match Detail Modal ──────────────────────────────────── */}
      <MatchDetailView
        open={matchDetailView.open}
        title={matchDetailView.title}
        filePath={matchDetailView.filePath}
        matches={matchDetailView.matches}
        total={matchDetailView.total}
        loading={matchDetailView.loading}
        scanMode={viewMode}
        side={matchDetailView.side}
        onSideFilter={(side) => handleFileClick(matchDetailView.filePath, matchDetailView.title, matchDetailView.analyzerId, side)}
        onClose={closeMatchDetail}
        onBack={backToFileList}
      />

      {/* ─── Fullscreen Chart Modal ──────────────────────────────── */}
      {fullscreenView.open && (
        <div
          className="fixed inset-0 z-[70] flex items-center justify-center bg-black/70 backdrop-blur-sm p-8"
          onClick={() => setFullscreenView({ open: false, title: '', type: 'chart' })}
        >
          <div
            className="w-full h-full max-w-[calc(100vw-4rem)] max-h-[calc(100vh-4rem)] flex flex-col border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800/50 backdrop-blur shadow-sm dark:shadow-xl rounded-xl border overflow-hidden"
            onClick={e => e.stopPropagation()}
          >
            {/* Header */}
            <div className="flex flex-row items-center justify-between px-6 py-4 border-b border-slate-100 dark:border-slate-700/50 shrink-0">
              <h2 className="text-xl font-semibold text-slate-800 dark:text-slate-200">{fullscreenView.title}</h2>
              <button
                onClick={() => setFullscreenView({ open: false, title: '', type: 'chart' })}
                className="p-2 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-700 text-slate-400 hover:text-sky-500 transition-colors"
              >
                <Minimize2 className="h-5 w-5" />
              </button>
            </div>
            {/* Content */}
            <div className="flex-1 min-h-0 flex flex-col">
              <div className="flex-1 p-6 min-h-0">
                {fullscreenView.type === 'chart' && fullscreenView.isTopN && fullscreenView.rawData && fullscreenView.chartType ? (
                  (() => {
                    const limit = topnLimits[fullscreenView.viewId!] || 10;
                    const limitedData = limit > 0 && fullscreenView.rawData!.length > limit
                      ? fullscreenView.rawData!.slice(0, limit)
                      : fullscreenView.rawData!;
                    const baseColor = fullscreenView.viewId!.includes('complexity') ? '#ef4444' : '#38bdf8';
                    let opt;
                    switch (fullscreenView.chartType) {
                      case 'bar_row':
                      case 'bar_horizontal':
                        opt = getBarRowOption(fullscreenView.title, limitedData, baseColor);
                        break;
                      case 'bar_col':
                      case 'bar_vertical':
                        opt = getBarColOption(fullscreenView.title, limitedData, baseColor);
                        break;
                      case 'pie':
                        opt = getPieOption(fullscreenView.title, limitedData);
                        break;
                      case 'line':
                        opt = getLineOption(fullscreenView.title, limitedData, baseColor);
                        break;
                      case 'stacked_bar':
                        opt = getStackedBarOption(fullscreenView.title, limitedData);
                        break;
                      case 'heatmap':
                        opt = getHeatmapOption(fullscreenView.title, limitedData);
                        break;
                      case 'radar':
                        opt = getRadarOption(fullscreenView.title, limitedData);
                        break;
                      case 'gauge':
                        opt = getGaugeOption(fullscreenView.title, limitedData);
                        break;
                      default:
                        opt = getBarRowOption(fullscreenView.title, limitedData, baseColor);
                    }
                    return <ChartRenderer options={opt} theme={theme} height="100%" />;
                  })()
                ) : fullscreenView.type === 'chart' && fullscreenView.options && (
                  <ChartRenderer options={fullscreenView.options} theme={theme} height="100%" />
                )}
                {fullscreenView.type === 'card' && fullscreenView.value && (
                  <div className="flex items-center justify-center h-full">
                    <div className="text-center">
                      <div className="text-7xl font-bold text-slate-800 dark:text-slate-200">{fullscreenView.value}</div>
                      <div className="text-lg text-slate-500 dark:text-slate-400 mt-2">{fullscreenView.title}</div>
                    </div>
                  </div>
                )}
                {fullscreenView.type === 'table' && fullscreenView.data && (
                  <div className="overflow-auto h-full">
                    <table className="w-full text-sm text-left text-slate-600 dark:text-slate-300">
                      <thead className="text-xs text-slate-500 dark:text-slate-400 uppercase bg-slate-100 dark:bg-slate-800/50 sticky top-0">
                        <tr>
                          <th className="px-4 py-2">Label</th>
                          <th className="px-4 py-2 text-right">Value</th>
                        </tr>
                      </thead>
                      <tbody>
                        {[...fullscreenView.data]
                          .sort((a, b) => a.label.localeCompare(b.label))
                          .map((d, i) => (
                          <tr key={i} className="border-b border-slate-200 dark:border-slate-700/50">
                            <td className="px-4 py-2 font-medium">{d.label}</td>
                            <td className="px-4 py-2 text-right">{Math.round(d.value).toLocaleString()}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                )}
                {fullscreenView.type === 'trend' && fullscreenView.isTopN && fullscreenView.rawTrendSeries ? (
                  (() => {
                    const limit = topnLimits[fullscreenView.viewId!] || 10;
                    const limitedSeries = limit > 0 && fullscreenView.rawTrendSeries!.length > limit
                      ? fullscreenView.rawTrendSeries!.slice(0, limit)
                      : fullscreenView.rawTrendSeries!;
                    const fsNowMs = Date.now();
                    const fsDays = TIME_RANGE_PRESETS[trendTimeRange] || 90;
                    let fsXMin = fsNowMs - fsDays * 86400 * 1000;
                    const allTs = limitedSeries.flatMap(s => s.data.map(d => d.timestamp * 1000));
                    if (allTs.length > 0) fsXMin = Math.min(fsXMin, Math.min(...allTs));
                    return (
                      <div className="w-full h-full">
                        <TrendRenderer key={`trend-modal-${limit}`} series={limitedSeries} theme={theme} height="100%"
                          xAxisMin={fsXMin} xAxisMax={fsNowMs} />
                      </div>
                    );
                  })()
                ) : fullscreenView.type === 'trend' && fullscreenView.trendSeries && (
                  (() => {
                    const fsNowMs = Date.now();
                    const fsDays = TIME_RANGE_PRESETS[trendTimeRange] || 90;
                    let fsXMin = fsNowMs - fsDays * 86400 * 1000;
                    const allTs = fullscreenView.trendSeries!.flatMap(s => s.data.map(d => d.timestamp * 1000));
                    if (allTs.length > 0) fsXMin = Math.min(fsXMin, Math.min(...allTs));
                    return (
                      <div className="w-full h-full">
                        <TrendRenderer key="trend-modal-orig" series={fullscreenView.trendSeries} theme={theme} height="100%"
                          xAxisMin={fsXMin} xAxisMax={fsNowMs} />
                      </div>
                    );
                  })()
                )}
              </div>
              {/* Time range presets in fullscreen trend view */}
              {fullscreenView.type === 'trend' && (
                <div className="flex items-center justify-center gap-1 px-6 pb-2 pt-2 border-t border-slate-100 dark:border-slate-700/50 shrink-0">
                  <span className="text-xs text-slate-400 mr-2">{t('trend.timeRange')}</span>
                  {TIME_RANGE_KEYS.map(range => {
                    const isActive = trendTimeRange === range;
                    return (
                      <button
                        key={range}
                        onClick={() => setTrendTimeRange(range)}
                        className={`px-2 py-0.5 text-xs font-medium rounded transition-all ${
                          isActive
                            ? 'bg-sky-500 text-white'
                            : 'bg-slate-100 dark:bg-slate-700 text-slate-500 dark:text-slate-400 hover:bg-slate-200 dark:hover:bg-slate-600'
                        }`}
                      >
                        {range}
                      </button>
                    );
                  })}
                </div>
              )}
              {fullscreenView.isTopN && fullscreenView.viewId && (
                <div className="flex items-center justify-center gap-1 px-6 pb-4 pt-3 border-t border-slate-100 dark:border-slate-700/50 shrink-0">
                  <span className="text-xs text-slate-400 mr-2">{t('dashboard.showTop')}</span>
                  {[3, 5, 10, 30, 50, 80, 100, 0].map(n => {
                    const isActive = (topnLimits[fullscreenView.viewId!] || 10) === n;
                    return (
                      <button
                        key={n}
                        onClick={() => setTopnLimits(prev => ({ ...prev, [fullscreenView.viewId!]: n }))}
                        className={`px-2 py-0.5 text-xs font-medium rounded transition-all ${
                          isActive
                            ? 'bg-sky-500 text-white'
                            : 'bg-slate-100 dark:bg-slate-700 text-slate-500 dark:text-slate-400 hover:bg-slate-200 dark:hover:bg-slate-600'
                        }`}
                      >
                        {n === 0 ? t('dashboard.unlimited') : n}
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      {/* ─── Trend Scan Selector Modal ──────────────────────────── */}
      {scanSelectorView && (
        <TrendScanSelector
          open={scanSelectorView.open}
          projectName={scanSelectorView.projectName}
          scanMode={scanSelectorView.scanMode}
          baseCommit={scanSelectorView.baseCommit}
          initialSelectedScanIds={scanSelectorView.initialSelectedScanIds}
          onConfirm={(scanIds) => {
            setTrendCustomScanIds(prev => ({ ...prev, [scanSelectorView.viewId]: scanIds }));
            setScanSelectorView(null);
          }}
          onClose={() => setScanSelectorView(null)}
        />
      )}

    </div>
  );
};

export default Dashboard;
