import type { UiTargetSettings } from './loader-settings'

export const TARGET_UI_PAGES = [
  'activity',
  'news',
  'compatibility',
  'settings',
  'wiki',
  'discover',
  'installed',
  'updates',
  'recycle-bin',
] as const

export const TARGET_UI_MODULES = [
  'modDetection',
  'dllManager',
  'debugConsole',
  'worldEditor',
  'modloaderUi',
  'wiki',
  'network',
  'runtimeDiagnostics',
  'gameProfileDiscovery',
  'updates',
] as const

export type TargetUiPage = (typeof TARGET_UI_PAGES)[number]
export type TargetUiModule = (typeof TARGET_UI_MODULES)[number]

export const DEFAULT_UI_TARGET_SETTINGS: UiTargetSettings = {
  visibility: {
    client: {
      pages: Object.fromEntries(TARGET_UI_PAGES.map((page) => [page, true])) as Record<TargetUiPage, boolean>,
      modules: Object.fromEntries(TARGET_UI_MODULES.map((module) => [module, true])) as Record<TargetUiModule, boolean>,
    },
    server: {
      pages: {
        activity: true,
        news: true,
        compatibility: true,
        settings: true,
        wiki: false,
        discover: false,
        installed: true,
        updates: false,
        'recycle-bin': false,
      },
      modules: Object.fromEntries(
        TARGET_UI_MODULES.map((module) => [module, module !== 'wiki']),
      ) as Record<TargetUiModule, boolean>,
    },
  },
  wikiLinkTarget: 'client',
}

export function resolveUiTargetSettings(
  settings?: Partial<UiTargetSettings>,
): UiTargetSettings {
  const visibility = settings?.visibility
  const mergeTarget = (target: 'client' | 'server') => ({
    pages: {
      ...DEFAULT_UI_TARGET_SETTINGS.visibility[target].pages,
      ...visibility?.[target]?.pages,
    } as Record<TargetUiPage, boolean>,
    modules: {
      ...DEFAULT_UI_TARGET_SETTINGS.visibility[target].modules,
      ...visibility?.[target]?.modules,
    } as Record<TargetUiModule, boolean>,
  })
  const wikiLinkTarget = settings?.wikiLinkTarget
  return {
    visibility: {
      client: mergeTarget('client'),
      server: mergeTarget('server'),
    },
    wikiLinkTarget:
      wikiLinkTarget === 'current' || wikiLinkTarget === 'client' || wikiLinkTarget === 'server'
        ? wikiLinkTarget
        : DEFAULT_UI_TARGET_SETTINGS.wikiLinkTarget,
  }
}

export function isTargetUiVisible(
  type: 'pages' | 'modules',
  id: string,
  target: 'client' | 'server',
  settings?: UiTargetSettings,
): boolean {
  if (type === 'pages' && id === 'settings') return true
  const resolved = resolveUiTargetSettings(settings)
  return resolved.visibility[target][type][id] ?? (type !== 'pages' || id !== 'wiki')
}
