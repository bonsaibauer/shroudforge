export type SettingOption = { value: string | number | boolean; label: string }
export type SettingCondition = { setting: string; equals: string | number | boolean }

export type SettingDefinition = {
  key: string
  label: string
  description?: string
  type: 'boolean' | 'number' | 'integer' | 'string' | 'array'
  control?:
    | 'toggle'
    | 'checkbox'
    | 'text'
    | 'textarea'
    | 'number'
    | 'slider'
    | 'select'
    | 'radio'
    | 'segmented'
    | 'multiselect'
    | 'keybind'
    | 'color'
  default: unknown
  group?: string
  minimum?: number
  maximum?: number
  step?: number
  minimumLength?: number
  maximumLength?: number
  options?: SettingOption[]
  enabledWhen?: SettingCondition
  applyAt?: 'live' | 'restart'
  restartRequired?: boolean
  assetPreparation?: boolean
}

export type ModAction = {
  id: string
  label: string
  style?: string
  confirm?: string
}

export type SettingGroup = {
  label: string
  description?: string
  settings: string[]
  actions: ModAction[]
}

/** Keyed by the controller setting. Each entry of `values` is one preset. */
export type SettingPreset = {
  custom: string
  settings: string[]
  values: Record<string, Record<string, unknown>>
}

export type ModDependencyInfo = {
  id: string
  version: string
  optional: boolean
  installedVersion?: string
  status: string
}

export type ModInfo = {
  revision: string
  id: string
  name: string
  version: string
  dependencies: ModDependencyInfo[]
  ecosystem: 'EML' | 'SF'
  target: string
  asset: boolean
  runtime: boolean
  description?: string
  authors: string[]
  license?: string
  icon?: string
  links: Record<string, string | undefined>
  source: string
  enabled: boolean
  settings: SettingDefinition[]
  groups: SettingGroup[]
  presets?: Record<string, SettingPreset>
  changelog: string[]
  assets: Record<string, string>
  settingValues: Record<string, unknown>
}
