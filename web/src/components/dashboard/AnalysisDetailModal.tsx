import { useTranslation } from 'react-i18next';
import type { DuplicationInfo } from '@/services/data';
import { X, Code, FileCode, Hash, Layers, ExternalLink } from 'lucide-react';

export interface AnalysisDetailModalProps {
  open: boolean;
  /** Duplication data (category=duplication). Pass either duplication or a generic item. */
  duplication: DuplicationInfo | null;
  /** Analysis category for display logic. Defaults to "duplication". */
  category?: string;
  viewMode?: 'snapshot' | 'diff';
  onClose: () => void;
  onFileClick: (filePath: string, analyzerId: string, contentHash: string) => void;
}

/** Parse scope string like "analyzerName:42-89" into { start, end } or null. */
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

export function AnalysisDetailModal({
  open,
  duplication,
  viewMode,
  onClose,
  onFileClick,
}: AnalysisDetailModalProps) {
  const { t } = useTranslation();

  if (!open || !duplication) return null;

  const analyzerLabel = duplication.analyzer_id.replace(/_aggregated$/, '');
  const isDiff = viewMode === 'diff';

  return (
    <div
      className="fixed inset-0 z-[60] flex items-center justify-center bg-black/60 backdrop-blur-sm p-4 sm:p-8"
      onClick={onClose}
    >
      <div
        className="w-full max-w-4xl max-h-[90vh] flex flex-col bg-white dark:bg-slate-800/95 backdrop-blur border border-slate-200 dark:border-slate-700 rounded-2xl shadow-2xl overflow-hidden"
        onClick={e => e.stopPropagation()}
      >
        {/* ─── Header ─────────────────────────────────────────── */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-slate-100 dark:border-slate-700/50 shrink-0">
          <div className="min-w-0 flex-1">
            <h2 className="text-lg font-semibold text-slate-800 dark:text-slate-200 truncate">
              {t('duplications.detailTitle', 'Duplicate Block')}
            </h2>
            <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
              {analyzerLabel}
              {' · '}
              {duplication.occurrence_count} {duplication.occurrence_count > 1 ? 'occurrences' : 'occurrence'}
              {' · '}
              {duplication.files.length} {duplication.files.length > 1 ? 'files' : 'file'}
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

        {/* ─── Content Area (scrollable) ──────────────────────── */}
        <div className="flex-1 overflow-y-auto p-6 space-y-6">
          {/* Code preview */}
          <div>
            <h3 className="text-sm font-semibold text-slate-700 dark:text-slate-300 mb-2 flex items-center gap-2">
              <Code className="h-4 w-4 text-sky-500" />
              {t('duplications.blockContent', 'Block Content')}
            </h3>
            <pre className="text-xs leading-relaxed font-mono bg-slate-50 dark:bg-slate-900/50 border border-slate-200 dark:border-slate-700 rounded-lg p-4 overflow-x-auto whitespace-pre text-slate-800 dark:text-slate-200 max-h-[40vh] overflow-y-auto">
              {duplication.block_content || '(empty)'}
            </pre>
          </div>

          {/* File list */}
          <div>
            <h3 className="text-sm font-semibold text-slate-700 dark:text-slate-300 mb-3 flex items-center gap-2">
              <Layers className="h-4 w-4 text-sky-500" />
              {t('duplications.files', 'Files')}
              <span className="text-xs font-normal text-slate-400 dark:text-slate-500 ml-1">
                ({duplication.files.length})
              </span>
            </h3>

            <div className="space-y-2">
              {duplication.files.map((file, idx) => {
                const scopeInfo = parseScope(file.scope);
                const hasDiff = isDiff && (file.value_before !== file.value_after);

                return (
                  <button
                    key={`${file.path}-${idx}`}
                    onClick={() =>
                      onFileClick(file.path, duplication.analyzer_id, duplication.content_hash)
                    }
                    className="w-full flex items-center justify-between gap-3 px-4 py-3 bg-slate-50 dark:bg-slate-900/30 hover:bg-sky-50 dark:hover:bg-sky-900/20 border border-slate-200 dark:border-slate-700 hover:border-sky-300 dark:hover:border-sky-700 rounded-lg transition-all group text-left"
                  >
                    <div className="flex items-center gap-3 min-w-0">
                      <FileCode className="h-4 w-4 text-slate-400 group-hover:text-sky-500 shrink-0 transition-colors" />
                      <div className="min-w-0">
                        <p className="text-sm font-medium text-slate-700 dark:text-slate-300 truncate group-hover:text-sky-600 dark:group-hover:text-sky-400 transition-colors">
                          {file.path}
                        </p>
                        <div className="flex items-center gap-3 text-xs text-slate-400 dark:text-slate-500 mt-0.5">
                          {scopeInfo && (
                            <span className="flex items-center gap-1">
                              <Hash className="h-3 w-3" />
                              lines {scopeInfo.start}–{scopeInfo.end}
                            </span>
                          )}
                          {hasDiff && (
                            <span className="text-amber-500 font-medium">
                              {Math.round(file.value_before)} → {Math.round(file.value_after)}
                            </span>
                          )}
                        </div>
                      </div>
                    </div>
                    <ExternalLink className="h-4 w-4 text-slate-400 group-hover:text-sky-500 shrink-0 transition-colors" />
                  </button>
                );
              })}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
