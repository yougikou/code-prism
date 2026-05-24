import React, { useRef, useState, useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { TechStackInfo } from '@/services/data';

interface TechStackTabsProps {
  techStacks: TechStackInfo[];
  selectedStack: string;
  onSelect: (stack: string) => void;
}

export const TechStackTabs: React.FC<TechStackTabsProps> = ({ techStacks, selectedStack, onSelect }) => {
  const { t } = useTranslation();
  const scrollRef = useRef<HTMLDivElement>(null);
  const [canScrollLeft, setCanScrollLeft] = useState(false);
  const [canScrollRight, setCanScrollRight] = useState(false);
  const [openCategory, setOpenCategory] = useState<string | null>(null);
  const categoryBtnRefs = useRef<Map<string, HTMLButtonElement>>(new Map());

  // Close dropdown on page scroll / resize to prevent stale positioning
  useEffect(() => {
    if (!openCategory) return;
    const handleClose = () => setOpenCategory(null);
    window.addEventListener('scroll', handleClose, { passive: true });
    window.addEventListener('resize', handleClose);
    return () => {
      window.removeEventListener('scroll', handleClose);
      window.removeEventListener('resize', handleClose);
    };
  }, [openCategory]);

  // Group techStacks by category
  const groups = useMemo(() => {
    const map = new Map<string, string[]>();
    for (const ts of techStacks) {
      const cat = ts.category && ts.category.trim() ? ts.category : '\0other';
      if (!map.has(cat)) map.set(cat, []);
      map.get(cat)!.push(ts.name);
    }
    // Sort categories: named categories alphabetically, "Other" always last
    const entries = Array.from(map.entries());
    entries.sort((a, b) => {
      if (a[0] === '\0other') return 1;
      if (b[0] === '\0other') return -1;
      return a[0].localeCompare(b[0]);
    });
    // Sort stacks within each category
    for (const [, stacks] of entries) {
      stacks.sort();
    }
    return entries;
  }, [techStacks]);

  // Find which category the selected stack belongs to
  const selectedCategory = useMemo(() => {
    if (selectedStack === 'Summary') return null;
    for (const [cat, stacks] of groups) {
      if (stacks.includes(selectedStack)) return cat;
    }
    return null;
  }, [selectedStack, groups]);

  const updateScrollState = useCallback(() => {
    const el = scrollRef.current;
    if (el) {
      setCanScrollLeft(el.scrollLeft > 2);
      setCanScrollRight(el.scrollLeft < el.scrollWidth - el.clientWidth - 2);
    }
  }, []);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;

    updateScrollState();

    const observer = new ResizeObserver(updateScrollState);
    observer.observe(el);

    el.addEventListener('scroll', updateScrollState, { passive: true });
    return () => {
      observer.disconnect();
      el.removeEventListener('scroll', updateScrollState);
    };
  }, [updateScrollState, techStacks]);

  const scroll = (direction: 'left' | 'right') => {
    const el = scrollRef.current;
    if (el) {
      el.scrollBy({ left: direction === 'left' ? -300 : 300, behavior: 'smooth' });
    }
  };

  const handleWheel = (e: React.WheelEvent) => {
    const el = scrollRef.current;
    if (el) {
      el.scrollBy({ left: e.deltaY, behavior: 'auto' });
    }
  };

  const handleCategorySelect = (stackName: string) => {
    onSelect(stackName);
    setOpenCategory(null);
  };

  const handleSummarySelect = () => {
    onSelect('Summary');
    setOpenCategory(null);
  };

  const isSelected = (stackName: string) => selectedStack === stackName;
  const isCategoryActive = (category: string) => selectedCategory === category;

  // Build a lookup for selected stack's category display
  const selectedStackCategory = useMemo(() => {
    if (selectedStack === 'Summary') return null;
    const ts = techStacks.find(s => s.name === selectedStack);
    return ts?.category || null;
  }, [selectedStack, techStacks]);

  return (
    <div className="mb-3 border-b border-slate-200 dark:border-slate-700">
      <style>{`
        .tech-stack-scroll::-webkit-scrollbar { display: none; }
        .tech-stack-scroll { -ms-overflow-style: none; scrollbar-width: none; }
      `}</style>

      {/* Category indicator — fixed min-height to prevent layout shift */}
      <div className="min-h-[1.25rem] flex items-center">
        {selectedStack !== 'Summary' && selectedStackCategory ? (
          <span className="text-xs text-slate-400 dark:text-slate-500">
            <span className="font-medium text-slate-600 dark:text-slate-300">{selectedStackCategory}</span>
            <span className="mx-1 text-slate-300 dark:text-slate-600">&gt;</span>
            <span>{selectedStack}</span>
          </span>
        ) : null}
      </div>

      <div className="flex items-center">
        {canScrollLeft && (
          <button
            onClick={() => scroll('left')}
            className="shrink-0 px-1 py-4 cursor-pointer text-slate-400 hover:text-sky-600 dark:hover:text-sky-400 transition-colors"
            aria-label="Scroll left"
          >
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
            </svg>
          </button>
        )}

        <div
          ref={scrollRef}
          onWheel={handleWheel}
          className="flex gap-8 overflow-x-auto flex-nowrap flex-1 tech-stack-scroll"
        >
          {/* Summary tab */}
          <button
            key="Summary"
            onClick={handleSummarySelect}
            className={`
              pb-4 pt-4 text-sm font-medium transition-colors relative whitespace-nowrap shrink-0
              ${isSelected('Summary') ? 'text-sky-600 dark:text-sky-400' : 'text-slate-500 dark:text-slate-400 hover:text-slate-800 dark:hover:text-slate-200'}
            `}
          >
            {t('dashboard.summary')}
            {isSelected('Summary') && (
              <div className="absolute bottom-0 left-0 w-full h-0.5 bg-sky-600 dark:bg-sky-400 shadow-[0_0_10px_rgba(56,189,248,0.5)]" />
            )}
          </button>

          {/* Category dropdown buttons */}
          {groups.map(([category]) => {
            const displayName = category === '\0other' ? t('dashboard.otherCategory') : category;
            const isActive = isCategoryActive(category);
            const isOpen = openCategory === category;

            return (
              <div key={category} className="relative shrink-0">
                <button
                  ref={(el) => {
                    if (el) categoryBtnRefs.current.set(category, el);
                    else categoryBtnRefs.current.delete(category);
                  }}
                  onClick={() => setOpenCategory(isOpen ? null : category)}
                  className={`
                    pb-4 pt-4 text-sm font-medium transition-colors relative whitespace-nowrap flex items-center gap-1
                    ${isActive ? 'text-sky-600 dark:text-sky-400' : 'text-slate-500 dark:text-slate-400 hover:text-slate-800 dark:hover:text-slate-200'}
                  `}
                >
                  {displayName}
                  <svg className={`w-3.5 h-3.5 transition-transform ${isOpen ? 'rotate-180' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                  </svg>
                  {isActive && (
                    <div className="absolute bottom-0 left-0 w-full h-0.5 bg-sky-600 dark:bg-sky-400 shadow-[0_0_10px_rgba(56,189,248,0.5)]" />
                  )}
                </button>
              </div>
            );
          })}
        </div>

        {canScrollRight && (
          <button
            onClick={() => scroll('right')}
            className="shrink-0 px-1 py-4 cursor-pointer text-slate-400 hover:text-sky-600 dark:hover:text-sky-400 transition-colors"
            aria-label="Scroll right"
          >
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
            </svg>
          </button>
        )}
      </div>

      {/* Dropdown rendered outside overflow container to avoid CSS clipping */}
      {openCategory && (() => {
        const stacks = groups.find(([cat]) => cat === openCategory)?.[1];
        const btn = categoryBtnRefs.current.get(openCategory);
        if (!stacks || !btn) return null;

        const rect = btn.getBoundingClientRect();

        return (
          <>
            <div
              className="fixed inset-0 z-40"
              onClick={() => setOpenCategory(null)}
            />
            <div
              className="fixed z-50 bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg shadow-lg py-1 max-h-[60vh] overflow-y-auto"
              style={{
                top: rect.bottom + 4,
                left: rect.left,
                minWidth: Math.max(rect.width, 140),
              }}
            >
              {stacks.map(stackName => (
                <button
                  key={stackName}
                  onClick={() => handleCategorySelect(stackName)}
                  className={`
                    w-full text-left px-4 py-2 text-sm transition-colors whitespace-nowrap
                    ${isSelected(stackName)
                      ? 'text-sky-600 dark:text-sky-400 bg-sky-50 dark:bg-sky-900/20 font-medium'
                      : 'text-slate-600 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-700/50'}
                  `}
                >
                  {stackName}
                </button>
              ))}
            </div>
          </>
        );
      })()}
    </div>
  );
};
