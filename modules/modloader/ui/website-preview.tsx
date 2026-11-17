import React, { useEffect, useMemo, useState } from 'react'
import { createRoot } from 'react-dom/client'
import { Card } from '../../ui-resources/show-card'
import { Segmented } from '../../ui-resources/choose-segment'
import { HeaderLink } from '../../ui-resources/show-header-link'
import { Icon } from '../../ui-resources/show-icon'
import { PageHeader } from '../../ui-resources/show-page-header'
import { SettingSwitch } from '../../ui-resources/edit-setting'
import { I18nProvider, useI18n } from '../../ui-resources/i18n'
import { ModSetting, settingIsEnabled } from './edit-mod-setting'
import { ModIcon } from './show-mod-icon'
import { DependencyList } from './show-mod-dependencies'
import { applySettingChange } from './apply-setting-presets'
import { linkDefinitions } from './show-mod'
import linkOrder from '../mod_link_order.json'
import type { LinkDefinition } from './ui-snapshot'
import type {
  ModInfo,
  SettingDefinition,
  SettingGroup,
} from './mod-types'
import './styles.css'
import './website-preview.css'

type JsonObject = Record<string, unknown>
type PreviewPayload = {
  source: 'shroudforge-mod-settings-tool'
  type: 'render'
  version: string
  manifest: JsonObject
  extended: JsonObject
}

function isObject(value: unknown): value is JsonObject {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

function optionValue(value: string, type: SettingDefinition['type']) {
  if (type === 'boolean') return value === 'true'
  if (type === 'integer') {
    const number = Number(value)
    return Number.isInteger(number) ? number : value
  }
  if (type === 'number') {
    const number = Number(value)
    return Number.isFinite(number) ? number : value
  }
  return value
}

function parseSettings(extended: JsonObject): SettingDefinition[] {
  const rawSettings = isObject(extended.settings) ? extended.settings : {}
  return Object.entries(rawSettings).map(([key, raw]) => {
    const metadata = isObject(raw) ? raw : { value: raw }
    const value = metadata.value
    const type: SettingDefinition['type'] = Array.isArray(value)
      ? 'array'
      : typeof value === 'boolean'
        ? 'boolean'
        : typeof value === 'number'
          ? Number.isInteger(value)
            ? 'integer'
            : 'number'
          : typeof value === 'string'
            ? 'string'
            : 'string'
    const optionEntries = isObject(metadata.options)
      ? Object.entries(metadata.options)
      : []
    const options = optionEntries.map(([rawValue, rawLabel]) => {
      const optionLabel = isObject(rawLabel) ? rawLabel.label : rawLabel
      const group = isObject(rawLabel) ? rawLabel.group : undefined
      return {
        value: optionValue(rawValue, type),
        label: typeof optionLabel === 'string' ? optionLabel : rawValue,
        ...(typeof group === 'string' ? { group } : {}),
      }
    })
    const inferredControl = optionEntries.length
      ? type === 'array'
        ? 'multiselect'
        : 'select'
      : type === 'boolean'
        ? 'toggle'
        : type === 'integer' || type === 'number'
          ? 'number'
          : 'text'
    const displayLabel = key.replace(/[A-Z]/g, letter => ` ${letter}`).trim()
    return {
      key,
      label: typeof metadata.label === 'string' ? metadata.label : displayLabel,
      description:
        typeof metadata.description === 'string' ? metadata.description : undefined,
      type,
      control:
        typeof metadata.control === 'string'
          ? (metadata.control as SettingDefinition['control'])
          : inferredControl,
      default: value,
      minimum: typeof metadata.min === 'number' ? metadata.min : undefined,
      maximum: typeof metadata.max === 'number' ? metadata.max : undefined,
      step: typeof metadata.step === 'number' ? metadata.step : undefined,
      minimumLength:
        typeof metadata.minLength === 'number' ? metadata.minLength : undefined,
      maximumLength:
        typeof metadata.maxLength === 'number' ? metadata.maxLength : undefined,
      options,
      enabledWhen: isObject(metadata.enabledWhen)
        ? (metadata.enabledWhen as SettingDefinition['enabledWhen'])
        : undefined,
      reserve: isObject(metadata.reserve)
        ? (metadata.reserve as SettingDefinition['reserve'])
        : undefined,
      wheel: isObject(metadata.wheel)
        ? (metadata.wheel as SettingDefinition['wheel'])
        : undefined,
    }
  })
}

function makeModInfo(manifest: JsonObject, extended: JsonObject): ModInfo {
  const settings = parseSettings(extended)
  const rawGroups = Array.isArray(extended.groups) ? extended.groups : []
  const groups: SettingGroup[] = rawGroups
    .filter(isObject)
    .map(group => ({
      label: typeof group.label === 'string' ? group.label : 'Settings',
      description:
        typeof group.description === 'string' ? group.description : undefined,
      settings: Array.isArray(group.settings)
        ? group.settings.filter((key): key is string => typeof key === 'string')
        : [],
      actions: Array.isArray(group.actions)
        ? group.actions.filter(isObject).map(action => ({
            id: typeof action.id === 'string' ? action.id : 'action',
            label: typeof action.label === 'string' ? action.label : 'Action',
            style:
              typeof action.style === 'string'
                ? (action.style as 'primary' | 'secondary' | 'danger')
                : undefined,
            confirm:
              typeof action.confirm === 'string' ? action.confirm : undefined,
          }))
        : [],
      tab: group.tab === true,
    }))
  const groupedKeys = new Set(groups.flatMap(group => group.settings))
  const ungrouped = settings.filter(setting => !groupedKeys.has(setting.key))
  if (ungrouped.length > 0 || groups.length === 0) {
    groups.push({
      label: 'Settings',
      settings: ungrouped.map(setting => setting.key),
      actions: [],
    })
  }
  const capabilities = Array.isArray(manifest.capabilities)
    ? manifest.capabilities.filter((item): item is string => typeof item === 'string')
    : []
  const dependencies = Array.isArray(manifest.dependencies)
    ? manifest.dependencies.filter(isObject).map(item => ({
        id: typeof item.id === 'string' ? item.id : 'dependency',
        version: typeof item.version === 'string' ? item.version : '*',
        optional: item.optional === true,
        status: item.optional === true ? 'optional-missing' : 'missing',
      }))
    : []
  const icon = typeof manifest.icon === 'string' ? manifest.icon : undefined
  const settingValues = Object.fromEntries(
    settings.map(setting => [setting.key, setting.default]),
  )
  return {
    revision: 'website-preview',
    id: typeof manifest.id === 'string' ? manifest.id : 'example.mod',
    name: typeof manifest.name === 'string' ? manifest.name : 'Example Mod',
    version: typeof manifest.version === 'string' ? manifest.version : '1.0.0',
    dependencies,
    ecosystem: extended.launcher === 'EML' ? 'EML' : 'SF',
    target: Array.isArray(extended.targets)
      ? extended.targets.filter(item => typeof item === 'string').join(', ') || 'client'
      : 'client',
    asset: capabilities.includes('patch') || capabilities.includes('export'),
    runtime: capabilities.includes('runtime'),
    description:
      typeof manifest.description === 'string' ? manifest.description : undefined,
    authors: Array.isArray(manifest.authors)
      ? manifest.authors.filter((author): author is string => typeof author === 'string')
      : [],
    license: typeof manifest.license === 'string' ? manifest.license : undefined,
    icon,
    links: isObject(extended.links)
      ? (extended.links as Record<string, string | undefined>)
      : {},
    source: 'website-preview',
    enabled: extended.enabled !== false,
    settings,
    groups,
    presets: isObject(extended.presets)
      ? (extended.presets as ModInfo['presets'])
      : undefined,
    changelog: Array.isArray(extended.changelog)
      ? extended.changelog.filter((item): item is string => typeof item === 'string')
      : [],
    assets: icon ? { [icon]: `./${icon}` } : {},
    settingValues,
    inputContexts: {},
  }
}

function PreviewActionButton({ action }: { action: ModInfo['groups'][number]['actions'][number] }) {
  return (
    <button
      type="button"
      className={`button ${action.style === 'danger' ? 'danger' : action.style === 'secondary' ? 'ghost' : ''}`}
      disabled
      title="Actions are available in the installed Modloader."
    >
      {action.label || action.id}
    </button>
  )
}

function ModSettingsPreview({
  manifest,
  extended,
}: {
  manifest: JsonObject
  extended: JsonObject
}) {
  const { t } = useI18n()
  const mod = useMemo(() => makeModInfo(manifest, extended), [manifest, extended])
  const [pageTab, setPageTab] = useState('settings')
  const [enabled, setEnabled] = useState(mod.enabled)
  const [values, setValues] = useState<Record<string, unknown>>(mod.settingValues)
  const tabGroups = mod.groups.filter(group => group.tab)
  const settingsGroups = mod.groups.filter(group => !group.tab)
  const hasSettingsTab = settingsGroups.length > 0 || tabGroups.length === 0
  const defaultTab = hasSettingsTab ? 'settings' : 'group-0'
  const validTabs = new Set([
    'information',
    'dependencies',
    ...(hasSettingsTab ? ['settings'] : []),
    ...tabGroups.map((_, index) => `group-${index}`),
  ])
  const activeTab = validTabs.has(pageTab) ? pageTab : defaultTab
  const activeGroup = activeTab.startsWith('group-')
    ? tabGroups[Number(activeTab.slice('group-'.length))]
    : undefined
  const shownGroups =
    activeTab === 'settings'
      ? settingsGroups
      : activeGroup
        ? [activeGroup]
        : []
  const target =
    mod.target === 'client'
      ? 'Client'
      : mod.target === 'server'
        ? 'Server'
        : 'Client, Server'
  const visibleLinks = linkOrder.flatMap(key => {
    const definition = linkDefinitions[key] as LinkDefinition | undefined
    const url = mod.links?.[key]
    if (!url || !definition) return []
    const appearance = definition.appearance || ''
    return [
      {
        key,
        url,
        label: t(definition.labelKey as Parameters<typeof t>[0]),
        description: t(definition.descriptionKey as Parameters<typeof t>[0]),
        icon: definition.icon,
        tone: appearance.startsWith('support-') ? `support ${appearance}` : appearance,
      },
    ]
  })
  const authorByline = mod.authors.length > 0 ? (
    <span className="mod-author-byline">
      <em>by</em>
      <span>{mod.authors.length < 2 ? mod.authors[0] : `${mod.authors.slice(0, -1).join(', ')} & ${mod.authors.at(-1)}`}</span>
    </span>
  ) : null
  const title = (
    <span className="mod-title-lockup">
      <ModIcon mod={mod} variant="page" />
      <span className="mod-title-copy">
        <span>{mod.name}</span>
        {authorByline}
      </span>
    </span>
  )
  const sendChange = (type: string, detail: Record<string, unknown>) => {
    window.parent.postMessage(
      { source: 'shroudforge-mod-settings-preview', type, ...detail },
      window.location.origin,
    )
  }
  const setEnabledLocally = (next: boolean) => {
    setEnabled(next)
    sendChange('enabled-change', { enabled: next })
  }
  const changeSetting = (key: string, value: unknown) => {
    const next = applySettingChange(values, mod.presets, key, value)
    setValues(next)
    sendChange('settings-change', { values: next })
  }
  useEffect(() => {
    setPageTab(defaultTab)
  }, [mod.id, defaultTab])
  useEffect(() => {
    setEnabled(mod.enabled)
    setValues(mod.settingValues)
  }, [mod.id, mod.revision, mod.enabled, mod.settingValues])

  return (
    <div className="website-preview-shell">
      <header className="website-preview-topbar">
        <span className="preview-brand-mark">SF</span>
        <strong>SHROUDFORGE MODLOADER</strong>
      </header>
      <main className="content website-preview-content">
        <PageHeader
          eyebrow="MOD"
          title={title}
          subtitle={mod.description || mod.id}
          actions={
            <div className="website-preview-actions">
              <button type="button" className="button ghost" disabled>Refresh</button>
              <button type="button" className="button" disabled>Save</button>
            </div>
          }
        />
        <section className="mod-status-row">
          <strong className={`mod-state-chip ${enabled ? 'ready' : 'disabled'}`}>
            {enabled ? t('mod.mode.ready') : t('mod.mode.disabled')}
          </strong>
          <div className="mod-enable-group">
            <span>{t('common.active')}</span>
            <SettingSwitch
              label={enabled ? t('mod.disable') : t('mod.enable')}
              checked={enabled}
              onChange={setEnabledLocally}
            />
          </div>
        </section>
        <Segmented
          value={activeTab}
          onChange={setPageTab}
          items={[
            ['information', t('mod.details')],
            ['dependencies', t('mod.tab.dependencies')],
            ...(hasSettingsTab
              ? [[ 'settings', t('mod.tab.settings') ] as [string, React.ReactNode]]
              : []),
            ...tabGroups.map(
              (group, index) => [`group-${index}`, group.label] as [string, React.ReactNode],
            ),
          ]}
        />
        {activeTab === 'information' && (
          <Card title={t('mod.details')}>
            <div className="mod-meta">
              <span className={`mod-info-badge ecosystem ${mod.ecosystem.toLowerCase()}`}>
                {t(mod.ecosystem === 'SF' ? 'mod.ecosystem.sf' : 'mod.ecosystem.eml')}
              </span>
              <span className="mod-info-badge version"><Icon name="version" />{mod.version}</span>
              <span className="mod-info-badge target"><Icon name={mod.target === 'server' ? 'server' : 'client'} />{target}</span>
              {mod.asset && <span className="mod-info-badge asset"><Icon name="blueprint" />{t('mod.type.asset')}</span>}
              {mod.runtime && <span className="mod-info-badge runtime"><Icon name="bolt" />{t('mod.type.runtime')}</span>}
              {mod.license && <span className="mod-info-badge license">{mod.license}</span>}
            </div>
            {visibleLinks.length > 0 && (
              <nav className="mod-links" aria-label={t('mod.links')}>
                {visibleLinks.map(link => (
                  <HeaderLink
                    key={link.key}
                    label={link.label}
                    icon={link.icon}
                    tone={link.tone}
                    title={link.description}
                    onClick={() => {
                      try {
                        const url = new URL(link.url)
                        if (url.protocol === 'https:') window.open(url.href, '_blank', 'noopener,noreferrer')
                      } catch { /* Invalid preview URLs stay inert. */ }
                    }}
                  />
                ))}
              </nav>
            )}
            {mod.changelog.length > 0 && (
              <div className="mod-changelog">
                <h4>{t('mod.versions')}</h4>
                <ul>{mod.changelog.map((item, index) => <li key={index}>{item}</li>)}</ul>
              </div>
            )}
          </Card>
        )}
        {activeTab === 'dependencies' && (
          <Card title={t('mod.dependencies.title')}>
            <DependencyList dependencies={mod.dependencies} dependencyStates={{}} />
          </Card>
        )}
        {(activeTab === 'settings' || activeGroup !== undefined) && (
          <Card title={t('mod.settings')}>
            <div className="settings-module-list">
              {shownGroups.map((group, index) => (
                <section className="settings-module mod-settings-section" key={`${group.label}-${index}`}>
                  <header><div><h3>{group.label}</h3>{group.description && <p className="hint">{group.description}</p>}</div></header>
                  <div className="settings-options">
                    {group.settings
                      .map(key => mod.settings.find(setting => setting.key === key))
                      .filter((definition): definition is SettingDefinition => Boolean(definition))
                      .map(definition => (
                        <ModSetting
                          key={definition.key}
                          definition={definition}
                          value={values[definition.key]}
                          assessmentTarget={{
                            scope: 'mod',
                            modId: mod.id,
                            key: definition.key,
                            revision: mod.revision,
                            values,
                          }}
                          disabled={!settingIsEnabled(definition, mod.settings, values)}
                          onChange={value => changeSetting(definition.key, value)}
                          contexts={mod.inputContexts}
                          onReserve={() => undefined}
                        />
                      ))}
                  </div>
                  {group.actions.length > 0 && (
                    <div className="mod-actions">
                      {group.actions.map((action, index) => (
                        <PreviewActionButton key={`${action.id}-${index}`} action={action} />
                      ))}
                    </div>
                  )}
                </section>
              ))}
              {shownGroups.length === 0 && <p className="hint">{t('mod.settings.empty')}</p>}
            </div>
          </Card>
        )}
      </main>
    </div>
  )
}

function WebsitePreview() {
  const [payload, setPayload] = useState<PreviewPayload | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    const receive = (event: MessageEvent) => {
      if (event.source !== window.parent || event.origin !== window.location.origin) return
      const data = event.data
      if (!isObject(data) || data.source !== 'shroudforge-mod-settings-tool' || data.type !== 'render') return
      if (!isObject(data.manifest) || !isObject(data.extended) || typeof data.version !== 'string') {
        setError('The example data is not valid JSON objects.')
        return
      }
      try {
        makeModInfo(data.manifest, data.extended)
        setError(null)
        setPayload(data as unknown as PreviewPayload)
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : 'Could not render this mod preview.')
      }
    }
    window.addEventListener('message', receive)
    window.parent.postMessage({ source: 'shroudforge-mod-settings-preview', type: 'ready' }, window.location.origin)
    return () => window.removeEventListener('message', receive)
  }, [])

  useEffect(() => {
    if (!payload) return
    const shell = document.querySelector<HTMLElement>('.website-preview-shell')
    if (!shell) return
    let frame = 0
    const sendHeight = () => {
      window.cancelAnimationFrame(frame)
      frame = window.requestAnimationFrame(() => {
        window.parent.postMessage(
          {
            source: 'shroudforge-mod-settings-preview',
            type: 'resize',
            height: Math.ceil(shell.getBoundingClientRect().height),
          },
          window.location.origin,
        )
      })
    }
    const observer = new ResizeObserver(sendHeight)
    observer.observe(shell)
    window.addEventListener('resize', sendHeight)
    sendHeight()
    return () => {
      window.cancelAnimationFrame(frame)
      observer.disconnect()
      window.removeEventListener('resize', sendHeight)
    }
  }, [payload])

  return (
    <I18nProvider>
      {error ? (
        <div className="website-preview-error" role="alert">{error}</div>
      ) : payload ? (
        <ModSettingsPreview
          manifest={payload.manifest}
          extended={payload.extended}
        />
      ) : (
        <div className="website-preview-loading">Loading the current Modloader UI…</div>
      )}
    </I18nProvider>
  )
}

const previewRoot = document.getElementById('root')
if (previewRoot) createRoot(previewRoot).render(<WebsitePreview />)
