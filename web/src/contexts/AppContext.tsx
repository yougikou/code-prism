import React, { createContext, useContext, useState, useEffect, useCallback, type ReactNode } from 'react';
import { fetchUnifiedProjects, type UnifiedProjectInfo } from '../services/data';

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
  configVersion: number;
  triggerConfigRefresh: () => void;
  projectList: UnifiedProjectInfo[];
  loadUnifiedProjects: () => Promise<void>;
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

  const [configVersion, setConfigVersion] = useState(0);
  const [projectList, setProjectList] = useState<UnifiedProjectInfo[]>([]);

  useEffect(() => {
    const root = window.document.documentElement;
    root.classList.remove('light', 'dark');
    root.classList.add(theme);
    localStorage.setItem('theme', theme);
  }, [theme]);

  const toggleTheme = () => {
    setTheme(prev => (prev === 'dark' ? 'light' : 'dark'));
  };

  const triggerConfigRefresh = () => setConfigVersion(v => v + 1);

  const loadUnifiedProjects = useCallback(async () => {
    try {
      const projects = await fetchUnifiedProjects();
      setProjectList(projects);
      // Auto-select first project if current project is not in the list
      setCurrentProject(prev => {
        if (projects.length > 0 && !projects.find(p => p.name === prev)) {
          return projects[0].name;
        }
        return prev;
      });
    } catch (e) {
      console.error('Failed to load unified projects:', e);
    }
  }, []);

  // Load unified projects on mount and whenever configVersion changes
  useEffect(() => {
    loadUnifiedProjects();
  }, [configVersion, loadUnifiedProjects]);

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
        configVersion,
        triggerConfigRefresh,
        projectList,
        loadUnifiedProjects,
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
