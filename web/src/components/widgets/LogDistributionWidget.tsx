import React from 'react';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card';
import ChartRenderer from '../ChartRenderer';
import { AlertCircle } from 'lucide-react';

interface LogStats {
  error: number;
  warn: number;
  info: number;
}

interface LogDistributionWidgetProps {
  data: LogStats;
  loading?: boolean;
}

export const LogDistributionWidget: React.FC<LogDistributionWidgetProps> = ({ data, loading }) => {
  const getDonutOption = () => ({
    tooltip: { trigger: 'item' },
    legend: {
      bottom: '0%',
      left: 'center',
      textStyle: { color: '#94a3b8' },
      icon: 'circle'
    },
    series: [
      {
        name: 'Log Levels',
        type: 'pie',
        radius: ['50%', '70%'],
        center: ['50%', '45%'], // Shift up slightly to make room for legend
        avoidLabelOverlap: false,
        label: { show: false },
        itemStyle: {
          borderRadius: 5,
          borderColor: '#1e293b',
          borderWidth: 2
        },
        data: [
          { value: data.error, name: 'Error', itemStyle: { color: '#f43f5e' } },
          { value: data.warn, name: 'Warn', itemStyle: { color: '#eab308' } },
          { value: data.info, name: 'Info', itemStyle: { color: '#38bdf8' } }
        ]
      }
    ]
  });

  return (
    <Card className="border-slate-700 bg-slate-800/50 backdrop-blur shadow-xl">
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2 border-b border-slate-700/50">
        <CardTitle className="text-xl font-semibold text-slate-200">
          Log Distribution
        </CardTitle>
        <AlertCircle className="h-5 w-5 text-slate-500" />
      </CardHeader>
      <CardContent className="pt-6">
        {loading ? (
          <div className="animate-pulse h-[200px] bg-slate-700/20 rounded"></div>
        ) : (
          <ChartRenderer options={getDonutOption()} height="250px" />
        )}
      </CardContent>
    </Card>
  );
};
