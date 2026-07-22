import * as echarts from 'echarts/core'
import { BarChart, GaugeChart, HeatmapChart, LineChart, PieChart, RadarChart } from 'echarts/charts'
import {
  DataZoomComponent,
  GridComponent,
  LegendComponent,
  RadarComponent,
  TooltipComponent,
  VisualMapComponent,
} from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'

echarts.use([
  BarChart,
  GaugeChart,
  HeatmapChart,
  LineChart,
  PieChart,
  RadarChart,
  DataZoomComponent,
  GridComponent,
  LegendComponent,
  RadarComponent,
  TooltipComponent,
  VisualMapComponent,
  CanvasRenderer,
])

export default echarts
