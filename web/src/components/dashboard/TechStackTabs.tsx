import React from 'react';

type TechStack = string;

interface TechStackTabsProps {
  stacks: TechStack[];
  selectedStack: TechStack;
  onSelect: (stack: TechStack) => void;
}

export const TechStackTabs: React.FC<TechStackTabsProps> = ({ stacks, selectedStack, onSelect }) => {

  return (
    <div className="flex gap-8 border-b border-slate-200 dark:border-slate-700 mb-8">
      {stacks.map((stack) => (
        <button
          key={stack}
          onClick={() => onSelect(stack)}
          className={`
            pb-4 font-medium transition-colors relative
            ${selectedStack === stack ? 'text-sky-600 dark:text-sky-400' : 'text-slate-500 dark:text-slate-400 hover:text-slate-800 dark:hover:text-slate-200'}
          `}
        >
          {stack}
          {selectedStack === stack && (
            <div className="absolute bottom-0 left-0 w-full h-0.5 bg-sky-600 dark:bg-sky-400 shadow-[0_0_10px_rgba(56,189,248,0.5)]" />
          )}
        </button>
      ))}
    </div>
  );
};
