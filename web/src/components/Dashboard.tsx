import { useEffect, useState } from 'react';
import { useApp } from '@/contexts/AppContext';
import { Header } from './layout/Header';
import { Sidebar } from './layout/Sidebar';
import { TechStackTabs } from './dashboard/TechStackTabs';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { MetricCard } from './widgets/MetricCard';

import ChartRenderer from './ChartRenderer';
import { fetchView, type AggregationResult, type AppConfig, getDefaultProject, getProjectNames } from '@/services/data';
import { Activity, FileText } from 'lucide-react';



const Dashboard = () => {
  const {
    currentProject, setProject,
    viewMode, setViewMode,
    selectedTechStack, setSelectedTechStack,
    selectedRunId, setSelectedRunId,
    availableTechStacks, setAvailableTechStacks
  } = useApp();

  const [appConfig, setAppConfig] = useState<AppConfig | null>(null);
  const configProjectNames = appConfig ? getProjectNames(appConfig) : [];

  const [viewsConfig, setViewsConfig] = useState<any[]>([]);
  const [activeViews, setActiveViews] = useState<any[]>([]);
  const [viewDataMap, setViewDataMap] = useState<Record<string, AggregationResult[]>>({});
  const [loading, setLoading] = useState(true);
  const [runs, setRuns] = useState<any[]>([]);
  // Track change_type filter per view (for switchable mode)
  const [changeTypeFilters, setChangeTypeFilters] = useState<Record<string, string>>({});

  // Fetch Config
  useEffect(() => {
    const loadConfig = async () => {
      const config = await import('@/services/data').then(m => m.fetchConfig());
      setAppConfig(config);

      // Set default project if currentProject is not in config or just starting
      const defaultProject = getDefaultProject(config);
      if (defaultProject) {
        // If currentProject is not in config, use the first project from config
        if (!getProjectNames(config).includes(currentProject)) {
          setProject(defaultProject.name);
        }

        const projectConfig = config.projects.find(p => p.name === currentProject) || defaultProject;
        setViewsConfig(projectConfig.views);
        setAvailableTechStacks(projectConfig.tech_stacks);

        // Ensure "Summary" is selected if selection invalid
        if (!['Summary', ...projectConfig.tech_stacks].includes(selectedTechStack)) {
          setSelectedTechStack('Summary');
        }
      }
    };
    loadConfig();
  }, [currentProject]);

  // Update config when currentProject changes
  useEffect(() => {
    if (appConfig) {
      const projectConfig = appConfig.projects.find(p => p.name === currentProject);
      if (projectConfig) {
        setViewsConfig(projectConfig.views);
        setAvailableTechStacks(projectConfig.tech_stacks);
        setSelectedTechStack('Summary'); // Reset to Summary on project change
        setViewDataMap({}); // Clear old data
        // Note: selectedRunId is handled by the fetchRuns effect
      }
    }
  }, [currentProject, appConfig]);

  // Filter Active Views based on selection
  useEffect(() => {
    if (selectedTechStack === 'Summary') {
      // Show views with NO specific tech stack (global views)
      setActiveViews(viewsConfig.filter(v => !v.tech_stacks || v.tech_stacks.length === 0));
    } else {
      // Show views that include this tech stack
      setActiveViews(viewsConfig.filter(v => v.tech_stacks && v.tech_stacks.includes(selectedTechStack)));
    }
  }, [selectedTechStack, viewsConfig]);

  // Fetch Runs - depends on selected project and view mode
  useEffect(() => {
    if (!currentProject) return;

    const loadRuns = async () => {
      const data = await import('@/services/data').then(m => m.fetchRuns(currentProject, viewMode === 'snapshot' ? 'SNAPSHOT' : 'DIFF'));
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
  }, [viewMode, currentProject]);

  // Fetch View Data - Initial load and global changes (run, tech stack, active views)
  useEffect(() => {
    if (!selectedRunId || activeViews.length === 0) return;

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
          // Use 'A', 'M', 'D' directly as these are the values stored in the database
          if (view.change_type_mode === 'switchable') {
            const currentFilter = changeTypeFilters[view.id] || 'A';
            options.changeType = currentFilter;
          }

          return fetchView(currentProject, selectedRunId, view.id, options);
        });
        const results = await Promise.all(promises);

        const newMap: Record<string, AggregationResult[]> = {};
        results.forEach((res, index) => {
          newMap[activeViews[index].id] = res.items;
        });
        setViewDataMap(newMap);
      } catch (e) {
        console.error(e);
      } finally {
        setLoading(false);
      }
    };
    load();
    // Note: changeTypeFilters is intentionally excluded from dependencies
    // Individual filter changes are handled by the effect below
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentProject, selectedRunId, activeViews, selectedTechStack]);

  // Handle individual switchable filter changes - only fetch the affected view
  const [lastChangeTypeFilters, setLastChangeTypeFilters] = useState<Record<string, string>>({});

  useEffect(() => {
    if (!selectedRunId || activeViews.length === 0) return;

    // Find which view's filter changed
    const changedViewId = Object.keys(changeTypeFilters).find(
      viewId => changeTypeFilters[viewId] !== lastChangeTypeFilters[viewId]
    );

    if (!changedViewId) return;

    // Update last filters to track changes
    setLastChangeTypeFilters(changeTypeFilters);

    // Find the view config
    const view = activeViews.find(v => v.id === changedViewId);
    if (!view || view.change_type_mode !== 'switchable') return;

    // Fetch only the changed view
    const fetchSingleView = async () => {
      try {
        const techStackFilter = selectedTechStack !== 'Summary' ? selectedTechStack : undefined;
        const options: { techStack?: string; changeType?: string } = {};
        if (techStackFilter) options.techStack = techStackFilter;
        options.changeType = changeTypeFilters[changedViewId] || 'A';

        const result = await fetchView(currentProject, selectedRunId, changedViewId, options);

        setViewDataMap(prev => ({
          ...prev,
          [changedViewId]: result.items
        }));
      } catch (e) {
        console.error(e);
      }
    };
    fetchSingleView();
  }, [changeTypeFilters, selectedRunId, activeViews, selectedTechStack, lastChangeTypeFilters]);

  // --- Chart Option Generators ---

  // 1. Horizontal Bar (Row)
  const getBarRowOption = (_title: string, data: AggregationResult[], color: string) => ({
    tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
    xAxis: {
      type: 'value',
      splitLine: { show: true, lineStyle: { color: '#334155' } },
      axisLabel: { color: '#94a3b8' }
    },
    yAxis: {
      type: 'category',
      data: data.map(d => d.label.split('/').pop()),
      axisLabel: {
        width: 120,
        overflow: 'truncate',
        interval: 0,
        color: '#94a3b8'
      },
      inverse: true
    },
    series: [{
      data: data.map(d => Math.round(d.value)),
      type: 'bar',
      itemStyle: { color: color, borderRadius: [0, 4, 4, 0] },
      label: { show: true, position: 'right', color: '#f8fafc', formatter: (params: { value: number }) => Math.round(params.value).toLocaleString() }
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
        color: '#94a3b8',
        rotate: 30, // Rotate labels if many
        interval: 0
      }
    },
    yAxis: {
      type: 'value',
      splitLine: { show: true, lineStyle: { color: '#334155' } },
      axisLabel: { color: '#94a3b8' }
    },
    series: [{
      data: data.map(d => Math.round(d.value)),
      type: 'bar',
      itemStyle: { color: color, borderRadius: [4, 4, 0, 0] },
      label: { show: true, position: 'top', color: '#f8fafc', formatter: (params: { value: number }) => Math.round(params.value).toLocaleString() }
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
        textStyle: { color: '#94a3b8', fontSize: 11 },
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
            borderColor: '#1e293b',
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
      axisLabel: { color: '#94a3b8' }
    },
    yAxis: {
      type: 'value',
      splitLine: { show: true, lineStyle: { color: '#334155' } },
      axisLabel: { color: '#94a3b8' }
    },
    series: [{
      data: data.map(d => Math.round(d.value)),
      type: 'line',
      smooth: false, // Point to point
      symbol: 'circle',
      symbolSize: 8,
      itemStyle: { color: color },
      lineStyle: { width: 3 },
      label: { show: true, position: 'top', color: '#f8fafc', formatter: (params: { value: number }) => Math.round(params.value).toLocaleString() }
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
        textStyle: { color: '#94a3b8' },
        top: 0
      },
      xAxis: {
        type: 'category',
        data: labels,
        axisLabel: { color: '#94a3b8', rotate: 30 }
      },
      yAxis: {
        type: 'value',
        splitLine: { show: true, lineStyle: { color: '#334155' } },
        axisLabel: { color: '#94a3b8' }
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
        axisLabel: { color: '#94a3b8' },
        splitArea: { show: true }
      },
      yAxis: {
        type: 'category',
        data: yLabels,
        axisLabel: { color: '#94a3b8' },
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
        textStyle: { color: '#94a3b8' }
      },
      series: [{
        type: 'heatmap',
        data: heatmapData,
        label: { show: true, color: '#f8fafc' },
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
        textStyle: { color: '#94a3b8' }
      },
      radar: {
        indicator: indicators,
        shape: 'polygon',
        splitNumber: 5,
        axisName: { color: '#94a3b8' },
        splitLine: { lineStyle: { color: '#334155' } },
        splitArea: { areaStyle: { color: ['rgba(56, 189, 248, 0.05)', 'rgba(56, 189, 248, 0.1)'] } },
        axisLine: { lineStyle: { color: '#475569' } }
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
        axisLabel: { color: '#94a3b8', distance: 30, fontSize: 12 },
        detail: {
          valueAnimation: true,
          formatter: '{value}',
          color: '#f8fafc',
          fontSize: 24,
          offsetCenter: [0, '70%']
        },
        title: {
          offsetCenter: [0, '90%'],
          color: '#94a3b8',
          fontSize: 14
        },
        data: [{ value: Math.round(value), name: title }]
      }]
    };
  };


  return (
    <div className="flex flex-col h-screen overflow-hidden bg-slate-900 text-slate-100">
      <Header
        viewMode={viewMode}
        onViewModeChange={setViewMode}
        projects={configProjectNames}
        selectedProject={currentProject}
        onProjectChange={setProject}
      />

      <div className="flex flex-1 overflow-hidden">
        <Sidebar
          runs={runs}
          selectedRunId={selectedRunId}
          onRunSelect={setSelectedRunId}
        />

        <main className="flex-1 overflow-y-auto relative">
          {/* Background gradient effect */}
          <div className="absolute inset-0 bg-gradient-to-br from-slate-900 via-slate-900 to-blue-950/20 pointer-events-none -z-10" />

          <div className="container mx-auto p-8 space-y-8 min-h-full">
            <TechStackTabs
              stacks={['Summary', ...availableTechStacks]}
              selectedStack={selectedTechStack}
              onSelect={setSelectedTechStack}
            />

            {/* Dynamic Widgets Grid */}
            <div className="grid gap-6 md:grid-cols-2 lg:grid-cols-2">
              {activeViews.map(view => {
                let data = viewDataMap[view.id] || [];
                const title = view.title || view.id.replace(/_/g, ' ').toUpperCase();

                // For "all" mode: flatten hierarchical data (e.g., tech_stack -> change_type) 
                // into flat labels like "Rust (A)", "Rust (M)", etc.
                if (view.change_type_mode === 'all' && data.length > 0 && data[0].children) {
                  const flatData: AggregationResult[] = [];
                  for (const parent of data) {
                    if (parent.children && parent.children.length > 0) {
                      for (const child of parent.children) {
                        flatData.push({
                          label: `${parent.label} (${child.label})`,
                          value: child.value,
                          tech_stack: parent.tech_stack,
                          category: parent.category,
                          change_type: child.change_type,
                        });
                      }
                    } else {
                      // No children, keep as is
                      flatData.push(parent);
                    }
                  }
                  data = flatData;
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

                if (actualChartType === 'card') {
                  // Metric Card
                  const totalValue = data.reduce((acc, curr) => acc + curr.value, 0);
                  content = (
                    <MetricCard
                      key={view.id}
                      title={title}
                      value={totalValue.toLocaleString()}
                      subValue="Total Aggregated Value"
                      loading={loading}
                    />
                  );
                  return content;
                } else if (actualChartType === 'table') {
                  // Simple Table
                  content = (
                    <div className="overflow-x-auto">
                      <table className="w-full text-sm text-left text-slate-300">
                        <thead className="text-xs text-slate-400 uppercase bg-slate-800/50">
                          <tr>
                            <th className="px-4 py-2">Label</th>
                            <th className="px-4 py-2 text-right">Value</th>
                          </tr>
                        </thead>
                        <tbody>
                          {data.map((d, i) => (
                            <tr key={i} className="border-b border-slate-700/50 hover:bg-slate-700/20">
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
                  content = <ChartRenderer options={options} />;
                }

                // Wrapper Card
                const changeTypeMode = view.change_type_mode;
                const currentFilter = changeTypeFilters[view.id] || 'A'; // Default to Add

                return (
                  <Card key={view.id} className="col-span-1 border-slate-700 bg-slate-800/50 backdrop-blur shadow-xl">
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2 border-b border-slate-700/50">
                      <CardTitle className="text-xl font-semibold text-slate-200">
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
                                  : 'bg-slate-700 text-slate-400 hover:bg-slate-600'
                                  }`}
                              >
                                {type}
                              </button>
                            ))}
                          </div>
                        )}
                        {view.id.includes('complexity') ?
                          <Activity className="h-5 w-5 text-slate-500" /> :
                          <FileText className="h-5 w-5 text-slate-500" />
                        }
                      </div>
                    </CardHeader>
                    <CardContent className="pt-6">
                      {loading ? <div className="animate-pulse h-[300px] bg-slate-700/20 rounded"></div> : content}
                    </CardContent>
                  </Card>
                );
              })}
            </div>

          </div>
        </main>
      </div>
    </div>
  );
};

export default Dashboard;
