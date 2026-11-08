import type { SettingPreset } from './mod-types'

function sameValue(left: unknown, right: unknown): boolean {
  if (typeof left === 'number' && typeof right === 'number') return left === right
  return JSON.stringify(left) === JSON.stringify(right)
}

/**
 * Change one setting and follow the mod's presets. Choosing a preset in its
 * controller sets every member to that preset's value. Changing a member by
 * hand selects the controller's custom value, unless the new value is the
 * one the current preset already gives.
 */
export function applySettingChange(
  values: Record<string, unknown>,
  presets: Record<string, SettingPreset> | undefined,
  key: string,
  value: unknown,
): Record<string, unknown> {
  const next = { ...values, [key]: value }
  if (!presets) return next
  const chosen = presets[key]
  if (chosen) {
    const preset = typeof value === 'string' ? chosen.values[value] : undefined
    if (preset) for (const member of chosen.settings) next[member] = preset[member]
    return next
  }
  for (const [controller, preset] of Object.entries(presets)) {
    if (!preset.settings.includes(key)) continue
    const current = next[controller]
    if (current === preset.custom) continue
    const expected =
      typeof current === 'string' ? preset.values[current] : undefined
    if (!expected || !sameValue(expected[key], value))
      next[controller] = preset.custom
  }
  return next
}
