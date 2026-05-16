import { useState, useEffect, useCallback, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { useApp } from '@/contexts/AppContext'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import {
  fetchConfig,
  fetchFullProjectConfig,
  updateProjectConfig,
  fetchTemplates,
  saveTemplate,
  deleteTemplate,
  reloadConfig,
  createProject,
  deleteProject as deleteProjectApi,
  type AppConfig,
  type FullProjectConfig,
  type FullTechStack,
  type UnifiedProjectInfo,
} from '@/services/data'
import { Settings, BookTemplate, Trash2, Database, GitBranch, ChevronDown, ChevronRight } from 'lucide-react'

// ─── Tag Input ──────────────────────────────────────────────────────────────

function TagInput({ tags, onChange, placeholder, suggestions, allowCustom = true, maxTags }: {
  tags: string[];
  onChange: (tags: string[]) => void;
  placeholder?: string;
  suggestions?: string[];
  allowCustom?: boolean;
  maxTags?: number;
}) {
  const [input, setInput] = useState('')
  const [showDropdown, setShowDropdown] = useState(false)
  const [activeIndex, setActiveIndex] = useState(-1)

  const filtered = suggestions
    ? suggestions.filter(s => !tags.includes(s) && s.toLowerCase().includes(input.toLowerCase()))
    : []

  const addTag = (val?: string) => {
    if (maxTags !== undefined && tags.length >= maxTags) return
    const value = val || input.trim()
    if (!value) return
    if (suggestions && !allowCustom && !suggestions.includes(value)) return
    if (!tags.includes(value)) {
      onChange([...tags, value])
    }
    setInput('')
    setShowDropdown(false)
    setActiveIndex(-1)
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      if (activeIndex >= 0 && filtered[activeIndex]) {
        addTag(filtered[activeIndex])
      } else {
        addTag()
      }
      return
    }
    if (e.key === ',' && input.trim()) {
      e.preventDefault()
      addTag()
      return
    }
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setActiveIndex(i => Math.min(i + 1, filtered.length - 1))
      return
    }
    if (e.key === 'ArrowUp') {
      e.preventDefault()
      setActiveIndex(i => Math.max(i - 1, 0))
      return
    }
    if (e.key === 'Escape') {
      setShowDropdown(false)
      setActiveIndex(-1)
    }
  }

  return (
    <div className="relative">
      <div className="flex flex-wrap gap-1.5 p-2 border rounded-lg bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 min-h-[42px]">
        {tags.map(tag => (
          <span key={tag} className="inline-flex items-center gap-1 px-2.5 py-0.5 text-xs font-medium rounded-md bg-sky-100 dark:bg-sky-900/50 text-sky-700 dark:text-sky-300">
            {tag}
            <button onClick={() => onChange(tags.filter(t => t !== tag))} className="hover:text-red-500 ml-0.5">&times;</button>
          </span>
        ))}
        <input
          value={input}
          onChange={e => { setInput(e.target.value); setShowDropdown(true); setActiveIndex(-1) }}
          onKeyDown={handleKeyDown}
          onFocus={() => setShowDropdown(true)}
          onBlur={() => setTimeout(() => setShowDropdown(false), 150)}
          placeholder={tags.length === 0 ? (placeholder || 'Type and press Enter...') : ''}
          className="flex-1 min-w-[100px] outline-none bg-transparent text-sm text-slate-700 dark:text-slate-300 placeholder-slate-400"
          disabled={maxTags !== undefined && tags.length >= maxTags}
        />
        {maxTags !== undefined && (
          <span className="text-xs text-slate-400 dark:text-slate-500 ml-1 self-center">{tags.length}/{maxTags}</span>
        )}
      </div>
      {showDropdown && filtered.length > 0 && (
        <div className="absolute z-20 top-full mt-1 left-0 right-0 bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg shadow-lg max-h-48 overflow-y-auto">
          {filtered.map((s, i) => (
            <button
              key={s}
              onMouseDown={e => { e.preventDefault(); addTag(s) }}
              className={`w-full text-left px-3 py-1.5 text-sm transition-colors ${
                i === activeIndex
                  ? 'bg-sky-100 dark:bg-sky-900/50 text-sky-700 dark:text-sky-300'
                  : 'text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-700'
              }`}
            >
              {s}
            </button>
          ))}
        </div>
      )}
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
  const [expandedStacks, setExpandedStacks] = useState<Set<number>>(new Set())

  const toggleStack = (index: number) => {
    setExpandedStacks(prev => {
      const next = new Set(prev)
      if (next.has(index)) next.delete(index)
      else next.add(index)
      return next
    })
  }

  const expandAllStacks = () => {
    setExpandedStacks(new Set(config.tech_stacks.map((_, i) => i)))
  }

  const collapseAllStacks = () => {
    setExpandedStacks(new Set())
  }

  const validAnalyzerIds = useMemo(() => {
    const ids = new Set<string>(['file_count', 'char_count'])
    for (const key of Object.keys(config.custom_regex_analyzers || {})) ids.add(key)
    for (const key of Object.keys(config.custom_impl_analyzers || {})) ids.add(key)
    for (const key of Object.keys(config.external_analyzers || {})) ids.add(key)
    return Array.from(ids).sort()
  }, [config.custom_regex_analyzers, config.custom_impl_analyzers, config.external_analyzers])

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
    setExpandedStacks(prev => {
      const next = new Set<number>()
      for (const i of prev) {
        if (i === index) continue
        next.add(i > index ? i - 1 : i)
      }
      return next
    })
  }

  return (
    <div className="space-y-4">
      {config.tech_stacks.length > 0 && (
        <div className="flex items-center gap-2">
          <button onClick={expandAllStacks} className="text-xs px-2 py-1 rounded text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors">
            {t('config.expandAll')}
          </button>
          <button onClick={collapseAllStacks} className="text-xs px-2 py-1 rounded text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors">
            {t('config.collapseAll')}
          </button>
        </div>
      )}
      {config.tech_stacks.map((stack, i) => (
        <Card key={i}>
          <button className="w-full text-left" onClick={() => toggleStack(i)}>
            <CardHeader className="flex flex-row items-center justify-between pb-2">
              <span className="flex items-center gap-2 min-w-0">
                {expandedStacks.has(i) ? <ChevronDown className="w-4 h-4 shrink-0 text-slate-400" /> : <ChevronRight className="w-4 h-4 shrink-0 text-slate-400" />}
                <CardTitle className="text-base truncate">{t('config.techStack.title', { name: stack.name || `#${i + 1}` })}</CardTitle>
              </span>
              <span className="shrink-0">
                <button
                  onClick={e => { e.stopPropagation(); removeStack(i) }}
                  className="text-xs px-2 py-1 rounded text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
                >
                  {t('config.techStack.delete')}
                </button>
              </span>
            </CardHeader>
          </button>
          <div className={`transition-all duration-200 ease-in-out ${expandedStacks.has(i) ? 'max-h-[3000px] opacity-100 overflow-visible' : 'max-h-0 opacity-0 overflow-hidden'}`}>
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
              <TagInput tags={stack.analyzers} onChange={v => updateStack(i, 'analyzers', v)} placeholder="e.g. file_count" suggestions={validAnalyzerIds} allowCustom={false} />
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
          </div>
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

  // Track temporary name field edits (for smooth controlled input while renaming)
  const [editValues, setEditValues] = useState<Record<string, string>>({})
  const [nameErrors, setNameErrors] = useState<Record<string, string>>({})

  const getAllAnalyzerNames = (excludeNames: string[] = []): Set<string> => {
    const names = new Set<string>()
    for (const key of Object.keys(config.custom_regex_analyzers || {})) { if (!excludeNames.includes(key)) names.add(key) }
    for (const key of Object.keys(config.custom_impl_analyzers || {})) { if (!excludeNames.includes(key)) names.add(key) }
    for (const key of Object.keys(config.external_analyzers || {})) { if (!excludeNames.includes(key)) names.add(key) }
    return names
  }

  // ── Regex Analyzers ──

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

  const renameRegex = (oldName: string, newName: string) => {
    if (oldName === newName || !newName.trim()) return
    if (getAllAnalyzerNames([oldName]).has(newName)) {
      setNameErrors(prev => ({ ...prev, [`regex:${oldName}`]: t('config.analyzers.nameDuplicate') }))
      return
    }
    setNameErrors(prev => { const next = { ...prev }; delete next[`regex:${oldName}`]; return next })
    const analyzers = { ...config.custom_regex_analyzers }
    analyzers[newName] = analyzers[oldName]
    delete analyzers[oldName]
    onChange({ ...config, custom_regex_analyzers: analyzers })
  }

  // ── Impl Analyzers ──

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

  const renameImpl = (oldName: string, newName: string) => {
    if (oldName === newName || !newName.trim()) return
    if (getAllAnalyzerNames([oldName]).has(newName)) {
      setNameErrors(prev => ({ ...prev, [`impl:${oldName}`]: t('config.analyzers.nameDuplicate') }))
      return
    }
    setNameErrors(prev => { const next = { ...prev }; delete next[`impl:${oldName}`]; return next })
    const analyzers = { ...config.custom_impl_analyzers }
    analyzers[newName] = analyzers[oldName]
    delete analyzers[oldName]
    onChange({ ...config, custom_impl_analyzers: analyzers })
  }

  // ── External Analyzers ──

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

  const renameExternal = (oldName: string, newName: string) => {
    if (oldName === newName || !newName.trim()) return
    if (getAllAnalyzerNames([oldName]).has(newName)) {
      setNameErrors(prev => ({ ...prev, [`external:${oldName}`]: t('config.analyzers.nameDuplicate') }))
      return
    }
    setNameErrors(prev => { const next = { ...prev }; delete next[`external:${oldName}`]; return next })
    const analyzers = { ...config.external_analyzers }
    analyzers[newName] = analyzers[oldName]
    delete analyzers[oldName]
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
          {Object.entries(config.custom_regex_analyzers).map(([name, analyzer], index) => {
            const displayName = editValues[`regex:${name}`] ?? name
            const error = nameErrors[`regex:${name}`]
            return (
            <div key={`regex-${index}`} className="flex items-start gap-2 p-3 border rounded-lg dark:border-slate-700">
              <div className="flex-1 grid grid-cols-3 gap-2">
                <div className="flex flex-col gap-0.5">
                  <input
                    type="text"
                    value={displayName}
                    onChange={e => setEditValues(prev => ({ ...prev, [`regex:${name}`]: e.target.value }))}
                    onFocus={() => setNameErrors(prev => { const n = { ...prev }; delete n[`regex:${name}`]; return n })}
                    onBlur={() => {
                      const val = editValues[`regex:${name}`]
                      if (val !== undefined) {
                        setEditValues(prev => { const n = { ...prev }; delete n[`regex:${name}`]; return n })
                        renameRegex(name, val)
                      }
                    }}
                    placeholder="Analyzer name"
                    className="px-2 py-1 text-xs border rounded bg-white dark:bg-slate-800 dark:border-slate-700 text-slate-700 dark:text-slate-300 outline-none focus:ring-1 focus:ring-sky-500"
                  />
                  {error && <span className="text-red-500 text-[10px] leading-tight">{error}</span>}
                </div>
                <input type="text" value={analyzer.metric_key} onChange={e => updateRegex(name, 'metric_key', e.target.value)} placeholder="Metric key" className="px-2 py-1 text-xs border rounded bg-white dark:bg-slate-800 dark:border-slate-700 text-slate-700 dark:text-slate-300 outline-none focus:ring-1 focus:ring-sky-500" />
                <div className="flex gap-2">
                  <input type="text" value={analyzer.category || ''} onChange={e => updateRegex(name, 'category', e.target.value)} placeholder="Category" className="flex-1 px-2 py-1 text-xs border rounded bg-white dark:bg-slate-800 dark:border-slate-700 text-slate-700 dark:text-slate-300 outline-none focus:ring-1 focus:ring-sky-500" />
                  <button onClick={() => removeRegex(name)} className="text-red-500 hover:text-red-700 text-xs px-1">&times;</button>
                </div>
                <input type="text" value={analyzer.pattern} onChange={e => updateRegex(name, 'pattern', e.target.value)} placeholder="Pattern" className="col-span-3 px-2 py-1 text-xs border rounded bg-white dark:bg-slate-800 dark:border-slate-700 text-slate-700 dark:text-slate-300 outline-none focus:ring-1 focus:ring-sky-500" />
              </div>
            </div>
            )
          })}
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
          {Object.entries(config.custom_impl_analyzers).map(([name, analyzer], index) => {
            const displayName = editValues[`impl:${name}`] ?? name
            const error = nameErrors[`impl:${name}`]
            return (
            <div key={`impl-${index}`} className="flex items-start gap-2 p-3 border rounded-lg dark:border-slate-700">
              <div className="flex-1 grid grid-cols-3 gap-2">
                <div className="flex flex-col gap-0.5">
                  <input
                    type="text"
                    value={displayName}
                    onChange={e => setEditValues(prev => ({ ...prev, [`impl:${name}`]: e.target.value }))}
                    onFocus={() => setNameErrors(prev => { const n = { ...prev }; delete n[`impl:${name}`]; return n })}
                    onBlur={() => {
                      const val = editValues[`impl:${name}`]
                      if (val !== undefined) {
                        setEditValues(prev => { const n = { ...prev }; delete n[`impl:${name}`]; return n })
                        renameImpl(name, val)
                      }
                    }}
                    placeholder="Analyzer name"
                    className="px-2 py-1 text-xs border rounded bg-white dark:bg-slate-800 dark:border-slate-700 text-slate-700 dark:text-slate-300 outline-none focus:ring-1 focus:ring-sky-500"
                  />
                  {error && <span className="text-red-500 text-[10px] leading-tight">{error}</span>}
                </div>
                <input type="text" value={analyzer.metric_key || ''} onChange={e => updateImpl(name, 'metric_key', e.target.value)} placeholder="Metric key" className="px-2 py-1 text-xs border rounded bg-white dark:bg-slate-800 dark:border-slate-700 text-slate-700 dark:text-slate-300 outline-none focus:ring-1 focus:ring-sky-500" />
                <div className="flex gap-2">
                  <input type="text" value={analyzer.category || ''} onChange={e => updateImpl(name, 'category', e.target.value)} placeholder="Category" className="flex-1 px-2 py-1 text-xs border rounded bg-white dark:bg-slate-800 dark:border-slate-700 text-slate-700 dark:text-slate-300 outline-none focus:ring-1 focus:ring-sky-500" />
                  <button onClick={() => removeImpl(name)} className="text-red-500 hover:text-red-700 text-xs px-1">&times;</button>
                </div>
              </div>
            </div>
            )
          })}
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
          {Object.entries(config.external_analyzers).map(([name, path], index) => {
            const displayName = editValues[`external:${name}`] ?? name
            const error = nameErrors[`external:${name}`]
            return (
            <div key={`external-${index}`} className="flex items-start gap-2 p-3 border rounded-lg dark:border-slate-700">
              <div className="flex-1 grid grid-cols-3 gap-2">
                <div className="flex flex-col gap-0.5">
                  <input
                    type="text"
                    value={displayName}
                    onChange={e => setEditValues(prev => ({ ...prev, [`external:${name}`]: e.target.value }))}
                    onFocus={() => setNameErrors(prev => { const n = { ...prev }; delete n[`external:${name}`]; return n })}
                    onBlur={() => {
                      const val = editValues[`external:${name}`]
                      if (val !== undefined) {
                        setEditValues(prev => { const n = { ...prev }; delete n[`external:${name}`]; return n })
                        renameExternal(name, val)
                      }
                    }}
                    placeholder="Analyzer name"
                    className="px-2 py-1 text-xs border rounded bg-white dark:bg-slate-800 dark:border-slate-700 text-slate-700 dark:text-slate-300 outline-none focus:ring-1 focus:ring-sky-500"
                  />
                  {error && <span className="text-red-500 text-[10px] leading-tight">{error}</span>}
                </div>
                <input type="text" value={path} onChange={e => updateExternal(name, e.target.value)} placeholder="Path to WASM" className="col-span-1 px-2 py-1 text-xs border rounded bg-white dark:bg-slate-800 dark:border-slate-700 text-slate-700 dark:text-slate-300 outline-none focus:ring-1 focus:ring-sky-500" />
                <button onClick={() => removeExternal(name)} className="text-red-500 hover:text-red-700 text-xs px-1 justify-self-end">&times;</button>
              </div>
            </div>
            )
          })}
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

const GROUP_BY_OPTIONS = ['tech_stack', 'category', 'metric_key', 'analyzer_id']

function ViewsEditor({ config, onChange }: {
  config: FullProjectConfig;
  onChange: (c: FullProjectConfig) => void;
}) {
  const { t } = useTranslation()
  const [expandedViews, setExpandedViews] = useState<Set<string>>(new Set())

  const toggleView = (id: string) => {
    setExpandedViews(prev => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const expandAllViews = () => {
    setExpandedViews(new Set(Object.keys(config.aggregation_views)))
  }

  const collapseAllViews = () => {
    setExpandedViews(new Set())
  }

  const validAnalyzerIds = useMemo(() => {
    const ids = new Set<string>(['file_count', 'char_count'])
    for (const key of Object.keys(config.custom_regex_analyzers || {})) ids.add(key)
    for (const key of Object.keys(config.custom_impl_analyzers || {})) ids.add(key)
    for (const key of Object.keys(config.external_analyzers || {})) ids.add(key)
    return Array.from(ids).sort()
  }, [config.custom_regex_analyzers, config.custom_impl_analyzers, config.external_analyzers])

  const updateView = (id: string, field: string, value: string | boolean | string[] | undefined) => {
    const views = { ...config.aggregation_views }
    views[id] = { ...views[id], [field]: value }
    onChange({ ...config, aggregation_views: views })
  }

  const updateFunc = (id: string, field: string, value: string | string[] | number | number[] | undefined) => {
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
    setExpandedViews(prev => {
      const next = new Set(prev)
      next.delete(id)
      return next
    })
  }

  return (
    <div className="space-y-4">
      {Object.keys(config.aggregation_views).length > 0 && (
        <div className="flex items-center gap-2">
          <button onClick={expandAllViews} className="text-xs px-2 py-1 rounded text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors">
            {t('config.expandAll')}
          </button>
          <button onClick={collapseAllViews} className="text-xs px-2 py-1 rounded text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors">
            {t('config.collapseAll')}
          </button>
        </div>
      )}
      {Object.entries(config.aggregation_views).map(([id, view]) => (
        <Card key={id}>
          <button className="w-full text-left" onClick={() => toggleView(id)}>
            <CardHeader className="flex flex-row items-center justify-between pb-2">
              <span className="flex items-center gap-2 min-w-0">
                {expandedViews.has(id) ? <ChevronDown className="w-4 h-4 shrink-0 text-slate-400" /> : <ChevronRight className="w-4 h-4 shrink-0 text-slate-400" />}
                <CardTitle className="text-base truncate">{t('config.views.title', { id })}</CardTitle>
              </span>
              <span className="shrink-0">
                <button
                  onClick={e => { e.stopPropagation(); removeView(id) }}
                  className="text-xs px-2 py-1 rounded text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
                >
                  {t('config.techStack.delete')}
                </button>
              </span>
            </CardHeader>
          </button>
          <div className={`transition-all duration-200 ease-in-out ${expandedViews.has(id) ? 'max-h-[3000px] opacity-100 overflow-visible' : 'max-h-0 opacity-0 overflow-hidden'}`}>
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
              <TagInput tags={view.tech_stacks || []} onChange={v => updateView(id, 'tech_stacks', v)} placeholder="Leave empty for Summary" suggestions={config.tech_stacks.map(s => s.name)} allowCustom={false} />
            </div>
            <div>
              <label className="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1">{t('config.views.changeTypeMode')}</label>
              <SelectInput value={view.change_type_mode || ''} onChange={v => updateView(id, 'change_type_mode', v || undefined)} options={CHANGE_TYPE_MODES} />
            </div>
            <div>
              <label className="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1">{t('config.views.groupBy')}</label>
              <TagInput tags={view.group_by || []} onChange={v => updateView(id, 'group_by', v)} suggestions={GROUP_BY_OPTIONS} allowCustom={false} placeholder="e.g. tech_stack, category" maxTags={2} />
            </div>
            <div className="border-t dark:border-slate-700 pt-3">
              <label className="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-2">{t('config.views.funcType')}: {view.func.type}</label>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-xs text-slate-400 mb-1">{t('config.views.analyzerId')}</label>
                  <TagInput tags={view.func.analyzer_id || []} onChange={v => updateFunc(id, 'analyzer_id', v.length > 0 ? v : undefined)} suggestions={validAnalyzerIds} allowCustom={false} placeholder="Select analyzer IDs..." />
                </div>
                <div className="space-y-3">
                  <div>
                    <label className="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1">{t('config.views.includeChildren')}</label>
                    <label className="relative inline-flex items-center cursor-pointer mt-2">
                      <input type="checkbox" checked={view.include_children !== false} onChange={e => updateView(id, 'include_children', e.target.checked)}
                        className="sr-only peer" />
                      <div className="w-9 h-5 bg-slate-300 dark:bg-slate-600 peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-sky-500 rounded-full peer peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-slate-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-sky-500"></div>
                    </label>
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
            </div>
          </CardContent>
          </div>
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

// ─── Templates Editor ────────────────────────────────────────────────────────

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
  const { currentProject, setProject, triggerConfigRefresh, configVersion, projectList } = useApp()

  const [appConfig, setAppConfig] = useState<AppConfig | null>(null)
  const [config, setConfig] = useState<FullProjectConfig | null>(null)
  const [originalConfig, setOriginalConfig] = useState<string>('')
  const [activeTab, setActiveTab] = useState<ConfigTab>('tech_stacks')
  const [saving, setSaving] = useState(false)
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null)
  const [loading, setLoading] = useState(true)
  const [showSaveAsTemplate, setShowSaveAsTemplate] = useState(false)
  const [templateNameInput, setTemplateNameInput] = useState('')
  const [templateSaving, setTemplateSaving] = useState(false)
  // Modal states
  const [showProjectManager, setShowProjectManager] = useState(false)
  const [showTemplateManager, setShowTemplateManager] = useState(false)

  // Load project list (re-fetch when configVersion changes)
  useEffect(() => {
    fetchConfig().then(setAppConfig).catch(() => {})
  }, [configVersion])

  // Load full project config (re-fetch when configVersion changes)
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
  }, [currentProject, configVersion])

  const hasChanges = config && originalConfig !== JSON.stringify(config)

  const handleSave = async () => {
    if (!config || !currentProject) return

    // Validate no duplicate analyzer names across all categories
    const allNames = new Set<string>()
    const duplicateNames: string[] = []
    const checkNames = (names: string[]) => {
      for (const name of names) {
        if (allNames.has(name)) duplicateNames.push(name)
        else allNames.add(name)
      }
    }
    checkNames(Object.keys(config.custom_regex_analyzers || {}))
    checkNames(Object.keys(config.custom_impl_analyzers || {}))
    checkNames(Object.keys(config.external_analyzers || {}))
    if (duplicateNames.length > 0) {
      setMessage({ type: 'error', text: `${t('config.analyzers.nameDuplicate')}: ${duplicateNames.join(', ')}` })
      return
    }

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

  const handleSaveAsTemplate = async () => {
    if (!config || !templateNameInput.trim()) return
    setTemplateSaving(true)
    try {
      await saveTemplate(templateNameInput.trim(), config)
      setMessage({ type: 'success', text: t('templates.saved', { name: templateNameInput.trim() }) })
      setShowSaveAsTemplate(false)
      setTemplateNameInput('')
    } catch (err) {
      setMessage({ type: 'error', text: err instanceof Error ? err.message : t('templates.saveError') })
    } finally {
      setTemplateSaving(false)
    }
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

  const projects = projectList.length > 0 ? projectList : (appConfig?.projects || []).map(p => ({ name: p.name, has_config: true } as UnifiedProjectInfo))

  return (
    <div className="flex flex-col h-full bg-slate-50 dark:bg-slate-900">
      {/* Header */}
      <div className="sticky top-0 z-10 bg-white dark:bg-slate-800 border-b border-slate-200 dark:border-slate-700 px-4 sm:px-6 py-3">
        <div className="flex items-center justify-between max-w-7xl mx-auto">
          <div className="flex items-center gap-4">
            <h1 className="text-lg font-semibold text-slate-800 dark:text-slate-100">{t('config.title')}</h1>
            {projects.length > 1 ? (
              <select
                value={currentProject}
                onChange={e => setProject(e.target.value)}
                className="px-3 py-1.5 text-sm border rounded-lg bg-white dark:bg-slate-700 border-slate-200 dark:border-slate-600 text-slate-700 dark:text-slate-300 outline-none focus:ring-2 focus:ring-sky-500"
              >
                {projects.map((p: UnifiedProjectInfo) => (
                  <option key={p.name} value={p.name}>
                    {p.name}{p.has_config ? '' : ' (no config)'}
                  </option>
                ))}
              </select>
            ) : projects.length === 1 ? (
              <span className="text-sm font-medium text-slate-700 dark:text-slate-300 px-2">
                {projects[0].name}
              </span>
            ) : null}
            {/* Project Management button */}
            <button
              onClick={() => setShowProjectManager(true)}
              className="flex items-center gap-1.5 px-3 py-1.5 text-sm rounded-lg border border-slate-300 dark:border-slate-600 text-slate-600 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-700 transition-colors"
              title={t('config.manageProjects') || 'Manage Projects'}
            >
              <Settings className="w-4 h-4" />
              <span className="hidden sm:inline">{t('config.manageProjects') || 'Projects'}</span>
            </button>
            {/* Templates button */}
            <button
              onClick={() => setShowTemplateManager(true)}
              className="flex items-center gap-1.5 px-3 py-1.5 text-sm rounded-lg border border-slate-300 dark:border-slate-600 text-slate-600 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-700 transition-colors"
              title={t('templates.title') || 'Templates'}
            >
              <BookTemplate className="w-4 h-4" />
              <span className="hidden sm:inline">{t('templates.title') || 'Templates'}</span>
            </button>
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
            {config && (
              <button
                onClick={() => setShowSaveAsTemplate(true)}
                disabled={loading}
                className="px-3 py-1.5 text-sm rounded-lg bg-emerald-600 text-white hover:bg-emerald-700 disabled:opacity-40 transition-colors"
              >
                {t('templates.saveAsTemplate')}
              </button>
            )}
            <button
              onClick={async () => {
                try {
                  await reloadConfig()
                  triggerConfigRefresh()
                  setMessage({ type: 'success', text: 'Configuration reloaded from disk' })
                } catch (err) {
                  setMessage({ type: 'error', text: err instanceof Error ? err.message : 'Failed to reload config' })
                }
              }}
              className="px-3 py-1.5 text-sm rounded-lg border border-slate-300 dark:border-slate-600 text-slate-600 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-700 transition-colors"
              title="Reload config from YAML file"
            >
              Reload
            </button>
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
            <div className="text-center py-20 text-slate-400 space-y-4">
              <Database className="w-12 h-12 mx-auto text-slate-300 dark:text-slate-600" />
              <p>{t('config.noConfigForProject', { project: currentProject })}</p>
              <button
                onClick={async () => {
                  try {
                    await createProject(currentProject)
                    triggerConfigRefresh()
                  } catch (err) {
                    console.error(err)
                  }
                }}
                className="px-4 py-2 text-sm rounded-lg bg-sky-500 text-white hover:bg-sky-600 transition-colors"
              >
                {t('config.createConfig') || 'Create Config'}
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Save as Template Modal */}
      {showSaveAsTemplate && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
          <div className="bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-600 rounded-xl shadow-2xl max-w-md w-full mx-4 p-5 space-y-4">
            <h3 className="text-base font-semibold text-slate-900 dark:text-slate-100">
              {t('templates.saveAsTemplate')}
            </h3>
            <input
              type="text"
              value={templateNameInput}
              onChange={e => setTemplateNameInput(e.target.value)}
              placeholder={t('templates.templateNamePlaceholder')}
              className="w-full px-4 py-2 bg-slate-100 dark:bg-slate-700 border border-slate-300 dark:border-slate-600 rounded-lg text-slate-900 dark:text-slate-100 placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-sky-500 text-sm"
              autoFocus
            />
            <div className="flex justify-end gap-2">
              <button
                onClick={() => { setShowSaveAsTemplate(false); setTemplateNameInput('') }}
                className="px-4 py-2 text-sm text-slate-700 dark:text-slate-300 bg-slate-100 dark:bg-slate-700 rounded-lg hover:bg-slate-200 dark:hover:bg-slate-600 transition-colors"
              >
                {t('execute.cancel')}
              </button>
              <button
                onClick={handleSaveAsTemplate}
                disabled={!templateNameInput.trim() || templateSaving}
                className="px-4 py-2 text-sm text-white bg-emerald-600 rounded-lg hover:bg-emerald-700 disabled:opacity-40 transition-colors"
              >
                {templateSaving ? t('config.saving') : t('templates.saveAsTemplate')}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ── Project Management Modal ─────────────────────────────────── */}
      {showProjectManager && (
        <ProjectManagementModal
          projectList={projectList}
          currentProject={currentProject}
          onSelect={(name) => { setProject(name); setShowProjectManager(false) }}
          onCreateConfig={async (name) => {
            try {
              await createProject(name)
              triggerConfigRefresh()
            } catch (err) {
              console.error(err)
            }
          }}
          onDelete={async (name) => {
            if (!confirm(t('config.deleteProjectConfirm', { name }) || `Delete project '${name}'? This will remove all config, scan data, and cached repos.`)) return
            try {
              await deleteProjectApi(name)
              triggerConfigRefresh()
            } catch (err) {
              console.error(err)
            }
          }}
          onClose={() => setShowProjectManager(false)}
          t={t}
        />
      )}

      {/* ── Template Management Modal ────────────────────────────────── */}
      {showTemplateManager && (
        <TemplateManagementModal
          onClose={() => setShowTemplateManager(false)}
          t={t}
        />
      )}
    </div>
  )
}

// ─── Project Management Modal ───────────────────────────────────────────

function ProjectManagementModal({ projectList, currentProject, onSelect, onCreateConfig, onDelete, onClose, t }: {
  projectList: UnifiedProjectInfo[]
  currentProject: string
  onSelect: (name: string) => void
  onCreateConfig: (name: string) => Promise<void>
  onDelete: (name: string) => Promise<void>
  onClose: () => void
  t: (key: string, opts?: any) => string
}) {
  const [deleting, setDeleting] = useState<string | null>(null)
  const [creating, setCreating] = useState<string | null>(null)

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm" onClick={onClose}>
      <div className="bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-600 rounded-xl shadow-2xl max-w-2xl w-full mx-4 p-5 space-y-4 max-h-[80vh] flex flex-col" onClick={e => e.stopPropagation()}>
        <div className="flex items-center justify-between">
          <h3 className="text-base font-semibold text-slate-900 dark:text-slate-100">
            {t('config.manageProjects') || 'Manage Projects'}
          </h3>
          <button onClick={onClose} className="text-slate-400 hover:text-slate-600 dark:hover:text-slate-300">
            ✕
          </button>
        </div>

        <div className="flex-1 overflow-y-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-slate-200 dark:border-slate-700 text-slate-500 dark:text-slate-400">
                <th className="text-left py-2 px-2 font-medium">Name</th>
                <th className="text-center py-2 px-2 font-medium">Config</th>
                <th className="text-center py-2 px-2 font-medium">Scans</th>
                <th className="text-center py-2 px-2 font-medium">Repo</th>
                <th className="text-right py-2 px-2 font-medium">Actions</th>
              </tr>
            </thead>
            <tbody>
              {projectList.map(p => (
                <tr key={p.name} className={`border-b border-slate-100 dark:border-slate-700/50 hover:bg-slate-50 dark:hover:bg-slate-700/30 ${p.name === currentProject ? 'bg-sky-50 dark:bg-sky-900/20' : ''}`}>
                  <td className="py-2.5 px-2">
                    <button
                      onClick={() => onSelect(p.name)}
                      className="text-sky-600 dark:text-sky-400 hover:underline font-medium text-left"
                    >
                      {p.name}
                    </button>
                  </td>
                  <td className="py-2.5 px-2 text-center">
                    {p.has_config ? (
                      <span className="text-xs px-2 py-0.5 rounded bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300">Yes</span>
                    ) : (
                      <button
                        onClick={async () => { setCreating(p.name); await onCreateConfig(p.name); setCreating(null) }}
                        disabled={creating === p.name}
                        className="text-xs px-2 py-0.5 rounded bg-sky-100 dark:bg-sky-900/30 text-sky-700 dark:text-sky-300 hover:bg-sky-200 dark:hover:bg-sky-900/50 disabled:opacity-40"
                      >
                        {creating === p.name ? '...' : 'Create'}
                      </button>
                    )}
                  </td>
                  <td className="py-2.5 px-2 text-center text-slate-600 dark:text-slate-400">
                    {p.total_scans > 0 ? p.total_scans : '-'}
                  </td>
                  <td className="py-2.5 px-2 text-center">
                    {p.has_cached_repo ? (
                      <span className="inline-flex items-center gap-1 text-xs text-slate-600 dark:text-slate-400">
                        <GitBranch className="w-3 h-3" /> {p.cached_repo_branch || 'yes'}
                      </span>
                    ) : (
                      <span className="text-xs text-slate-400">-</span>
                    )}
                  </td>
                  <td className="py-2.5 px-2 text-right">
                    <button
                      onClick={async () => { setDeleting(p.name); await onDelete(p.name); setDeleting(null) }}
                      disabled={deleting === p.name}
                      className="text-xs px-2 py-1 rounded text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors disabled:opacity-50"
                    >
                      {deleting === p.name ? '...' : <Trash2 className="w-3.5 h-3.5" />}
                    </button>
                  </td>
                </tr>
              ))}
              {projectList.length === 0 && (
                <tr>
                  <td colSpan={5} className="text-center py-10 text-slate-400">
                    {t('dashboard.noData')}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  )
}

// ─── Template Management Modal ─────────────────────────────────────────

function TemplateManagementModal({ onClose, t }: {
  onClose: () => void
  t: (key: string, opts?: any) => string
}) {
  const [templateList, setTemplateList] = useState<Record<string, FullProjectConfig>>({})
  const [loading, setLoading] = useState(true)
  const [deleting, setDeleting] = useState<string | null>(null)

  const loadTemplates = useCallback(async () => {
    setLoading(true)
    try {
      const list = await fetchTemplates()
      setTemplateList(list)
    } catch { /* ignore */ }
    setLoading(false)
  }, [])

  useEffect(() => { loadTemplates() }, [loadTemplates])

  const handleDelete = async (name: string) => {
    if (!confirm(t('templates.deleteConfirm', { name }))) return
    setDeleting(name)
    try {
      await deleteTemplate(name)
      setTemplateList(prev => {
        const next = { ...prev }
        delete next[name]
        return next
      })
    } catch (err) {
      console.error('Failed to delete template:', err)
    } finally {
      setDeleting(null)
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm" onClick={onClose}>
      <div className="bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-600 rounded-xl shadow-2xl max-w-2xl w-full mx-4 p-5 space-y-4 max-h-[80vh] flex flex-col" onClick={e => e.stopPropagation()}>
        <div className="flex items-center justify-between">
          <h3 className="text-base font-semibold text-slate-900 dark:text-slate-100">
            {t('templates.title') || 'Templates'}
          </h3>
          <button onClick={onClose} className="text-slate-400 hover:text-slate-600 dark:hover:text-slate-300">
            ✕
          </button>
        </div>

        <div className="flex-1 overflow-y-auto">
          {loading ? (
            <div className="flex items-center justify-center py-20">
              <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-sky-500"></div>
            </div>
          ) : Object.keys(templateList).length === 0 ? (
            <div className="text-center py-20 text-slate-400">
              <p>{t('templates.noTemplates')}</p>
            </div>
          ) : (
            <div className="space-y-3">
              {Object.entries(templateList).map(([name, cfg]) => (
                <Card key={name}>
                  <CardHeader className="flex flex-row items-center justify-between pb-2">
                    <CardTitle className="text-base">{name}</CardTitle>
                    <button
                      onClick={() => handleDelete(name)}
                      disabled={deleting === name}
                      className="text-xs px-2 py-1 rounded text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors disabled:opacity-50"
                    >
                      {t('config.techStack.delete')}
                    </button>
                  </CardHeader>
                  <CardContent>
                    <p className="text-xs text-slate-500">
                      {cfg.tech_stacks.length} {t('templates.techStacks')} · {Object.keys(cfg.custom_regex_analyzers).length} {t('templates.regexAnalyzers')} · {Object.keys(cfg.aggregation_views).length} {t('templates.views')}
                    </p>
                    {cfg.tech_stacks.length > 0 && (
                      <div className="mt-2 flex flex-wrap gap-1">
                        {cfg.tech_stacks.map(ts => (
                          <span key={ts.name} className="px-2 py-0.5 text-xs rounded bg-sky-100 dark:bg-sky-900/40 text-sky-700 dark:text-sky-300">
                            {ts.name} ({ts.extensions.join(', ')})
                          </span>
                        ))}
                      </div>
                    )}
                  </CardContent>
                </Card>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
