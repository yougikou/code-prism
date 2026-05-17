import { useState, useMemo, useCallback } from 'react';
import { X, Search, CaseSensitive, Regex, Download } from 'lucide-react';
import { useTranslation } from 'react-i18next';

interface LeafItem {
  label: string;
  value: number;
  group?: string;
}

interface ChildrenViewerProps {
  open: boolean;
  title: string;
  items: LeafItem[];
  onClose: () => void;
  onFileClick?: (label: string) => void;
}

export function ChildrenViewer({ open, title, items, onClose, onFileClick }: ChildrenViewerProps) {
  const { t } = useTranslation();
  const [searchQuery, setSearchQuery] = useState('');
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [useRegex, setUseRegex] = useState(false);

  const filteredItems = useMemo(() => {
    if (!searchQuery.trim()) return items;

    let matchFn: (text: string) => boolean;

    try {
      if (useRegex) {
        const flags = caseSensitive ? 'u' : 'iu';
        const regex = new RegExp(searchQuery, flags);
        matchFn = (text: string) => regex.test(text);
      } else if (caseSensitive) {
        matchFn = (text: string) => text.includes(searchQuery);
      } else {
        const lowerQuery = searchQuery.toLowerCase();
        matchFn = (text: string) => text.toLowerCase().includes(lowerQuery);
      }
    } catch {
      // Invalid regex — show all items
      return items;
    }

    return items.filter(item => matchFn(item.label) || (item.group && matchFn(item.group)));
  }, [items, searchQuery, caseSensitive, useRegex]);

  const hasGroup = items.length > 0 && items.some(item => item.group);

  const [copiedLabel, setCopiedLabel] = useState<string | null>(null);

  const copyPath = useCallback(async (label: string) => {
    try {
      await navigator.clipboard.writeText(label);
      setCopiedLabel(label);
      setTimeout(() => setCopiedLabel(null), 1500);
    } catch {
      // Clipboard API may fail in insecure contexts
    }
  }, []);

  const downloadCSV = useCallback(() => {
    const headers = hasGroup ? ['Group', 'Label', 'Value'] : ['Label', 'Value'];
    const rows = filteredItems.map(item => {
      const value = Math.round(item.value).toString();
      return hasGroup
        ? [item.group || '', item.label, value]
        : [item.label, value];
    });

    const csvContent = [headers.join(','), ...rows.map(r => r.map(c => `"${c.replace(/"/g, '""')}"`).join(','))].join('\n');
    const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${title.replace(/\s+/g, '_')}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  }, [filteredItems, hasGroup, title]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm" onClick={onClose}>
      <div
        className="bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-600 rounded-xl shadow-2xl max-w-3xl w-full mx-4 max-h-[80vh] flex flex-col overflow-hidden"
        onClick={e => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between p-5 border-b border-slate-200 dark:border-slate-700">
          <h2 className="text-lg font-semibold text-slate-800 dark:text-slate-200">
            {title}
          </h2>
          <button
            onClick={onClose}
            className="p-1.5 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-700 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 transition-colors"
          >
            <X className="h-5 w-5" />
          </button>
        </div>

        {/* Search bar */}
        <div className="flex items-center gap-2 px-5 py-3 border-b border-slate-200 dark:border-slate-700">
          <div className="relative flex-1">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-slate-400" />
            <input
              type="text"
              value={searchQuery}
              onChange={e => setSearchQuery(e.target.value)}
              placeholder={t('common.searchPlaceholder') || 'Search paths...'}
              className="w-full pl-9 pr-3 py-2 text-sm rounded-lg border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-700 text-slate-700 dark:text-slate-200 placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-sky-500/50 dark:focus:ring-sky-400/50 transition-shadow"
              autoFocus
            />
          </div>
          <button
            onClick={() => setCaseSensitive(!caseSensitive)}
            className={`p-2 rounded-lg border transition-colors ${
              caseSensitive
                ? 'bg-sky-100 dark:bg-sky-900/40 border-sky-300 dark:border-sky-600 text-sky-600 dark:text-sky-400'
                : 'border-slate-300 dark:border-slate-600 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-700'
            }`}
            title={caseSensitive ? t('common.caseSensitiveOn') || 'Case-sensitive: ON' : t('common.caseSensitiveOff') || 'Case-sensitive: OFF'}
          >
            <CaseSensitive className="h-4 w-4" />
          </button>
          <button
            onClick={() => setUseRegex(!useRegex)}
            className={`p-2 rounded-lg border transition-colors ${
              useRegex
                ? 'bg-sky-100 dark:bg-sky-900/40 border-sky-300 dark:border-sky-600 text-sky-600 dark:text-sky-400'
                : 'border-slate-300 dark:border-slate-600 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-700'
            }`}
            title={useRegex ? t('common.regexOn') || 'Regex: ON' : t('common.regexOff') || 'Regex: OFF'}
          >
            <Regex className="h-4 w-4" />
          </button>
          <button
            onClick={downloadCSV}
            className="p-2 rounded-lg border border-slate-300 dark:border-slate-600 text-slate-400 hover:text-sky-600 dark:hover:text-sky-400 hover:bg-slate-100 dark:hover:bg-slate-700 transition-colors"
            title={t('common.downloadCSV') || 'Download CSV'}
          >
            <Download className="h-4 w-4" />
          </button>
        </div>

        {/* Body */}
        <div className="flex-1 overflow-y-auto p-5">
          {filteredItems.length === 0 ? (
            <p className="text-center text-slate-400 dark:text-slate-500 py-8">
              {t('dashboard.noData')}
            </p>
          ) : (
            <table className="w-full text-sm text-left">
              <thead className="text-xs text-slate-500 dark:text-slate-400 uppercase bg-slate-100 dark:bg-slate-700/50 sticky top-0">
                <tr>
                  {hasGroup && <th className="px-4 py-2 whitespace-nowrap">{t('table.group')}</th>}
                  <th className="px-4 py-2 w-full">{t('table.label')}</th>
                  <th className="px-4 py-2 text-right whitespace-nowrap">{t('table.value')}</th>
                </tr>
              </thead>
              <tbody>
                {filteredItems.map((item, i) => (
                  <tr
                    key={i}
                    onClick={() => copyPath(item.label)}
                    className="border-b border-slate-100 dark:border-slate-700/30 hover:bg-slate-50 dark:hover:bg-slate-700/20 transition-colors cursor-pointer"
                  >
                    {hasGroup && (
                      <td className="px-4 py-2 text-xs text-slate-500 dark:text-slate-400 whitespace-nowrap">
                        {item.group || '-'}
                      </td>
                    )}
                    <td className="px-4 py-2 font-mono text-xs text-slate-700 dark:text-slate-300 break-all max-w-[500px]">
                      {onFileClick ? (
                        <div className="flex items-center gap-2">
                          <button
                            onClick={e => { e.stopPropagation(); onFileClick(item.label); }}
                            className="text-left hover:text-sky-600 dark:hover:text-sky-400 hover:underline transition-colors truncate"
                          >
                            {item.label}
                          </button>
                          {copiedLabel === item.label && (
                            <span className="shrink-0 text-[10px] text-sky-500 dark:text-sky-400 animate-pulse">
                              Copied!
                            </span>
                          )}
                        </div>
                      ) : (
                        <div className="flex items-center gap-2">
                          <span className="truncate">{item.label}</span>
                          {copiedLabel === item.label && (
                            <span className="shrink-0 text-[10px] text-sky-500 dark:text-sky-400 animate-pulse">
                              Copied!
                            </span>
                          )}
                        </div>
                      )}
                    </td>
                    <td className="px-4 py-2 text-right text-slate-600 dark:text-slate-400 tabular-nums">
                      {Math.round(item.value).toLocaleString()}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>

        {/* Footer with count */}
        <div className="flex items-center justify-between px-5 py-3 border-t border-slate-200 dark:border-slate-700 text-xs text-slate-400 dark:text-slate-500">
          <span>
            {filteredItems.length} / {items.length} {t('dashboard.items')}
          </span>
        </div>
      </div>
    </div>
  );
}
