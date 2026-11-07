import type { SettingDefinition } from './mod-types'
import type React from 'react'
import { useEffect, useState } from 'react'

import { SettingSwitch, SettingsRow } from '../../ui-resources/edit-setting'
import { KeybindInput } from '../../hotkeys/ui/edit-keybind'
import type { InputDevice, KeybindAssessmentTarget } from '../../hotkeys/ui/edit-keybind'
import { KeyReservationLock } from '../../hotkeys/ui/toggle-key-reservation'
import { useI18n } from '../../ui-resources/i18n'

export function ModSetting({
  definition,
  value,
  assessmentTarget,
  onChange,
  disabled = false,
  contexts = {},
  onReserve,
}: {
  definition: SettingDefinition
  value: unknown
  assessmentTarget: KeybindAssessmentTarget
  onChange(value: unknown): void
  disabled?: boolean
  /** The mod's input context labels, for the condition of a reserved key. */
  contexts?: Record<string, string>
  /** Saves a padlock of this keybind. */
  onReserve?(device: InputDevice, reserved: boolean): void
}) {
  const { t } = useI18n()
  // A padlock turns at the click; the saved state replaces it when the page
  // refreshes, or after a few seconds if the save failed.
  const [turned, setTurned] = useState<Partial<Record<InputDevice, boolean>>>({})
  const savedPadlocks = JSON.stringify(definition.reserved ?? {})
  useEffect(() => setTurned({}), [savedPadlocks])
  useEffect(() => {
    if (Object.keys(turned).length === 0) return
    const timer = window.setTimeout(() => setTurned({}), 4000)
    return () => window.clearTimeout(timer)
  }, [turned])
  const control =
    definition.control ||
    (definition.type === 'boolean'
      ? 'toggle'
      : definition.type === 'number' || definition.type === 'integer'
        ? 'number'
        : 'text')
  const numeric = definition.type === 'number' || definition.type === 'integer'
  let input: React.ReactNode
  const wheel = definition.wheel
  const reserve = definition.reserve
  if (wheel && definition.type === 'boolean') {
    // The mouse wheel the mod reads: its padlock is the setting's value.
    const condition = contexts[wheel.when] ?? wheel.when
    const reserved = Boolean(value)
    input = (
      <div className="keybind-editor">
        <div className="keybind-device">
          <span className="keybind-device-label">{t('hotkeys.mouse')}</span>
          <div className="keybind-binding">
            <KeyReservationLock
              reservation={{ condition, reserved, onToggle: (next) => onChange(next) }}
              keyLabel={t('hotkeys.reservation.wheel')}
              disabled={disabled}
            />
            <span className={'button secondary keybind-input fixed' + (reserved ? ' reserved' : '')}>
              {t('hotkeys.reservation.wheel')}
            </span>
          </div>
          {reserved && <span className="keybind-reserved-when">{condition}</span>}
        </div>
      </div>
    )
  } else if (control === 'toggle' || control === 'checkbox')
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
      <div className={`mod-multiselect ${disabled ? 'disabled' : ''}`}>
        <div className="mod-multiselect-title">
          <strong>{definition.label}</strong>
          {definition.description && <small>{definition.description}</small>}
        </div>
        <div className="settings-options">
          {(definition.options || []).map((option) => {
            const selected =
              Array.isArray(value) &&
              value.some((item) => String(item) === String(option.value))
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
        reservations={
          reserve && onReserve
            ? Object.fromEntries(
                (['keyboard', 'mouse'] as const).map((device) => [
                  device,
                  {
                    condition: contexts[reserve.when] ?? reserve.when,
                    reserved:
                      turned[device] ?? definition.reserved?.[device] ?? reserve[device] ?? false,
                    onToggle: (reserved: boolean) => {
                      setTurned((current) => ({ ...current, [device]: reserved }))
                      onReserve(device, reserved)
                    },
                  },
                ]),
              )
            : undefined
        }
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
        control === 'keybind' || wheel ? 'keybind-setting-row' : '',
        control === 'slider' ? 'slider-setting' : '',
        control === 'radio' || control === 'segmented' ? 'expanded' : '',
        disabled ? 'disabled' : '',
      ]
        .filter(Boolean)
        .join(' ')}
    />
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
