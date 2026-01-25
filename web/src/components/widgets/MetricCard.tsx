import React from 'react';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card';

interface MetricCardProps {
  title: string;
  value: string | number;
  subValue?: string;
  loading?: boolean;
}

export const MetricCard: React.FC<MetricCardProps> = ({ title, value, subValue, loading }) => {
  return (
    <Card className="border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800/50 backdrop-blur shadow-sm dark:shadow-xl transition-all">
      <CardHeader>
        <CardTitle className="text-slate-500 dark:text-slate-200 text-sm font-medium uppercase tracking-wide">
          {title}
        </CardTitle>
      </CardHeader>
      <CardContent>
        {loading ? (
          <div className="animate-pulse h-16 bg-slate-100 dark:bg-slate-700/20 rounded"></div>
        ) : (
          <div>
            <div className="text-5xl font-bold text-slate-800 dark:text-slate-100">{value}</div>
            {subValue && (
              <div className="text-sm text-slate-500 mt-2">{subValue}</div>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  );
};
