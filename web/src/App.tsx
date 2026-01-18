import { AppProvider } from './contexts/AppContext'
import Dashboard from './components/Dashboard'

function App() {
  return (
    <AppProvider>
      <div className="min-h-screen bg-slate-900 text-slate-100 font-sans antialiased selection:bg-sky-500/30">
        <Dashboard />
      </div>
    </AppProvider>
  )
}

export default App
