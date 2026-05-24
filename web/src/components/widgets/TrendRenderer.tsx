import React from 'react';
import ChartRenderer from '../ChartRenderer';
import type { TrendSeries } from '../../services/data';

interface TrendRendererProps {
  series: TrendSeries[];
  theme?: 'light' | 'dark';
  height?: string;
  xAxisMin?: number;
  xAxisMax?: number;
}

const CHART_COLORS = ['#38bdf8', '#22c55e', '#f59e0b', '#ef4444', '#a855f7', '#06b6d4', '#ec4899', '#14b8a6'];

const TrendRenderer = React.memo(({ series, theme = 'dark', height = '300px', xAxisMin, xAxisMax }: TrendRendererProps) => {
  const textColor = theme === 'dark' ? '#94a3b8' : '#64748b';
  const splitLineColor = theme === 'dark' ? '#334155' : '#e2e8f0';

  const options = {
    tooltip: {
      trigger: 'axis' as const,
      formatter: (params: any[]) => {
        if (!params || params.length === 0) return '';
        const first = params[0];
        const date = new Date(first.data[0]).toLocaleDateString();
        let html = `<strong>${date}</strong><br/>`;
        params.forEach((p: any) => {
          html += `${p.marker} ${p.seriesName}: ${Number(p.data[1]).toLocaleString()}<br/>`;
        });
        return html;
      },
    },
    legend: {
      type: 'scroll' as const,
      bottom: 30,
      textStyle: { color: textColor, fontSize: 11 },
      pageIconColor: textColor,
      pageIconInactiveColor: splitLineColor,
    },
    xAxis: {
      type: 'time' as const,
      splitLine: { show: true, lineStyle: { color: splitLineColor } },
      axisLabel: { color: textColor },
      ...(xAxisMin !== undefined ? { min: xAxisMin } : {}),
      ...(xAxisMax !== undefined ? { max: xAxisMax } : {}),
    },
    yAxis: {
      type: 'value' as const,
      splitLine: { show: true, lineStyle: { color: splitLineColor } },
      axisLabel: { color: textColor },
    },
    series: series.map((s, i) => ({
      name: s.label,
      type: 'line' as const,
      data: s.data.map(d => [d.timestamp * 1000, d.value]),
      smooth: false,
      symbol: 'circle' as const,
      symbolSize: 6,
      itemStyle: { color: CHART_COLORS[i % CHART_COLORS.length] },
      lineStyle: { width: 2 },
      emphasis: { focus: 'series' as const },
      label: {
        show: false,
      },
    })),
    grid: {
      left: '3%',
      right: '4%',
      bottom: '26%',
      top: '5%',
      containLabel: true,
    },
    dataZoom: [
      { type: 'inside' as const, start: 0, end: 100 },
    ],
  };

  return <ChartRenderer options={options} height={height} theme={theme} />;
});

export default TrendRenderer;
