import { cn } from '@/lib/utils';

export function Skeleton({ className, style }: { className?: string; style?: React.CSSProperties }) {
  return (
    <div className={cn('animate-pulse rounded-md bg-slate-200 dark:bg-slate-700/40', className)} style={style} />
  );
}

export function ChartSkeleton({ type = 'bar', height = '300px' }: { type?: 'bar' | 'card' | 'table'; height?: string }) {
  if (type === 'card') {
    return (
      <div className="space-y-4 p-6" style={{ height }}>
        <Skeleton className="h-4 w-1/2" />
        <Skeleton className="h-12 w-3/4" />
        <Skeleton className="h-3 w-1/4" />
      </div>
    );
  }

  if (type === 'table') {
    return (
      <div className="space-y-3 p-4" style={{ height }}>
        <Skeleton className="h-4 w-full" />
        <Skeleton className="h-3 w-full" />
        <Skeleton className="h-3 w-11/12" />
        <Skeleton className="h-3 w-4/5" />
        <Skeleton className="h-3 w-9/12" />
        <Skeleton className="h-3 w-3/4" />
      </div>
    );
  }

  // Default: bar chart skeleton
  return (
    <div className="flex items-end gap-3 px-6 pb-6 pt-8" style={{ height }}>
      {[70, 45, 90, 55, 80, 35, 65].map((h, i) => (
        <Skeleton key={i} className="flex-1 rounded-t" style={{ height: `${h}%` }} />
      ))}
    </div>
  );
}
