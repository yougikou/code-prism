/* eslint-disable react-refresh/only-export-components -- provider and hook form one public context API */
import { createContext, useContext, useState, useCallback, type ReactNode } from 'react';
import { X, CheckCircle, AlertCircle, Info } from 'lucide-react';

type ToastType = 'success' | 'error' | 'info';

interface Toast {
  id: number;
  type: ToastType;
  message: string;
}

interface ToastContextType {
  toast: (type: ToastType, message: string) => void;
}

const ToastContext = createContext<ToastContextType | undefined>(undefined);

let nextId = 0;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const addToast = useCallback((type: ToastType, message: string) => {
    const id = nextId++;
    setToasts(prev => [...prev, { id, type, message }]);
    setTimeout(() => {
      setToasts(prev => prev.filter(t => t.id !== id));
    }, 4000);
  }, []);

  const removeToast = useCallback((id: number) => {
    setToasts(prev => prev.filter(t => t.id !== id));
  }, []);

  return (
    <ToastContext.Provider value={{ toast: addToast }}>
      {children}
      {/* Toast container */}
      <div className="fixed bottom-6 right-6 z-[100] flex flex-col gap-2 max-w-sm">
        {toasts.map(t => (
          <div
            key={t.id}
            className={`flex items-start gap-3 px-4 py-3 rounded-xl shadow-2xl border backdrop-blur-sm transition-all animate-in slide-in-from-right ${
              t.type === 'success'
                ? 'bg-emerald-600/95 border-emerald-500 text-white'
                : t.type === 'error'
                ? 'bg-red-600/95 border-red-500 text-white'
                : 'bg-sky-600/95 border-sky-500 text-white'
            }`}
          >
            {t.type === 'success' && <CheckCircle className="w-5 h-5 shrink-0 mt-0.5" />}
            {t.type === 'error' && <AlertCircle className="w-5 h-5 shrink-0 mt-0.5" />}
            {t.type === 'info' && <Info className="w-5 h-5 shrink-0 mt-0.5" />}
            <p className="text-sm flex-1">{t.message}</p>
            <button onClick={() => removeToast(t.id)} className="shrink-0 opacity-70 hover:opacity-100 transition-opacity">
              <X className="w-4 h-4" />
            </button>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}

export function useToast() {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error('useToast must be used within ToastProvider');
  return {
    success: (msg: string) => ctx.toast('success', msg),
    error: (msg: string) => ctx.toast('error', msg),
    info: (msg: string) => ctx.toast('info', msg),
  };
}
