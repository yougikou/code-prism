import { useTranslation } from 'react-i18next';
import { Box, Layers, GitGraph, Sun, Moon, ChevronDown } from 'lucide-react';
import { useApp } from '@/contexts/AppContext';
import { LanguageSwitcher } from './LanguageSwitcher';

export const Header: React.FC = () => {
  const { t } = useTranslation();
  const {
    currentPage, navigateTo,
    currentProject, setProject,
    projectList,
    viewMode, setViewMode,
    theme, toggleTheme,
  } = useApp();

  const isMultiProject = projectList.length > 1;

  const pageNav = [
    { key: 'dashboard' as const, label: t('nav.dashboard') },
    { key: 'execute' as const, label: t('nav.execute') },
    { key: 'config' as const, label: t('nav.config') },
  ];

  return (
    <header className="h-[70px] bg-white/95 dark:bg-slate-900/95 border-b border-slate-200 dark:border-slate-700 flex items-center justify-between px-8 shrink-0 z-[60] sticky top-0 backdrop-blur-md transition-colors">
      {/* Left: Logo + Project Selector + Page Navigation */}
      <div className="flex items-center gap-3">
        <div className="flex items-center gap-2 text-sky-600 dark:text-sky-400 text-xl font-bold shrink-0">
          <Box className="w-7 h-7" />
          <span>CodePrism</span>
        </div>

        {/* Compact Project Selector */}
        {isMultiProject ? (
          <div className="relative group">
            <button className="px-2 py-1 rounded-md text-sm font-medium text-slate-500 dark:text-slate-400 hover:text-slate-800 dark:hover:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors flex items-center gap-1">
              {currentProject || projectList[0]?.name}
              <ChevronDown className="w-3.5 h-3.5" />
            </button>
            <div className="absolute top-full left-0 mt-1 bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg shadow-xl opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all z-[70] min-w-[130px]">
              {projectList.map(p => (
                <button
                  key={p.name}
                  onClick={() => setProject(p.name)}
                  className={`w-full text-left px-3 py-1.5 text-sm hover:bg-slate-100 dark:hover:bg-slate-700 transition-colors first:rounded-t-lg last:rounded-b-lg ${p.name === currentProject
                    ? 'text-sky-600 dark:text-sky-400 bg-slate-50 dark:bg-slate-700/50'
                    : 'text-slate-700 dark:text-slate-100'
                    }`}
                >
                  {p.name}
                </button>
              ))}
            </div>
          </div>
        ) : (
          <span className="px-2 py-1 text-sm font-medium text-slate-400 dark:text-slate-500 opacity-80 cursor-default">
            {currentProject || projectList[0]?.name || 'default'}
          </span>
        )}

        <div className="w-px h-5 bg-slate-200 dark:bg-slate-700" />

        <nav className="flex items-center gap-1">
          {pageNav.map(({ key, label }) => (
            <button
              key={key}
              onClick={() => navigateTo(key)}
              className={`
                px-3 py-1.5 rounded-md text-sm font-medium transition-colors
                ${currentPage === key
                  ? 'bg-sky-600 text-white'
                  : 'text-slate-500 dark:text-slate-400 hover:text-slate-800 dark:hover:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800'
                }
              `}
            >
              {label}
            </button>
          ))}
        </nav>
      </div>

      {/* Right: Global Controls */}
      <div className="flex items-center gap-4">
        {/* View Mode Toggle — only on Dashboard */}
        {currentPage === 'dashboard' && (
          <div className="flex bg-slate-100 dark:bg-slate-800 p-1 rounded-lg border border-slate-200 dark:border-slate-700">
            <button
              onClick={() => setViewMode('snapshot')}
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
              onClick={() => setViewMode('diff')}
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
        )}

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
