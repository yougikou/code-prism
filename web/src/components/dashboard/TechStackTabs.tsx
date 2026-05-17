import React, { useRef, useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';

type TechStack = string;

interface TechStackTabsProps {
  stacks: TechStack[];
  selectedStack: TechStack;
  onSelect: (stack: TechStack) => void;
}

export const TechStackTabs: React.FC<TechStackTabsProps> = ({ stacks, selectedStack, onSelect }) => {
  const { t } = useTranslation();
  const scrollRef = useRef<HTMLDivElement>(null);
  const [canScrollLeft, setCanScrollLeft] = useState(false);
  const [canScrollRight, setCanScrollRight] = useState(false);

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
  }, [updateScrollState, stacks]);

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

  return (
    <div className="mb-8 border-b border-slate-200 dark:border-slate-700">
      <style>{`
        .tech-stack-scroll::-webkit-scrollbar { display: none; }
        .tech-stack-scroll { -ms-overflow-style: none; scrollbar-width: none; }
      `}</style>

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
          {stacks.map((stack) => (
            <button
              key={stack}
              onClick={() => onSelect(stack)}
              className={`
                pb-4 pt-4 text-sm font-medium transition-colors relative whitespace-nowrap shrink-0
                ${selectedStack === stack ? 'text-sky-600 dark:text-sky-400' : 'text-slate-500 dark:text-slate-400 hover:text-slate-800 dark:hover:text-slate-200'}
              `}
            >
              {stack === 'Summary' ? t('dashboard.summary') : stack}
              {selectedStack === stack && (
                <div className="absolute bottom-0 left-0 w-full h-0.5 bg-sky-600 dark:bg-sky-400 shadow-[0_0_10px_rgba(56,189,248,0.5)]" />
              )}
            </button>
          ))}
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
    </div>
  );
};
