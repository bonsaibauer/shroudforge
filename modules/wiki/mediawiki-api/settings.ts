const WIKI_SITE_ORIGIN = 'https://wiki.shroudforge.net'

export const WIKI_API_SETTINGS = {
  siteOrigin: WIKI_SITE_ORIGIN,
  endpoint: `${WIKI_SITE_ORIGIN}/api.php`,
  cacheKey: 'sf-wiki-cache-v1',
  defaultPageSize: 30,
  defaultMaxResultsPerQuery: 120,
  defaultRefreshIntervalMinutes: 5,
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
  },
} as const

export type WikiModuleSettings = {
  enabled?: boolean
  pageSize?: number
  maxResultsPerQuery?: number
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
    1,
    1440,
  )
  return {
    enabled: settings?.enabled === true,
    pageSize: boundedInteger(settings?.pageSize, WIKI_API_SETTINGS.defaultPageSize, 1, 100),
    maxResultsPerQuery: boundedInteger(
      settings?.maxResultsPerQuery,
      WIKI_API_SETTINGS.defaultMaxResultsPerQuery,
      1,
      WIKI_API_SETTINGS.maxApiPageSize,
    ),
    refreshIntervalMs: refreshIntervalMinutes * 60 * 1000,
  }
}
