import ReactECharts from 'echarts-for-react';

interface ChartRendererProps {
  options: any;
  height?: string;
  className?: string;
  theme?: 'light' | 'dark';
}

const ChartRenderer = ({ options, height = '300px', className, theme = 'dark' }: ChartRendererProps) => {
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

  return <ReactECharts option={finalOptions} style={{ height }} className={className} opts={{ renderer: 'canvas' }} />;
};

export default ChartRenderer;
