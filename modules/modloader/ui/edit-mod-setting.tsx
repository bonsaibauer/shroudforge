import type { SettingDefinition } from './mod-types'
import type React from 'react'
import { useMemo, useState } from 'react'

import { SettingSwitch, SettingsRow } from '../../ui-resources/edit-setting'
import { useI18n } from '../../ui-resources/i18n'
import { KeybindInput } from '../../hotkeys/ui/edit-keybind'
import type { KeybindAssessmentTarget } from '../../hotkeys/ui/edit-keybind'

export function ModSetting({
  definition,
  value,
  assessmentTarget,
  onChange,
  disabled = false,
}: {
  definition: SettingDefinition
  value: unknown
  assessmentTarget: KeybindAssessmentTarget
  onChange(value: unknown): void
  disabled?: boolean
}) {
  const control =
    definition.control ||
    (definition.type === 'boolean'
      ? 'toggle'
      : definition.type === 'number' || definition.type === 'integer'
        ? 'number'
        : 'text')
  const numeric = definition.type === 'number' || definition.type === 'integer'
  let input: React.ReactNode
  if (control === 'toggle' || control === 'checkbox')
    input = (
      <SettingSwitch
        label={definition.label}
        checked={Boolean(value)}
        disabled={disabled}
        onChange={(checked) => onChange(checked)}
      />
    )
  else if (control === 'slider')
    input = (
      <div className="slider-control">
        <input
          type="range"
          value={Number(value ?? definition.default)}
          min={definition.minimum}
          max={definition.maximum}
          step={definition.step}
          disabled={disabled}
          onChange={(event) => onChange(Number(event.target.value))}
        />
        <output>{String(value ?? definition.default)}</output>
      </div>
    )
  else if (control === 'select')
    input = (
      <select
        value={String(value ?? '')}
        disabled={disabled}
        onChange={(event) =>
          onChange(optionValue(definition, event.target.value))
        }
      >
        {(definition.options || []).map((option) => (
          <option key={String(option.value)} value={String(option.value)}>
            {option.label}
          </option>
        ))}
      </select>
    )
  else if (control === 'radio' || control === 'segmented')
    input = (
      <div className={'choice-control ' + control}>
        {(definition.options || []).map((option) => (
          <button
            type="button"
            className={String(value) === String(option.value) ? 'active' : ''}
            key={String(option.value)}
            disabled={disabled}
            onClick={() => onChange(option.value)}
          >
            {option.label}
          </button>
        ))}
      </div>
    )
  else if (control === 'multiselect')
    return (
      <MultiSelectSetting
        definition={definition}
        value={value}
        onChange={onChange}
        disabled={disabled}
      />
    )
  else if (control === 'textarea')
    input = (
      <textarea
        className="mod-input"
        value={String(value ?? '')}
        minLength={definition.minimumLength}
        maxLength={definition.maximumLength}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value)}
      />
    )
  else if (control === 'color')
    input = (
      <input
        className="mod-input mod-color"
        type="color"
        value={String(value ?? definition.default)}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value)}
      />
    )
  else if (control === 'keybind')
    input = (
      <KeybindInput
        value={value}
        label={definition.label}
        assessmentTarget={assessmentTarget}
        disabled={disabled}
        onChange={onChange}
      />
    )
  else
    input = (
      <input
        className="mod-input"
        type={numeric ? 'number' : 'text'}
        value={String(value ?? '')}
        min={definition.minimum}
        max={definition.maximum}
        step={definition.step}
        minLength={definition.minimumLength}
        maxLength={definition.maximumLength}
        disabled={disabled}
        onChange={(event) =>
          onChange(numeric ? Number(event.target.value) : event.target.value)
        }
      />
    )
  return (
    <SettingsRow
      label={definition.label}
      detail={definition.description}
      control={
        <div className="mod-setting-control" aria-disabled={disabled}>
          {input}
        </div>
      }
      rowClass={[
        control === 'keybind' ? 'keybind-setting-row' : '',
        control === 'slider' ? 'slider-setting' : '',
        control === 'radio' || control === 'segmented' ? 'expanded' : '',
        disabled ? 'disabled' : '',
      ]
        .filter(Boolean)
        .join(' ')}
    />
  )
}

// Long choice lists (more than MULTISELECT_SEARCH_FROM options) get a search
// field and an "only switched on" filter; short lists render as before.
const MULTISELECT_SEARCH_FROM = 12

function MultiSelectSetting({
  definition,
  value,
  onChange,
  disabled,
}: {
  definition: SettingDefinition
  value: unknown
  onChange(value: unknown): void
  disabled: boolean
}) {
  const { t } = useI18n()
  const [query, setQuery] = useState('')
  const [selectedOnly, setSelectedOnly] = useState(false)
  const options = definition.options || []
  const selectedValues = useMemo(
    () => new Set(Array.isArray(value) ? value.map((item) => String(item)) : []),
    [value],
  )
  const searchable = options.length > MULTISELECT_SEARCH_FROM
  const needle = query.trim().toLocaleLowerCase()
  const shown = options.filter((option) => {
    if (selectedOnly && !selectedValues.has(String(option.value))) return false
    if (!needle) return true
    return (
      String(option.label ?? '').toLocaleLowerCase().includes(needle) ||
      String(option.value).toLocaleLowerCase().includes(needle)
    )
  })
  return (
    <div className={`mod-multiselect ${disabled ? 'disabled' : ''}`}>
      <div className="mod-multiselect-title">
        <strong>{definition.label}</strong>
        {definition.description && <small>{definition.description}</small>}
      </div>
      {searchable && (
        <div className="mod-multiselect-filter">
          <input
            className="mod-input"
            type="search"
            value={query}
            aria-label={t('mod.settings.multiselect.search')}
            placeholder={t('mod.settings.multiselect.search')}
            disabled={disabled}
            onChange={(event) => setQuery(event.currentTarget.value)}
          />
          <label className="mod-multiselect-selected-only">
            <input
              type="checkbox"
              className="settings-checkbox"
              checked={selectedOnly}
              disabled={disabled}
              onChange={(event) => setSelectedOnly(event.currentTarget.checked)}
            />
            {t('mod.settings.multiselect.selectedOnly')}
          </label>
          <small>
            {t('mod.settings.multiselect.count', {
              shown: shown.length,
              total: options.length,
              selected: selectedValues.size,
            })}
          </small>
        </div>
      )}
      <div className="settings-options">
        {shown.length === 0 && searchable && (
          <p className="hint">{t('mod.settings.multiselect.noMatch')}</p>
        )}
        {shown.map((option) => {
          const selected = selectedValues.has(String(option.value))
          return (
            <SettingsRow
              key={String(option.value)}
              label={option.label}
              rowClass={disabled ? 'disabled' : ''}
              control={
                <SettingSwitch
                  label={option.label}
                  checked={selected}
                  disabled={disabled}
                  onChange={() =>
                    onChange(
                      selected
                        ? (value as unknown[]).filter(
                            (item) => String(item) !== String(option.value),
                          )
                        : [
                            ...(Array.isArray(value) ? value : []),
                            option.value,
                          ],
                    )
                  }
                />
              }
            />
          )
        })}
      </div>
    </div>
  )
}

export function optionValue(definition: SettingDefinition, value: string) {
  return (
    definition.options?.find((option) => String(option.value) === value)
      ?.value ?? value
  )
}

export function settingIsEnabled(
  definition: SettingDefinition,
  settings: SettingDefinition[],
  values: Record<string, unknown>,
  seen = new Set<string>(),
): boolean {
  const condition = definition.enabledWhen
  if (!condition) return true
  if (seen.has(definition.key)) return false
  if (values[condition.setting] !== condition.equals) return false
  const controller = settings.find((setting) => setting.key === condition.setting)
  if (!controller) return false
  const nextSeen = new Set(seen)
  nextSeen.add(definition.key)
  return settingIsEnabled(controller, settings, values, nextSeen)
}
