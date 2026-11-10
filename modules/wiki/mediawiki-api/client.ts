import DOMPurify from 'dompurify'
import { WIKI_API_SETTINGS } from './settings'
import type {
  CachedWikiCategories,
  CachedWikiSearch,
  MediaWikiArticle,
  WikiPageSummary,
  WikiSearchFilters,
  WikiSearchPage,
} from './types'
import { logUiEvent } from '../../logging/ui/log-ui-event'

const WIKI_ORIGIN = WIKI_API_SETTINGS.siteOrigin
const WIKI_API = WIKI_API_SETTINGS.endpoint
const CACHE_KEY = WIKI_API_SETTINGS.cacheKey
const PAGE_LOCALE_SUFFIXES = new Set<string>(WIKI_API_SETTINGS.pageLocaleSuffixes)

type MediaWikiPage = {
  pageid?: number
  ns?: number
  title?: string
  index?: number
  missing?: boolean
  contentmodel?: string
  pagelanguage?: string
  extract?: string
  categories?: Array<{ title?: string }>
  lastrevid?: number
  pageprops?: Record<string, string>
  thumbnail?: { source?: string }
}

type MediaWikiListPage = {
  ns?: number
  pageid?: number
  title?: string
}

type MediaWikiResponse = {
  error?: { code?: string; info?: string }
  query?: {
    pages?: MediaWikiPage[]
    allpages?: MediaWikiListPage[]
    categorymembers?: MediaWikiListPage[]
    search?: MediaWikiListPage[]
    allcategories?: Array<{ category?: string; pages?: number }>
  }
  continue?: Record<string, string>
  parse?: {
    title?: string
    displaytitle?: string
    revid?: number
    text?: string | { '*': string }
  }
}

type CachedArticle = {
  savedAt: number
  article: MediaWikiArticle
}

type WikiCache = {
  searches: Record<string, CachedWikiSearch>
  articles: Record<string, CachedArticle>
  categories?: CachedWikiCategories
}

function emptyCache(): WikiCache {
  return { searches: {}, articles: {} }
}

function readCache(): WikiCache {
  try {
    const value = JSON.parse(window.localStorage.getItem(CACHE_KEY) || 'null')
    if (!value || typeof value !== 'object') return emptyCache()
    return {
      searches: value.searches && typeof value.searches === 'object' ? value.searches : {},
      articles: value.articles && typeof value.articles === 'object' ? value.articles : {},
      categories: value.categories && typeof value.categories === 'object' ? value.categories : undefined,
    }
  } catch {
    return emptyCache()
  }
}

function writeCache(cache: WikiCache) {
  const recent = <T extends { savedAt: number }>(items: Record<string, T>, limit: number) =>
    Object.fromEntries(
      Object.entries(items)
        .sort((left, right) => right[1].savedAt - left[1].savedAt)
        .slice(0, limit),
    )
  const bounded = {
    searches: recent(cache.searches, WIKI_API_SETTINGS.cacheSearchLimit),
    articles: recent(cache.articles, WIKI_API_SETTINGS.cacheArticleLimit),
    categories: cache.categories,
  }
  try {
    let serialized = JSON.stringify(bounded)
    while (serialized.length > WIKI_API_SETTINGS.cacheCharacterLimit && Object.keys(bounded.articles).length > 0) {
      const oldest = Object.entries(bounded.articles)
        .sort((left, right) => left[1].savedAt - right[1].savedAt)[0]?.[0]
      if (!oldest) break
      delete bounded.articles[oldest]
      serialized = JSON.stringify(bounded)
    }
    while (serialized.length > WIKI_API_SETTINGS.cacheCharacterLimit && Object.keys(bounded.searches).length > 0) {
      const oldest = Object.entries(bounded.searches)
        .sort((left, right) => left[1].savedAt - right[1].savedAt)[0]?.[0]
      if (!oldest) break
      delete bounded.searches[oldest]
      serialized = JSON.stringify(bounded)
    }
    window.localStorage.setItem(CACHE_KEY, serialized)
  } catch {
    // The in-memory UI remains usable if WebView storage is unavailable.
  }
}

function searchKey(query: string, locale: string, category = '', language = 'auto') {
  return JSON.stringify([locale, language, category.trim(), query.trim()]
    .map((part) => part.toLocaleLowerCase()))
}

export function readCachedWikiSearch(
  query: string,
  locale: string,
  maximumResults: number = WIKI_API_SETTINGS.defaultMaxResultsPerQuery,
  category = '',
  language = 'auto',
): CachedWikiSearch | undefined {
  const cached = readCache().searches[searchKey(query, locale, category, language)]
  if (!cached) return undefined
  const resultLimit = Math.max(1, Math.min(WIKI_API_SETTINGS.maxApiPageSize, Math.floor(maximumResults)))
  const pages = cached.pages.slice(0, resultLimit)
  return {
    ...cached,
    pages,
    continuation: pages.length >= resultLimit ? undefined : cached.continuation,
  }
}

export function writeCachedWikiSearch(
  query: string,
  locale: string,
  pages: WikiPageSummary[],
  continuation?: Record<string, string>,
  maximumResults: number = WIKI_API_SETTINGS.defaultMaxResultsPerQuery,
  category = '',
  language = 'auto',
) {
  const resultLimit = Math.max(1, Math.min(WIKI_API_SETTINGS.maxApiPageSize, Math.floor(maximumResults)))
  const cache = readCache()
  cache.searches[searchKey(query, locale, category, language)] = {
    savedAt: Date.now(),
    pages: pages.slice(0, resultLimit),
    continuation: pages.length >= resultLimit ? undefined : continuation,
  }
  writeCache(cache)
}

export function readCachedWikiCategories(): CachedWikiCategories | undefined {
  const cached = readCache().categories
  if (!cached || !Array.isArray(cached.categories) || typeof cached.savedAt !== 'number') return undefined
  return cached
}

export async function refreshWikiCategories(signal?: AbortSignal): Promise<CachedWikiCategories> {
  const categories = new Set<string>()
  let continuation: Record<string, string> | undefined
  let batches = 0
  do {
    const params = new URLSearchParams({
      action: 'query',
      list: 'allcategories',
      acmin: '1',
      acprop: 'size',
      aclimit: 'max',
    })
    for (const [key, value] of Object.entries(continuation || {})) params.set(key, value)
    const data = await apiRequest(params, signal)
    for (const item of data.query?.allcategories || []) {
      const category = item.category?.trim()
      if (category) categories.add(category)
    }
    continuation = data.continue
    batches += 1
  } while (continuation && batches < 20 && categories.size < 5000)

  const result = {
    savedAt: Date.now(),
    categories: [...categories].sort((left, right) => left.localeCompare(right)),
  }
  const cache = readCache()
  cache.categories = result
  writeCache(cache)
  return result
}

export function clearWikiCache(): boolean {
  try {
    window.localStorage.removeItem(CACHE_KEY)
    return true
  } catch {
    return false
  }
}

function normalizeWikiLocale(locale: string): string {
  const normalized = locale.toLowerCase()
  if (normalized.startsWith('en')) return 'en-gb'
  return normalized.split('-')[0]
}

function languageSuffix(title: string): string | undefined {
  const match = title.match(/(?:\/|\s\()([a-z]{2,3}(?:-[a-z0-9]+)?)\)?$/i)
  if (!match || !PAGE_LOCALE_SUFFIXES.has(match[1].toLowerCase())) return undefined
  return normalizeWikiLocale(match[1])
}

function titleFamily(title: string): string {
  const suffix = languageSuffix(title)
  const base = suffix
    ? title.replace(/(?:\/|\s\()([a-z]{2,3}(?:-[a-z0-9]+)?)\)?$/i, '')
    : title
  return base.replaceAll('_', ' ').trim().toLocaleLowerCase()
}

function fallbackPageTitle(title: string): string {
  const base = languageSuffix(title)
    ? title.replace(/(?:\/|\s\()([a-z]{2,3}(?:-[a-z0-9]+)?)\)?$/i, '')
    : title
  return base.replaceAll('_', ' ').trim()
}

function displayTitle(value: string | undefined, fallback: string): string {
  if (!value) return fallback
  try {
    return new DOMParser().parseFromString(value, 'text/html').body.textContent?.trim() || fallback
  } catch {
    return value.replace(/<[^>]*>/g, '').trim() || fallback
  }
}

function summaryFromExtract(value: string | undefined): string {
  if (!value) return ''
  const limit = (text: string) => {
    const normalized = text.replace(/\s+/g, ' ').trim()
    return normalized.length > WIKI_API_SETTINGS.summaryCharacters
      ? `${normalized.slice(0, WIKI_API_SETTINGS.summaryCharacters - 1).trimEnd()}…`
      : normalized
  }
  try {
    const parsed = new DOMParser().parseFromString(value, 'text/html')
    const paragraph = [...parsed.querySelectorAll('p')]
      .find((item) => Boolean(item.textContent?.replace(/\s+/g, ' ').trim()))
    const text = paragraph?.textContent || parsed.body.textContent || ''
    return limit(text)
  } catch {
    return limit(value.replace(/<[^>]*>/g, ' '))
  }
}

function mediaWikiPage(page: MediaWikiPage): WikiPageSummary | undefined {
  if (
    page.ns !== 0 || page.missing || page.contentmodel !== 'wikitext' ||
    !page.title
  ) return undefined
  const locale = languageSuffix(page.title) || normalizeWikiLocale(page.pagelanguage || 'en-gb')
  return {
    id: `mediawiki:${page.pageid ?? page.title}`,
    pageTitle: page.title,
    locale,
    title: displayTitle(page.pageprops?.displaytitle, fallbackPageTitle(page.title)),
    summary: summaryFromExtract(page.pageprops?.description) || summaryFromExtract(page.extract),
    categories: page.categories?.map((category) => category.title || '')
      .map((category) => category.replace(/^Category:/i, '').trim())
      .filter(Boolean),
    illustration: page.thumbnail?.source,
    revisionId: page.lastrevid,
    sortIndex: page.index,
  }
}

export function selectWikiLocalePages(
  pages: WikiPageSummary[],
  requestedLocale: string,
  languageFilter = 'auto',
): WikiPageSummary[] {
  if (languageFilter === 'all') return pages
  const automatic = languageFilter === 'auto'
  const preferred = normalizeWikiLocale(automatic ? requestedLocale : languageFilter)
  const allowedLocales = automatic && preferred !== 'en-gb' ? [preferred, 'en-gb'] : [preferred]
  const groups = new Map<string, WikiPageSummary[]>()
  for (const page of pages) {
    const key = titleFamily(page.pageTitle)
    const group = groups.get(key) || []
    group.push(page)
    groups.set(key, group)
  }

  return [...groups.values()].flatMap((group) => {
    const ordered = group.filter((page) => allowedLocales.includes(page.locale)).sort((left, right) => {
      const languageOrder = allowedLocales.indexOf(left.locale) - allowedLocales.indexOf(right.locale)
      if (languageOrder !== 0) return languageOrder
      const leftExplicit = languageSuffix(left.pageTitle) === left.locale ? 0 : 1
      const rightExplicit = languageSuffix(right.pageTitle) === right.locale ? 0 : 1
      if (preferred === 'en-gb') {
        return rightExplicit - leftExplicit || (left.sortIndex ?? 0) - (right.sortIndex ?? 0)
      }
      return leftExplicit - rightExplicit || (left.sortIndex ?? 0) - (right.sortIndex ?? 0)
    })
    return ordered[0] ? [ordered[0]] : []
  })
}

export function mergeWikiPages(current: WikiPageSummary[], incoming: WikiPageSummary[]): WikiPageSummary[] {
  const merged = new Map(current.map((page) => [page.pageTitle.toLowerCase(), page]))
  for (const page of incoming) merged.set(page.pageTitle.toLowerCase(), page)
  return [...merged.values()].sort((left, right) => {
    if (left.sortIndex === undefined && right.sortIndex === undefined) return 0
    if (left.sortIndex === undefined) return 1
    if (right.sortIndex === undefined) return -1
    return left.sortIndex - right.sortIndex
  })
}

export async function refreshWikiPageSummaries(
  pages: WikiPageSummary[],
  signal?: AbortSignal,
): Promise<WikiPageSummary[]> {
  const refreshed: WikiPageSummary[] = []
  for (let offset = 0; offset < pages.length; offset += WIKI_API_SETTINGS.summaryTitleBatchSize) {
    const batch = pages.slice(offset, offset + WIKI_API_SETTINGS.summaryTitleBatchSize)
    const titleKey = (title: string) => title.replaceAll('_', ' ').toLowerCase()
    const sortIndexes = new Map(batch.map((page) => [titleKey(page.pageTitle), page.sortIndex]))
    const params = new URLSearchParams({
      action: 'query',
      titles: batch.map((page) => page.pageTitle).join('|'),
      prop: 'pageimages|extracts|info|pageprops|categories',
      ppprop: 'displaytitle|description',
      piprop: 'thumbnail',
      pilicense: 'any',
      pithumbsize: String(WIKI_API_SETTINGS.thumbnailSize),
      exchars: String(WIKI_API_SETTINGS.extractCharacters),
      clshow: '!hidden',
      cllimit: 'max',
    })
    const data = await apiRequest(params, signal)
    for (const page of data.query?.pages || []) {
      const summary = mediaWikiPage(page)
      if (!summary) continue
      summary.sortIndex = sortIndexes.get(titleKey(summary.pageTitle))
      refreshed.push(summary)
    }
  }
  return refreshed.sort((left, right) =>
    (left.sortIndex ?? Number.MAX_SAFE_INTEGER) - (right.sortIndex ?? Number.MAX_SAFE_INTEGER),
  )
}

const API_RETRY_DELAYS_MS = [500, 1500, 4000] as const
const MAX_RETRY_AFTER_MS = 60_000

function abortReason(signal?: AbortSignal): unknown {
  return signal?.reason ?? new DOMException('The operation was aborted.', 'AbortError')
}

function waitForRetry(milliseconds: number, signal?: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) {
      reject(abortReason(signal))
      return
    }
    const finish = () => {
      signal?.removeEventListener('abort', abort)
      resolve()
    }
    const timer = window.setTimeout(finish, milliseconds)
    const abort = () => {
      window.clearTimeout(timer)
      signal?.removeEventListener('abort', abort)
      reject(abortReason(signal))
    }
    signal?.addEventListener('abort', abort, { once: true })
    if (signal?.aborted) abort()
  })
}

async function waitAndLogForRetry(
  retry: number,
  milliseconds: number,
  reason: string,
  signal?: AbortSignal,
): Promise<void> {
  await waitForRetry(milliseconds, signal)
  if (signal?.aborted) throw abortReason(signal)
  logUiEvent({
    level: 'DEBUG',
    source: 'wiki',
    operation: 'Retry MediaWiki API request',
    subject: 'mediawiki-api',
    message: `Transient ${reason}. Retrying request ${retry + 2} of ${API_RETRY_DELAYS_MS.length + 1} after ${milliseconds} ms.`,
  })
}

function retryAfterMilliseconds(value: string | null): number | undefined {
  if (!value) return undefined
  const seconds = Number(value)
  const date = Number.isFinite(seconds) ? Date.now() + seconds * 1000 : Date.parse(value)
  if (!Number.isFinite(date)) return undefined
  return Math.min(MAX_RETRY_AFTER_MS, Math.max(0, date - Date.now()))
}

async function apiRequest(params: URLSearchParams, signal?: AbortSignal): Promise<MediaWikiResponse> {
  params.set('format', 'json')
  params.set('formatversion', '2')
  params.set('origin', '*')
  for (let retry = 0; ; retry += 1) {
    try {
      if (signal?.aborted) throw abortReason(signal)
      const response = await fetch(`${WIKI_API}?${params}`, {
        signal,
        credentials: 'omit',
        headers: { Accept: 'application/json' },
      })
      if (!response.ok) {
        const retryable = response.status === 408 || response.status === 429 || response.status >= 500
        if (retryable && retry < API_RETRY_DELAYS_MS.length) {
          await waitAndLogForRetry(
            retry,
            retryAfterMilliseconds(response.headers.get('Retry-After')) ?? API_RETRY_DELAYS_MS[retry],
            `HTTP ${response.status}`,
            signal,
          )
          continue
        }
        throw new Error(`MediaWiki returned HTTP ${response.status}`)
      }
      const data = await response.json() as MediaWikiResponse
      if (data.error) {
        const retryable = ['maxlag', 'ratelimited'].includes(data.error.code || '')
        if (retryable && retry < API_RETRY_DELAYS_MS.length) {
          await waitAndLogForRetry(
            retry,
            retryAfterMilliseconds(response.headers.get('Retry-After')) ?? API_RETRY_DELAYS_MS[retry],
            `MediaWiki API ${data.error.code}`,
            signal,
          )
          continue
        }
        throw new Error(data.error.info || 'MediaWiki API request failed')
      }
      return data
    } catch (error) {
      if (signal?.aborted || (error instanceof DOMException && error.name === 'AbortError')) throw error
      const retryable = error instanceof TypeError || error instanceof SyntaxError
      if (!retryable || retry >= API_RETRY_DELAYS_MS.length) throw error
      const reason = error instanceof SyntaxError ? 'invalid JSON response' : 'network error'
      await waitAndLogForRetry(retry, API_RETRY_DELAYS_MS[retry], reason, signal)
    }
  }
}

async function loadPageLeadSummary(pageTitle: string, signal?: AbortSignal): Promise<string> {
  const params = new URLSearchParams({ action: 'parse', page: pageTitle, prop: 'text' })
  const data = await apiRequest(params, signal)
  const parsedText = typeof data.parse?.text === 'string'
    ? data.parse.text
    : data.parse?.text?.['*'] || ''
  return summaryFromExtract(parsedText)
}

async function fillMissingPageSummaries(pages: WikiPageSummary[], signal?: AbortSignal) {
  const missing = pages.filter((page) => !page.summary)
  let next = 0
  const workers = Array.from({ length: Math.min(3, missing.length) }, async () => {
    while (next < missing.length) {
      const page = missing[next++]
      try {
        page.summary = await loadPageLeadSummary(page.pageTitle, signal)
      } catch (error) {
        if (signal?.aborted) throw error
      }
    }
  })
  await Promise.all(workers)
}

export async function searchWikiPages(
  filters: WikiSearchFilters,
  continuation?: Record<string, string>,
  signal?: AbortSignal,
  requestedLimit: number = WIKI_API_SETTINGS.defaultPageSize,
): Promise<WikiSearchPage> {
  const query = filters.query.trim()
  const category = filters.category.trim()
  const limit = Math.max(1, Math.min(WIKI_API_SETTINGS.maxApiPageSize, Math.floor(requestedLimit)))
  const mode = query ? 'search' : category ? 'categorymembers' : 'allpages'
  const matches = new Map<string, WikiPageSummary>()
  let next = continuation
  let batches = 0

  while (batches < 5) {
    const visibleCount = selectWikiLocalePages(
      [...matches.values()], filters.locale, filters.language,
    ).length
    if (visibleCount >= limit) break
    const batchLimit = Math.max(1, Math.min(limit - visibleCount, WIKI_API_SETTINGS.maxApiPageSize))
    const params = new URLSearchParams({ action: 'query', list: mode })
    if (mode === 'search') {
      params.set('srsearch', query)
      params.set('srnamespace', '0')
      params.set('srlimit', String(batchLimit))
    } else if (mode === 'categorymembers') {
      params.set('cmtitle', `Category:${category}`)
      params.set('cmnamespace', '0')
      params.set('cmtype', 'page')
      params.set('cmlimit', String(batchLimit))
    } else {
      params.set('apnamespace', '0')
      params.set('apfilterredir', 'nonredirects')
      params.set('aplimit', String(batchLimit))
    }
    for (const [key, value] of Object.entries(next || {})) params.set(key, value)
    const data = await apiRequest(params, signal)
    const listPages = mode === 'search'
      ? data.query?.search || []
      : mode === 'categorymembers'
        ? data.query?.categorymembers || []
        : data.query?.allpages || []
    const candidates = listPages
      .filter((page) => page.ns === 0 && Boolean(page.title))
      .map((page) => ({
        id: `mediawiki:${page.pageid ?? page.title}`,
        pageTitle: page.title!,
        locale: languageSuffix(page.title!) || 'en-gb',
        title: fallbackPageTitle(page.title!),
        summary: '',
      }))
    const summaries = await refreshWikiPageSummaries(candidates, signal)
    const summariesByTitle = new Map(summaries.map((page) => [page.pageTitle.toLocaleLowerCase(), page]))
    for (const candidate of candidates) {
      const summary = summariesByTitle.get(candidate.pageTitle.toLocaleLowerCase())
      if (!summary) continue
      if (category && !summary.categories?.some((item) => item.toLocaleLowerCase() === category.toLocaleLowerCase())) continue
      summary.sortIndex = undefined
      matches.set(summary.pageTitle.toLocaleLowerCase(), summary)
    }
    const selected = selectWikiLocalePages(
      [...matches.values()], filters.locale, filters.language,
    )
    await fillMissingPageSummaries(selected, signal)
    const previous = JSON.stringify(next)
    next = data.continue
    batches += 1
    if (!next || JSON.stringify(next) === previous || listPages.length === 0) break
  }

  const pages = selectWikiLocalePages(
    [...matches.values()], filters.locale, filters.language,
  ).slice(0, limit)
  return { pages, continuation: next }
}

export async function refreshWikiSearchCache(
  query: string,
  locale: string,
  requestedLimit: number,
  maximumResults: number,
  signal?: AbortSignal,
): Promise<WikiSearchPage> {
  const result = await searchWikiPages({ query, category: '', language: 'auto', locale }, undefined, signal, requestedLimit)
  writeCachedWikiSearch(query, locale, result.pages, result.continuation, maximumResults)
  return result
}

export function readCachedWikiArticle(pageTitle: string): MediaWikiArticle | undefined {
  const entry = readCache().articles[pageTitle.toLowerCase()]
  if (!entry?.article || typeof entry.article.bodyHtml !== 'string') return undefined
  return {
    ...entry.article,
    bodyHtml: sanitizeMediaWikiHtml(entry.article.bodyHtml),
  }
}

export function getCachedWikiArticleRefreshDelay(pageTitle: string, maxAgeMs: number): number | undefined {
  const savedAt = readCache().articles[pageTitle.toLowerCase()]?.savedAt
  return typeof savedAt === 'number' ? Math.max(0, savedAt + maxAgeMs - Date.now()) : undefined
}

function pageTitleCandidates(reference: string, locale: string): string[] {
  const legacyTitle = WIKI_API_SETTINGS.legacyPageTitles[
    reference.toLowerCase() as keyof typeof WIKI_API_SETTINGS.legacyPageTitles
  ]
  const baseTitle = fallbackPageTitle(legacyTitle || reference)
  const preferred = normalizeWikiLocale(locale)
  return preferred === 'en-gb'
    ? [...new Set([baseTitle, `${baseTitle}/en-gb`])]
    : [...new Set([`${baseTitle}/${preferred}`, baseTitle, `${baseTitle}/en-gb`])]
}

export function readCachedWikiArticleForReference(reference: string, locale: string): MediaWikiArticle | undefined {
  for (const pageTitle of pageTitleCandidates(reference, locale)) {
    const cached = readCachedWikiArticle(pageTitle)
    if (cached) return cached
  }
  return undefined
}

function sanitizeMediaWikiHtml(source: string): string {
  const parsed = new DOMParser().parseFromString(source, 'text/html')
  parsed.querySelectorAll('.mw-pt-languages, .mw-editsection, .noprint, .toc').forEach((node) => node.remove())
  parsed.querySelectorAll('p').forEach((paragraph) => {
    const hasAnchorOrMedia = Boolean(paragraph.querySelector('[id], img, video, audio, table, figure'))
    const onlyBreaks = [...paragraph.children].every((child) => child.tagName === 'BR')
    if (!paragraph.textContent?.trim() && !hasAnchorOrMedia && onlyBreaks) paragraph.remove()
  })

  for (const image of parsed.querySelectorAll('img[src], source[src]')) {
    const sourceUrl = image.getAttribute('src')
    try {
      const url = new URL(sourceUrl || '', WIKI_ORIGIN)
      if (url.protocol !== 'https:' || url.origin !== WIKI_ORIGIN) image.remove()
      else image.setAttribute('src', url.href)
    } catch {
      image.remove()
    }
    image.removeAttribute('srcset')
  }

  for (const link of parsed.querySelectorAll('a[href]')) {
    const href = link.getAttribute('href') || ''
    if (href.startsWith('#')) continue
    try {
      const url = new URL(href, WIKI_ORIGIN)
      if (url.protocol === 'https:' || url.protocol === 'http:' || url.protocol === 'mailto:') {
        link.setAttribute('href', url.href)
      } else {
        link.removeAttribute('href')
      }
    } catch {
      link.removeAttribute('href')
    }
  }

  return DOMPurify.sanitize(parsed.body.innerHTML, {
    USE_PROFILES: { html: true },
    FORBID_TAGS: ['script', 'style', 'iframe', 'object', 'embed', 'form', 'input', 'button'],
    FORBID_ATTR: ['style'],
  })
}

function summaryFromPage(page: MediaWikiPage): WikiPageSummary | undefined {
  return mediaWikiPage(page)
}

async function requestPageSummary(
  reference: string,
  locale: string,
  signal?: AbortSignal,
): Promise<WikiPageSummary | undefined> {
  const titles = pageTitleCandidates(reference, locale)
  const params = new URLSearchParams({
    action: 'query',
    titles: titles.join('|'),
    prop: 'pageimages|extracts|info|pageprops|categories',
    ppprop: 'displaytitle|description',
    piprop: 'thumbnail',
    pilicense: 'any',
    pithumbsize: String(WIKI_API_SETTINGS.thumbnailSize),
    exchars: String(WIKI_API_SETTINGS.extractCharacters),
    clshow: '!hidden',
    cllimit: 'max',
    redirects: '1',
  })
  const data = await apiRequest(params, signal)
  const pages = (data.query?.pages || [])
    .map(summaryFromPage)
    .filter((page): page is WikiPageSummary => Boolean(page))
  return selectWikiLocalePages(pages, locale)[0]
}

export async function loadWikiArticle(
  reference: string,
  locale: string,
  knownSummary?: WikiPageSummary,
  signal?: AbortSignal,
  revalidate = false,
): Promise<MediaWikiArticle | undefined> {
  const summary = knownSummary && !revalidate
    ? knownSummary
    : await requestPageSummary(reference, locale, signal)
  if (!summary) return undefined

  const cached = readCachedWikiArticle(summary.pageTitle)
  if (cached && cached.revisionId === summary.revisionId) {
    const refreshed = {
      ...cached,
      id: summary.id,
      pageTitle: summary.pageTitle,
      locale: summary.locale,
      title: summary.title,
      summary: summary.summary || summaryFromExtract(cached.bodyHtml),
      categories: summary.categories,
      illustration: summary.illustration,
      fallback: summary.locale !== normalizeWikiLocale(locale),
    }
    const cache = readCache()
    cache.articles[refreshed.pageTitle.toLowerCase()] = { savedAt: Date.now(), article: refreshed }
    writeCache(cache)
    return refreshed
  }

  const params = new URLSearchParams({
    action: 'parse',
    page: summary.pageTitle,
    prop: 'text|displaytitle|revid',
    redirects: '1',
  })
  const data = await apiRequest(params, signal)
  if (!data.parse) return undefined
  const parsedText = typeof data.parse.text === 'string'
    ? data.parse.text
    : data.parse.text?.['*'] || ''
  const article: MediaWikiArticle = {
    id: summary.id,
    pageTitle: data.parse.title || summary.pageTitle,
    locale: summary.locale,
    title: displayTitle(data.parse.displaytitle, summary.title),
    summary: summary.summary || summaryFromExtract(parsedText),
    categories: summary.categories,
    bodyHtml: sanitizeMediaWikiHtml(parsedText),
    revisionId: data.parse.revid ?? summary.revisionId,
    illustration: summary.illustration,
    fallback: summary.locale !== normalizeWikiLocale(locale),
  }

  const cache = readCache()
  cache.articles[article.pageTitle!.toLowerCase()] = { savedAt: Date.now(), article }
  writeCache(cache)
  return article
}
