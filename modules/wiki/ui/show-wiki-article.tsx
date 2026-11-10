import { Component, useEffect, useId, useRef, useState, type MouseEvent as ReactMouseEvent, type ReactNode } from 'react'
import { logUiEvent } from '../../logging/ui/log-ui-event'
import { postRequest } from '../../transport/ui/send-host-command'
import { useI18n } from '../../ui-resources/i18n'
import { FeatureBanner } from '../../ui-resources/show-feature-banner'
import { WIKI_API_SETTINGS, type MediaWikiArticle } from '../mediawiki-api'
import fallbackArtwork from './wiki-article-fallback.svg'
import './wiki.css'

class ArticleBoundary extends Component<{
  articleId: string
  fallback: string
  children: ReactNode
}, { failed: boolean }> {
  state = { failed: false }
  static getDerivedStateFromError() { return { failed: true } }
  componentDidCatch(error: Error) {
    logUiEvent({ level: 'ERROR', source: 'wiki', operation: 'Display article',
      subject: this.props.articleId, message: error.message, notify: true,
      notifyMessage: this.props.fallback })
  }
  render() {
    return this.state.failed ? <p role="alert">{this.props.fallback}</p> : this.props.children
  }
}

export function WikiArtwork({ article }: { article: Pick<MediaWikiArticle, 'id' | 'illustration'> }) {
  const [failedSource, setFailedSource] = useState<string>()
  if (!article.illustration || failedSource === article.illustration) return <img src={fallbackArtwork} alt="" />
  return <img src={article.illustration} alt="" onError={() => {
    setFailedSource(article.illustration)
    logUiEvent({ level: 'WARN', source: 'wiki', operation: 'Load illustration', subject: article.id,
      message: 'The article illustration could not be displayed. The book icon is shown instead.' })
  }} />
}

function mediaWikiPageTitle(url: URL): string | undefined {
  if (url.origin !== WIKI_API_SETTINGS.siteOrigin) return undefined
  let title = url.searchParams.get('title')
  if (!title && url.pathname.startsWith('/index.php/')) {
    try {
      title = decodeURIComponent(url.pathname.slice('/index.php/'.length))
    } catch {
      return undefined
    }
  }
  if (!title) return undefined
  title = title.replaceAll('_', ' ').trim()
  if (/^(special|category|file|image|template|help|mediawiki|talk|user|module):/i.test(title)) return undefined
  return title
}

function openMediaWikiLink(
  event: ReactMouseEvent<HTMLDivElement>,
  articleId: string,
  onOpenWikiArticle?: (pageTitle: string) => void,
) {
  const target = event.target
  if (!(target instanceof Element)) return
  const anchor = target.closest('a[href]')
  if (!(anchor instanceof HTMLAnchorElement)) return
  const href = anchor.getAttribute('href')
  if (!href || href.startsWith('#')) return

  event.preventDefault()
  let url: URL
  try {
    url = new URL(href, WIKI_API_SETTINGS.siteOrigin)
  } catch {
    return
  }
  const pageTitle = mediaWikiPageTitle(url)
  if (pageTitle) {
    if (onOpenWikiArticle) onOpenWikiArticle(pageTitle)
    else postRequest('open-url', { url: url.href })
    return
  }
  if (/^(https?:|mailto:)/i.test(url.href)) {
    logUiEvent({ level: 'DEBUG', source: 'wiki', operation: 'Open article link',
      subject: articleId, message: `Opening ${url.href}` })
    postRequest('open-url', { url: url.href })
  }
}

export function WikiArticle({ article, expanded = true, children, onOpenWikiArticle, onSelectCategory }: {
  article: MediaWikiArticle
  expanded?: boolean
  children?: ReactNode
  onOpenWikiArticle?(articleId: string): void
  onSelectCategory?(category: string): void
}) {
  const { t } = useI18n()
  const id = useId()
  const lastDisplay = useRef('')
  useEffect(() => {
    const display = `${article.id}:${article.locale}:${expanded}`
    if (lastDisplay.current === display) return
    lastDisplay.current = display
    logUiEvent({ level: 'TRACE', source: 'wiki', operation: 'Display article', subject: article.id,
      message: `Language: ${article.locale}. View: ${expanded ? 'full article' : 'banner'}.` })
  }, [article.id, article.locale, expanded])
  const lastFallback = useRef('')
  useEffect(() => {
    if (!article.fallback) return
    const fallback = `${article.id}:${article.locale}`
    if (lastFallback.current === fallback) return
    lastFallback.current = fallback
    logUiEvent({ level: 'INFO', source: 'wiki', operation: 'Select translation', subject: article.id,
      message: `The requested translation is unavailable. Showing ${article.locale}.` })
  }, [article.id, article.locale, article.fallback])
  return (
    <>
      <FeatureBanner variant="info" brand="SHROUDFORGE WIKI" title={article.title}
        titleId={`${id}-title`} description={article.summary}
        badge={article.categories && article.categories.length > 0
          ? <div className="wiki-category-badges" aria-label={t('wiki.categories')}>
            {article.categories.map((category) => onSelectCategory
              ? <button type="button" className="wiki-article-category" key={category}
                onClick={() => onSelectCategory(category)}>{category}</button>
              : <span className="wiki-article-category" key={category}>{category}</span>)}
          </div>
          : null}
        media={<WikiArtwork article={article} />}>
        {children}
      </FeatureBanner>
      {expanded && <article className="wiki-article" aria-labelledby={`${id}-title`}>
        {article.fallback && <p className="hint">{t('wiki.fallback', { language: article.locale.toUpperCase() })}</p>}
        <ArticleBoundary key={`${article.id}:${article.locale}`} articleId={article.id} fallback={t('wiki.unavailable')}>
        <div className="wiki-mediawiki-content"
          onClick={(event) => openMediaWikiLink(event, article.id, onOpenWikiArticle)}
          dangerouslySetInnerHTML={{ __html: article.bodyHtml }} />
        </ArticleBoundary>
      </article>}
    </>
  )
}
