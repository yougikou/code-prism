import { type FormEvent, type ReactNode, lazy, Suspense, useEffect, useState } from 'react'
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

function AuthenticationGate({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<'loading' | 'ready' | 'login' | 'error'>('loading')
  const [token, setToken] = useState('')
  const [error, setError] = useState('')

  useEffect(() => {
    fetch('/api/v1/auth/status')
      .then(async response => {
        if (!response.ok) throw new Error('Unable to check server authentication')
        return response.json() as Promise<{ authenticated: boolean }>
      })
      .then(result => setStatus(result.authenticated ? 'ready' : 'login'))
      .catch(message => {
        setError(message instanceof Error ? message.message : 'Unable to contact CodePrism')
        setStatus('error')
      })
  }, [])

  const login = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    setError('')
    const response = await fetch('/api/v1/auth/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ token }),
    })
    if (response.ok) {
      setToken('')
      setStatus('ready')
    } else {
      setError('The access token is invalid.')
    }
  }

  if (status === 'ready') return <>{children}</>
  if (status === 'loading') return <PageFallback />

  return (
    <main className="min-h-screen grid place-items-center bg-slate-950 p-6 text-slate-100">
      <form onSubmit={login} className="w-full max-w-sm space-y-4 rounded-xl border border-slate-700 bg-slate-900 p-6 shadow-xl">
        <div>
          <h1 className="text-lg font-semibold">CodePrism access</h1>
          <p className="mt-1 text-sm text-slate-400">Enter the server access token to continue.</p>
        </div>
        {status === 'error' && <p className="text-sm text-red-400">{error}</p>}
        {status === 'login' && <>
          <input
            className="w-full rounded-md border border-slate-600 bg-slate-800 px-3 py-2 text-sm outline-none focus:border-sky-400"
            type="password"
            value={token}
            onChange={event => setToken(event.target.value)}
            autoComplete="current-password"
            aria-label="Access token"
            required
          />
          {error && <p className="text-sm text-red-400">{error}</p>}
          <button className="w-full rounded-md bg-sky-600 px-3 py-2 text-sm font-medium hover:bg-sky-500" type="submit">
            Sign in
          </button>
        </>}
      </form>
    </main>
  )
}

function App() {
  return (
    <AuthenticationGate>
      <AppProvider>
        <ToastProvider>
          <AppContent />
        </ToastProvider>
      </AppProvider>
    </AuthenticationGate>
  )
}

export default App
