import React, { createContext, useContext, useState, type ReactNode } from 'react';

type ViewMode = 'snapshot' | 'diff';
type TechStack = string; // Dynamic
type Page = 'dashboard' | 'execute' | 'config';

interface AppState {
  currentProject: string;
  viewMode: ViewMode;
  selectedRunId: string | null;
  selectedTechStack: TechStack;
  availableTechStacks: TechStack[];
  currentPage: Page;
}

interface AppContextType extends AppState {
  setProject: (project: string) => void;
  setViewMode: (mode: ViewMode) => void;
  setSelectedRunId: (runId: string | null) => void;
  setSelectedTechStack: (stack: TechStack) => void;
  setAvailableTechStacks: (stacks: TechStack[]) => void;
  navigateTo: (page: Page) => void;
  theme: 'light' | 'dark';
  toggleTheme: () => void;
  navVisible: boolean;
  toggleNav: () => void;
  configVersion: number;
  triggerConfigRefresh: () => void;
}

const AppContext = createContext<AppContextType | undefined>(undefined);

export const AppProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  const [currentProject, setCurrentProject] = useState('code-prism');
  const [viewMode, setViewMode] = useState<ViewMode>('snapshot');
  const [selectedRunId, setSelectedRunId] = useState<string | null>('1'); // Default to first run
  const [selectedTechStack, setSelectedTechStack] = useState<TechStack>('Summary');
  const [availableTechStacks, setAvailableTechStacks] = useState<TechStack[]>([]);
  const [currentPage, setCurrentPage] = useState<Page>('dashboard');

  /* Theme Support */
  const [theme, setTheme] = useState<'light' | 'dark'>(() => {
    if (typeof window !== 'undefined') {
      return (localStorage.getItem('theme') as 'light' | 'dark') || 'dark';
    }
    return 'dark';
  });

  const [navVisible, setNavVisible] = useState(false);
  const [configVersion, setConfigVersion] = useState(0);

  React.useEffect(() => {
    const root = window.document.documentElement;
    root.classList.remove('light', 'dark');
    root.classList.add(theme);
    localStorage.setItem('theme', theme);
  }, [theme]);

  const toggleTheme = () => {
    setTheme(prev => (prev === 'dark' ? 'light' : 'dark'));
  };

  const toggleNav = () => setNavVisible(prev => !prev);
  const triggerConfigRefresh = () => setConfigVersion(v => v + 1);

  return (
    <AppContext.Provider
      value={{
        currentProject,
        setProject: setCurrentProject,
        viewMode,
        setViewMode,
        selectedRunId,
        setSelectedRunId,
        selectedTechStack,
        setSelectedTechStack,
        availableTechStacks,
        setAvailableTechStacks,
        currentPage,
        navigateTo: setCurrentPage,
        theme,
        toggleTheme,
        navVisible,
        toggleNav,
        configVersion,
        triggerConfigRefresh,
      }}
    >
      {children}
    </AppContext.Provider>
  );
};

export const useApp = () => {
  const context = useContext(AppContext);
  if (context === undefined) {
    throw new Error('useApp must be used within an AppProvider');
  }
  return context;
};
