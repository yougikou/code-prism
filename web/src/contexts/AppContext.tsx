import React, { createContext, useContext, useState, type ReactNode } from 'react';

type ViewMode = 'snapshot' | 'diff';
type TechStack = string; // Dynamic

interface AppState {
  currentProject: string;
  viewMode: ViewMode;
  selectedRunId: string | null;
  selectedTechStack: TechStack;
  availableTechStacks: TechStack[];
}

interface AppContextType extends AppState {
  setProject: (project: string) => void;
  setViewMode: (mode: ViewMode) => void;
  setSelectedRunId: (runId: string | null) => void;
  setSelectedTechStack: (stack: TechStack) => void;
  setAvailableTechStacks: (stacks: TechStack[]) => void;
  theme: 'light' | 'dark';
  toggleTheme: () => void;
}

const AppContext = createContext<AppContextType | undefined>(undefined);

export const AppProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  const [currentProject, setCurrentProject] = useState('code-prism');
  const [viewMode, setViewMode] = useState<ViewMode>('snapshot');
  const [selectedRunId, setSelectedRunId] = useState<string | null>('1'); // Default to first run
  const [selectedTechStack, setSelectedTechStack] = useState<TechStack>('Summary');
  const [availableTechStacks, setAvailableTechStacks] = useState<TechStack[]>([]);

  /* Theme Support */
  const [theme, setTheme] = useState<'light' | 'dark'>(() => {
    if (typeof window !== 'undefined') {
      return (localStorage.getItem('theme') as 'light' | 'dark') || 'dark';
    }
    return 'dark';
  });

  React.useEffect(() => {
    const root = window.document.documentElement;
    root.classList.remove('light', 'dark');
    root.classList.add(theme);
    localStorage.setItem('theme', theme);
  }, [theme]);

  const toggleTheme = () => {
    setTheme(prev => (prev === 'dark' ? 'light' : 'dark'));
  };

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
        theme,
        toggleTheme
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
