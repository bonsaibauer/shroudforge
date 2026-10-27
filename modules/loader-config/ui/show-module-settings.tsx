import type { Settings } from './loader-settings'
import { postRequest } from '../../transport/ui/send-host-command'
import { TranslationKey, useI18n } from '../../ui-resources/i18n'
import type { Snapshot } from '../../modloader/ui/ui-snapshot'
import { Card } from '../../ui-resources/show-card'
import {
  ModuleEnabledRow,
  SettingSwitch,
  SettingsNumber,
  SettingsRow,
  Toggle,
} from '../../ui-resources/edit-setting'
import {
  DiagnosticsControls,
  diagnosticSettingsFields,
} from '../../runtime-diagnostics/ui/show-diagnostics'
import { debugConsoleSettingsFields } from '../../debug-console/ui/settings'
import { worldEditorSettingsFields } from '../../world-editor/ui/settings'
import { NetworkSettings } from '../../transport/ui/settings'
import { UpdatePreferences } from '../../updater/ui/edit-update-preferences'
import { KeybindInput } from '../../hotkeys/ui/edit-keybind'

function WindowGeometryControls({
  module,
  disabled = false,
}: {
  module: 'debugConsole' | 'modloaderUi' | 'updates' | 'worldEditor'
  disabled?: boolean
}) {
  const { t } = useI18n()
  const reset = (geometryPart: 'position' | 'size') => {
    if (!disabled) {
      postRequest('reset-window-geometry', { module, geometryPart })
    }
  }
  return (
    <>
      <SettingsRow
        label={t('settings.modules.windowGeometry.position')}
        control={
          <button
            className="button ghost"
            disabled={disabled}
            onClick={() => reset('position')}
          >
            {t('settings.modules.windowGeometry.reset')}
          </button>
        }
      />
      <SettingsRow
        label={t('settings.modules.windowGeometry.size')}
        control={
          <button
            className="button ghost"
            disabled={disabled}
            onClick={() => reset('size')}
          >
            {t('settings.modules.windowGeometry.reset')}
          </button>
        }
      />
      {disabled && (
        <p className="hint settings-module-hint">
          {t('settings.modules.windowGeometry.worldEditorFixed')}
        </p>
      )}
    </>
  )
}

export function ModulePreferencesEditor({
  data,
  settings,
  setSettings,
}: {
  data: Snapshot
  settings: Settings
  setSettings(v: Settings): void
}) {
  const { t } = useI18n()
  const prefs = settings.modulePreferences || {}
  const system = prefs.updates?.system || {
    includePrereleases: false,
  }
  const automatic = prefs.updates?.automatic || {
    enabled: true,
    checkMinutes: 60,
    queueUpdates: true,
  }
  const recycleBin = prefs.updates?.recycleBin || {
    automaticCleanup: true,
    retentionDays: 30,
  }
  const saveModule = (module: string, key: string, value: unknown) =>
    postRequest('save-setting', { scope: 'module', module, key, value })
  const changeSystem = (key: string, value: unknown) => {
    setSettings({
      ...settings,
      modulePreferences: {
        ...prefs,
        updates: { ...prefs.updates, system: { ...system, [key]: value } },
      },
    })
    saveModule('updates.system', key, value)
  }
  const changeAutomatic = (key: string, value: unknown) => {
    setSettings({
      ...settings,
      modulePreferences: {
        ...prefs,
        updates: {
          ...prefs.updates,
          automatic: { ...automatic, [key]: value },
        },
      },
    })
    saveModule('updates.automatic', key, value)
  }
  const changeRecycleBin = (key: string, value: unknown) => {
    setSettings({
      ...settings,
      modulePreferences: {
        ...prefs,
        updates: {
          ...prefs.updates,
          recycleBin: { ...recycleBin, [key]: value },
        },
      },
    })
    saveModule('updates.recycleBin', key, value)
  }
  const change = (module: string, key: string, value: unknown) => {
    setSettings({
      ...settings,
      modulePreferences: {
        ...prefs,
        [module]: { ...prefs[module], [key]: value },
      },
    })
    saveModule(module, key, value)
  }
  const fields: Array<[string, string, TranslationKey, string[]?]> = [
    ...diagnosticSettingsFields,
    ...debugConsoleSettingsFields,
    ...worldEditorSettingsFields,
    ['modloaderUi', 'toggleKey', 'settings.modules.field.hotkey'],
    ['modloaderUi', 'refreshMilliseconds', 'settings.modules.field.refresh'],
  ]
  const controlVisibility = (
    module: 'debugConsole' | 'modloaderUi' | 'worldEditor' | 'updates',
    visible: boolean,
  ) => postRequest('set-window-visibility', { module, visible })
  const moduleEnabled = (module: string) =>
    prefs[module]?.enabled ?? (module === 'debugConsole')
  const updaterAvailable = data.release.updaterAvailable === true
  const updatesEnabled = moduleEnabled('updates') && updaterAvailable
  const ranges: Record<string, [number, number]> = {
    intervalMilliseconds: [500, 60000],
    maximumDurationSeconds: [1, 3600],
    slowCallbackMilliseconds: [0.1, 60000],
    popupCooldownSeconds: [0, 3600],
    refreshMilliseconds: [100, 10000],
    tailBytes: [1024, 1048576],
  }
  return (
    <>
      <UpdatePreferences
        system={system}
        automatic={automatic}
        enabled={updatesEnabled}
        updaterAvailable={updaterAvailable}
        onCommand={postRequest}
        changeEnabled={(value) => {
          if (value && !updaterAvailable) return
          change('updates', 'enabled', value)
          if (!value) controlVisibility('updates', false)
        }}
        windowVisible={Boolean(data.windows?.updates?.visible)}
        changeWindowVisibility={(visible) => controlVisibility('updates', visible)}
        changeSystem={changeSystem}
        changeAutomatic={changeAutomatic}
        recycleBin={recycleBin}
        changeRecycleBin={changeRecycleBin}
        geometryControls={<WindowGeometryControls module="updates" />}
      />
      <NetworkSettings
        prefs={prefs}
        enabled={moduleEnabled('network')}
        change={change}
      />
      <div className="settings-module-list">
        {(
          [
            'debugConsole',
            'worldEditor',
            'modloaderUi',
            'runtimeDiagnostics',
          ] as const
        ).map((module) => (
          <Card key={module} title={t(`settings.modules.${module}.title`)}>
            <p className="hint settings-module-description">
              {t(`settings.modules.${module}.description`)}
            </p>
            {(module === 'debugConsole' ||
              module === 'modloaderUi' ||
              module === 'worldEditor') && (
              <div className="settings-window-control">
                <Toggle
                  label={t('settings.modules.windowVisible')}
                  checked={Boolean(data.windows?.[module]?.visible)}
                  disabled={!moduleEnabled(module)}
                  onChange={(visible) => controlVisibility(module, visible)}
                />
                <span
                  className={`state-pill ${data.windows?.[module]?.visible ? 'ok' : 'neutral'}`}
                >
                  {data.windows?.[module]?.visible
                    ? t('settings.modules.windowOpen')
                    : t('settings.modules.windowClosed')}
                </span>
              </div>
            )}
            <div className="settings-options">
              <ModuleEnabledRow
                label={t('settings.modules.field.active')}
                enabled={moduleEnabled(module)}
                onChange={(value) => change(module, 'enabled', value)}
              />
              {!moduleEnabled(module) && module !== 'runtimeDiagnostics' && (
                <p className="builder-notice warning" role="status">
                  {t('settings.moduleUnavailable')}
                </p>
              )}
              {module === 'debugConsole' && (
                <p className="hint settings-module-hint">
                  {t('settings.modules.debugConsole.popupHint')}
                </p>
              )}
              {fields
                .filter(
                  (field) =>
                    field[0] === module &&
                    (field[1] !== 'popupCooldownSeconds' ||
                      ['all', 'errorsAndWarnings'].includes(
                        prefs.debugConsole?.popupMode,
                      )),
                )
                .map(([, key, label, options]) => (
                  <SettingsRow
                    key={key}
                    label={t(label)}
                    rowClass={key === 'toggleKey' ? 'keybind-setting-row' : ''}
                    control={
                      key === 'toggleKey' ? (
                        <KeybindInput
                          label={t(label)}
                          value={prefs[module]?.[key]}
                          assessmentTarget={{ scope: 'module', module }}
                          onChange={(value) => change(module, key, value)}
                          onReset={() => saveModule(module, key, null)}
                        />
                      ) : options ? (
                        <select
                          value={prefs[module]?.[key] ?? options[0]}
                          onChange={(e) => change(module, key, e.target.value)}
                        >
                          {options.map((option) => (
                            <option key={option} value={option}>
                              {key === 'popupMode'
                                ? t(
                                    `settings.modules.debugConsole.popupMode.${option}` as TranslationKey,
                                  )
                                : option}
                            </option>
                          ))}
                        </select>
                      ) : (
                        typeof prefs[module]?.[key] === 'boolean' ||
                        ['autoScroll', 'onlyChanges'].includes(key)
                      ) ? (
                        <SettingSwitch
                          label={t(label)}
                          checked={prefs[module]?.[key] !== false}
                          onChange={(value) => change(module, key, value)}
                        />
                      ) : (
                        <SettingsNumber
                          value={
                            prefs[module]?.[key] ??
                            (key === 'popupCooldownSeconds' ? 30 : 0)
                          }
                          minimum={ranges[key]?.[0]}
                          maximum={
                            module === 'worldEditor' &&
                            key === 'refreshMilliseconds'
                              ? 5000
                              : ranges[key]?.[1]
                          }
                          onCommit={(value) => change(module, key, value)}
                        />
                      )
                    }
                  />
                ))}
              {(module === 'debugConsole' ||
                module === 'modloaderUi' ||
                module === 'worldEditor') && (
                <WindowGeometryControls
                  module={module}
                  disabled={module === 'worldEditor'}
                />
              )}
            </div>
            {module === 'runtimeDiagnostics' && (
              <DiagnosticsControls
                data={data}
                prefs={prefs}
                module={module}
                change={change}
                jobs={data.settings.diagnosticJobs || []}
                jobErrors={data.settings.diagnosticJobErrors || []}
                jobDirectory={data.settings.diagnosticJobDirectory || ''}
                selectedJobId={data.settings.diagnosticSelectedJobId || 'runtime.overview'}
              />
            )}
          </Card>
        ))}
      </div>
    </>
  )
}
