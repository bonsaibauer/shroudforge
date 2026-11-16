const WIKI_SITE_ORIGIN = 'https://wiki.shroudforge.net'

export const DEFAULT_WIKI_CATEGORY = 'Guide:Shroudforge'

export const WIKI_PAGE_SIZE_OPTIONS = [10, 20, 30, 50] as const

export const WIKI_API_SETTINGS = {
  siteOrigin: WIKI_SITE_ORIGIN,
  endpoint: `${WIKI_SITE_ORIGIN}/api.php`,
  cacheKey: 'sf-wiki-cache-v1',
  defaultItemsPerPage: 30,
  defaultRefreshIntervalMinutes: 1440,
  maxApiPageSize: 500,
  summaryTitleBatchSize: 50,
  searchDebounceMs: 300,
  extractCharacters: 1200,
  summaryCharacters: 360,
  thumbnailSize: 128,
  cacheSearchLimit: 12,
  cacheArticleLimit: 30,
  cacheCharacterLimit: 1_500_000,
  pageLocaleSuffixes: [
    'de', 'de-at', 'de-ch', 'en', 'en-gb', 'en-us', 'es', 'fr', 'hi', 'it',
    'ja', 'nl', 'pl', 'sv', 'zh',
  ],
  legacyPageTitles: {
    dependencies: 'Dependencies',
    'dll-manager': 'DLL Manager',
    engine: 'Engine',
    'game-profile-discovery': 'Automatic beta game profiles',
    hotkeys: 'Keyboard shortcuts',
    'mod-detection': 'Mod Detection',
    'modloader-p2p-connection': 'Modloader P2P connection',
  },
} as const

export type WikiModuleSettings = {
  enabled?: boolean
  itemsPerPage?: number
  refreshIntervalMinutes?: number
}

function boundedInteger(value: number | undefined, fallback: number, minimum: number, maximum: number) {
  return typeof value === 'number' && Number.isFinite(value)
    ? Math.max(minimum, Math.min(maximum, Math.floor(value)))
    : fallback
}

export function resolveWikiApiSettings(settings?: WikiModuleSettings) {
  const refreshIntervalMinutes = boundedInteger(
    settings?.refreshIntervalMinutes,
    WIKI_API_SETTINGS.defaultRefreshIntervalMinutes,
    60,
    1440,
  )
  const itemsPerPage = boundedInteger(
    settings?.itemsPerPage,
    WIKI_API_SETTINGS.defaultItemsPerPage,
    WIKI_PAGE_SIZE_OPTIONS[0],
    WIKI_PAGE_SIZE_OPTIONS[WIKI_PAGE_SIZE_OPTIONS.length - 1],
  )
  return {
    enabled: settings?.enabled === true,
    itemsPerPage: WIKI_PAGE_SIZE_OPTIONS.some((option) => option === itemsPerPage)
      ? itemsPerPage
      : WIKI_API_SETTINGS.defaultItemsPerPage,
    refreshIntervalMs: refreshIntervalMinutes * 60 * 1000,
  }
}
