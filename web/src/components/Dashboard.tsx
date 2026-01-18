import { useEffect, useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import ChartRenderer from './ChartRenderer';
import { fetchView, type AggregationResult } from '@/services/data';
import { Activity, FileText } from 'lucide-react';

const Dashboard = () => {
  const [fileSizeData, setFileSizeData] = useState<AggregationResult[]>([]);
  const [complexityData, setComplexityData] = useState<AggregationResult[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const load = async () => {
      setLoading(true);
      try {
        // Fetch two views in parallel
        const [s1, s2] = await Promise.all([
          fetchView(1, 1, 'top_file_size'),
          fetchView(1, 1, 'top_complexity')
        ]);
        setFileSizeData(s1.items);
        setComplexityData(s2.items);
      } catch (e) {
        console.error(e);
      } finally {
        setLoading(false);
      }
    };
    load();
  }, []);

  // Transform for ECharts
  const getBarOption = (_title: string, data: AggregationResult[], color: string) => ({
    // title: { text: title }, // Rendered in Card Title
    xAxis: { type: 'value', splitLine: { show: false } },
    yAxis: {
      type: 'category',
      data: data.map(d => d.label.split('/').pop()), // Shorten label to filename
      axisLabel: {
        width: 120,
        overflow: 'truncate',
        interval: 0
      },
      inverse: true // Top items at top
    },
    series: [{
      data: data.map(d => d.value),
      type: 'bar',
      itemStyle: { color: color, borderRadius: [0, 4, 4, 0] },
      label: { show: true, position: 'right' }
    }],
    grid: { left: 140, right: 40, top: 10, bottom: 20 }
  });

  return (
    <div className="container mx-auto p-8 space-y-8 animate-in fade-in duration-700">
      <header className="flex items-center justify-between mb-8 pb-6 border-b">
        <div>
          <h1 className="text-4xl font-extrabold tracking-tight lg:text-5xl mb-2">CodePrism</h1>
          <p className="text-muted-foreground text-lg">Project Analysis Dashboard</p>
        </div>
        <div className="flex gap-2">
          <div className="bg-primary/10 text-primary px-4 py-2 rounded-full font-medium text-sm">
            Snapshot Mode
          </div>
          <div className="bg-muted text-muted-foreground px-4 py-2 rounded-full font-medium text-sm">
            Scan #1
          </div>
        </div>
      </header>

      <div className="grid gap-6 md:grid-cols-2 lg:grid-cols-2">
        <Card className="col-span-1 shadow-md hover:shadow-lg transition-shadow duration-300 border-t-4 border-t-blue-500 bg-card/50 backdrop-blur">
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-xl font-semibold">
              Top 10 File Size
            </CardTitle>
            <FileText className="h-5 w-5 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            {loading ? <div className="animate-pulse h-[300px] bg-muted/20 rounded"></div> :
              <ChartRenderer options={getBarOption('File Size', fileSizeData, '#3b82f6')} />
            }
          </CardContent>
        </Card>

        <Card className="col-span-1 shadow-md hover:shadow-lg transition-shadow duration-300 border-t-4 border-t-orange-500 bg-card/50 backdrop-blur">
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-xl font-semibold">
              Top 10 Complexity
            </CardTitle>
            <Activity className="h-5 w-5 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            {loading ? <div className="animate-pulse h-[300px] bg-muted/20 rounded"></div> :
              <ChartRenderer options={getBarOption('Complexity', complexityData, '#f97316')} />
            }
          </CardContent>
        </Card>
      </div>

      <div className="grid gap-6 md:grid-cols-1 mt-6">
        <Card>
          <CardHeader><CardTitle>System Overview</CardTitle></CardHeader>
          <CardContent>
            <div className="flex items-center justify-center h-24 bg-muted/20 rounded border border-dashed border-muted-foreground/20 text-muted-foreground">
              Interactive Drill-down (Coming Soon)
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
};

export default Dashboard;
