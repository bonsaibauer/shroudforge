import { postRequest } from '../../transport/ui/send-host-command'
import { logUiEvent } from '../../logging/ui/log-ui-event'
import { TranslationKey, useI18n } from '../../ui-resources/i18n'
import type { DllCandidate, DllLoadMode, DllManagerProfile, Snapshot } from './ui-snapshot'
import type { NewsItem } from '../../notifications/ui/notification-types'
import { EcosystemBadge } from '../../ui-resources/show-ecosystem-badge'
import { Icon } from '../../ui-resources/show-icon'
import { PageHeader } from '../../ui-resources/show-page-header'
import { DependencyList } from './show-mod-dependencies'
import { useEffect, useRef, useState } from 'react'
import { Segmented } from '../../ui-resources/choose-segment'
import { Card } from '../../ui-resources/show-card'
import { WikiHelp } from '../../wiki/ui/show-wiki-help'
import type { ModDependencyInfo, ModInfo } from './mod-types'
import { Hotkeys, hasHotkeyConflicts } from '../../hotkeys/ui/show-hotkeys'
import {
  ModuleEnabledRow,
  SettingsRow,
  SettingSwitch,
} from '../../ui-resources/edit-setting'

export type CompatibilityStatus = 'ok' | 'warning' | 'error' | 'neutral'

function combinedStatus(statuses: CompatibilityStatus[]): CompatibilityStatus {
  return statuses.includes('error') ? 'error'
    : statuses.includes('warning') ? 'warning'
      : statuses.includes('neutral') ? 'neutral' : 'ok'
}

function assetCompatibilityStatus(state: string): CompatibilityStatus {
  return state === 'failed' ? 'error'
    : ['applied', 'not-required'].includes(state) ? 'ok'
      : ['prepare-required', 'partial'].includes(state) ? 'warning' : 'neutral'
}

function modDetectionStatus(data: Snapshot): CompatibilityStatus {
  if (data.settings.modulePreferences?.modDetection?.enabled === false) return 'neutral'
  if (data.dllCandidates.some((candidate) => candidate.manifestState === 'invalid')) return 'error'
  return data.configuration ? 'ok' : 'neutral'
}

function dependencyHasProblem(dependency: ModDependencyInfo, data: Snapshot): boolean {
  if (['optional-missing', 'optional-disabled'].includes(dependency.status)) return false
  if (dependency.optional && ['missing', 'disabled'].includes(dependency.status)) return false
  return ['missing', 'disabled', 'version-mismatch', 'invalid-version', 'blocked'].includes(dependency.status) ||
    ['blocked', 'failed'].includes(data.configuration?.modStates?.[dependency.id]?.state || '')
}

function dependencyPlanIssue(data: Snapshot, modId: string) {
  const state = data.configuration?.modStates?.[modId]
  return state?.state === 'blocked' && [
    'missing-dependency', 'dependency-disabled', 'dependency-version',
    'dependency-cycle', 'conflict-or-plan-error',
  ].includes(state.code || '') ? state : undefined
}

function dependencyModIsActive(mod: ModInfo, data: Snapshot): boolean {
  return mod.enabled && mod.target !== (data.mode === 'SERVER' ? 'client' : 'server')
}

function dependencyStatus(data: Snapshot): CompatibilityStatus {
  if (data.mods.some((mod) => dependencyModIsActive(mod, data) && (
    mod.dependencies.some((dependency) => dependencyHasProblem(dependency, data)) ||
    dependencyPlanIssue(data, mod.id)
  ))) return 'warning'
  return data.configuration ? 'ok' : 'neutral'
}

export function compatibilityNavigationStatus(
  data: Snapshot,
): CompatibilityStatus {
  const configuration = data.configuration
  if (!configuration) return 'neutral'
  const engineStatuses: CompatibilityStatus[] = configuration.checks
    .filter((check) => ['game', 'runtime', 'api', 'parser'].includes(check.id))
    .map((check) => check.state)
  if (configuration.errors.length) engineStatuses.push('error')
  if (configuration.assets) engineStatuses.push(assetCompatibilityStatus(configuration.assets.state))
  return combinedStatus([
    modDetectionStatus(data), dllManagerStatus(data), dependencyStatus(data),
    hasHotkeyConflicts(data) ? 'warning' : 'ok',
    ...engineStatuses,
  ])
}

export type CompatibilityRowData = {
  id: string
  title: string
  detail: string
  status: CompatibilityStatus
  technicalDetail?: string
  modId?: string
  ecosystem?: 'EML' | 'SF'
  action?: { label: string; run(): void }
}

export function Compatibility({
  data,
  news,
  openNews,
  selectMod,
  openLog,
}: {
  data: Snapshot
  news: NewsItem[]
  openNews(): void
  selectMod(id: string): void
  openLog(): void
}) {
  const { t } = useI18n()
  const [tab, setTab] = useState('mod-detection')
  const [showAllDependencies, setShowAllDependencies] = useState(false)
  const loggedTab = useRef<string | null>(null)
  useEffect(() => {
    if (loggedTab.current === tab) return
    loggedTab.current = tab
    logUiEvent({
      level: 'DEBUG', source: 'compatibility', operation: 'open-section', subject: tab,
      message: `Opened compatibility section: ${tab}.`,
    })
  }, [tab])
  const inventoryModCount = data.mods.length
  const inventoryNativeCount = data.dllCandidates.length
  const inventoryIssueCount = data.dllCandidates.reduce((count, item) => count + item.issues.length, 0)
  const loggedInventory = useRef('')
  useEffect(() => {
    const signature = `${inventoryModCount}:${inventoryNativeCount}:${inventoryIssueCount}`
    if (loggedInventory.current === signature) return
    loggedInventory.current = signature
    logUiEvent({
      level: 'TRACE', source: 'compatibility', operation: 'inventory-summary', subject: 'mod-detection',
      message: `Inventory: ${inventoryModCount} mods, ${inventoryNativeCount} native candidates, ${inventoryIssueCount} native inventory issues.`,
    })
  }, [inventoryModCount, inventoryNativeCount, inventoryIssueCount])
  const modulePreferences = data.settings.modulePreferences || {}
  const detection = modulePreferences.modDetection || {}
  const [localDetection, setLocalDetection] = useState(detection)
  useEffect(() => setLocalDetection(detection), [
    detection.enabled,
    detection.tolerateUnknownManifestFields,
  ])
  const detectionEnabled = localDetection.enabled !== false
  const tolerateUnknownFields =
    localDetection.tolerateUnknownManifestFields !== false
  const dllStatus = dllManagerStatus(data)
  const changeDetection = (key: string, value: boolean) => {
    logUiEvent({
      level: 'DEBUG', source: 'compatibility', operation: 'save-setting-requested', subject: 'mod-detection',
      message: `Requested Mod Detection setting change: ${key}=${value}.`,
    })
    setLocalDetection({ ...localDetection, [key]: value })
    postRequest('save-setting', {
      scope: 'module',
      module: 'modDetection',
      key,
      value,
    })
  }
  const checks = data.configuration?.checks || []
  const check = (id: string) => checks.find((item) => item.id === id)
  const statusOf = (value?: { state: string }): CompatibilityStatus =>
    value?.state === 'error'
      ? 'error'
      : value?.state === 'ok'
        ? 'ok'
        : value?.state === 'warning'
          ? 'warning'
          : 'neutral'
  const gameCheck = check('game'),
    runtimeCheck = check('runtime'),
    apiCheck = check('api'),
    parserCheck = check('parser')
  const gameRows: CompatibilityRowData[] = [
    {
      id: 'game',
      title: t('compatibility.row.game'),
      status: statusOf(gameCheck),
      detail: t(
        gameCheck?.state === 'ok'
          ? 'compatibility.detail.game.ok'
          : gameCheck?.state === 'warning'
            ? 'compatibility.detail.game.warning'
            : 'compatibility.detail.game.pending',
        { version: data.gameVersion },
      ),
      technicalDetail: gameCheck?.detail,
    },
    {
      id: 'runtime',
      title: t('compatibility.row.runtime'),
      status: statusOf(runtimeCheck),
      detail: t(
        runtimeCheck?.state === 'ok' && runtimeCheck.phase === 'desktop'
          ? 'compatibility.detail.runtime.desktop'
          : runtimeCheck?.state === 'ok' && runtimeCheck.phase === 'main-menu'
            ? 'compatibility.detail.runtime.mainMenu'
            : runtimeCheck?.phase === 'waiting-world'
              ? 'compatibility.detail.runtime.waitingWorld'
              : runtimeCheck?.state === 'ok'
                ? 'compatibility.detail.runtime.ok'
                : runtimeCheck?.state === 'warning'
                  ? 'compatibility.detail.runtime.warning'
                  : data.mode === 'DESKTOP'
                    ? 'compatibility.detail.runtime.desktop'
                    : 'compatibility.detail.runtime.pending',
      ),
      technicalDetail: runtimeCheck?.detail,
      action:
        runtimeCheck?.state === 'warning'
          ? { label: t('compatibility.action.openLog'), run: openLog }
          : undefined,
    },
  ]
  const assets = data.configuration?.assets
  if (assets) {
    gameRows.push({
      id: 'assets',
      title: t('compatibility.row.assets'),
      status: assetCompatibilityStatus(assets.state),
      detail: t(
        assets.state === 'applied'
          ? 'compatibility.detail.assets.applied'
          : assets.state === 'partial'
            ? 'compatibility.detail.assets.partial'
            : assets.state === 'not-required'
              ? 'compatibility.detail.assets.notRequired'
              : assets.state === 'prepare-required'
                ? 'compatibility.detail.assets.required'
                : assets.state === 'failed'
                  ? 'compatibility.detail.assets.failed'
                  : 'compatibility.detail.assets.unknown',
      ),
      technicalDetail: assets.detail,
    })
  }
  for (const [index, error] of (data.configuration?.errors || []).entries())
    gameRows.push({
      id: `config-error-${index}`,
      title: t('compatibility.row.configuration'),
      status: 'error',
      detail: t('compatibility.detail.configuration.error'),
      technicalDetail: error,
      action: { label: t('compatibility.action.openLog'), run: openLog },
    })
  const apiRows: CompatibilityRowData[] = [
    {
      id: 'api',
      title: t('compatibility.row.api'),
      status: statusOf(apiCheck),
      detail: t(
        apiCheck?.state === 'ok'
          ? 'compatibility.detail.api.ok'
          : apiCheck?.state === 'warning'
            ? 'compatibility.detail.api.warning'
            : 'compatibility.detail.api.pending',
      ),
      technicalDetail: apiCheck?.detail,
      action:
        apiCheck?.state === 'warning'
          ? { label: t('compatibility.action.openLog'), run: openLog }
          : undefined,
    },
  ]
  const parserRows: CompatibilityRowData[] = [
    {
      id: 'parser',
      title: t('compatibility.row.parser'),
      status: statusOf(parserCheck),
      detail: t(
        parserCheck?.state === 'ok'
          ? 'compatibility.detail.parser.ok'
          : parserCheck?.state === 'warning'
            ? 'compatibility.detail.parser.warning'
            : 'compatibility.detail.parser.pending',
      ),
      technicalDetail: parserCheck?.detail,
      action:
        parserCheck?.state === 'warning'
          ? { label: t('common.refresh'), run: () => postRequest('refresh') }
          : undefined,
    },
  ]
  const knownStates = data.configuration?.modStates || {}
  const sfMods = data.mods.filter((mod) => mod.ecosystem === 'SF')
  const modApiRows = [...apiRows]
  if (sfMods.length) {
    modApiRows.push({
      id: 'sf-api-check',
      title: t('compatibility.row.sfApi'),
      status: statusOf(apiCheck),
      detail: t(
        apiCheck?.state === 'ok'
          ? 'compatibility.detail.sfApi.ok'
          : apiCheck?.state === 'warning'
            ? 'compatibility.detail.sfApi.warning'
            : 'compatibility.detail.sfApi.pending',
      ),
      technicalDetail: apiCheck?.detail,
      action:
        apiCheck?.state === 'warning'
          ? { label: t('compatibility.action.openLog'), run: openLog }
          : undefined,
    })
  }
  const gameOnlyRows = gameRows.filter((row) => row.id !== 'runtime')
  const rowsStatus = (rows: CompatibilityRowData[]): CompatibilityStatus =>
    combinedStatus(rows.map((row) => row.status))
  const engineRows = [...gameRows, ...modApiRows, ...parserRows]
  const engineStatus = rowsStatus(engineRows)
  const dependencyMods = data.mods.map((mod) => ({
    mod,
    problems: dependencyModIsActive(mod, data)
      ? mod.dependencies.filter((dependency) => dependencyHasProblem(dependency, data)) : [],
    issue: dependencyModIsActive(mod, data) ? dependencyPlanIssue(data, mod.id) : undefined,
  })).filter(({ mod, issue }) => mod.dependencies.length || issue)
  const dependencyProblemCount = dependencyMods.reduce((count, { problems, issue }) =>
    count + problems.length + (issue && !problems.length ? 1 : 0), 0)
  const visibleDependencyMods = showAllDependencies ? dependencyMods
    : dependencyMods.filter(({ problems, issue }) => problems.length || issue)
  const errorCount = engineRows.filter((row) => row.status === 'error').length
  const warningCount = engineRows.filter((row) => row.status === 'warning').length
  const pendingCount = engineRows.filter((row) => row.status === 'neutral').length
  const overall = engineStatus
  const headlineKey =
    overall === 'error'
      ? 'compatibility.summary.error'
      : overall === 'warning'
        ? 'compatibility.summary.warning'
        : overall === 'neutral'
          ? 'compatibility.summary.pending'
          : 'compatibility.summary.ok'
  const unreadAlerts = news.filter(
    (item) =>
      (item.level === 'warn' || item.level === 'error') &&
      !data.readNoticeIds.includes(item.id),
  ).length
  const renderSection = (
    id: string,
    title: string,
    description: string,
    rows: CompatibilityRowData[],
  ) => {
    const state = rows.some((row) => row.status === 'error')
      ? 'error'
      : rows.some((row) => row.status === 'warning')
        ? 'warning'
        : rows.some((row) => row.status === 'neutral')
          ? 'neutral'
          : 'ok'
    return (
      <section className="settings-module compatibility-section" key={id}>
        <header>
          <div>
            <h3>{title}</h3>
            <p className="hint">{description}</p>
          </div>
          <CompatibilityStatusBadge status={state} />
        </header>
        <div className="settings-options">
          {rows.map((row) => <CompatibilityRow key={row.id} row={row} />)}
        </div>
      </section>
    )
  }
  return (
    <>
      <PageHeader
        eyebrow={t('nav.modloader')}
        title={t('compatibility.title')}
        subtitle={t('compatibility.subtitle')}
        actions={
          <>
            {unreadAlerts > 0 && (
              <button className="button secondary" onClick={openNews}>
                {t('compatibility.action.openNews', { count: unreadAlerts })}
              </button>
            )}
            <button className="button" onClick={() => postRequest('refresh')}>
              <Icon name="refresh" />
              {t('compatibility.action.refresh')}
            </button>
          </>
        }
      />
      <Segmented
        value={tab}
        onChange={setTab}
        items={[
          [
            'mod-detection',
            <CompatibilityTabLabel
              label={t('compatibility.tab.modDetection')}
              status={modDetectionStatus(data)}
            />,
          ],
          [
            'dll-manager',
            <CompatibilityTabLabel
              label={t('compatibility.tab.dllManager')}
              status={dllStatus}
            />,
          ],
          [
            'hotkeys',
            <CompatibilityTabLabel
              label={t('compatibility.tab.hotkeys')}
              status={hasHotkeyConflicts(data) ? 'warning' : 'ok'}
            />,
          ],
          [
            'dependencies',
            <CompatibilityTabLabel
              label={t('compatibility.tab.dependencies')}
              status={dependencyStatus(data)}
            />,
          ],
          [
            'engine',
            <CompatibilityTabLabel
              label={t('compatibility.tab.engine')}
              status={engineStatus}
            />,
          ],
        ]}
      />
      {tab === 'mod-detection' && (
        <div className="settings-module-list compatibility-sections">
          <WikiHelp articleId="mod-detection" />
          <Card title={t('compatibility.modDetection.title')}>
            <p className="hint settings-module-description">
              {t('compatibility.modDetection.description')}
            </p>
            <div className="settings-options">
              <ModuleEnabledRow
                label={t('settings.modules.field.active')}
                enabled={detectionEnabled}
                onChange={(value) => changeDetection('enabled', value)}
              />
              <SettingsRow
                label={t('compatibility.modDetection.tolerateUnknown')}
                control={
                  <SettingSwitch
                    label={t('compatibility.modDetection.tolerateUnknown')}
                    checked={tolerateUnknownFields}
                    disabled={!detectionEnabled}
                    onChange={(value) =>
                      changeDetection('tolerateUnknownManifestFields', value)
                    }
                  />
                }
              />
              <p className="hint settings-module-hint">
                {t('compatibility.modDetection.tolerateUnknown.detail')}
              </p>
            </div>
          </Card>
          <Card title={t('compatibility.modDetection.found')}>
            <p className="hint settings-module-description">{t('compatibility.modDetection.inventory')}</p>
            {data.mods.map((mod) => (
              <div className="settings-option" key={mod.id}>
                <span className="compatibility-row-copy">
                  <strong>{mod.name}<EcosystemBadge ecosystem={mod.ecosystem} /></strong>
                  <small>{mod.id} · {mod.version}</small>
                </span>
                <button className="button ghost" onClick={() => selectMod(mod.id)}>{t('compatibility.action.openMod')}</button>
              </div>
            ))}
            {data.dllCandidates.filter((item) => item.manifestState !== 'valid').map((item) => (
              <div className="settings-option" key={item.packageDirectory}>
                <span className="compatibility-row-copy">
                  <strong>{item.name}</strong>
                  <small>{item.packageDirectory} · {t(`compatibility.dllManager.manifest.${item.manifestState}` as TranslationKey)}</small>
                </span>
                <button className="button ghost" onClick={() => setTab('dll-manager')}>{t('compatibility.tab.dllManager')}</button>
              </div>
            ))}
            {!data.mods.length && !data.dllCandidates.length && <p className="hint">{t('compatibility.modDetection.empty')}</p>}
          </Card>
        </div>
      )}
      {tab === 'dll-manager' && <DllManagerPanel data={data} />}
      {tab === 'hotkeys' && <Hotkeys data={data} />}
      {tab === 'dependencies' && (
        <div className="settings-module-list compatibility-sections">
          <WikiHelp articleId="dependencies" />
          <section className="settings-module compatibility-section">
            <header>
              <div>
                <h3>{t('compatibility.section.dependencies')}</h3>
                <p className="hint">
                  {t('compatibility.section.dependencies.description')}
                </p>
              </div>
              <CompatibilityStatusBadge status={dependencyStatus(data)} />
            </header>
            <div className="settings-options">
              <p className="hint settings-module-description">
                {t('compatibility.dependencies.summary', {
                  mods: dependencyMods.length,
                  problems: dependencyProblemCount,
                })}
              </p>
              <SettingsRow label={t('compatibility.dependencies.showAll')} control={
                <SettingSwitch label={t('compatibility.dependencies.showAll')}
                  checked={showAllDependencies} onChange={(value) => {
                    logUiEvent({
                      level: 'TRACE', source: 'compatibility', operation: 'show-all-dependencies', subject: 'dependencies',
                      message: value ? 'Showing all dependencies.' : 'Showing dependency problems only.',
                    })
                    setShowAllDependencies(value)
                  }} />
              } />
            </div>
            {!visibleDependencyMods.length && <p className="hint settings-module-description">
              {t(showAllDependencies ? 'compatibility.dependencies.noDependencies' : 'compatibility.dependencies.ok')}
            </p>}
            {visibleDependencyMods.map(({ mod, problems, issue }) => (
              <div className="compatibility-dependency-mod" key={mod.id}>
                <div className="settings-option">
                  <h4>{mod.name}</h4>
                  <button className="button ghost" onClick={() => selectMod(mod.id)}>
                    {t('compatibility.action.openMod')}
                  </button>
                </div>
                {issue && <p className="hint">{issue.detail}</p>}
                {(showAllDependencies ? mod.dependencies : problems).length > 0 && (
                  <DependencyList
                    dependencies={showAllDependencies ? mod.dependencies : problems}
                    dependencyStates={knownStates}
                  />
                )}
              </div>
            ))}
          </section>
        </div>
      )}
      {tab === 'engine' && (
        <>
          <WikiHelp articleId="engine" />
          <section className={`compatibility-summary ${overall}`}>
            <span className={`compatibility-summary-icon ${overall}`}>
              <Icon
                name={
                  overall === 'ok'
                    ? 'check'
                    : overall === 'neutral'
                      ? 'compatibility'
                      : 'warning'
                }
              />
            </span>
            <div className="compatibility-summary-copy">
              <strong>{t(`${headlineKey}.title` as TranslationKey)}</strong>
              <p>
                {t(`${headlineKey}.message` as TranslationKey, {
                  errors: errorCount,
                  warnings: warningCount,
                  pending: pendingCount,
                  mods: data.mods.length,
                })}
              </p>
            </div>
            <span className="compatibility-summary-count">
              {t('compatibility.summary.counts', {
                checks: engineRows.length,
                mods: data.mods.length,
                attention: errorCount + warningCount,
              })}
            </span>
          </section>
          <div className="settings-module-list compatibility-sections">
            {renderSection(
              'game',
              t('compatibility.section.game'),
              t('compatibility.section.game.description'),
              gameOnlyRows,
            )}
            {renderSection(
              'runtime',
              t('compatibility.section.runtime'),
              t('compatibility.section.runtime.description'),
              [gameRows.find((row) => row.id === 'runtime')!],
            )}
            {renderSection(
              'api',
              t('compatibility.section.api'),
              t('compatibility.section.api.description'),
              modApiRows,
            )}
            {renderSection(
              'parser',
              t('compatibility.section.parser'),
              t('compatibility.section.parser.description'),
              parserRows,
            )}
          </div>
        </>
      )}
    </>
  )
}

function dllStateIsReady(state: string | undefined): boolean {
  return state === 'dll-loaded' || state === 'ready'
}

function dllManagerStatus(data: Snapshot): CompatibilityStatus {
  const manager = data.settings.modulePreferences?.dllManager || {}
  const runtimeStates = Object.entries(data.configuration?.modStates || {})
    .filter(([id]) => id.startsWith('dll:'))
  if (manager.enabled === false) return runtimeStates.some(([, state]) => state.state === 'restart-required')
    ? 'warning' : 'neutral'
  const candidates = data.dllCandidates || []
  const profiles = (manager.profiles || []) as DllManagerProfile[]
  const states = runtimeStates.map(([, state]) => state.state)
  if (states.includes('failed') || profiles.some((selection) =>
    selection.enabled && !candidates.some((item) => item.packageDirectory === selection.packageDirectory),
  )) return 'error'
  if (candidates.some((item) => item.issues.length || item.filteredDlls.length ||
    (!item.loadEntries.length && item.dlls.length &&
      !profiles.some((selection) => selection.packageDirectory === item.packageDirectory && !selection.enabled)),
  ) || states.some((state) => ['waiting', 'unconfirmed', 'restart-required'].includes(state))) return 'warning'
  return states.length && states.every((state) => dllStateIsReady(state) || state === 'disabled')
    ? 'ok' : 'neutral'
}

function defaultDllSelection(candidate: DllCandidate) {
  if (candidate.declaredDll && candidate.dlls.some((file) => file.path === candidate.declaredDll)) {
    return [candidate.declaredDll]
  }
  return candidate.dlls.length === 1 ? [candidate.dlls[0].path] : []
}

function formatFileSize(size: number) {
  if (size < 1024) return `${size} B`
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`
  return `${(size / (1024 * 1024)).toFixed(1)} MB`
}

function normalizedDllNames(names: unknown): string[] {
  return Array.isArray(names)
    ? [...new Set(names.filter((name): name is string => typeof name === 'string')
      .map((name) => name.trim().toLowerCase()).filter(Boolean))].sort()
    : []
}

function DllManagerPanel({ data }: { data: Snapshot }) {
  const { t } = useI18n()
  const manager = data.settings.modulePreferences?.dllManager || {}
  const [enabled, setEnabled] = useState(manager.enabled !== false)
  const [profiles, setProfiles] = useState((manager.profiles || []) as DllManagerProfile[])
  const [excludedDllNames, setExcludedDllNames] = useState(normalizedDllNames(manager.excludedDllNames))
  const [newExcludedDllName, setNewExcludedDllName] = useState('')
  const [selectedFiles, setSelectedFiles] = useState<Record<string, string[]>>({})
  const [selectedProfiles, setSelectedProfiles] = useState<Record<string, string>>({})
  const [targets, setTargets] = useState<Record<string, DllManagerProfile['target']>>({})
  const [modes, setModes] = useState<Record<string, DllLoadMode>>({})
  const [approved, setApproved] = useState<Record<string, boolean>>({})
  useEffect(() => {
    setEnabled(manager.enabled !== false)
    setProfiles((manager.profiles || []) as DllManagerProfile[])
    setExcludedDllNames(normalizedDllNames(manager.excludedDllNames))
  }, [manager.enabled, manager.profiles, manager.excludedDllNames])

  const save = (key: string, value: unknown, subject = 'dll-manager', detail?: string) => {
    logUiEvent({
      level: 'DEBUG', source: 'compatibility', operation: 'save-setting-requested', subject,
      message: detail || `Requested DLL Manager setting change: ${key}=${String(value)}.`,
    })
    postRequest('save-setting', {
      scope: 'module', module: 'dllManager', key, value,
    })
  }
  const saveProfiles = (next: DllManagerProfile[], directory: string) => {
    const selection = next.find((item) => item.packageDirectory === directory)
    const detail = selection
      ? `Requested DLL selection for ${selection.name}: enabled=${selection.enabled}, approved=${selection.approved}, target=${selection.target}, profile=${selection.compatibilityProfile || 'automatic'}, ordered files=${JSON.stringify(selection.files)}.`
      : `Requested removal of DLL selection for ${directory}.`
    setProfiles(next)
    save('profiles', next, directory, detail)
  }
  const saveExcludedDllNames = (next: string[]) => {
    const names = normalizedDllNames(next)
    setExcludedDllNames(names)
    save('excludedDllNames', names, 'dll-manager', `Requested excluded DLL file names: ${names.join(', ') || '(none)'}.`)
  }
  const newName = newExcludedDllName.trim().toLowerCase()
  const canAddName = /^[^\\/:*?"<>|]+\.dll$/i.test(newName) &&
    !excludedDllNames.includes(newName) && excludedDllNames.length < 64
  const addName = () => {
    if (!canAddName) return
    saveExcludedDllNames([...excludedDllNames, newName])
    setNewExcludedDllName('')
  }
  const sourceLabel = (source: string) => t(
    `compatibility.dllManager.source.${source || 'manual'}` as TranslationKey,
  )
  const statusLabel = (state: string) => t(
    state === 'ready' ? 'compatibility.status.ok' :
      `compatibility.dllManager.state.${state === 'dll-loaded' ? 'loaded' :
        state === 'failed' ? 'failed' : state === 'waiting' ? 'loading' :
          state === 'disabled' ? 'disabled' : state === 'restart-required' ? 'restart' : 'pending'}` as TranslationKey,
  )

  return (
    <div className="settings-module-list compatibility-sections dll-manager">
      <WikiHelp articleId="dll-manager" />
      <Card title={t('compatibility.dllManager.title')}>
        <p className="hint settings-module-description">{t('compatibility.dllManager.description')}</p>
        <ModuleEnabledRow label={t('settings.modules.field.active')} enabled={enabled} onChange={(value) => {
          setEnabled(value)
          save('enabled', value)
        }} />
        <p className="hint settings-module-hint">{t('compatibility.dllManager.safety')}</p>
        {!enabled && <p className="hint">{t('compatibility.dllManager.moduleDisabled')}</p>}
      </Card>
      {data.dllChainEntries.length > 0 && <Card title="dinput8-chain.ini">
        <p className="hint settings-module-description">{t('compatibility.dllManager.chainDescription')}</p>
        <ol className="dll-manager-file-list ordered">
          {data.dllChainEntries.map((entry) => {
            const state = data.configuration?.modStates?.[entry.statusId]
            return <li key={entry.statusId}>
              <span>{entry.path}<small> · {statusLabel(state?.state || '')}</small></span>
              <CompatibilityStatusBadge status={state?.state === 'failed' ? 'error' :
                dllStateIsReady(state?.state) ? 'ok' : !enabled || state?.state === 'disabled' ? 'neutral' : 'warning'} />
              {state?.state === 'failed' && <small className="dll-manager-error">{state.detail}</small>}
            </li>
          })}
        </ol>
      </Card>}
      {data.dllCandidates.length === 0 && profiles.length === 0 && (
        <Card title={t('compatibility.dllManager.emptyTitle')}>
          <p className="hint">{t('compatibility.dllManager.empty')}</p>
        </Card>
      )}
      {data.dllCandidates.map((candidate) => {
        const directory = candidate.packageDirectory
        const profile = profiles.find((item) => item.packageDirectory === directory)
        const selectedProfile = selectedProfiles[directory] ?? profile?.compatibilityProfile ??
          (profile?.files.length ? 'none' : '')
        const currentSelection = selectedFiles[directory] ?? profile?.files.map((file) => file.path) ??
          defaultDllSelection(candidate)
        const target = targets[directory] || profile?.target || candidate.target
        const accepted = approved[directory] ?? profile?.approved ?? false
        const entries = [...candidate.loadEntries]
        const statusPrefix = `dll:${directory.toLowerCase()}:`
        for (const [id, state] of Object.entries(data.configuration?.modStates || {})) {
          if (id.startsWith(statusPrefix) && ['restart-required', 'dll-loaded', 'ready', 'waiting'].includes(state.state) &&
            !entries.some((entry) => entry.statusId === id)) {
            entries.push({ statusId: id, path: id.slice(statusPrefix.length), mode: 'library', source: 'previous' })
          }
        }
        const getState = (id: string) => data.configuration?.modStates?.[id]
        const failure = entries.some((entry) => getState(entry.statusId)?.state === 'failed')
        const ready = entries.length > 0 && entries.every((entry) => dllStateIsReady(getState(entry.statusId)?.state))
        const selectionEnabled = profile?.enabled !== false
        const active = selectionEnabled && candidate.manifestEnabled !== false &&
          (target === 'both' || target === (data.mode === 'SERVER' ? 'server' : 'client'))
        const pendingRestart = entries.some((entry) => getState(entry.statusId)?.state === 'restart-required')
        const status: CompatibilityStatus = failure ? 'error' : pendingRestart ? 'warning' : !enabled || !active ? 'neutral' :
          candidate.issues.length ? 'warning' : ready ? 'ok' : entries.length || candidate.dlls.length ? 'warning' : 'neutral'
        const usingAdapter = selectedProfile !== 'none' && Boolean(selectedProfile || candidate.selectedProfile)
        const modeFor = (path: string): DllLoadMode => modes[`${directory}/${path}`] ||
          profile?.files.find((file) => file.path === path)?.mode ||
          entries.find((entry) => entry.path === path)?.mode ||
          candidate.dlls.find((entry) => entry.path === path)?.mode || 'library'
        const replaceSelection = (selection: DllManagerProfile) => saveProfiles([
          ...profiles.filter((item) => item.packageDirectory !== directory), selection,
        ], directory)
        const saveSelection = () => {
          if (!accepted || (!usingAdapter && !currentSelection.length)) return
          replaceSelection({
            id: profile?.id || directory, packageDirectory: directory, name: candidate.name,
            target, enabled: true, approved: true,
            compatibilityProfile: selectedProfile || undefined,
            files: usingAdapter ? [] : currentSelection.map((path) => ({ path, mode: modeFor(path) })),
          })
        }
        const moveFile = (index: number, offset: number) => {
          const next = [...currentSelection]
          const destination = index + offset
          if (destination < 0 || destination >= next.length) return
          ;[next[index], next[destination]] = [next[destination], next[index]]
          setSelectedFiles((value) => ({ ...value, [directory]: next }))
        }
        return (
          <Card key={directory} title={candidate.name}>
            <div className="dll-manager-heading">
              <span className="hint">{directory}</span>
              <CompatibilityStatusBadge status={status} />
            </div>
            <div className="dll-manager-facts">
              <span>{t(`compatibility.dllManager.target.${target}` as TranslationKey)}</span>
              <span>{t(`compatibility.dllManager.manifest.${candidate.manifestState}` as TranslationKey)}</span>
              <span>{candidate.hasLua ? t('compatibility.dllManager.luaFound') : t('compatibility.dllManager.luaMissing')}</span>
              {candidate.iniState !== 'missing' && <span>{t(`compatibility.dllManager.ini.${candidate.iniState}` as TranslationKey)}</span>}
              {candidate.source && <span>{sourceLabel(candidate.source)}</span>}
              {candidate.shippedProfile && <span>{t('compatibility.dllManager.shippedProfile', { name: candidate.shippedProfile })}</span>}
              {candidate.filteredDlls.map((file) => <span className="dll-manager-filtered" key={file}>
                {t('compatibility.dllManager.filteredFile', { name: file })}
              </span>)}
            </div>
            {candidate.issues.map((issue) => <p className="hint dll-manager-error" key={issue}>{issue}</p>)}
            {entries.length > 0 && <ol className="dll-manager-file-list ordered">
              {entries.map((entry) => {
                const state = getState(entry.statusId)
                return <li key={entry.statusId}>
                  <span>{entry.path}<small> · {sourceLabel(entry.source)} · {statusLabel(state?.state || (!enabled || !active ? 'disabled' : ''))}</small></span>
                  <CompatibilityStatusBadge status={state?.state === 'failed' ? 'error' :
                    dllStateIsReady(state?.state) ? 'ok' : state?.state === 'disabled' || (!state && (!enabled || !active)) ? 'neutral' : 'warning'} />
                  {state && ['failed', 'restart-required'].includes(state.state) &&
                    <small className={state.state === 'failed' ? 'dll-manager-error' : 'hint'}>{state.detail}</small>}
                </li>
              })}
            </ol>}
            {enabled && active && entries.length > 0 && !ready && <p className="hint">{t('compatibility.dllManager.restart')}</p>}
            <SettingsRow label={t('compatibility.dllManager.profileEnabled')} control={
              <SettingSwitch label={t('compatibility.dllManager.profileEnabled')} checked={selectionEnabled} disabled={!enabled}
                onChange={(value) => replaceSelection(profile ? { ...profile, enabled: value } : {
                  id: directory, packageDirectory: directory, name: candidate.name, target,
                  enabled: value, approved: true, files: [],
                })} />
            } />
            <details>
              <summary>{t('compatibility.dllManager.editProfile')}</summary>
              <SettingsRow label={t('compatibility.dllManager.adapterLabel')} control={
                <select value={selectedProfile} disabled={!enabled} onChange={(event) => {
                  const selected = event.currentTarget.value
                  logUiEvent({
                    level: 'TRACE', source: 'compatibility', operation: 'choose-profile', subject: directory,
                    message: `Selected compatibility profile ${selected || 'automatic'} for ${candidate.name}. Save the selection to apply it.`,
                  })
                  setSelectedProfiles((value) => ({ ...value, [directory]: selected }))
                }}>
                  <option value="">{t('compatibility.dllManager.adapterAutomatic')}</option>
                  <option value="none">{t('compatibility.dllManager.adapterNone')}</option>
                  {candidate.availableProfiles.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
                </select>
              } />
              {!usingAdapter && <>
                <p className="hint dll-manager-profile-summary">{t('compatibility.dllManager.chooseFiles')}</p>
                <div className="dll-manager-files">
                  {candidate.dlls.map((file) => <label className="dll-manager-file" key={file.path}>
                    <input type="checkbox" checked={currentSelection.includes(file.path)} disabled={!enabled}
                      onChange={(event) => setSelectedFiles((value) => ({ ...value, [directory]: event.currentTarget.checked ?
                        [...currentSelection, file.path] : currentSelection.filter((path) => path !== file.path) }))} />
                    <span>{file.path}</span><small>{formatFileSize(file.size)}</small>
                  </label>)}
                </div>
                <ol className="dll-manager-file-list ordered">
                  {currentSelection.map((path, index) => <li key={path}>
                    <span>{path}</span>
                    <select value={modeFor(path)} disabled={!enabled} aria-label={t('compatibility.dllManager.modeLabel')}
                      onChange={(event) => setModes((value) => ({ ...value, [`${directory}/${path}`]: event.currentTarget.value as DllLoadMode }))}>
                      {(['library', 'dinput8'] as const).map((mode) => <option key={mode} value={mode}>
                        {t(`compatibility.dllManager.mode.${mode}` as TranslationKey)}
                      </option>)}
                    </select>
                    <button className="button ghost" disabled={!enabled || index === 0}
                      aria-label={t('compatibility.dllManager.moveUp')} onClick={() => moveFile(index, -1)}>↑</button>
                    <button className="button ghost" disabled={!enabled || index === currentSelection.length - 1}
                      aria-label={t('compatibility.dllManager.moveDown')} onClick={() => moveFile(index, 1)}>↓</button>
                  </li>)}
                </ol>
              </>}
              <SettingsRow label={t('compatibility.dllManager.targetLabel')} control={
                <select value={target} disabled={!enabled} onChange={(event) =>
                  setTargets((value) => ({ ...value, [directory]: event.currentTarget.value as DllManagerProfile['target'] }))}>
                  {(['client', 'server', 'both'] as const).map((value) => <option key={value} value={value}>
                    {t(`compatibility.dllManager.target.${value}` as TranslationKey)}
                  </option>)}
                </select>
              } />
              <label className="dll-manager-consent">
                <input type="checkbox" checked={accepted} disabled={!enabled} onChange={(event) =>
                  setApproved((value) => ({ ...value, [directory]: event.currentTarget.checked }))} />
                <span>{t('compatibility.dllManager.consent')}</span>
              </label>
              <p className="hint dll-manager-profile-summary">{t('compatibility.dllManager.virtualFiles')}</p>
              <div className="settings-diagnostic-actions">
                <button className="button settings-module-action" disabled={!enabled || !accepted || (!usingAdapter && !currentSelection.length)}
                  onClick={saveSelection}>{t('compatibility.dllManager.saveProfile')}</button>
                {profile && <button className="button ghost" onClick={() => {
                  saveProfiles(profiles.filter((item) => item.packageDirectory !== directory), directory)
                  setSelectedFiles((value) => { const next = { ...value }; delete next[directory]; return next })
                  setSelectedProfiles((value) => { const next = { ...value }; delete next[directory]; return next })
                  setTargets((value) => { const next = { ...value }; delete next[directory]; return next })
                  setApproved((value) => { const next = { ...value }; delete next[directory]; return next })
                }}>
                  {t('compatibility.dllManager.removeProfile')}
                </button>}
              </div>
            </details>
          </Card>
        )
      })}
      {profiles.filter((profile) => !data.dllCandidates.some((item) => item.packageDirectory === profile.packageDirectory))
        .map((profile) => <Card key={profile.id} title={profile.name}>
          <CompatibilityStatusBadge status={profile.enabled ? 'error' : 'neutral'} />
          <p className="hint">{t('compatibility.dllManager.packageMissing', { folder: profile.packageDirectory })}</p>
          <button className="button ghost" onClick={() => saveProfiles(profiles.filter((item) => item.id !== profile.id), profile.packageDirectory)}>
            {t('compatibility.dllManager.removeProfile')}
          </button>
        </Card>)}
      <Card title={t('compatibility.dllManager.filters.title')}>
        <p className="hint settings-module-description">{t('compatibility.dllManager.filters.description')}</p>
        <div className="dll-manager-filter-add">
          <input className="dll-manager-filter-input" type="text" value={newExcludedDllName}
            aria-label={t('compatibility.dllManager.filters.name')} placeholder={t('compatibility.dllManager.filters.name')}
            onChange={(event) => setNewExcludedDllName(event.currentTarget.value)}
            onKeyDown={(event) => { if (event.key === 'Enter') addName() }} />
          <button className="button secondary" disabled={!canAddName} onClick={addName}>{t('compatibility.dllManager.filters.add')}</button>
        </div>
        <ul className="dll-manager-filter-list">
          {excludedDllNames.map((name) => <li key={name}>
            <code>{name}</code>
            <button className="button ghost" onClick={() => saveExcludedDllNames(excludedDllNames.filter((item) => item !== name))}>
              {t('compatibility.dllManager.filters.remove')}
            </button>
          </li>)}
        </ul>
      </Card>
    </div>
  )
}

export function CompatibilityStatusBadge({
  status,
}: {
  status: CompatibilityStatus
}) {
  const { t } = useI18n()
  return (
    <span className={`compatibility-status ${status}`}>
      <Icon name={statusIcon(status)} />
      {t(`compatibility.status.${status}` as TranslationKey)}
    </span>
  )
}

function statusIcon(status: CompatibilityStatus) {
  return status === 'error'
    ? 'issue'
    : status === 'warning'
      ? 'warning'
      : status === 'ok'
        ? 'check'
        : 'compatibility'
}

export function CompatibilityTabLabel({
  label,
  status,
}: {
  label: string
  status: CompatibilityStatus
}) {
  return (
    <span className={`compatibility-tab-label ${status}`}>
      {label}
      <Icon name={statusIcon(status)} />
    </span>
  )
}

export function CompatibilityRow({ row }: { row: CompatibilityRowData }) {
  const { t } = useI18n()
  return (
    <div className={`settings-option compatibility-row ${row.status}`}>
      <span className="compatibility-row-copy">
        <strong>
          {row.title}
          <EcosystemBadge ecosystem={row.ecosystem} />
        </strong>
        <small>{row.detail}</small>
        {row.technicalDetail && (
          <details>
            <summary>{t('compatibility.details')}</summary>
            <p>{row.technicalDetail}</p>
          </details>
        )}
      </span>
      <div className="compatibility-row-control">
        <CompatibilityStatusBadge status={row.status} />
        {row.action && (
          <button className="button ghost" onClick={row.action.run}>
            {row.action.label}
          </button>
        )}
      </div>
    </div>
  )
}
