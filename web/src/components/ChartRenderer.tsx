import React from 'react';
import ReactEChartsCore from 'echarts-for-react/lib/core';
import echarts from '@/lib/echarts';

export interface ChartOptions {
  tooltip?: Record<string, unknown>;
  [key: string]: unknown;
}

interface ChartRendererProps {
  options: ChartOptions;
  height?: string;
  className?: string;
  theme?: 'light' | 'dark';
}

const ChartRenderer = React.memo(
  ({ options, height = '300px', className, theme = 'dark' }: ChartRendererProps) => {
    // Common chart options (theming)
    const defaultOptions = {
      animation: true,
      backgroundColor: 'transparent',
      grid: {
        left: '3%',
        right: '4%',
        bottom: '3%',
        containLabel: true,
      },
      tooltip: {
        trigger: 'axis',
        backgroundColor: theme === 'dark' ? 'rgba(50,50,50,0.9)' : 'rgba(255,255,255,0.9)',
        borderColor: theme === 'dark' ? '#333' : '#ccc',
        textStyle: {
          color: theme === 'dark' ? '#fff' : '#333',
        },
        extraCssText: theme === 'light' ? 'box-shadow: 0 0 10px rgba(0,0,0,0.1);' : undefined
      }
    };

    // Deep merge tooltips to ensure we don't lose the trigger: 'item' etc overrides
    const finalOptions = {
      ...defaultOptions,
      ...options,
      tooltip: {
        ...defaultOptions.tooltip,
        ...(options.tooltip || {})
      }
    };

    return <ReactEChartsCore echarts={echarts} option={finalOptions} style={{ height }} className={className} opts={{ renderer: 'canvas' }} />;
  },
  (prevProps, nextProps) => {
    return prevProps.height === nextProps.height
      && prevProps.className === nextProps.className
      && prevProps.theme === nextProps.theme
      && JSON.stringify(prevProps.options) === JSON.stringify(nextProps.options);
  }
);

export default ChartRenderer;
