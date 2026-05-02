import { useTranslation } from 'react-i18next'
import { AppProvider, useApp } from './contexts/AppContext'
import Dashboard from './components/Dashboard'
import ExecutePage from './components/ExecutePage'
import ConfigPage from './components/ConfigPage'

function AppContent() {
  const { currentPage, navigateTo, navVisible } = useApp()
  const { t } = useTranslation()

  return (
    <div className="min-h-screen bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 font-sans antialiased selection:bg-sky-500/30 transition-colors">
      <nav className="bg-white/95 dark:bg-slate-800 border-b border-slate-200 dark:border-slate-700 backdrop-blur-md transition-all">
        <div
          className={`overflow-hidden transition-all duration-300 ease-in-out ${
            navVisible ? 'max-h-20 opacity-100' : 'max-h-0 opacity-0'
          }`}
        >
          <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
            <div className="flex justify-between items-center h-16">
              <div className="flex space-x-4">
                <button
                  onClick={() => navigateTo('dashboard')}
                  className={`px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                    currentPage === 'dashboard'
                      ? 'bg-sky-600 text-white'
                      : 'text-slate-600 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-700'
                  }`}
                >
                  {t('nav.dashboard')}
                </button>
                <button
                  onClick={() => navigateTo('execute')}
                  className={`px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                    currentPage === 'execute'
                      ? 'bg-sky-600 text-white'
                      : 'text-slate-600 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-700'
                  }`}
                >
                  {t('nav.execute')}
                </button>
                <button
                  onClick={() => navigateTo('config')}
                  className={`px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                    currentPage === 'config'
                      ? 'bg-sky-600 text-white'
                      : 'text-slate-600 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-700'
                  }`}
                >
                  {t('nav.config')}
                </button>
              </div>
              <div className="flex items-center gap-3">
                <div className="text-sm text-slate-500 dark:text-slate-400">CodePrism</div>
              </div>
            </div>
          </div>
        </div>
      </nav>

      {currentPage === 'dashboard' && <Dashboard />}
      {currentPage === 'execute' && <ExecutePage />}
      {currentPage === 'config' && <ConfigPage />}
    </div>
  )
}

function App() {
  return (
    <AppProvider>
      <AppContent />
    </AppProvider>
  )
}

export default App
