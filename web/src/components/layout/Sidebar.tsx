import React, { useState, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { GitCommit, Calendar, Clock, ChevronLeft, ChevronRight } from 'lucide-react';

interface Run {
  id: string;
  hash: string;
  date: string;
  message?: string;
}

interface SidebarProps {
  runs: Run[];
  selectedRunId: string | null;
  onRunSelect: (runId: string) => void;
  width: number;
  isCollapsed: boolean;
  onWidthChange: (width: number) => void;
  onToggleCollapse: () => void;
}

export const Sidebar: React.FC<SidebarProps> = ({
  runs,
  selectedRunId,
  onRunSelect,
  width,
  isCollapsed,
  onWidthChange,
  onToggleCollapse,
}) => {
  const { t } = useTranslation();
  const [isResizing, setIsResizing] = useState(false);

  const startResizing = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setIsResizing(true);
  }, []);

  const stopResizing = useCallback(() => {
    setIsResizing(false);
  }, []);

  const resize = useCallback(
    (mouseMoveEvent: MouseEvent) => {
      if (isResizing) {
        const newWidth = mouseMoveEvent.clientX;
        if (newWidth > 180 && newWidth < 600) {
          onWidthChange(newWidth);
        }
      }
    },
    [isResizing, onWidthChange]
  );

  useEffect(() => {
    window.addEventListener("mousemove", resize);
    window.addEventListener("mouseup", stopResizing);
    return () => {
      window.removeEventListener("mousemove", resize);
      window.removeEventListener("mouseup", stopResizing);
    };
  }, [resize, stopResizing]);

  return (
    <aside
      className="bg-white dark:bg-slate-800 border-r border-slate-200 dark:border-slate-700 flex flex-col shrink-0 h-[calc(100vh-70px)] relative transition-width duration-200"
      style={{ width: isCollapsed ? 64 : width }}
    >
      {/* Toggle Button */}
      <button
        onClick={onToggleCollapse}
        className="absolute -right-3 top-4 z-50 bg-white dark:bg-slate-700 border border-slate-200 dark:border-slate-600 rounded-full p-1 text-slate-500 dark:text-slate-300 hover:text-sky-500 dark:hover:text-white hover:bg-slate-50 dark:hover:bg-slate-600 shadow-md transition-colors"
      >
        {isCollapsed ? <ChevronRight size={14} /> : <ChevronLeft size={14} />}
      </button>

      <div className={`p-6 pb-2 transition-opacity duration-200 ${isCollapsed ? 'px-2 flex justify-center' : ''}`}>
        <div className={`font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-4 flex items-center gap-2 ${isCollapsed ? 'justify-center' : ''}`}>
          <Clock className="w-4 h-4" />
          {!isCollapsed && <span className="text-xs">{t('sidebar.runHistory')}</span>}
        </div>
      </div>

      <div className="flex-1 overflow-y-auto px-3 pb-4 space-y-2 custom-scrollbar overflow-x-hidden">
        {runs.map((run) => (
          <div
            key={run.id}
            onClick={() => onRunSelect(run.id)}
            title={isCollapsed ? `${run.hash} - ${run.date}` : undefined}
            className={`
              rounded-lg cursor-pointer border transition-all duration-200 group
              ${selectedRunId === run.id
                ? 'bg-sky-50 dark:bg-sky-500/10 border-sky-200 dark:border-sky-500/50 shadow-sm'
                : 'bg-transparent border-transparent hover:bg-slate-100 dark:hover:bg-slate-700/50 hover:border-slate-200 dark:hover:border-slate-600'}
              ${isCollapsed ? 'p-2 flex justify-center' : 'p-3'}
            `}
          >
            <div className={`flex items-center ${isCollapsed ? 'justify-center' : 'gap-2 mb-1'} transition-all`}>
              <GitCommit className={`w-4 h-4 shrink-0 ${selectedRunId === run.id ? 'text-sky-600 dark:text-sky-400' : 'text-slate-400 dark:text-slate-500 group-hover:text-slate-500 dark:group-hover:text-slate-400'}`} />
              {!isCollapsed && (
                <span className={`font-mono font-semibold truncate ${selectedRunId === run.id ? 'text-sky-900 dark:text-sky-100' : 'text-slate-600 dark:text-slate-300'}`}>
                  {run.hash}
                </span>
              )}
            </div>
            {!isCollapsed && (
              <div className="flex items-center gap-1.5 pl-6 overflow-hidden">
                <Calendar className="w-3 h-3 text-slate-400 dark:text-slate-500 shrink-0" />
                <span className="text-xs text-slate-500 truncate">
                  {run.date}
                </span>
              </div>
            )}
          </div>
        ))}
      </div>

      {/* Resize Handle */}
      {!isCollapsed && (
        <div
          className="absolute right-0 top-0 bottom-0 w-1 cursor-ew-resize hover:bg-sky-500/50 transition-colors z-40"
          onMouseDown={startResizing}
        />
      )}
    </aside>
  );
};
