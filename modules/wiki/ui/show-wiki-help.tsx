import { useEffect, useRef, useState } from 'react'
import { logUiEvent } from '../../logging/ui/log-ui-event'
import { useI18n } from '../../ui-resources/i18n'
import { Icon } from '../../ui-resources/show-icon'
import {
  loadWikiArticle,
  readCachedWikiArticleForReference,
  resolveWikiApiSettings,
  type MediaWikiArticle,
  type WikiModuleSettings,
} from '../mediawiki-api'
import { WikiArticle } from './show-wiki-article'

function isAbort(error: unknown): boolean {
  return error instanceof DOMException && error.name === 'AbortError'
}

export function WikiHelp({ articleId, compact = false, moduleSettings }: {
  articleId: string
  compact?: boolean
  moduleSettings?: WikiModuleSettings
}) {
  const { t, locale } = useI18n()
  const apiSettings = resolveWikiApiSettings(moduleSettings)
  const [visible, setVisible] = useState(!compact)
  const [expanded, setExpanded] = useState(false)
  const [article, setArticle] = useState<MediaWikiArticle | undefined>(undefined)
  const [articleReference, setArticleReference] = useState<string | undefined>(undefined)
  const [failed, setFailed] = useState(false)
  const [refreshFailed, setRefreshFailed] = useState(false)
  const [retryAttempt, setRetryAttempt] = useState(0)
  const lastFailure = useRef('')
  const disabledLogged = useRef(false)
  useEffect(() => {
    if (!apiSettings.enabled) {
      if (!disabledLogged.current) {
        disabledLogged.current = true
        logUiEvent({ level: 'DEBUG', source: 'wiki', operation: 'Skip help', subject: articleId,
          message: 'Wiki module is disabled. Help content was not requested.' })
      }
      setArticle(undefined)
      setArticleReference(undefined)
      setFailed(false)
      setRefreshFailed(false)
      return
    }
    disabledLogged.current = false
    let active = true
    const controller = new AbortController()
    const cached = readCachedWikiArticleForReference(articleId, locale)
    let hasCachedArticle = Boolean(cached)
    lastFailure.current = ''
    setArticle(cached)
    setArticleReference(cached ? `${articleId}:${locale}` : undefined)
    setFailed(false)
    setRefreshFailed(false)

    const refresh = async (revalidate = false) => {
      try {
        const remote = await loadWikiArticle(articleId, locale, undefined, controller.signal, revalidate)
        if (!active) return
        if (remote) {
          hasCachedArticle = true
          setArticle(remote)
          setArticleReference(`${articleId}:${locale}`)
          setFailed(false)
          setRefreshFailed(false)
          lastFailure.current = ''
        } else if (!hasCachedArticle) {
          setFailed(true)
          if (lastFailure.current !== `${articleId}:${locale}`) {
            lastFailure.current = `${articleId}:${locale}`
            logUiEvent({ level: 'WARN', source: 'wiki', operation: 'Load help',
              subject: articleId, message: 'MediaWiki did not return an article for this title.', notify: true,
              notifyMessage: t('wiki.loadFailed') })
          }
        } else {
          setRefreshFailed(true)
          if (lastFailure.current !== `${articleId}:${locale}`) {
            lastFailure.current = `${articleId}:${locale}`
            logUiEvent({ level: 'WARN', source: 'wiki', operation: 'Refresh help',
              subject: articleId, message: 'MediaWiki did not return the article. Cached content remains visible.' })
          }
        }
      } catch (error) {
        if (!active || isAbort(error)) return
        if (!hasCachedArticle) setFailed(true)
        else setRefreshFailed(true)
        if (lastFailure.current !== `${articleId}:${locale}`) {
          lastFailure.current = `${articleId}:${locale}`
          logUiEvent({ level: hasCachedArticle ? 'WARN' : 'ERROR', source: 'wiki', operation: 'Load help',
            subject: articleId, message: String(error), notify: !hasCachedArticle,
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
  }, [apiSettings.enabled, apiSettings.refreshIntervalMs, articleId, locale, retryAttempt, t])
  function showHelp(nextVisible: boolean, nextExpanded: boolean) {
    logUiEvent({ level: 'DEBUG', source: 'wiki', operation: 'Change help view', subject: articleId,
      message: !nextVisible ? 'Help banner hidden.' : nextExpanded ? 'Full article opened.' : 'Article collapsed to its banner.' })
    setVisible(nextVisible)
    setExpanded(nextExpanded)
  }
  function retryHelp() {
    logUiEvent({ level: 'DEBUG', source: 'wiki', operation: 'Retry help',
      subject: articleId, message: 'User requested a help article retry.' })
    setRetryAttempt((attempt) => attempt + 1)
  }
  const currentArticle = articleReference === `${articleId}:${locale}` ? article : undefined
  if (!apiSettings.enabled) return null
  if (!currentArticle) return failed ? <>
    <p className="hint" role="alert">{t('wiki.unavailable')}</p>
    <button type="button" className="button ghost" onClick={retryHelp}>{t('updater.retry')}</button>
  </> : null
  if (!visible) return <button type="button" className="button ghost wiki-help-trigger"
    aria-label={t('wiki.openGuide', { title: currentArticle.title })} title={t('wiki.openGuide', { title: currentArticle.title })}
    onClick={() => showHelp(true, true)}>
    <Icon name="wiki" />{!compact && t('wiki.showGuide')}
  </button>
  return <section className="wiki-help">
    <WikiArticle article={currentArticle} expanded={expanded}>
      <div className="feature-banner-actions">
        <button type="button" className="button ghost" aria-expanded={expanded}
          onClick={() => showHelp(true, !expanded)}><Icon name="wiki" />
          {t(expanded ? 'wiki.collapseGuide' : 'wiki.showGuide')}
        </button>
        <button type="button" className="button ghost" onClick={() => showHelp(false, expanded)}>{t('wiki.hideGuide')}</button>
      </div>
    </WikiArticle>
    {refreshFailed && <>
      <p className="hint" role="status">{t('wiki.loadFailed')}</p>
      <button type="button" className="button ghost" onClick={retryHelp}>{t('updater.retry')}</button>
    </>}
  </section>
}
