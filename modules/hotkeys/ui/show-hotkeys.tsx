import { postRequest } from '../../transport/ui/send-host-command'
import { Card } from '../../ui-resources/show-card'
import { SettingsRow } from '../../ui-resources/edit-setting'
import { useI18n } from '../../ui-resources/i18n'
import type { TranslationKey } from '../../ui-resources/i18n'
import type { Snapshot } from '../../modloader/ui/ui-snapshot'
import { settingIsEnabled } from '../../modloader/ui/edit-mod-setting'
import { KeybindInput, keybindText } from './edit-keybind'
import type { KeybindAssessmentTarget } from './edit-keybind'
import { WikiHelp } from '../../wiki/ui/show-wiki-help'

type HotkeyRow = {
  id: string
  conflictId: string
  owner: string
  label: string
  description?: string
  binding: string
  enabled: boolean
  assessmentTarget: KeybindAssessmentTarget
  save(binding: string): void
  reset?(): void
}

function hotkeyRows(data: Snapshot): HotkeyRow[] {
  const client = data.mode !== 'SERVER'
  const modules = Object.entries(data.settings.modulePreferences || {})
    .filter(([, preferences]) => Object.prototype.hasOwnProperty.call(preferences, 'toggleKey'))
    .map(([module, preferences]): HotkeyRow => ({
      id: `module:${module}`,
      conflictId: `modules.${module}.toggleKey`,
      owner: module,
      label: '',
      binding: keybindText(preferences.toggleKey),
      enabled: client && preferences.enabled !== false,
      assessmentTarget: { scope: 'module', module },
      save: (binding) => postRequest('save-setting', {
        scope: 'module', module, key: 'toggleKey', value: binding,
      }),
      reset: () => postRequest('save-setting', {
        scope: 'module', module, key: 'toggleKey', value: null,
      }),
    }))
  const mods = data.mods.flatMap((mod) => {
    const values = Object.fromEntries(mod.settings.map((definition) => [
      definition.key, mod.settingValues[definition.key] ?? definition.default,
    ]))
    return mod.settings
    .filter((definition) => definition.control === 'keybind')
    .map((definition): HotkeyRow => ({
      id: `mod:${mod.id}:${definition.key}`,
      conflictId: `${mod.id}.${definition.key}`,
      owner: mod.name,
      label: definition.label,
      description: definition.description,
      binding: keybindText(mod.settingValues[definition.key] ?? definition.default),
      enabled: client && mod.enabled && mod.target !== 'server' &&
        (mod.id !== 'world-editor' || data.settings.modulePreferences?.worldEditor?.enabled !== false) &&
        settingIsEnabled(definition, mod.settings, values),
      assessmentTarget: {
        scope: 'mod',
        modId: mod.id,
        key: definition.key,
        revision: mod.revision,
        values,
      },
      save: (binding) => postRequest('save-mod-settings', {
        modId: mod.id, values: { [definition.key]: binding }, revision: mod.revision,
      }),
    }))
  })
  return [...modules, ...mods]
}

function conflictsFor(
  rows: HotkeyRow[],
  row: HotkeyRow,
  conflicts: Snapshot['hotkeyConflicts'],
): HotkeyRow[] {
  const conflictIds = conflicts.flatMap((conflict) => {
    if (conflict.firstId === row.conflictId) return [conflict.secondId]
    if (conflict.secondId === row.conflictId) return [conflict.firstId]
    return []
  })
  return rows.filter((other) =>
    other.id !== row.id && other.enabled && conflictIds.includes(other.conflictId),
  )
}

export function hasHotkeyConflicts(data: Snapshot): boolean {
  return data.hotkeyConflicts.length > 0
}

export function Hotkeys({ data }: { data: Snapshot }) {
  const { t } = useI18n()
  const rows = hotkeyRows(data)
  const ownerLabel = (row: HotkeyRow) => row.id.startsWith('module:')
    ? t(`settings.modules.${row.owner}.title` as TranslationKey) : row.owner
  const actionLabel = (row: HotkeyRow) => row.label || t('hotkeys.toggleWindow')
  const label = (row: HotkeyRow) => `${ownerLabel(row)}: ${actionLabel(row)}`
  return (
    <div className="settings-module-list compatibility-sections">
      <WikiHelp articleId="hotkeys" />
      <Card title={t('compatibility.tab.hotkeys')}>
        <p className="hint settings-module-description">{t('hotkeys.description')}</p>
        <p className="hint settings-module-description">{t('hotkeys.captureHint')}</p>
        {hasHotkeyConflicts(data) && <p className="builder-notice warning" role="status">
          {t('hotkeys.conflicts')}
        </p>}
        <div className="settings-options">
          {rows.map((row) => {
            const conflicts = conflictsFor(rows, row, data.hotkeyConflicts)
            const detail = [
              row.description,
              !row.enabled ? t('hotkeys.inactive') : '',
              conflicts.length ? t('hotkeys.conflict', { actions: conflicts.map(label).join(', ') }) : '',
            ].filter(Boolean).join(' ')
            return <SettingsRow
              key={row.id}
              label={label(row)}
              detail={detail || undefined}
              rowClass={`keybind-setting-row${conflicts.length ? ' warning' : ''}`}
              control={<KeybindInput
                value={row.binding}
                label={label(row)}
                assessmentTarget={row.assessmentTarget}
                onChange={row.save}
                onReset={row.reset}
              />}
            />
          })}
        </div>
        {!rows.length && <p className="hint">{t('hotkeys.empty')}</p>}
        {data.settings.protectedHotkeys?.length ? <div className="settings-options">
          <p className="hint settings-module-description">{t('hotkeys.policyTitle')}</p>
          <p className="hint settings-module-description">{t('hotkeys.policySource')}</p>
          {data.settings.protectedHotkeys.map((key) => {
            const detail = key.action === 'confirm'
              ? key.scancode === 58
                ? t('hotkeys.reservedF1')
                : key.scancode === 69
                  ? t('hotkeys.reservedF12')
                  : t('hotkeys.policyConfirm', { key: key.label })
              : key.action === 'cancel'
                ? t('hotkeys.policyCancel', { key: key.label })
                : t('hotkeys.policyBlocked', { key: key.label })
            const icon = key.action === 'confirm'
              ? '⚠'
              : key.action === 'cancel'
                ? '↩'
                : '⊘'
            return <SettingsRow
              key={key.scancode}
              label={key.label}
              detail={detail}
              rowClass={`hotkey-policy-${key.action}`}
              control={<span aria-hidden="true">{icon}</span>}
            />
          })}
        </div> : null}
      </Card>
    </div>
  )
}
