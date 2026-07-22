import { lazy, Suspense, useEffect } from 'react'
import { AppProvider, useApp } from './contexts/AppContext'
import { Header } from './components/layout/Header'
import { ToastProvider } from './components/ui/toast'

const Dashboard = lazy(() => import('./components/Dashboard'))
const ExecutePage = lazy(() => import('./components/ExecutePage'))
const ConfigPage = lazy(() => import('./components/ConfigPage'))

function PageFallback() {
  return <div className="h-full grid place-items-center text-sm text-slate-500">Loading…</div>
}

function AppContent() {
  const { currentPage, currentProject } = useApp()

  useEffect(() => {
    document.title = `CodePrism - ${currentProject}`
  }, [currentProject])

  return (
    <div className="h-screen flex flex-col bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 font-sans antialiased selection:bg-sky-500/30 transition-colors">
      <Header />

      <Suspense fallback={<PageFallback />}>
        <div className="flex-1 overflow-hidden">
          {currentPage === 'dashboard' && <Dashboard />}
          {currentPage === 'execute' && <ExecutePage />}
          {currentPage === 'config' && <ConfigPage />}
        </div>
      </Suspense>
    </div>
  )
}

function App() {
  return (
    <AppProvider>
      <ToastProvider>
        <AppContent />
      </ToastProvider>
    </AppProvider>
  )
}

export default App
