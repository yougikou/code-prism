import { useEffect, useState } from 'react';
import { useApp } from '@/contexts/AppContext';
import { Header } from './layout/Header';
import { Sidebar } from './layout/Sidebar';
import { TechStackTabs } from './dashboard/TechStackTabs';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { MetricCard } from './widgets/MetricCard';
import { LogDistributionWidget } from './widgets/LogDistributionWidget';
import ChartRenderer from './ChartRenderer';
import { fetchView, type AggregationResult } from '@/services/data';
import { Activity, FileText } from 'lucide-react';

const MOCK_LOG_STATS = {
  error: 12,
  warn: 28,
  info: 145
};

const Dashboard = () => {
  const {
    currentProject,
    viewMode, setViewMode,
    selectedTechStack, setSelectedTechStack,
    selectedRunId, setSelectedRunId,
    availableTechStacks, setAvailableTechStacks
  } = useApp();

  const [viewsConfig, setViewsConfig] = useState<any[]>([]);
  const [activeViews, setActiveViews] = useState<any[]>([]);
  const [viewDataMap, setViewDataMap] = useState<Record<string, AggregationResult[]>>({});
  const [loading, setLoading] = useState(true);
  const [runs, setRuns] = useState<any[]>([]);

  // Fetch Config
  useEffect(() => {
    const loadConfig = async () => {
      const config = await import('@/services/data').then(m => m.fetchConfig());
      if (config.views) {
        setViewsConfig(config.views);
      }
      if (config.tech_stacks) {
        setAvailableTechStacks(config.tech_stacks);
        // Ensure "Summary" is selected if selection invalid
        if (!['Summary', ...config.tech_stacks].includes(selectedTechStack)) {
          setSelectedTechStack('Summary');
        }
      }
    };
    loadConfig();
  }, []);

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

  // Fetch Runs
  useEffect(() => {
    const loadRuns = async () => {
      const data = await import('@/services/data').then(m => m.fetchRuns(1, viewMode === 'snapshot' ? 'SNAPSHOT' : 'DIFF'));
      setRuns(data.map(r => ({
        id: r.id,
        hash: r.commit_hash.substring(0, 7),
        date: r.scan_time
      })));

      if (data.length > 0 && !selectedRunId) {
        setSelectedRunId(data[0].id);
      } else if (data.length === 0) {
        setSelectedRunId(null);
      }
    };
    loadRuns();
  }, [viewMode]);

  // Fetch View Data
  useEffect(() => {
    if (!selectedRunId || activeViews.length === 0) return;

    const load = async () => {
      setLoading(true);
      try {
        const promises = activeViews.map(view => fetchView(1, selectedRunId, view.id));
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
  }, [currentProject, selectedRunId, activeViews]);

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
      data: data.map(d => d.value),
      type: 'bar',
      itemStyle: { color: color, borderRadius: [0, 4, 4, 0] },
      label: { show: true, position: 'right', color: '#f8fafc' }
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
      data: data.map(d => d.value),
      type: 'bar',
      itemStyle: { color: color, borderRadius: [4, 4, 0, 0] },
      label: { show: true, position: 'top', color: '#f8fafc' }
    }],
    grid: { left: 10, right: 10, top: 30, bottom: 10, containLabel: true }
  });

  // 3. Pie Chart
  const getPieOption = (title: string, data: AggregationResult[]) => ({
    tooltip: { trigger: 'item' },
    legend: {
      top: '5%',
      left: 'center',
      textStyle: { color: '#94a3b8' }
    },
    series: [
      {
        name: title,
        type: 'pie',
        radius: ['40%', '70%'],
        avoidLabelOverlap: false,
        itemStyle: {
          borderRadius: 10,
          borderColor: '#1e293b',
          borderWidth: 2
        },
        label: {
          show: true, // Show labels explicitly
          position: 'outside',
          formatter: '{b}: {c} ({d}%)',
          color: '#f8fafc'
        },
        emphasis: {
          label: { show: true, fontSize: 16, fontWeight: 'bold' }
        },
        labelLine: { show: true, lineColor: '#94a3b8' },
        data: data.map(d => ({ value: d.value, name: d.label }))
      }
    ]
  });

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
      data: data.map(d => d.value),
      type: 'line',
      smooth: false, // Point to point
      symbol: 'circle',
      symbolSize: 8,
      itemStyle: { color: color },
      lineStyle: { width: 3 },
      label: { show: true, position: 'top', color: '#f8fafc' }
    }],
    grid: { left: 10, right: 20, top: 30, bottom: 20, containLabel: true }
  });

  return (
    <div className="flex flex-col h-screen overflow-hidden bg-slate-900 text-slate-100">
      <Header
        viewMode={viewMode}
        onViewModeChange={setViewMode}
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
                const data = viewDataMap[view.id] || [];
                const title = view.title || view.id.replace(/_/g, ' ').toUpperCase();

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
                              <td className="px-4 py-2 text-right">{d.value.toLocaleString()}</td>
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
                    default:
                      options = getBarRowOption(title, data, baseColor);
                  }
                  content = <ChartRenderer options={options} />;
                }

                // Wrapper Card
                return (
                  <Card key={view.id} className="col-span-1 border-slate-700 bg-slate-800/50 backdrop-blur shadow-xl">
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2 border-b border-slate-700/50">
                      <CardTitle className="text-xl font-semibold text-slate-200">
                        {title}
                      </CardTitle>
                      {view.id.includes('complexity') ?
                        <Activity className="h-5 w-5 text-slate-500" /> :
                        <FileText className="h-5 w-5 text-slate-500" />
                      }
                    </CardHeader>
                    <CardContent className="pt-6">
                      {loading ? <div className="animate-pulse h-[300px] bg-slate-700/20 rounded"></div> : content}
                    </CardContent>
                  </Card>
                );
              })}
            </div>

            {/* Log Distribution Widget */}
            <div className="mt-6">
              <LogDistributionWidget
                data={MOCK_LOG_STATS}
                loading={loading}
              />
            </div>

          </div>
        </main>
      </div>
    </div>
  );
};

export default Dashboard;
