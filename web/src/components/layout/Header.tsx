import React from 'react';
import { Box, Layers, GitGraph } from 'lucide-react';

interface HeaderProps {
  viewMode: 'snapshot' | 'diff';
  onViewModeChange: (mode: 'snapshot' | 'diff') => void;
}

export const Header: React.FC<HeaderProps> = ({
  viewMode,
  onViewModeChange,
}) => {
  return (
    <header className="h-[70px] bg-slate-900/95 border-b border-slate-700 flex items-center justify-between px-8 shrink-0 z-10 sticky top-0 backdrop-blur-md">
      <div className="flex items-center gap-8">
        <div className="flex items-center gap-2 text-sky-400 text-2xl font-bold">
          <Box className="w-8 h-8" />
          CodePrism
        </div>

        <div className="flex items-center gap-3">
          <span className="text-slate-400 text-sm font-medium uppercase tracking-wide">Project:</span>
          <div className="bg-slate-800 text-slate-100 border border-slate-700 px-4 py-2 rounded-lg opacity-80 cursor-not-allowed">
            code-prism
          </div>
        </div>
      </div>

      <div className="flex bg-slate-800 p-1 rounded-lg border border-slate-700">
        <button
          onClick={() => onViewModeChange('snapshot')}
          className={`
            px-5 py-1.5 rounded-md text-sm font-medium transition-all flex items-center gap-2
            ${viewMode === 'snapshot'
              ? 'bg-sky-500 text-slate-900 shadow-lg shadow-sky-500/20'
              : 'text-slate-400 hover:text-slate-100 hover:bg-slate-700/50'}
          `}
        >
          <Layers className="w-4 h-4" />
          Snapshot
        </button>
        <button
          onClick={() => onViewModeChange('diff')}
          className={`
            px-5 py-1.5 rounded-md text-sm font-medium transition-all flex items-center gap-2
            ${viewMode === 'diff'
              ? 'bg-sky-500 text-slate-900 shadow-lg shadow-sky-500/20'
              : 'text-slate-400 hover:text-slate-100 hover:bg-slate-700/50'}
          `}
        >
          <GitGraph className="w-4 h-4" />
          Diff
        </button>
      </div>
    </header>
  );
};
