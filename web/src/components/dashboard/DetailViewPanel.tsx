import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  type DuplicationInfo,
  type DuplicationsResponse,
  fetchDuplications,
} from '@/services/data';
import { X, BarChart3 } from 'lucide-react';

interface DetailViewPanelProps {
  open: boolean;
  projectName: string;
  scanId: number | string;
  onClose: () => void;
  onItemClick: (dup: DuplicationInfo) => void;
}

/** Extract a short one-line heading from a block of code. */
function firstMeaningfulLine(code: string, maxLen = 80): string {
  const lines = code.split('\n');
  for (const raw of lines) {
    const line = raw.trim();
    if (!line) continue;
    if (/^[{}\[\]()#\s/*]+$/.test(line)) continue;
    return line.length > maxLen ? line.slice(0, maxLen) + '…' : line;
  }
  return '(empty block)';
}

/** Parse scope string like "analyzerName:42-89" into { lineStart, lineEnd } or null. */
function parseScope(scope: string | null): { start: number; end: number } | null {
  if (!scope) return null;
  const parts = scope.split(':');
  if (parts.length < 2) return null;
  const rangeParts = parts[parts.length - 1].split('-');
  const start = parseInt(rangeParts[0], 10);
  const end = parseInt(rangeParts[1], 10);
  if (isNaN(start) || isNaN(end)) return null;
  return { start, end };
}

/** Group duplications by analyzer_id (strip trailing _aggregated for display). */
function displayAnalyzerId(raw: string): string {
  return raw.replace(/_aggregated$/, '');
}

// ─── Stats Row ─────────────────────────────────────────────

function StatsRow({ data }: { data: DuplicationInfo[] }) {
  const { t } = useTranslation();

  const byAnalyzer = new Map<string, DuplicationInfo[]>();
  for (const d of data) {
    const key = displayAnalyzerId(d.analyzer_id);
    if (!byAnalyzer.has(key)) byAnalyzer.set(key, []);
    byAnalyzer.get(key)!.push(d);
  }

  const totalBlocks = data.length;
  const totalFiles = new Set(data.flatMap(d => d.files.map(f => f.path))).size;
  const maxOccurrences = Math.max(...data.map(d => d.occurrence_count), 0);

  const stats = [
    { label: t('duplications.totalBlocks', 'Duplicate Blocks'), value: totalBlocks.toLocaleString(), color: 'text-sky-600 dark:text-sky-400' },
    { label: t('duplications.affectedFiles', 'Affected Files'), value: totalFiles.toLocaleString(), color: 'text-violet-600 dark:text-violet-400' },
    { label: t('duplications.maxOccurrences', 'Max Occurrences'), value: maxOccurrences.toLocaleString(), color: 'text-amber-600 dark:text-amber-400' },
    { label: t('duplications.analyzers', 'Analyzers'), value: byAnalyzer.size.toLocaleString(), color: 'text-emerald-600 dark:text-emerald-400' },
  ];

  return (
    <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
      {stats.map(s => (
        <div key={s.label} className="bg-white dark:bg-slate-800/50 backdrop-blur border border-slate-200 dark:border-slate-700 rounded-xl p-4 shadow-sm dark:shadow-xl">
          <p className="text-xs text-slate-500 dark:text-slate-400 mb-1">{s.label}</p>
          <p className={`text-2xl font-bold ${s.color}`}>{s.value}</p>
        </div>
      ))}
    </div>
  );
}

// ─── Analyzer Card ─────────────────────────────────────────

function AnalyzerCard({
  title,
  items,
  onItemClick,
}: {
  title: string;
  items: DuplicationInfo[];
  onItemClick: (d: DuplicationInfo) => void;
}) {
  const { t } = useTranslation();
  const [showAll, setShowAll] = useState(false);
  const displayItems = showAll ? items : items.slice(0, 10);

  if (items.length === 0) return null;

  const maxOccurrences = Math.max(...items.map(d => d.occurrence_count), 1);

  return (
    <div className="bg-white dark:bg-slate-800/50 backdrop-blur border border-slate-200 dark:border-slate-700 rounded-xl shadow-sm dark:shadow-xl transition-colors duration-200 overflow-hidden">
      <div className="flex items-center justify-between px-5 py-4 border-b border-slate-100 dark:border-slate-700/50">
        <div>
          <h3 className="text-lg font-semibold text-slate-800 dark:text-slate-200">{title}</h3>
          <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
            {t('duplications.count', '{{count}} duplicate blocks', { count: items.length })}
          </p>
        </div>
        <span className="text-xs font-medium text-slate-400 dark:text-slate-500 bg-slate-100 dark:bg-slate-700/50 px-2.5 py-1 rounded-full">
          {t('duplications.occurrences', 'Occurrences')}
        </span>
      </div>

      <div className="divide-y divide-slate-100 dark:divide-slate-700/50">
        {displayItems.map((dup) => {
          const pct = (dup.occurrence_count / maxOccurrences) * 100;
          const firstLine = firstMeaningfulLine(dup.block_content);
          const lineInfo = parseScope(dup.files[0]?.scope ?? null);
          const fileCount = dup.files.length;

          return (
            <button
              key={dup.content_hash}
              onClick={() => onItemClick(dup)}
              className="w-full flex items-center gap-3 px-5 py-3 text-left hover:bg-slate-50 dark:hover:bg-slate-700/30 transition-colors group"
            >
              <div className="flex-1 min-w-0">
                <div className="flex items-center justify-between mb-1">
                  <span className="text-sm font-medium text-slate-700 dark:text-slate-300 truncate group-hover:text-sky-600 dark:group-hover:text-sky-400 transition-colors">
                    {firstLine}
                  </span>
                  <span className="text-sm font-bold text-sky-600 dark:text-sky-400 ml-3 tabular-nums shrink-0">
                    {dup.occurrence_count}
                  </span>
                </div>
                <div className="flex items-center gap-2 text-xs text-slate-400 dark:text-slate-500">
                  <span>{fileCount} {fileCount > 1 ? 'files' : 'file'}</span>
                  {lineInfo && (
                    <span>lines {lineInfo.start}–{lineInfo.end}</span>
                  )}
                </div>
                <div className="mt-1.5 h-1.5 bg-slate-100 dark:bg-slate-700/50 rounded-full overflow-hidden">
                  <div
                    className="h-full bg-gradient-to-r from-sky-400 to-sky-500 rounded-full transition-all duration-300"
                    style={{ width: `${pct}%` }}
                  />
                </div>
              </div>
            </button>
          );
        })}
      </div>

      {items.length > 10 && (
        <div className="px-5 py-3 border-t border-slate-100 dark:border-slate-700/50">
          <button
            onClick={() => setShowAll(v => !v)}
            className="text-xs font-medium text-sky-600 dark:text-sky-400 hover:text-sky-700 dark:hover:text-sky-300 transition-colors"
          >
            {showAll
              ? t('duplications.showLess', 'Show less')
              : t('duplications.showMore', 'Show all {{count}}', { count: items.length })}
          </button>
        </div>
      )}
    </div>
  );
}

// ─── Main Panel ────────────────────────────────────────────

export function DetailViewPanel({
  open,
  projectName,
  scanId,
  onClose,
  onItemClick,
}: DetailViewPanelProps) {
  const { t } = useTranslation();
  const [data, setData] = useState<DuplicationInfo[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);

  const loadData = useCallback(async () => {
    setLoading(true);
    try {
      const res: DuplicationsResponse = await fetchDuplications(projectName, scanId, {
        page: 1,
        page_size: 200,
        min_occurrences: 2,
      });
      setData(res.duplications);
      setTotal(res.total);
    } catch (err) {
      console.error('Failed to load detail data:', err);
    } finally {
      setLoading(false);
    }
  }, [projectName, scanId]);

  useEffect(() => {
    if (open) {
      loadData();
    }
  }, [open, loadData]);

  // Group by analyzer
  const byAnalyzer = new Map<string, DuplicationInfo[]>();
  for (const d of data) {
    const key = displayAnalyzerId(d.analyzer_id);
    if (!byAnalyzer.has(key)) byAnalyzer.set(key, []);
    byAnalyzer.get(key)!.push(d);
  }

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-[60] flex items-center justify-center bg-black/60 backdrop-blur-sm p-4 sm:p-8"
      onClick={onClose}
    >
      <div
        className="w-full max-w-4xl max-h-[90vh] flex flex-col bg-white dark:bg-slate-800/95 backdrop-blur border border-slate-200 dark:border-slate-700 rounded-2xl shadow-2xl overflow-hidden"
        onClick={e => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-slate-100 dark:border-slate-700/50 shrink-0">
          <div className="min-w-0 flex-1">
            <h2 className="text-lg font-semibold text-slate-800 dark:text-slate-200 flex items-center gap-2">
              <BarChart3 className="h-5 w-5 text-sky-500" />
              {t('duplications.detailViewTitle', 'Duplication Details')}
            </h2>
            <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
              {total} {total > 1 ? 'blocks' : 'block'} across {data.length > 0 ? new Set(data.flatMap(d => d.files.map(f => f.path))).size : 0} files
            </p>
          </div>
          <button
            onClick={onClose}
            className="p-2 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-700 text-slate-400 hover:text-sky-500 transition-colors shrink-0"
            aria-label="Close"
          >
            <X className="h-5 w-5" />
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-6 space-y-6">
          {loading ? (
            <div className="flex items-center justify-center py-20">
              <div className="animate-spin h-8 w-8 border-4 border-sky-500/30 border-t-sky-500 rounded-full" />
            </div>
          ) : data.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-20 text-slate-500 dark:text-slate-400">
              <div className="text-6xl mb-4 opacity-30">🔍</div>
              <p className="text-lg font-medium mb-2">{t('duplications.noDuplications', 'No duplicate blocks found')}</p>
              <p className="text-sm text-slate-400 dark:text-slate-500 max-w-md text-center">
                {t('duplications.noDuplicationsDesc', 'No cross-file duplicate blocks were detected in this scan.')}
              </p>
            </div>
          ) : (
            <>
              <StatsRow data={data} />
              <div className="space-y-6">
                {Array.from(byAnalyzer.entries()).map(([analyzerId, items]) => (
                  <AnalyzerCard
                    key={analyzerId}
                    title={analyzerId}
                    items={items}
                    onItemClick={onItemClick}
                  />
                ))}
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
