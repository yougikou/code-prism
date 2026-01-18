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
}

const AppContext = createContext<AppContextType | undefined>(undefined);

export const AppProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  const [currentProject, setCurrentProject] = useState('code-prism');
  const [viewMode, setViewMode] = useState<ViewMode>('snapshot');
  const [selectedRunId, setSelectedRunId] = useState<string | null>('1'); // Default to first run
  const [selectedTechStack, setSelectedTechStack] = useState<TechStack>('Summary');
  const [availableTechStacks, setAvailableTechStacks] = useState<TechStack[]>([]);

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
