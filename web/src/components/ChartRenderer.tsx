import ReactECharts from 'echarts-for-react';

interface ChartRendererProps {
  options: any;
  height?: string;
  className?: string;
}

const ChartRenderer = ({ options, height = '300px', className }: ChartRendererProps) => {
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
      backgroundColor: 'rgba(50,50,50,0.9)',
      borderColor: '#333',
      textStyle: {
        color: '#fff',
      },
    }
  };

  const finalOptions = { ...defaultOptions, ...options };

  return <ReactECharts option={finalOptions} style={{ height }} className={className} opts={{ renderer: 'canvas' }} />;
};

export default ChartRenderer;
