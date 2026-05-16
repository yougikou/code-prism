import { X } from 'lucide-react';
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
}

export function ChildrenViewer({ open, title, items, onClose }: ChildrenViewerProps) {
  const { t } = useTranslation();

  if (!open) return null;

  const hasGroup = items.length > 0 && items.some(item => item.group);

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

        {/* Body */}
        <div className="flex-1 overflow-y-auto p-5">
          {items.length === 0 ? (
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
                {items.map((item, i) => (
                  <tr
                    key={i}
                    className="border-b border-slate-100 dark:border-slate-700/30 hover:bg-slate-50 dark:hover:bg-slate-700/20 transition-colors"
                  >
                    {hasGroup && (
                      <td className="px-4 py-2 text-xs text-slate-500 dark:text-slate-400 whitespace-nowrap">
                        {item.group || '-'}
                      </td>
                    )}
                    <td className="px-4 py-2 font-mono text-xs text-slate-700 dark:text-slate-300 break-all max-w-[500px]">
                      {item.label}
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
          <span>{items.length} {t('dashboard.items')}</span>
        </div>
      </div>
    </div>
  );
}
