import { X, ArrowLeft, Code } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { MatchDetail } from '@/services/data';

interface MatchDetailViewProps {
  open: boolean;
  title: string;
  filePath: string;
  matches: MatchDetail[];
  total: number;
  loading: boolean;
  onClose: () => void;
  onBack: () => void;
}

export function MatchDetailView({
  open,
  title,
  filePath,
  matches,
  total,
  loading,
  onClose,
  onBack,
}: MatchDetailViewProps) {
  const { t } = useTranslation();

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm" onClick={onClose}>
      <div
        className="bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-600 rounded-xl shadow-2xl max-w-5xl w-full mx-4 max-h-[85vh] flex flex-col overflow-hidden"
        onClick={e => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-slate-200 dark:border-slate-700">
          <div className="flex items-center gap-3 min-w-0">
            <button
              onClick={onBack}
              className="p-1.5 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-700 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 transition-colors shrink-0"
              title={t('common.back')}
            >
              <ArrowLeft className="h-5 w-5" />
            </button>
            <div className="min-w-0">
              <h2 className="text-lg font-semibold text-slate-800 dark:text-slate-200 truncate">
                {title}
              </h2>
              <p className="text-xs text-slate-500 dark:text-slate-400 font-mono truncate">
                {filePath}
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-1.5 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-700 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 transition-colors shrink-0"
          >
            <X className="h-5 w-5" />
          </button>
        </div>

        {/* Body */}
        <div className="flex-1 overflow-y-auto p-4">
          {loading ? (
            <div className="flex items-center justify-center py-16">
              <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-sky-500" />
            </div>
          ) : matches.length === 0 ? (
            <p className="text-center text-slate-400 dark:text-slate-500 py-16">
              {t('dashboard.noData')}
            </p>
          ) : (
            <div className="space-y-2">
              {matches.map((match, i) => (
                <div
                  key={i}
                  className="border border-slate-200 dark:border-slate-700 rounded-lg overflow-hidden"
                >
                  {/* Match header: line number */}
                  <div className="flex items-center gap-2 px-3 py-1.5 bg-slate-50 dark:bg-slate-700/30 border-b border-slate-200 dark:border-slate-700 text-xs text-slate-500 dark:text-slate-400">
                    <Code className="h-3.5 w-3.5" />
                    <span className="font-mono font-medium">
                      L{match.line_number}
                      {match.column_start != null && `:${match.column_start}`}
                    </span>
                    {match.analyzer_id && (
                      <span className="ml-auto font-mono text-slate-400 dark:text-slate-500">
                        {match.analyzer_id}
                      </span>
                    )}
                  </div>

                  {/* Context before */}
                  {match.context_before && (
                    <div className="px-3 py-1 bg-slate-100/50 dark:bg-slate-700/10 text-xs text-slate-400 dark:text-slate-500 font-mono border-b border-slate-100 dark:border-slate-700/30 truncate">
                      {match.context_before}
                    </div>
                  )}

                  {/* Matched text */}
                  <div className="px-3 py-2 bg-yellow-50 dark:bg-yellow-900/10 border-b border-yellow-100 dark:border-yellow-800/20">
                    <code className="text-sm font-mono text-slate-800 dark:text-yellow-200 break-all whitespace-pre-wrap">
                      {match.matched_text}
                    </code>
                  </div>

                  {/* Context after */}
                  {match.context_after && (
                    <div className="px-3 py-1 bg-slate-100/50 dark:bg-slate-700/10 text-xs text-slate-400 dark:text-slate-500 font-mono truncate">
                      {match.context_after}
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between px-4 py-3 border-t border-slate-200 dark:border-slate-700 text-xs text-slate-400 dark:text-slate-500">
          <span>
            {loading ? '...' : `${matches.length} / ${total} ${t('dashboard.items')}`}
          </span>
        </div>
      </div>
    </div>
  );
}
