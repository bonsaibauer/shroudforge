import type { ModulePreferences } from './loader-settings'

export const SERVER_TARGET_VISIBILITY_MODULES = [
  'debugConsole',
  'worldEditor',
  'modloaderUi',
  'wiki',
  'runtimeDiagnostics',
] as const

export function isVisibleInServerTarget(
  module: string,
  preferences?: ModulePreferences,
): boolean {
  const configured = preferences?.[module]?.visibleInServerTarget
  if (typeof configured === 'boolean') return configured

  // Older server configurations may not include the optional setting yet.
  return module !== 'wiki'
}
