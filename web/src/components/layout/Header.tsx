import { useTranslation } from 'react-i18next';
import { Box, Layers, GitGraph, ChevronDown, ChevronUp, Sun, Moon } from 'lucide-react';
import { useApp } from '@/contexts/AppContext';
import { LanguageSwitcher } from './LanguageSwitcher';

interface HeaderProps {
  viewMode: 'snapshot' | 'diff';
  onViewModeChange: (mode: 'snapshot' | 'diff') => void;
  projects?: string[];
  selectedProject?: string;
  onProjectChange?: (project: string) => void;
}

export const Header: React.FC<HeaderProps> = ({
  viewMode,
  onViewModeChange,
  projects = [],
  selectedProject = '',
  onProjectChange,
}) => {
  const { theme, toggleTheme, navVisible, toggleNav } = useApp();
  const { t } = useTranslation();
  const isMultiProject = projects.length > 1;

  return (
    <header className="h-[70px] bg-slate-900/95 dark:bg-slate-900/95 bg-white/95 border-b border-slate-200 dark:border-slate-700 flex items-center justify-between px-8 shrink-0 z-10 sticky top-0 backdrop-blur-md transition-colors">
      <div className="flex items-center gap-8">
        <div className="flex items-center gap-2 text-sky-600 dark:text-sky-400 text-2xl font-bold">
          <Box className="w-8 h-8" />
          CodePrism
        </div>

        <div className="flex items-center gap-3">
          <span className="text-slate-500 dark:text-slate-400 text-sm font-medium uppercase tracking-wide">{t('header.project')}</span>
          {isMultiProject ? (
            <div className="relative group">
              <button className="bg-slate-100 dark:bg-slate-800 text-slate-800 dark:text-slate-100 border border-slate-300 dark:border-slate-700 px-4 py-2 rounded-lg hover:bg-slate-200 dark:hover:bg-slate-700 transition-colors flex items-center gap-2">
                {selectedProject || projects[0]}
                <ChevronDown className="w-4 h-4 text-slate-500 dark:text-slate-400" />
              </button>
              <div className="absolute top-full left-0 mt-1 bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg shadow-xl opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all z-50 min-w-[150px]">
                {projects.map(project => (
                  <button
                    key={project}
                    onClick={() => onProjectChange?.(project)}
                    className={`w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-slate-700 transition-colors first:rounded-t-lg last:rounded-b-lg ${project === selectedProject
                      ? 'text-sky-600 dark:text-sky-400 bg-slate-50 dark:bg-slate-700/50'
                      : 'text-slate-700 dark:text-slate-100'
                      }`}
                  >
                    {project}
                  </button>
                ))}
              </div>
            </div>
          ) : (
            <div className="bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-100 border border-slate-300 dark:border-slate-700 px-4 py-2 rounded-lg opacity-80 cursor-not-allowed">
              {selectedProject || projects[0] || 'default'}
            </div>
          )}
        </div>
      </div>

      <div className="flex items-center gap-4">
        {/* View Mode Toggle */}
        <div className="flex bg-slate-100 dark:bg-slate-800 p-1 rounded-lg border border-slate-200 dark:border-slate-700">
          <button
            onClick={() => onViewModeChange('snapshot')}
            className={`
              px-5 py-1.5 rounded-md text-sm font-medium transition-all flex items-center gap-2
              ${viewMode === 'snapshot'
                ? 'bg-white dark:bg-sky-500 text-sky-600 dark:text-slate-900 shadow-sm dark:shadow-lg dark:shadow-sky-500/20'
                : 'text-slate-500 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-100 hover:bg-slate-200/50 dark:hover:bg-slate-700/50'}
            `}
          >
            <Layers className="w-4 h-4" />
            {t('header.snapshot')}
          </button>
          <button
            onClick={() => onViewModeChange('diff')}
            className={`
              px-5 py-1.5 rounded-md text-sm font-medium transition-all flex items-center gap-2
              ${viewMode === 'diff'
                ? 'bg-white dark:bg-sky-500 text-sky-600 dark:text-slate-900 shadow-sm dark:shadow-lg dark:shadow-sky-500/20'
                : 'text-slate-500 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-100 hover:bg-slate-200/50 dark:hover:bg-slate-700/50'}
            `}
          >
            <GitGraph className="w-4 h-4" />
            {t('header.diff')}
          </button>
        </div>

        {/* Nav Toggle */}
        <button
          onClick={toggleNav}
          className="p-2.5 rounded-lg bg-slate-100 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 text-slate-500 dark:text-slate-400 hover:text-sky-500 dark:hover:text-sky-400 transition-colors"
          title={navVisible ? 'Collapse nav' : 'Expand nav'}
        >
          {navVisible ? <ChevronUp className="w-5 h-5" /> : <ChevronDown className="w-5 h-5" />}
        </button>

        {/* Language Switcher */}
        <LanguageSwitcher />

        {/* Theme Toggle */}
        <button
          onClick={toggleTheme}
          className="p-2.5 rounded-lg bg-slate-100 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 text-slate-500 dark:text-slate-400 hover:text-sky-500 dark:hover:text-sky-400 transition-colors"
          title={theme === 'dark' ? t('header.switchToLight') : t('header.switchToDark')}
        >
          {theme === 'dark' ? <Sun className="w-5 h-5" /> : <Moon className="w-5 h-5" />}
        </button>
      </div>
    </header>
  );
};
