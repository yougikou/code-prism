import React from 'react';
import { GitCommit, Calendar, Clock } from 'lucide-react';

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
}

export const Sidebar: React.FC<SidebarProps> = ({
  runs,
  selectedRunId,
  onRunSelect,
}) => {
  return (
    <aside className="w-[280px] bg-slate-800 border-r border-slate-700 flex flex-col shrink-0 h-[calc(100vh-70px)]">
      <div className="p-6 pb-2">
        <h3 className="text-xs font-bold text-slate-400 uppercase tracking-wider mb-4 flex items-center gap-2">
          <Clock className="w-3 h-3" />
          Run History
        </h3>
      </div>

      <div className="flex-1 overflow-y-auto px-4 pb-4 space-y-2 custom-scrollbar">
        {runs.map((run) => (
          <div
            key={run.id}
            onClick={() => onRunSelect(run.id)}
            className={`
              p-3 rounded-lg cursor-pointer border transition-all duration-200 group
              ${selectedRunId === run.id
                ? 'bg-sky-500/10 border-sky-500/50 shadow-sm'
                : 'bg-transparent border-transparent hover:bg-slate-700/50 hover:border-slate-600'}
            `}
          >
            <div className="flex items-center gap-2 mb-1">
              <GitCommit className={`w-4 h-4 ${selectedRunId === run.id ? 'text-sky-400' : 'text-slate-500 group-hover:text-slate-400'}`} />
              <span className={`font-mono font-semibold ${selectedRunId === run.id ? 'text-sky-100' : 'text-slate-300'}`}>
                {run.hash}
              </span>
            </div>
            <div className="flex items-center gap-1.5 pl-6">
              <Calendar className="w-3 h-3 text-slate-500" />
              <span className="text-xs text-slate-500">
                {run.date}
              </span>
            </div>
          </div>
        ))}
      </div>
    </aside>
  );
};
