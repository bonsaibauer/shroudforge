import { useEffect, useMemo, useRef, useState } from 'react'
import { logUiEvent } from '../../logging/ui/log-ui-event'
import { useI18n } from '../../ui-resources/i18n'
import { Icon } from '../../ui-resources/show-icon'
import { PageHeader } from '../../ui-resources/show-page-header'
import { postRequest } from '../../transport/ui/send-host-command'
import type { CommandResult } from '../../notifications/ui/notification-types'
import {
  DEFAULT_WIKI_CATEGORY,
  loadWikiArticle,
  getCachedWikiArticleRefreshDelay,
  readCachedWikiCategories,
  readCachedWikiArticle,
  readCachedWikiSearch,
  refreshWikiCategories,
  searchAllWikiPages,
  selectWikiLocalePages,
  writeCachedWikiSearch,
  resolveWikiApiSettings,
  WIKI_API_SETTINGS,
  WIKI_PAGE_SIZE_OPTIONS,
  type MediaWikiArticle,
  type WikiModuleSettings,
  type WikiPageSummary,
} from '../mediawiki-api'
import { WikiArticle, WikiArtwork } from './show-wiki-article'
import { WikiActions } from './show-wiki-actions'

function isAbort(error: unknown): boolean {
  return error instanceof DOMException && error.name === 'AbortError'
}

export function WikiPage({
  selectedArticleId,
  category,
  onSelectArticle,
  onOpenCategory,
  moduleSettings,
  notifyCommandResult,
}: {
  selectedArticleId: string | null
  category: string
  onSelectArticle(articleId: string | null): void
  onOpenCategory(category: string): void
  moduleSettings?: WikiModuleSettings
  notifyCommandResult(result: CommandResult): void
}) {
  const { t, locale, locales } = useI18n()
  const apiSettings = resolveWikiApiSettings(moduleSettings)
  const [query, setQuery] = useState('')
  const [languageFilter, setLanguageFilter] = useState('auto')
  const [itemsPerPage, setItemsPerPage] = useState(apiSettings.itemsPerPage)
  const [currentPage, setCurrentPage] = useState(1)
  const [categories, setCategories] = useState(() => readCachedWikiCategories()?.categories || [])
  const [listRetryAttempt, setListRetryAttempt] = useState(0)
  const [articleRetryAttempt, setArticleRetryAttempt] = useState(0)
  const [pages, setPages] = useState<WikiPageSummary[]>([])
  const [loadingPages, setLoadingPages] = useState(false)
  const [refreshingPages, setRefreshingPages] = useState(false)
  const [hasLoadedPages, setHasLoadedPages] = useState(false)
  const [listFailed, setListFailed] = useState(false)
  const [article, setArticle] = useState<MediaWikiArticle | undefined>(undefined)
  const [articleReference, setArticleReference] = useState<string>()
  const [articleFailedReference, setArticleFailedReference] = useState<string>()
  const [articleRefreshFailedReference, setArticleRefreshFailedReference] = useState<string>()
  const selected = selectedArticleId
  const filter = query.trim()
  const selectedCategoryOption = categories.find(
    (item) => item.toLocaleLowerCase() === category.toLocaleLowerCase(),
  ) ?? category
  const activeCategory = selectedCategoryOption
  const visiblePages = useMemo(
    () => selectWikiLocalePages(pages, locale, languageFilter),
    [pages, locale, languageFilter],
  )
  const pageCount = Math.max(1, Math.ceil(visiblePages.length / itemsPerPage))
  const displayedPage = Math.min(currentPage, pageCount)
  const pageStart = (displayedPage - 1) * itemsPerPage
  const pageItems = visiblePages.slice(pageStart, pageStart + itemsPerPage)
  const manualRefresh = useRef<(() => Promise<void>) | undefined>(undefined)
  const handledListRetryAttempt = useRef(0)
  const handledArticleRetryAttempt = useRef(0)
  const opened = useRef(false)
  const refreshingList = useRef(false)
  const lastArticleFailure = useRef('')

  useEffect(() => {
    setItemsPerPage(apiSettings.itemsPerPage)
  }, [apiSettings.itemsPerPage])

  useEffect(() => {
    setCurrentPage(1)
  }, [activeCategory, filter, languageFilter, locale])

  useEffect(() => {
    if (!apiSettings.enabled) return
    let active = true
    const controller = new AbortController()
    const cached = readCachedWikiCategories()
    if (cached) setCategories(cached.categories)
    let interval: number | undefined
    const refresh = async () => {
      try {
        const result = await refreshWikiCategories(controller.signal)
        if (active) setCategories(result.categories)
      } catch (error) {
        if (active && !isAbort(error)) {
          logUiEvent({ level: 'WARN', source: 'wiki', operation: 'Load MediaWiki categories',
            subject: 'categories', message: String(error) })
        }
      }
    }
    const delay = cached
      ? Math.max(0, cached.savedAt + apiSettings.refreshIntervalMs - Date.now())
      : 0
    const timer = window.setTimeout(() => {
      void refresh()
      interval = window.setInterval(refresh, apiSettings.refreshIntervalMs)
    }, delay)
    return () => {
      active = false
      controller.abort()
      window.clearTimeout(timer)
      if (interval !== undefined) window.clearInterval(interval)
    }
  }, [apiSettings.enabled, apiSettings.refreshIntervalMs])

  useEffect(() => {
    if (opened.current) return
    opened.current = true
    logUiEvent({ level: 'DEBUG', source: 'wiki', operation: 'Open wiki', subject: 'article-list',
      message: apiSettings.enabled
        ? `Wiki opened in ${locale}.`
        : `Wiki opened in ${locale} while its module is disabled. No MediaWiki request was made.` })
  }, [apiSettings.enabled, locale])

  useEffect(() => {
    if (selected || !apiSettings.enabled) {
      refreshingList.current = false
      setRefreshingPages(false)
      return
    }
    const forceRefresh = listRetryAttempt !== handledListRetryAttempt.current
    handledListRetryAttempt.current = listRetryAttempt
    refreshingList.current = false
    setRefreshingPages(false)
    let active = true
    const controller = new AbortController()
    const cached = readCachedWikiSearch(
      filter, locale, activeCategory, languageFilter,
    )
    const cacheIsFresh = Boolean(cached && !forceRefresh &&
      Date.now() - cached.savedAt < apiSettings.refreshIntervalMs)
    if (cached) {
      setPages(cached.pages)
      setHasLoadedPages(true)
    } else {
      setPages([])
      setHasLoadedPages(false)
    }
    setListFailed(false)
    setLoadingPages(!cacheIsFresh)

    const timer = cacheIsFresh ? undefined : window.setTimeout(() => {
      void searchAllWikiPages(
        { query: filter, category: activeCategory, language: languageFilter, locale },
        controller.signal,
      )
        .then((result) => {
          if (!active) return
          setPages(result.pages)
          setHasLoadedPages(true)
          setListFailed(false)
          writeCachedWikiSearch(
            filter, locale, result.pages, activeCategory, languageFilter,
          )
        })
        .catch((error: unknown) => {
          if (!active || isAbort(error)) return
          setHasLoadedPages(true)
          setListFailed(true)
          const hasCachedResults = Boolean(cached?.pages.length)
          logUiEvent({ level: hasCachedResults ? 'WARN' : 'ERROR', source: 'wiki', operation: 'Search MediaWiki',
            subject: activeCategory || filter || 'all-pages', message: String(error), notify: !hasCachedResults,
            notifyMessage: hasCachedResults ? undefined : t('wiki.loadFailed') })
        })
        .finally(() => {
          if (active) setLoadingPages(false)
        })
    }, filter ? WIKI_API_SETTINGS.searchDebounceMs : 0)

    const refreshList = async (manual = false) => {
      if (refreshingList.current || controller.signal.aborted) {
        if (manual) throw new Error(t('wiki.loadFailed'))
        return
      }
      refreshingList.current = true
      setRefreshingPages(true)
      try {
        const result = await searchAllWikiPages(
          { query: filter, category: activeCategory, language: languageFilter, locale },
          controller.signal,
        )
        if (!active) return
        setPages(result.pages)
        setHasLoadedPages(true)
        setListFailed(false)
        writeCachedWikiSearch(filter, locale, result.pages, activeCategory, languageFilter)
      } catch (error) {
        if (!active || isAbort(error)) return
        setListFailed(true)
        if (manual) throw error
        logUiEvent({ level: 'WARN', source: 'wiki', operation: 'Refresh MediaWiki pages',
          subject: activeCategory || filter || 'all-pages', message: String(error) })
      } finally {
        refreshingList.current = false
        if (active) setRefreshingPages(false)
      }
    }
    const refreshManually = () => refreshList(true)
    manualRefresh.current = refreshManually
    let interval: number | undefined
    const refreshDelayMs = cacheIsFresh && cached
      ? Math.max(0, cached.savedAt + apiSettings.refreshIntervalMs - Date.now())
      : apiSettings.refreshIntervalMs
    const refreshTimer = window.setTimeout(() => {
      refreshList()
      interval = window.setInterval(refreshList, apiSettings.refreshIntervalMs)
    }, refreshDelayMs)

    return () => {
      active = false
      if (timer !== undefined) window.clearTimeout(timer)
      window.clearTimeout(refreshTimer)
      controller.abort()
      refreshingList.current = false
      if (manualRefresh.current === refreshManually) manualRefresh.current = undefined
      if (interval !== undefined) window.clearInterval(interval)
    }
  }, [apiSettings.enabled, apiSettings.refreshIntervalMs, activeCategory, filter, languageFilter, listRetryAttempt, locale, selected, t])

  useEffect(() => {
    if (!selected || !apiSettings.enabled) {
      setArticle(undefined)
      setArticleReference(undefined)
      setArticleFailedReference(undefined)
      setArticleRefreshFailedReference(undefined)
      return
    }
    const forceRefresh = articleRetryAttempt !== handledArticleRetryAttempt.current
    handledArticleRetryAttempt.current = articleRetryAttempt

    let active = true
    const controller = new AbortController()
    const known = pages.find((page) => page.pageTitle === selected && page.id.startsWith('mediawiki:'))
    const articleLocale = languageFilter === 'auto'
      ? locale
      : languageFilter === 'all'
        ? known?.locale || locale
        : languageFilter
    const articleKey = `${selected}:${locale}:${languageFilter}`
    const cached = known ? readCachedWikiArticle(known.pageTitle) : readCachedWikiArticle(selected)
    let hasCachedArticle = Boolean(cached)
    lastArticleFailure.current = ''
    if (cached) {
      setArticle(cached)
      setArticleReference(articleKey)
    } else {
      setArticle(undefined)
      setArticleReference(undefined)
    }
    setArticleFailedReference(undefined)
    setArticleRefreshFailedReference(undefined)

    const refresh = async (revalidate = false, manual = false) => {
      try {
        const remote = await loadWikiArticle(selected, articleLocale, known, controller.signal, revalidate)
        if (!active) return
        if (remote) {
          hasCachedArticle = true
          setArticle(remote)
          setArticleReference(articleKey)
          setArticleFailedReference(undefined)
          setArticleRefreshFailedReference(undefined)
          lastArticleFailure.current = ''
        } else if (!hasCachedArticle) {
          setArticleFailedReference(selected)
          if (manual) throw new Error(t('wiki.loadFailed'))
          if (lastArticleFailure.current !== articleKey) {
            lastArticleFailure.current = articleKey
            logUiEvent({ level: 'WARN', source: 'wiki', operation: 'Load MediaWiki article',
              subject: selected, message: 'MediaWiki did not return an article for this title.', notify: true,
              notifyMessage: t('wiki.loadFailed') })
          }
        } else {
          setArticleRefreshFailedReference(selected)
          if (manual) throw new Error(t('wiki.loadFailed'))
          if (lastArticleFailure.current !== articleKey) {
            lastArticleFailure.current = articleKey
            logUiEvent({ level: 'WARN', source: 'wiki', operation: 'Refresh MediaWiki article',
              subject: selected, message: 'MediaWiki did not return the article. Cached content remains visible.' })
          }
        }
      } catch (error) {
        if (!active || isAbort(error)) return
        if (!hasCachedArticle) setArticleFailedReference(selected)
        else setArticleRefreshFailedReference(selected)
        if (manual) throw error
        if (lastArticleFailure.current !== articleKey) {
          lastArticleFailure.current = articleKey
          logUiEvent({ level: hasCachedArticle ? 'WARN' : 'ERROR', source: 'wiki', operation: 'Load MediaWiki article',
            subject: selected, message: String(error), notify: !hasCachedArticle,
            notifyMessage: hasCachedArticle ? undefined : t('wiki.loadFailed') })
        }
      }
    }
    const refreshManually = () => refresh(true, true)
    manualRefresh.current = refreshManually

    const cachedRefreshDelay = cached
      ? getCachedWikiArticleRefreshDelay(cached.pageTitle, apiSettings.refreshIntervalMs)
      : undefined
    const cachedFresh = Boolean(cached && !forceRefresh && cachedRefreshDelay &&
      (!known || cached.revisionId === known.revisionId))
    if (!cachedFresh || forceRefresh) void refresh(forceRefresh)
    let interval: number | undefined
    const refreshDelayMs = cachedFresh && cachedRefreshDelay !== undefined
      ? cachedRefreshDelay
      : apiSettings.refreshIntervalMs
    const refreshTimer = window.setTimeout(() => {
      void refresh(true)
      interval = window.setInterval(() => void refresh(true), apiSettings.refreshIntervalMs)
    }, refreshDelayMs)
    return () => {
      active = false
      controller.abort()
      window.clearTimeout(refreshTimer)
      if (interval !== undefined) window.clearInterval(interval)
      if (manualRefresh.current === refreshManually) manualRefresh.current = undefined
    }
  }, [apiSettings.enabled, apiSettings.refreshIntervalMs, selected, locale, languageFilter, pages, articleRetryAttempt, t])

  const currentArticle = selected && articleReference === `${selected}:${locale}:${languageFilter}` ? article : undefined
  const searchKey = `${locale}:${languageFilter}:${activeCategory}:${filter}:${visiblePages.length}`
  const lastSearch = useRef('')
  useEffect(() => {
    if (selected || lastSearch.current === searchKey) return
    lastSearch.current = searchKey
    logUiEvent({ level: 'TRACE', source: 'wiki', operation: 'Find articles', subject: 'article-list',
      message: `${visiblePages.length} articles shown. Search length: ${filter.length}. Category: ${category || 'all'}. Language: ${languageFilter}.` })
  }, [filter, locale, searchKey, selected, visiblePages.length])

  function selectArticle(pageTitle: string | null) {
    if (!apiSettings.enabled) return
    logUiEvent({ level: 'DEBUG', source: 'wiki', operation: pageTitle ? 'Open article' : 'Return to wiki',
      subject: pageTitle || selected || 'article-list', message: pageTitle ? 'Article selected.' : 'Article list opened.' })
    onSelectArticle(pageTitle)
  }

  function selectCategoryFilter(nextCategory: string) {
    onOpenCategory(nextCategory)
  }

  function selectArticleCategory(nextCategory: string) {
    onOpenCategory(nextCategory)
    setQuery('')
  }

  function changeItemsPerPage(value: string) {
    const next = Number(value)
    if (!WIKI_PAGE_SIZE_OPTIONS.some((option) => option === next)) return
    setItemsPerPage(next)
    setCurrentPage(1)
    postRequest('save-setting', {
      scope: 'module',
      module: 'wiki',
      key: 'itemsPerPage',
      value: next,
    })
  }

  function changeQuery(nextQuery: string) {
    const hadSearch = Boolean(filter)
    const hasSearch = Boolean(nextQuery.trim())
    if (!hadSearch && hasSearch && category) {
      onOpenCategory('')
    } else if (hadSearch && !hasSearch && !category) {
      onOpenCategory(DEFAULT_WIKI_CATEGORY)
    }
    setQuery(nextQuery)
  }

  const languageOptions = [...new Map(locales.map((option) => {
    const normalized = option.locale.toLowerCase().startsWith('en')
      ? 'en-gb'
      : option.locale.toLowerCase().split('-')[0]
    return [normalized, { value: normalized, label: option.name }] as const
  })).values()]

  function retryList() {
    logUiEvent({ level: 'DEBUG', source: 'wiki', operation: 'Retry MediaWiki search',
      subject: filter || 'all-pages', message: 'User requested a search retry.' })
    setListRetryAttempt((attempt) => attempt + 1)
  }

  function retryArticle() {
    if (!selected) return
    logUiEvent({ level: 'DEBUG', source: 'wiki', operation: 'Retry MediaWiki article',
      subject: selected, message: 'User requested an article retry.' })
    setArticleRetryAttempt((attempt) => attempt + 1)
  }

  return <>
    <PageHeader eyebrow={t('nav.modloader')} title={t('nav.wiki')} subtitle={t('wiki.subtitle')}
      actions={<WikiActions
        notifyCommandResult={notifyCommandResult}
        refreshDisabled={!apiSettings.enabled || (!selected && (loadingPages || refreshingPages))}
        onRefresh={() => manualRefresh.current?.() || Promise.reject(new Error(t('wiki.loadFailed')))}
      />} />
    {!apiSettings.enabled ? <p className="builder-notice warning" role="status">{t('settings.moduleUnavailable')}</p>
      : selected ? currentArticle ? <>
      <WikiArticle article={currentArticle} onOpenWikiArticle={(pageTitle) => selectArticle(pageTitle)}
        onSelectCategory={selectArticleCategory}>
        <div className="feature-banner-actions"><button type="button" className="button ghost"
          onClick={() => selectArticle(null)}>{t('wiki.back')}</button></div>
      </WikiArticle>
      {articleRefreshFailedReference === selected && <>
        <p className="hint" role="status">{t('wiki.loadFailed')}</p>
        <button type="button" className="button ghost" onClick={retryArticle}>{t('updater.retry')}</button>
      </>}
    </> : articleFailedReference === selected
      ? <><p className="hint" role="alert">{t('wiki.unavailable')}</p>
        <button type="button" className="button ghost" onClick={retryArticle}>{t('updater.retry')}</button></>
      : null : <>
      <label className="searchbar wiki-search"><Icon name="search" />
        <input type="search" aria-label={t('wiki.search')} placeholder={t('wiki.search')}
          value={query} onChange={(event) => changeQuery(event.currentTarget.value)} />
      </label>
      <div className="wiki-search-filters">
        <label>{t('wiki.filterCategory')}
          <select value={selectedCategoryOption} onChange={(event) => selectCategoryFilter(event.currentTarget.value)}>
            <option value="">{t('wiki.allCategories')}</option>
            {category && selectedCategoryOption === category && !categories.includes(category) &&
              <option value={category}>{category}</option>}
            {categories.map((item) => <option value={item} key={item}>{item}</option>)}
          </select>
        </label>
        <label>{t('wiki.filterLanguage')}
          <select value={languageFilter} onChange={(event) => setLanguageFilter(event.currentTarget.value)}>
            <option value="auto">{t('wiki.languageAutomatic')}</option>
            <option value="all">{t('wiki.languageAll')}</option>
            {languageOptions.map((option) => <option value={option.value} key={option.value}>{option.label}</option>)}
          </select>
        </label>
      </div>
      <p className="wiki-result-count" aria-live="polite">{t('wiki.resultsCount', { count: visiblePages.length })}</p>
      <div className="wiki-article-list" aria-busy={loadingPages || refreshingPages}>
        {pageItems.map((item) => <button type="button" className="wiki-article-link" key={item.pageTitle}
          onClick={() => selectArticle(item.pageTitle)}>
          <WikiArtwork article={item} />
          <span><strong>{item.title}</strong>{item.summary && <small>{item.summary}</small>}</span>
          <Icon name="wiki" />
        </button>)}
      </div>
      {hasLoadedPages && visiblePages.length > 0 && <nav className="wiki-pagination"
        aria-label={t('wiki.pagination')}>
        <label className="wiki-page-size">
          <span>{t('wiki.itemsPerPage')}</span>
          <select value={itemsPerPage} onChange={(event) => changeItemsPerPage(event.currentTarget.value)}>
            {WIKI_PAGE_SIZE_OPTIONS.map((option) => <option value={option} key={option}>{option}</option>)}
          </select>
        </label>
        <div className="wiki-page-controls">
          <button type="button" className="button ghost" aria-label={t('wiki.previousPage')}
            disabled={displayedPage <= 1 || loadingPages || refreshingPages}
            onClick={() => setCurrentPage(displayedPage - 1)}>‹</button>
          <span aria-live="polite">{t('wiki.pageIndicator', { current: displayedPage, total: pageCount })}</span>
          <button type="button" className="button ghost" aria-label={t('wiki.nextPage')}
            disabled={displayedPage >= pageCount || loadingPages || refreshingPages}
            onClick={() => setCurrentPage(displayedPage + 1)}>›</button>
        </div>
      </nav>}
      {listFailed && <><p className="hint" role="alert">{t('wiki.loadFailed')}</p>
        <button type="button" className="button ghost" onClick={retryList}>{t('updater.retry')}</button></>}
      {!listFailed && hasLoadedPages && visiblePages.length === 0 && <p className="hint">{t('wiki.empty')}</p>}
    </>}
  </>
}
