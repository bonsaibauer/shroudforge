import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { logUiEvent } from '../../logging/ui/log-ui-event'
import { useI18n } from '../../ui-resources/i18n'
import { Icon } from '../../ui-resources/show-icon'
import { PageHeader } from '../../ui-resources/show-page-header'
import {
  loadWikiArticle,
  mergeWikiPages,
  readCachedWikiArticle,
  readCachedWikiSearch,
  refreshWikiPageSummaries,
  searchWikiPages,
  selectWikiLocalePages,
  writeCachedWikiSearch,
  resolveWikiApiSettings,
  WIKI_API_SETTINGS,
  type MediaWikiArticle,
  type WikiModuleSettings,
  type WikiPageSummary,
} from '../mediawiki-api'
import { WikiArticle, WikiArtwork } from './show-wiki-article'

function isAbort(error: unknown): boolean {
  return error instanceof DOMException && error.name === 'AbortError'
}

export function WikiPage({
  selectedArticleId,
  onSelectArticle,
  moduleSettings,
}: {
  selectedArticleId: string | null
  onSelectArticle(articleId: string | null): void
  moduleSettings?: WikiModuleSettings
}) {
  const { t, locale } = useI18n()
  const apiSettings = resolveWikiApiSettings(moduleSettings)
  const [query, setQuery] = useState('')
  const [listRetryAttempt, setListRetryAttempt] = useState(0)
  const [articleRetryAttempt, setArticleRetryAttempt] = useState(0)
  const [pages, setPages] = useState<WikiPageSummary[]>([])
  const [continuation, setContinuation] = useState<Record<string, string> | undefined>(undefined)
  const [loadingPages, setLoadingPages] = useState(false)
  const [refreshingPages, setRefreshingPages] = useState(false)
  const [hasLoadedPages, setHasLoadedPages] = useState(false)
  const [listFailed, setListFailed] = useState(false)
  const [loadMoreFailed, setLoadMoreFailed] = useState(false)
  const [article, setArticle] = useState<MediaWikiArticle | undefined>(undefined)
  const [articleReference, setArticleReference] = useState<string>()
  const [articleFailedReference, setArticleFailedReference] = useState<string>()
  const [articleRefreshFailedReference, setArticleRefreshFailedReference] = useState<string>()
  const selected = selectedArticleId
  const filter = query.trim()
  const visiblePages = useMemo(() => selectWikiLocalePages(pages, locale), [pages, locale])
  const pagesRef = useRef<WikiPageSummary[]>([])
  const continuationRef = useRef<Record<string, string> | undefined>(undefined)
  const listController = useRef<AbortController | undefined>(undefined)
  const opened = useRef(false)
  const loadingMore = useRef(false)
  const refreshingList = useRef(false)
  const lastArticleFailure = useRef('')

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
    refreshingList.current = false
    setRefreshingPages(false)
    let active = true
    const controller = new AbortController()
    listController.current = controller
    const cached = readCachedWikiSearch(filter, locale, apiSettings.maxResultsPerQuery)
    if (cached) {
      pagesRef.current = cached.pages
      setPages(cached.pages)
      setContinuation(cached.continuation)
      continuationRef.current = cached.continuation
      setHasLoadedPages(true)
    } else {
      pagesRef.current = []
      setPages([])
      setContinuation(undefined)
      continuationRef.current = undefined
      setHasLoadedPages(false)
    }
    setListFailed(false)
    setLoadMoreFailed(false)
    setLoadingPages(true)
    const firstPageLimit = Math.min(
      apiSettings.pageSize,
      apiSettings.maxResultsPerQuery,
    )

    const timer = window.setTimeout(() => {
      void searchWikiPages(filter, undefined, controller.signal, firstPageLimit)
        .then((result) => {
          if (!active) return
          pagesRef.current = result.pages
          setPages(result.pages)
          setContinuation(result.continuation)
          continuationRef.current = result.continuation
          setHasLoadedPages(true)
          setListFailed(false)
          writeCachedWikiSearch(filter, locale, result.pages, result.continuation, apiSettings.maxResultsPerQuery)
        })
        .catch((error: unknown) => {
          if (!active || isAbort(error)) return
          setHasLoadedPages(true)
          setListFailed(true)
          const hasCachedResults = Boolean(cached?.pages.length)
          logUiEvent({ level: hasCachedResults ? 'WARN' : 'ERROR', source: 'wiki', operation: 'Search MediaWiki',
            subject: filter || 'all-pages', message: String(error), notify: !hasCachedResults,
            notifyMessage: hasCachedResults ? undefined : t('wiki.loadFailed') })
        })
        .finally(() => {
          if (active) setLoadingPages(false)
        })
    }, filter ? WIKI_API_SETTINGS.searchDebounceMs : 0)

    const refreshList = () => {
      if (loadingMore.current || refreshingList.current || controller.signal.aborted) return
      refreshingList.current = true
      setRefreshingPages(true)
      void searchWikiPages(filter, undefined, controller.signal, firstPageLimit)
        .then(async (result) => {
          if (!active) return
          const refreshedTitles = new Set(result.pages.map((page) => page.pageTitle.toLowerCase()))
          const loadedTail = pagesRef.current.filter((page) =>
            page.id.startsWith('mediawiki:') && !refreshedTitles.has(page.pageTitle.toLowerCase()),
          )
          const hasLoadedTail = loadedTail.length > 0
          const refreshedTail = await refreshWikiPageSummaries(loadedTail, controller.signal)
          if (!active) return
          const next = mergeWikiPages(result.pages, refreshedTail).slice(0, apiSettings.maxResultsPerQuery)
          pagesRef.current = next
          setPages(next)
          const nextContinuation = next.length >= apiSettings.maxResultsPerQuery
            ? undefined
            : hasLoadedTail ? continuationRef.current : result.continuation
          setContinuation(nextContinuation)
          continuationRef.current = nextContinuation
          setListFailed(false)
          writeCachedWikiSearch(filter, locale, next, nextContinuation, apiSettings.maxResultsPerQuery)
        })
        .catch((error: unknown) => {
          if (!active || isAbort(error)) return
          setListFailed(true)
          logUiEvent({ level: 'WARN', source: 'wiki', operation: 'Refresh MediaWiki pages',
            subject: filter || 'all-pages', message: String(error) })
        })
        .finally(() => {
          refreshingList.current = false
          if (active) setRefreshingPages(false)
        })
    }
    const interval = window.setInterval(refreshList, apiSettings.refreshIntervalMs)

    return () => {
      active = false
      window.clearTimeout(timer)
      controller.abort()
      refreshingList.current = false
      if (listController.current === controller) listController.current = undefined
      window.clearInterval(interval)
    }
  }, [apiSettings.enabled, apiSettings.maxResultsPerQuery, apiSettings.pageSize, apiSettings.refreshIntervalMs, filter, listRetryAttempt, locale, selected, t])

  const loadMore = useCallback(async () => {
    const controller = listController.current
    if (!apiSettings.enabled || !continuation || loadingMore.current || refreshingList.current || loadingPages || selected || !controller || controller.signal.aborted) return
    const remaining = apiSettings.maxResultsPerQuery - pagesRef.current.length
    if (remaining <= 0) {
      setContinuation(undefined)
      continuationRef.current = undefined
      return
    }
    loadingMore.current = true
    setLoadMoreFailed(false)
    setLoadingPages(true)
    logUiEvent({ level: 'DEBUG', source: 'wiki', operation: 'Load more MediaWiki pages',
      subject: filter || 'all-pages', message: `Requested more results. Current count: ${pagesRef.current.length}.` })
    try {
      const result = await searchWikiPages(
        filter,
        continuation,
        controller.signal,
        Math.min(apiSettings.pageSize, remaining),
      )
      const next = mergeWikiPages(pagesRef.current, result.pages).slice(0, apiSettings.maxResultsPerQuery)
      pagesRef.current = next
      setPages(next)
      const nextContinuation = next.length >= apiSettings.maxResultsPerQuery
        ? undefined
        : result.continuation
      setContinuation(nextContinuation)
      continuationRef.current = nextContinuation
      writeCachedWikiSearch(filter, locale, next, nextContinuation, apiSettings.maxResultsPerQuery)
    } catch (error) {
      if (!isAbort(error)) {
        setLoadMoreFailed(true)
        logUiEvent({ level: 'WARN', source: 'wiki', operation: 'Load more MediaWiki pages',
          subject: filter || 'all-pages', message: String(error), notify: true,
          notifyMessage: t('wiki.loadFailed') })
      }
    } finally {
      loadingMore.current = false
      if (!controller.signal.aborted) setLoadingPages(false)
    }
  }, [apiSettings.enabled, apiSettings.maxResultsPerQuery, apiSettings.pageSize, continuation, filter, loadingPages, locale, selected, t])

  useEffect(() => {
    if (!selected || !apiSettings.enabled) {
      setArticle(undefined)
      setArticleReference(undefined)
      setArticleFailedReference(undefined)
      setArticleRefreshFailedReference(undefined)
      return
    }

    let active = true
    const controller = new AbortController()
    const articleKey = `${selected}:${locale}`
    const known = pages.find((page) => page.pageTitle === selected && page.id.startsWith('mediawiki:'))
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

    const refresh = async (revalidate = false) => {
      try {
        const remote = await loadWikiArticle(selected, locale, known, controller.signal, revalidate)
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
          if (lastArticleFailure.current !== articleKey) {
            lastArticleFailure.current = articleKey
            logUiEvent({ level: 'WARN', source: 'wiki', operation: 'Load MediaWiki article',
              subject: selected, message: 'MediaWiki did not return an article for this title.', notify: true,
              notifyMessage: t('wiki.loadFailed') })
          }
        } else {
          setArticleRefreshFailedReference(selected)
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
        if (lastArticleFailure.current !== articleKey) {
          lastArticleFailure.current = articleKey
          logUiEvent({ level: hasCachedArticle ? 'WARN' : 'ERROR', source: 'wiki', operation: 'Load MediaWiki article',
            subject: selected, message: String(error), notify: !hasCachedArticle,
            notifyMessage: hasCachedArticle ? undefined : t('wiki.loadFailed') })
        }
      }
    }

    void refresh()
    const interval = window.setInterval(() => void refresh(true), apiSettings.refreshIntervalMs)
    return () => {
      active = false
      controller.abort()
      window.clearInterval(interval)
    }
  }, [apiSettings.enabled, apiSettings.refreshIntervalMs, selected, locale, articleRetryAttempt, t])

  const currentArticle = selected && articleReference === `${selected}:${locale}` ? article : undefined
  const searchKey = `${locale}:${filter}:${visiblePages.length}`
  const lastSearch = useRef('')
  useEffect(() => {
    if (selected || lastSearch.current === searchKey) return
    lastSearch.current = searchKey
    logUiEvent({ level: 'TRACE', source: 'wiki', operation: 'Find articles', subject: 'article-list',
      message: `${visiblePages.length} articles shown. Search length: ${filter.length}. Language: ${locale}.` })
  }, [filter, locale, searchKey, selected, visiblePages.length])

  function selectArticle(pageTitle: string | null) {
    if (!apiSettings.enabled) return
    logUiEvent({ level: 'DEBUG', source: 'wiki', operation: pageTitle ? 'Open article' : 'Return to wiki',
      subject: pageTitle || selected || 'article-list', message: pageTitle ? 'Article selected.' : 'Article list opened.' })
    onSelectArticle(pageTitle)
  }

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
    <PageHeader eyebrow={t('nav.modloader')} title={t('nav.wiki')} subtitle={t('wiki.subtitle')} />
    {!apiSettings.enabled ? <p className="builder-notice warning" role="status">{t('settings.moduleUnavailable')}</p>
      : selected ? currentArticle ? <>
      <WikiArticle article={currentArticle} onOpenWikiArticle={(pageTitle) => selectArticle(pageTitle)}>
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
          value={query} onChange={(event) => setQuery(event.currentTarget.value)} />
      </label>
      <div className="wiki-article-list" aria-busy={loadingPages || refreshingPages}>
        {visiblePages.map((item) => <button type="button" className="wiki-article-link" key={item.pageTitle}
          onClick={() => selectArticle(item.pageTitle)}>
          <WikiArtwork article={item} />
          <span><strong>{item.title}</strong><small>{item.summary}</small></span>
          <Icon name="wiki" />
        </button>)}
      </div>
      {continuation && <div className="wiki-load-more">
        <button type="button" className="button ghost" disabled={loadingPages || refreshingPages}
          onClick={() => void loadMore()}>{t('wiki.loadMore')}</button>
      </div>}
      {loadMoreFailed && <p className="hint" role="alert">{t('wiki.loadFailed')}</p>}
      {listFailed && <><p className="hint" role="alert">{t('wiki.loadFailed')}</p>
        <button type="button" className="button ghost" onClick={retryList}>{t('updater.retry')}</button></>}
      {!listFailed && hasLoadedPages && visiblePages.length === 0 && <p className="hint">{t('wiki.empty')}</p>}
    </>}
  </>
}
