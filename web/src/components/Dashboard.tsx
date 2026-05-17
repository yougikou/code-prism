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
import { fetchView, fetchScanSummary, fetchMatches, type AggregationResult, type AppConfig, type ScanSummary, type MatchDetail, getDefaultProject } from '@/services/data';
import { FileText } from 'lucide-react';



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

  // Children viewer modal state
  interface LeafItem {
    label: string;
    value: number;
    group?: string;
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
    matches: MatchDetail[];
    total: number;
    loading: boolean;
  }>({ open: false, title: '', filePath: '', matches: [], total: 0, loading: false });

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
        if (!['Summary', ...projectConfig.tech_stacks].includes(selectedTechStack)) {
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

    if (selectedTechStack === 'Summary') {
      // Show views with NO specific tech stack or containing 'All' (global views)
      setActiveViews(viewsConfig.filter(isGlobalView));
    } else {
      // Show views that include this tech stack (but not 'All'-only views)
      setActiveViews(viewsConfig.filter(v =>
        v.tech_stacks && v.tech_stacks.includes(selectedTechStack)
      ));
    }
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
        date: r.scan_time
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
        formatter: (name: string) => name.length > 12 ? name.substring(0, 12) + '..' : name
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
          data: data.map(d => ({ value: Math.round(d.value), name: d.label }))
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
            : groupLabel || undefined;

        result.push({ label: item.label, value: Math.round(item.value), group });
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

  const handleFileClick = async (filePath: string, viewTitle: string) => {
    if (!selectedRunId) return;
    setMatchDetailView({ open: true, title: viewTitle, filePath, matches: [], total: 0, loading: true });
    try {
      const res = await fetchMatches(currentProject, selectedRunId, { file_path: filePath });
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

          <div className="container mx-auto p-8 space-y-8 min-h-full">
            <TechStackTabs
              stacks={['Summary', ...availableTechStacks]}
              selectedStack={selectedTechStack}
              onSelect={setSelectedTechStack}
            />

            {/* Dynamic Widgets Grid */}
            <div key={`${selectedTechStack}-${theme}`} className="grid gap-6 md:grid-cols-2 lg:grid-cols-2">
              {activeViews.length === 0 ? (
                <div className="col-span-full flex flex-col items-center justify-center py-16 text-slate-400">
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

                // Determine Chart Type
                let content;
                const chartType = view.chart_type || (view.type === 'top_n' ? 'bar_row' : 'card');

                // Show empty state when data is empty and not loading
                if (data.length === 0 && !loading) {
                  content = (
                    <div className="flex flex-col items-center justify-center h-[300px] text-slate-400 dark:text-slate-500">
                      <p className="text-sm">{t('dashboard.noData') || 'No data available'}</p>
                    </div>
                  );
                } else {
                  // Special handling for legacy Sum fallback logic (grouped Sum -> Pie)
                  let actualChartType = chartType;
                  if (view.type === 'sum' && !view.chart_type) {
                    if (view.group_by && view.group_by.length > 0) {
                      actualChartType = 'pie';
                    } else {
                      actualChartType = 'card';
                    }
                  }

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
                    // Simple Table
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
                            {data.map((d, i) => (
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
                  <Card key={view.id} className={`col-span-1 ${view.width === 2 ? 'lg:col-span-2' : ''} border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800/50 backdrop-blur shadow-sm dark:shadow-xl transition-colors duration-200`}>
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
                        {view.include_children && (
                          <button
                            onClick={() => openChildrenView(view, title)}
                            className="p-1.5 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-700 text-slate-400 hover:text-sky-500 transition-colors"
                            title={t('dashboard.viewRawResults')}
                          >
                            <FileText className="h-5 w-5" />
                          </button>
                        )}
                      </div>
                    </CardHeader>
                    <CardContent className="pt-6">
                      {loading ? <div className="animate-pulse h-[300px] bg-slate-100 dark:bg-slate-700/20 rounded"></div> : content}
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
                              {scanSummary.analyzer_stats.map((stat) => (
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
        onFileClick={(filePath) => handleFileClick(filePath, childrenView.title)}
      />

      {/* ─── Match Detail Modal ──────────────────────────────────── */}
      <MatchDetailView
        open={matchDetailView.open}
        title={matchDetailView.title}
        filePath={matchDetailView.filePath}
        matches={matchDetailView.matches}
        total={matchDetailView.total}
        loading={matchDetailView.loading}
        onClose={closeMatchDetail}
        onBack={backToFileList}
      />

    </div>
  );
};

export default Dashboard;
