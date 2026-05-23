import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { X } from 'lucide-react';
import type { Run } from '@/services/data';

interface TrendScanSelectorProps {
  open: boolean;
  projectName: string;
  scanMode: 'snapshot' | 'diff';
  baseCommit?: string;
  initialSelectedScanIds?: number[];
  onConfirm: (scanIds: number[]) => void;
  onClose: () => void;
}

export function TrendScanSelector({
  open,
  projectName,
  scanMode,
  baseCommit,
  initialSelectedScanIds,
  onConfirm,
  onClose,
}: TrendScanSelectorProps) {
  const { t } = useTranslation();
  const [scans, setScans] = useState<Run[]>([]);
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
  const [loading, setLoading] = useState(false);

  const mode = scanMode === 'snapshot' ? 'SNAPSHOT' : 'DIFF';

  useEffect(() => {
    if (!open) return;

    const loadScans = async () => {
      setLoading(true);
      try {
        const { fetchRuns } = await import('@/services/data');
        const all = await fetchRuns(projectName, mode);
        // Only show available scans: for diff mode, filter by same base_commit
        const available = mode === 'DIFF' && baseCommit
          ? all.filter(s => s.base_commit_hash === baseCommit)
          : all;
        setScans(available);
      } finally {
        setLoading(false);
      }
    };

    loadScans();
  }, [open, projectName, mode, baseCommit]);

  useEffect(() => {
    if (initialSelectedScanIds && initialSelectedScanIds.length > 0) {
      // Only keep selected IDs that are in the available list
      const availableIds = new Set(scans.map(s => Number(s.id)));
      const validIds = initialSelectedScanIds.filter(id => availableIds.has(id));
      setSelectedIds(new Set(validIds));
    } else {
      // Default: select all available scans
      setSelectedIds(new Set(scans.map(s => Number(s.id))));
    }
  }, [scans, initialSelectedScanIds]);

  const toggleScan = (id: number) => {
    setSelectedIds(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const selectAll = () => {
    setSelectedIds(new Set(scans.map(s => Number(s.id))));
  };

  const clearAll = () => {
    setSelectedIds(new Set());
  };

  const formatDate = (dateStr: string) => {
    try {
      const d = new Date(dateStr);
      return d.toLocaleDateString() + ' ' + d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    } catch {
      return dateStr;
    }
  };

  if (!open) return null;

  const contextLabel = mode === 'SNAPSHOT'
    ? t('trend.snapshots')
    : `${t('trend.diffs')} (${t('trend.baseCommit')}: ${baseCommit ? baseCommit.substring(0, 8) : '?'})`;

  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/60 backdrop-blur-sm" onClick={onClose}>
      <div
        className="bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-600 rounded-xl shadow-2xl max-w-3xl w-full mx-4 max-h-[85vh] flex flex-col overflow-hidden"
        onClick={e => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-slate-200 dark:border-slate-700">
          <h2 className="text-lg font-semibold text-slate-800 dark:text-slate-200">
            {t('trend.selectScans')}
          </h2>
          <button onClick={onClose} className="p-1.5 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-700 text-slate-400 hover:text-sky-500 transition-colors">
            <X className="h-5 w-5" />
          </button>
        </div>

        {/* Context bar */}
        <div className="px-4 py-2 border-b border-slate-100 dark:border-slate-700/50">
          <span className="text-xs font-medium text-slate-500 dark:text-slate-400">
            {contextLabel}
          </span>
        </div>

        {/* Quick Select */}
        <div className="flex items-center gap-2 px-4 py-2 border-b border-slate-100 dark:border-slate-700/50">
          <button onClick={selectAll} className="px-2.5 py-1 text-xs font-medium rounded bg-slate-100 dark:bg-slate-700 text-slate-600 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-600 transition-colors">
            {t('trend.selectAll')}
          </button>
          <button onClick={clearAll} className="px-2.5 py-1 text-xs font-medium rounded bg-slate-100 dark:bg-slate-700 text-slate-600 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-600 transition-colors">
            {t('trend.clearSelection')}
          </button>
          <span className="text-xs text-slate-400 ml-auto">
            {t('trend.selectedCount', { count: selectedIds.size })}
          </span>
        </div>

        {/* Scan List */}
        <div className="flex-1 overflow-y-auto p-4 space-y-1">
          {loading ? (
            <div className="flex items-center justify-center h-32 text-slate-400">
              <div className="animate-pulse">{t('common.loading')}</div>
            </div>
          ) : scans.length === 0 ? (
            <div className="flex items-center justify-center h-32 text-slate-400">
              <p className="text-sm">{t('trend.noScans')}</p>
            </div>
          ) : (
            scans.map(scan => (
              <label
                key={scan.id}
                className={`flex items-center gap-3 px-3 py-2 rounded-lg cursor-pointer transition-colors ${
                  selectedIds.has(Number(scan.id))
                    ? 'bg-sky-50 dark:bg-sky-900/20 text-sky-700 dark:text-sky-300'
                    : 'hover:bg-slate-50 dark:hover:bg-slate-700/50 text-slate-700 dark:text-slate-300'
                }`}
              >
                <input
                  type="checkbox"
                  checked={selectedIds.has(Number(scan.id))}
                  onChange={() => toggleScan(Number(scan.id))}
                  className="rounded border-slate-300 dark:border-slate-600 text-sky-500 focus:ring-sky-500"
                />
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="text-xs font-mono text-slate-400">{scan.commit_hash?.substring(0, 8)}</span>
                    <span className="text-xs text-slate-400">{formatDate(scan.scan_time)}</span>
                    {mode === 'DIFF' && (
                      <span className="text-xs px-1.5 py-0.5 rounded bg-amber-100 dark:bg-amber-900/30 text-amber-600 dark:text-amber-400">
                        {t('trend.diff')}
                      </span>
                    )}
                  </div>
                </div>
              </label>
            ))
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between px-4 py-3 border-t border-slate-200 dark:border-slate-700">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm font-medium text-slate-600 dark:text-slate-400 hover:text-slate-800 dark:hover:text-slate-200 transition-colors"
          >
            {t('common.cancel')}
          </button>
          <button
            onClick={() => onConfirm(Array.from(selectedIds))}
            disabled={selectedIds.size === 0}
            className="px-4 py-2 text-sm font-medium rounded-lg bg-sky-500 text-white hover:bg-sky-600 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {t('trend.showTrend', { count: selectedIds.size })}
          </button>
        </div>
      </div>
    </div>
  );
}
