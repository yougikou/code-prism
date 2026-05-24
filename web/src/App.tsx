import { useEffect } from 'react'
import { AppProvider, useApp } from './contexts/AppContext'
import { Header } from './components/layout/Header'
import Dashboard from './components/Dashboard'
import ExecutePage from './components/ExecutePage'
import ConfigPage from './components/ConfigPage'
import { ToastProvider } from './components/ui/toast'

function AppContent() {
  const { currentPage, currentProject } = useApp()

  useEffect(() => {
    document.title = `CodePrism - ${currentProject}`
  }, [currentProject])

  return (
    <div className="h-screen flex flex-col bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 font-sans antialiased selection:bg-sky-500/30 transition-colors">
      <Header />

      <div className="flex-1 overflow-hidden">
        {currentPage === 'dashboard' && <Dashboard />}
        {currentPage === 'execute' && <ExecutePage />}
        {currentPage === 'config' && <ConfigPage />}
      </div>
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
