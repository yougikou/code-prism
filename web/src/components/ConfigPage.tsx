import { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { useApp } from '@/contexts/AppContext'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import {
  fetchConfig,
  fetchFullProjectConfig,
  updateProjectConfig,
  type AppConfig,
  type FullProjectConfig,
  type FullTechStack,
} from '@/services/data'

// ─── Tag Input ──────────────────────────────────────────────────────────────

function TagInput({ tags, onChange, placeholder }: {
  tags: string[];
  onChange: (tags: string[]) => void;
  placeholder?: string;
}) {
  const [input, setInput] = useState('')

  const addTag = () => {
    const val = input.trim()
    if (val && !tags.includes(val)) {
      onChange([...tags, val])
      setInput('')
    }
  }

  return (
    <div className="flex flex-wrap gap-1.5 p-2 border rounded-lg bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 min-h-[42px]">
      {tags.map(tag => (
        <span key={tag} className="inline-flex items-center gap-1 px-2.5 py-0.5 text-xs font-medium rounded-md bg-sky-100 dark:bg-sky-900/50 text-sky-700 dark:text-sky-300">
          {tag}
          <button onClick={() => onChange(tags.filter(t => t !== tag))} className="hover:text-red-500 ml-0.5">&times;</button>
        </span>
      ))}
      <input
        value={input}
        onChange={e => setInput(e.target.value)}
        onKeyDown={e => {
          if (e.key === 'Enter') { e.preventDefault(); addTag() }
          if (e.key === ',' && input.trim()) { e.preventDefault(); addTag() }
        }}
        onBlur={addTag}
        placeholder={tags.length === 0 ? (placeholder || 'Type and press Enter...') : ''}
        className="flex-1 min-w-[100px] outline-none bg-transparent text-sm text-slate-700 dark:text-slate-300 placeholder-slate-400"
      />
    </div>
  )
}

// ─── Select Input ───────────────────────────────────────────────────────────

function SelectInput({ value, onChange, options, label }: {
  value: string;
  onChange: (v: string) => void;
  options: { value: string; label: string }[];
  label?: string;
}) {
  return (
    <select
      value={value}
      onChange={e => onChange(e.target.value)}
      className="w-full px-3 py-2 text-sm border rounded-lg bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 text-slate-700 dark:text-slate-300 focus:ring-2 focus:ring-sky-500 focus:border-transparent outline-none"
    >
      {label && <option value="" disabled>{label}</option>}
      {options.map(o => (
        <option key={o.value} value={o.value}>{o.label}</option>
      ))}
    </select>
  )
}

// ─── Tech Stacks Editor ─────────────────────────────────────────────────────

function TechStacksEditor({ config, onChange }: {
  config: FullProjectConfig;
  onChange: (c: FullProjectConfig) => void;
}) {
  const { t } = useTranslation()

  const updateStack = <K extends keyof FullTechStack>(index: number, field: K, value: FullTechStack[K]) => {
    const stacks = [...config.tech_stacks]
    stacks[index] = { ...stacks[index], [field]: value }
    onChange({ ...config, tech_stacks: stacks })
  }

  const addStack = () => {
    onChange({
      ...config,
      tech_stacks: [...config.tech_stacks, { name: '', extensions: [], analyzers: [], paths: [], excludes: [] }],
    })
  }

  const removeStack = (index: number) => {
    onChange({ ...config, tech_stacks: config.tech_stacks.filter((_, i) => i !== index) })
  }

  return (
    <div className="space-y-4">
      {config.tech_stacks.map((stack, i) => (
        <Card key={i}>
          <CardHeader className="flex flex-row items-center justify-between pb-2">
            <CardTitle className="text-base">{t('config.techStack.title', { name: stack.name || `#${i + 1}` })}</CardTitle>
            <button
              onClick={() => removeStack(i)}
              className="text-xs px-2 py-1 rounded text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
            >
              {t('config.techStack.delete')}
            </button>
          </CardHeader>
          <CardContent className="space-y-3">
            <div>
              <label className="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1">{t('config.techStack.name')}</label>
              <input
                type="text"
                value={stack.name}
                onChange={e => updateStack(i, 'name', e.target.value)}
                className="w-full px-3 py-2 text-sm border rounded-lg bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 text-slate-700 dark:text-slate-300 focus:ring-2 focus:ring-sky-500 focus:border-transparent outline-none"
              />
            </div>
            <div>
              <label className="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1">{t('config.techStack.extensions')}</label>
              <TagInput tags={stack.extensions} onChange={v => updateStack(i, 'extensions', v)} placeholder="e.g. .rs" />
            </div>
            <div>
              <label className="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1">{t('config.techStack.analyzers')}</label>
              <TagInput tags={stack.analyzers} onChange={v => updateStack(i, 'analyzers', v)} placeholder="e.g. file_count" />
            </div>
            <div>
              <label className="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1">{t('config.techStack.paths')}</label>
              <TagInput tags={stack.paths} onChange={v => updateStack(i, 'paths', v)} placeholder="e.g. src/" />
            </div>
            <div>
              <label className="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1">{t('config.techStack.excludes')}</label>
              <TagInput tags={stack.excludes} onChange={v => updateStack(i, 'excludes', v)} placeholder="e.g. node_modules/" />
            </div>
          </CardContent>
        </Card>
      ))}
      <button
        onClick={addStack}
        className="w-full py-2 border-2 border-dashed border-slate-300 dark:border-slate-600 rounded-lg text-sm text-slate-500 dark:text-slate-400 hover:border-sky-400 hover:text-sky-500 transition-colors"
      >
        + {t('config.techStack.add')}
      </button>
    </div>
  )
}

// ─── Global Excludes Editor ─────────────────────────────────────────────────

function GlobalExcludesEditor({ config, onChange }: {
  config: FullProjectConfig;
  onChange: (c: FullProjectConfig) => void;
}) {
  const { t } = useTranslation()
  return (
    <div>
      <label className="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-2">{t('config.excludes.title')}</label>
      <TagInput
        tags={config.global_excludes}
        onChange={v => onChange({ ...config, global_excludes: v })}
        placeholder="e.g. node_modules/"
      />
    </div>
  )
}

// ─── Analyzers Editor ───────────────────────────────────────────────────────

function AnalyzersEditor({ config, onChange }: {
  config: FullProjectConfig;
  onChange: (c: FullProjectConfig) => void;
}) {
  const { t } = useTranslation()

  const updateRegex = (name: string, field: string, value: string) => {
    const analyzers = { ...config.custom_regex_analyzers }
    analyzers[name] = { ...analyzers[name], [field]: value }
    onChange({ ...config, custom_regex_analyzers: analyzers })
  }

  const addRegex = () => {
    const key = `new_regex_${Date.now()}`
    onChange({
      ...config,
      custom_regex_analyzers: { ...config.custom_regex_analyzers, [key]: { pattern: '', metric_key: '' } },
    })
  }

  const removeRegex = (name: string) => {
    const analyzers = { ...config.custom_regex_analyzers }
    delete analyzers[name]
    onChange({ ...config, custom_regex_analyzers: analyzers })
  }

  const updateImpl = (name: string, field: string, value: string | undefined) => {
    const analyzers = { ...config.custom_impl_analyzers }
    analyzers[name] = { ...analyzers[name], [field]: value || undefined }
    onChange({ ...config, custom_impl_analyzers: analyzers })
  }

  const addImpl = () => {
    const key = `new_impl_${Date.now()}`
    onChange({
      ...config,
      custom_impl_analyzers: { ...config.custom_impl_analyzers, [key]: {} },
    })
  }

  const removeImpl = (name: string) => {
    const analyzers = { ...config.custom_impl_analyzers }
    delete analyzers[name]
    onChange({ ...config, custom_impl_analyzers: analyzers })
  }

  const updateExternal = (name: string, value: string) => {
    const analyzers = { ...config.external_analyzers }
    analyzers[name] = value
    onChange({ ...config, external_analyzers: analyzers })
  }

  const addExternal = () => {
    const key = `new_external_${Date.now()}`
    onChange({ ...config, external_analyzers: { ...config.external_analyzers, [key]: '' } })
  }

  const removeExternal = (name: string) => {
    const analyzers = { ...config.external_analyzers }
    delete analyzers[name]
    onChange({ ...config, external_analyzers: analyzers })
  }

  return (
    <div className="space-y-6">
      {/* Regex Analyzers */}
      <Card>
        <CardHeader className="flex flex-row items-center justify-between pb-2">
          <CardTitle className="text-base">{t('config.analyzers.regexAnalyzers')}</CardTitle>
          <button onClick={addRegex} className="text-xs px-3 py-1.5 rounded-md bg-sky-500 text-white hover:bg-sky-600 transition-colors">
            + {t('config.analyzers.addRegex')}
          </button>
        </CardHeader>
        <CardContent className="space-y-3">
          {Object.entries(config.custom_regex_analyzers).length === 0 && (
            <p className="text-sm text-slate-400 italic">{t('dashboard.noData')}</p>
          )}
          {Object.entries(config.custom_regex_analyzers).map(([name, analyzer]) => (
            <div key={name} className="flex items-start gap-2 p-3 border rounded-lg dark:border-slate-700">
              <div className="flex-1 grid grid-cols-4 gap-2">
                <input type="text" value={name} readOnly className="px-2 py-1 text-xs border rounded bg-slate-50 dark:bg-slate-800 dark:border-slate-700 text-slate-500" />
                <input type="text" value={analyzer.pattern} onChange={e => updateRegex(name, 'pattern', e.target.value)} placeholder="Pattern" className="col-span-2 px-2 py-1 text-xs border rounded bg-white dark:bg-slate-800 dark:border-slate-700 text-slate-700 dark:text-slate-300 outline-none focus:ring-1 focus:ring-sky-500" />
                <div className="flex gap-2">
                  <input type="text" value={analyzer.metric_key} onChange={e => updateRegex(name, 'metric_key', e.target.value)} placeholder="Metric key" className="flex-1 px-2 py-1 text-xs border rounded bg-white dark:bg-slate-800 dark:border-slate-700 text-slate-700 dark:text-slate-300 outline-none focus:ring-1 focus:ring-sky-500" />
                  <button onClick={() => removeRegex(name)} className="text-red-500 hover:text-red-700 text-xs px-1">&times;</button>
                </div>
              </div>
            </div>
          ))}
        </CardContent>
      </Card>

      {/* Impl Analyzers */}
      <Card>
        <CardHeader className="flex flex-row items-center justify-between pb-2">
          <CardTitle className="text-base">{t('config.analyzers.implAnalyzers')}</CardTitle>
          <button onClick={addImpl} className="text-xs px-3 py-1.5 rounded-md bg-sky-500 text-white hover:bg-sky-600 transition-colors">
            + {t('config.analyzers.addImpl')}
          </button>
        </CardHeader>
        <CardContent className="space-y-3">
          {Object.entries(config.custom_impl_analyzers).length === 0 && (
            <p className="text-sm text-slate-400 italic">{t('dashboard.noData')}</p>
          )}
          {Object.entries(config.custom_impl_analyzers).map(([name, analyzer]) => (
            <div key={name} className="flex items-start gap-2 p-3 border rounded-lg dark:border-slate-700">
              <div className="flex-1 grid grid-cols-3 gap-2">
                <input type="text" value={name} readOnly className="px-2 py-1 text-xs border rounded bg-slate-50 dark:bg-slate-800 dark:border-slate-700 text-slate-500" />
                <input type="text" value={analyzer.metric_key || ''} onChange={e => updateImpl(name, 'metric_key', e.target.value)} placeholder="Metric key" className="px-2 py-1 text-xs border rounded bg-white dark:bg-slate-800 dark:border-slate-700 text-slate-700 dark:text-slate-300 outline-none focus:ring-1 focus:ring-sky-500" />
                <div className="flex gap-2">
                  <input type="text" value={analyzer.category || ''} onChange={e => updateImpl(name, 'category', e.target.value)} placeholder="Category" className="flex-1 px-2 py-1 text-xs border rounded bg-white dark:bg-slate-800 dark:border-slate-700 text-slate-700 dark:text-slate-300 outline-none focus:ring-1 focus:ring-sky-500" />
                  <button onClick={() => removeImpl(name)} className="text-red-500 hover:text-red-700 text-xs px-1">&times;</button>
                </div>
              </div>
            </div>
          ))}
        </CardContent>
      </Card>

      {/* External Analyzers */}
      <Card>
        <CardHeader className="flex flex-row items-center justify-between pb-2">
          <CardTitle className="text-base">{t('config.analyzers.externalAnalyzers')}</CardTitle>
          <button onClick={addExternal} className="text-xs px-3 py-1.5 rounded-md bg-sky-500 text-white hover:bg-sky-600 transition-colors">
            + {t('config.analyzers.addExternal')}
          </button>
        </CardHeader>
        <CardContent className="space-y-3">
          {Object.entries(config.external_analyzers).length === 0 && (
            <p className="text-sm text-slate-400 italic">{t('dashboard.noData')}</p>
          )}
          {Object.entries(config.external_analyzers).map(([name, path]) => (
            <div key={name} className="flex items-start gap-2 p-3 border rounded-lg dark:border-slate-700">
              <div className="flex-1 grid grid-cols-3 gap-2">
                <input type="text" value={name} readOnly className="px-2 py-1 text-xs border rounded bg-slate-50 dark:bg-slate-800 dark:border-slate-700 text-slate-500" />
                <input type="text" value={path} onChange={e => updateExternal(name, e.target.value)} placeholder="Path to WASM" className="col-span-1 px-2 py-1 text-xs border rounded bg-white dark:bg-slate-800 dark:border-slate-700 text-slate-700 dark:text-slate-300 outline-none focus:ring-1 focus:ring-sky-500" />
                <button onClick={() => removeExternal(name)} className="text-red-500 hover:text-red-700 text-xs px-1 justify-self-end">&times;</button>
              </div>
            </div>
          ))}
        </CardContent>
      </Card>
    </div>
  )
}

// ─── Views Editor ───────────────────────────────────────────────────────────

const CHART_TYPES = [
  { value: '', label: 'Default' },
  { value: 'bar_row', label: 'Horizontal Bar' },
  { value: 'bar_col', label: 'Vertical Bar' },
  { value: 'pie', label: 'Pie' },
  { value: 'line', label: 'Line' },
  { value: 'stacked_bar', label: 'Stacked Bar' },
  { value: 'heatmap', label: 'Heatmap' },
  { value: 'radar', label: 'Radar' },
  { value: 'gauge', label: 'Gauge' },
  { value: 'table', label: 'Table' },
  { value: 'card', label: 'Metric Card' },
]

const CHANGE_TYPE_MODES = [
  { value: '', label: 'None' },
  { value: 'all', label: 'All (Stacked)' },
  { value: 'switchable', label: 'Switchable (A/M/D)' },
]

function ViewsEditor({ config, onChange }: {
  config: FullProjectConfig;
  onChange: (c: FullProjectConfig) => void;
}) {
  const { t } = useTranslation()

  const updateView = (id: string, field: string, value: string | boolean | string[] | undefined) => {
    const views = { ...config.aggregation_views }
    views[id] = { ...views[id], [field]: value }
    onChange({ ...config, aggregation_views: views })
  }

  const updateFunc = (id: string, field: string, value: string | number | number[] | undefined) => {
    const views = { ...config.aggregation_views }
    views[id] = { ...views[id], func: { ...views[id].func, [field]: value } }
    onChange({ ...config, aggregation_views: views })
  }

  const addView = () => {
    const key = `new_view_${Date.now()}`
    onChange({
      ...config,
      aggregation_views: {
        ...config.aggregation_views,
        [key]: { title: 'New View', tech_stacks: [], func: { type: 'top_n', limit: 10 } },
      },
    })
  }

  const removeView = (id: string) => {
    const views = { ...config.aggregation_views }
    delete views[id]
    onChange({ ...config, aggregation_views: views })
  }

  return (
    <div className="space-y-4">
      {Object.entries(config.aggregation_views).map(([id, view]) => (
        <Card key={id}>
          <CardHeader className="flex flex-row items-center justify-between pb-2">
            <CardTitle className="text-base">{t('config.views.title', { id })}</CardTitle>
            <button
              onClick={() => removeView(id)}
              className="text-xs px-2 py-1 rounded text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
            >
              {t('config.techStack.delete')}
            </button>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1">{t('config.views.viewTitle')}</label>
                <input type="text" value={view.title} onChange={e => updateView(id, 'title', e.target.value)}
                  className="w-full px-3 py-2 text-sm border rounded-lg bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 text-slate-700 dark:text-slate-300 focus:ring-2 focus:ring-sky-500 outline-none" />
              </div>
              <div>
                <label className="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1">{t('config.views.chartType')}</label>
                <SelectInput value={view.chart_type || ''} onChange={v => updateView(id, 'chart_type', v || undefined)} options={CHART_TYPES} />
              </div>
            </div>
            <div>
              <label className="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1">{t('config.views.techStacks')}</label>
              <TagInput tags={view.tech_stacks || []} onChange={v => updateView(id, 'tech_stacks', v)} placeholder="Leave empty for Summary" />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1">{t('config.views.changeTypeMode')}</label>
                <SelectInput value={view.change_type_mode || ''} onChange={v => updateView(id, 'change_type_mode', v || undefined)} options={CHANGE_TYPE_MODES} />
              </div>
              <div>
                <label className="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1">{t('config.views.includeChildren')}</label>
                <label className="relative inline-flex items-center cursor-pointer mt-2">
                  <input type="checkbox" checked={view.include_children !== false} onChange={e => updateView(id, 'include_children', e.target.checked)}
                    className="sr-only peer" />
                  <div className="w-9 h-5 bg-slate-300 dark:bg-slate-600 peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-sky-500 rounded-full peer peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-slate-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-sky-500"></div>
                </label>
              </div>
            </div>
            <div>
              <label className="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1">{t('config.views.groupBy')}</label>
              <TagInput tags={view.group_by || []} onChange={v => updateView(id, 'group_by', v)} />
            </div>
            <div className="border-t dark:border-slate-700 pt-3">
              <label className="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-2">{t('config.views.funcType')}: {view.func.type}</label>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-xs text-slate-400 mb-1">{t('config.views.analyzerId')}</label>
                  <input type="text" value={view.func.analyzer_id || ''} onChange={e => updateFunc(id, 'analyzer_id', e.target.value || undefined)}
                    className="w-full px-3 py-2 text-sm border rounded-lg bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 text-slate-700 dark:text-slate-300 outline-none focus:ring-1 focus:ring-sky-500" />
                </div>
                <div>
                  <label className="block text-xs text-slate-400 mb-1">{t('config.views.metricKey')}</label>
                  <input type="text" value={view.func.metric_key || ''} onChange={e => updateFunc(id, 'metric_key', e.target.value || undefined)}
                    className="w-full px-3 py-2 text-sm border rounded-lg bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 text-slate-700 dark:text-slate-300 outline-none focus:ring-1 focus:ring-sky-500" />
                </div>
                <div>
                  <label className="block text-xs text-slate-400 mb-1">{t('config.views.category')}</label>
                  <input type="text" value={view.func.category || ''} onChange={e => updateFunc(id, 'category', e.target.value || undefined)}
                    className="w-full px-3 py-2 text-sm border rounded-lg bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 text-slate-700 dark:text-slate-300 outline-none focus:ring-1 focus:ring-sky-500" />
                </div>
                {view.func.type === 'top_n' && (
                  <div>
                    <label className="block text-xs text-slate-400 mb-1">{t('config.views.limit')}</label>
                    <input type="number" value={view.func.limit || 10} onChange={e => updateFunc(id, 'limit', parseInt(e.target.value) || 10)}
                      className="w-full px-3 py-2 text-sm border rounded-lg bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 text-slate-700 dark:text-slate-300 outline-none focus:ring-1 focus:ring-sky-500" />
                  </div>
                )}
                {view.func.type === 'distribution' && (
                  <div>
                    <label className="block text-xs text-slate-400 mb-1">{t('config.views.buckets')}</label>
                    <input type="text" value={(view.func.buckets || []).join(',')} onChange={e => updateFunc(id, 'buckets', e.target.value.split(',').map(Number).filter(n => !isNaN(n)))}
                      placeholder="e.g. 0,10,50,100"
                      className="w-full px-3 py-2 text-sm border rounded-lg bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 text-slate-700 dark:text-slate-300 outline-none focus:ring-1 focus:ring-sky-500" />
                  </div>
                )}
              </div>
            </div>
          </CardContent>
        </Card>
      ))}
      <button
        onClick={addView}
        className="w-full py-2 border-2 border-dashed border-slate-300 dark:border-slate-600 rounded-lg text-sm text-slate-500 dark:text-slate-400 hover:border-sky-400 hover:text-sky-500 transition-colors"
      >
        + {t('config.views.add')}
      </button>
    </div>
  )
}

// ─── Config Page ────────────────────────────────────────────────────────────

type ConfigTab = 'tech_stacks' | 'global_excludes' | 'analyzers' | 'views'

const TABS: { key: ConfigTab; labelKey: string }[] = [
  { key: 'tech_stacks', labelKey: 'config.tabs.techStacks' },
  { key: 'global_excludes', labelKey: 'config.tabs.globalExcludes' },
  { key: 'analyzers', labelKey: 'config.tabs.analyzers' },
  { key: 'views', labelKey: 'config.tabs.views' },
]

export default function ConfigPage() {
  const { t } = useTranslation()
  const { currentProject, setProject, triggerConfigRefresh } = useApp()

  const [appConfig, setAppConfig] = useState<AppConfig | null>(null)
  const [config, setConfig] = useState<FullProjectConfig | null>(null)
  const [originalConfig, setOriginalConfig] = useState<string>('')
  const [activeTab, setActiveTab] = useState<ConfigTab>('tech_stacks')
  const [saving, setSaving] = useState(false)
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null)
  const [loading, setLoading] = useState(true)

  // Load project list
  useEffect(() => {
    fetchConfig().then(setAppConfig).catch(() => {})
  }, [])

  // Load full project config
  useEffect(() => {
    if (!currentProject) return
    setLoading(true)
    setMessage(null)
    fetchFullProjectConfig(currentProject)
      .then(cfg => {
        setConfig(cfg)
        setOriginalConfig(JSON.stringify(cfg))
      })
      .catch((err: unknown) => setMessage({ type: 'error', text: err instanceof Error ? err.message : String(err) }))
      .finally(() => setLoading(false))
  }, [currentProject])

  const hasChanges = config && originalConfig !== JSON.stringify(config)

  const handleSave = async () => {
    if (!config || !currentProject) return
    setSaving(true)
    setMessage(null)
    try {
      const result = await updateProjectConfig(currentProject, config)
      setOriginalConfig(JSON.stringify(config))
      setMessage({ type: 'success', text: result.message || t('config.savedSuccess') })
      triggerConfigRefresh()
    } catch (err) {
      setMessage({ type: 'error', text: err instanceof Error ? err.message : t('config.saveError') })
    } finally {
      setSaving(false)
    }
  }

  const handleReset = () => {
    if (!currentProject) return
    setLoading(true)
    fetchFullProjectConfig(currentProject)
      .then(cfg => {
        setConfig(cfg)
        setOriginalConfig(JSON.stringify(cfg))
        setMessage(null)
      })
      .catch((err: unknown) => setMessage({ type: 'error', text: err instanceof Error ? err.message : String(err) }))
      .finally(() => setLoading(false))
  }

  const renderTabContent = () => {
    if (!config) return null
    switch (activeTab) {
      case 'tech_stacks':
        return <TechStacksEditor config={config} onChange={setConfig} />
      case 'global_excludes':
        return <GlobalExcludesEditor config={config} onChange={setConfig} />
      case 'analyzers':
        return <AnalyzersEditor config={config} onChange={setConfig} />
      case 'views':
        return <ViewsEditor config={config} onChange={setConfig} />
    }
  }

  const projects = appConfig?.projects || []

  return (
    <div className="flex flex-col h-full bg-slate-50 dark:bg-slate-900">
      {/* Header */}
      <div className="sticky top-0 z-10 bg-white dark:bg-slate-800 border-b border-slate-200 dark:border-slate-700 px-4 sm:px-6 py-3">
        <div className="flex items-center justify-between max-w-7xl mx-auto">
          <div className="flex items-center gap-4">
            <h1 className="text-lg font-semibold text-slate-800 dark:text-slate-100">{t('config.title')}</h1>
            {projects.length > 1 && (
              <select
                value={currentProject}
                onChange={e => setProject(e.target.value)}
                className="px-3 py-1.5 text-sm border rounded-lg bg-white dark:bg-slate-700 border-slate-200 dark:border-slate-600 text-slate-700 dark:text-slate-300 outline-none focus:ring-2 focus:ring-sky-500"
              >
                {projects.map(p => (
                  <option key={p.name} value={p.name}>{p.name}</option>
                ))}
              </select>
            )}
          </div>
          <div className="flex items-center gap-2">
            {message && (
              <span className={`text-sm px-3 py-1 rounded ${
                message.type === 'success'
                  ? 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300'
                  : 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300'
              }`}>
                {message.text}
              </span>
            )}
            <button
              onClick={handleReset}
              disabled={!hasChanges || loading}
              className="px-3 py-1.5 text-sm rounded-lg border border-slate-300 dark:border-slate-600 text-slate-600 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-700 disabled:opacity-40 transition-colors"
            >
              {t('config.reset')}
            </button>
            <button
              onClick={handleSave}
              disabled={!hasChanges || saving || loading}
              className="px-4 py-1.5 text-sm rounded-lg bg-sky-500 text-white hover:bg-sky-600 disabled:opacity-40 transition-colors"
            >
              {saving ? t('config.saving') : t('config.save')}
            </button>
          </div>
        </div>
      </div>

      {/* Tabs */}
      <div className="border-b border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800/50">
        <div className="flex max-w-7xl mx-auto px-4 sm:px-6">
          {TABS.map(tab => (
            <button
              key={tab.key}
              onClick={() => setActiveTab(tab.key)}
              className={`px-4 py-2.5 text-sm font-medium transition-colors border-b-2 -mb-px ${
                activeTab === tab.key
                  ? 'border-sky-500 text-sky-600 dark:text-sky-400'
                  : 'border-transparent text-slate-500 dark:text-slate-400 hover:text-slate-700 dark:hover:text-slate-300'
              }`}
            >
              {t(tab.labelKey)}
            </button>
          ))}
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 py-6">
          {loading ? (
            <div className="flex items-center justify-center py-20">
              <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-sky-500"></div>
              <span className="ml-3 text-sm text-slate-500">{t('config.loading')}</span>
            </div>
          ) : config ? (
            renderTabContent()
          ) : (
            <div className="text-center py-20 text-slate-400">
              <p>{t('dashboard.noData')}</p>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
