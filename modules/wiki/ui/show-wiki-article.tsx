import { Children, Component, isValidElement, useEffect, useId, useRef, useState, type ReactNode } from 'react'
import Markdown, { defaultUrlTransform } from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { logUiEvent } from '../../logging/ui/log-ui-event'
import { postRequest } from '../../transport/ui/send-host-command'
import { useI18n } from '../../ui-resources/i18n'
import { FeatureBanner } from '../../ui-resources/show-feature-banner'
import { Icon } from '../../ui-resources/show-icon'
import { wikiImage, type WikiArticle as Article } from './read-wiki-articles'
import './wiki.css'

function textContent(children: ReactNode): string {
  return Children.toArray(children).map((child): string => {
    if (typeof child === 'string' || typeof child === 'number') return String(child)
    return isValidElement<{ children?: ReactNode }>(child) ? textContent(child.props.children) : ''
  }).join('')
}

function headingId(value: string): string {
  return value.toLowerCase().normalize('NFKD').replace(/[\u0300-\u036f]/g, '')
    .replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '')
}

class ArticleBoundary extends Component<{
  articleId: string
  fallback: string
  children: ReactNode
}, { failed: boolean }> {
  state = { failed: false }
  static getDerivedStateFromError() { return { failed: true } }
  componentDidCatch(error: Error) {
    logUiEvent({ level: 'ERROR', source: 'wiki', operation: 'Display article',
      subject: this.props.articleId, message: `${this.props.fallback} ${error.message}`, notify: true })
  }
  render() {
    return this.state.failed ? <p role="alert">{this.props.fallback}</p> : this.props.children
  }
}

export function WikiArtwork({ article }: { article: Article }) {
  const [failedSource, setFailedSource] = useState<string>()
  if (!article.illustration || failedSource === article.illustration) return <Icon name="wiki" />
  return <img src={article.illustration} alt="" onError={() => {
    setFailedSource(article.illustration)
    logUiEvent({ level: 'WARN', source: 'wiki', operation: 'Load illustration', subject: article.id,
      message: 'The article illustration could not be displayed. The book icon is shown instead.' })
  }} />
}

export function WikiArticle({ article, expanded = true, children, onOpenWikiArticle }: {
  article: Article
  expanded?: boolean
  children?: ReactNode
  onOpenWikiArticle?(articleId: string): void
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
    logUiEvent({ level: 'WARN', source: 'wiki', operation: 'Select translation', subject: article.id,
      message: `The requested translation is unavailable. Showing ${article.locale}.` })
  }, [article.id, article.locale, article.fallback])
  return (
    <>
      <FeatureBanner variant="info" brand="SHROUDFORGE WIKI" title={article.title}
        titleId={`${id}-title`} description={article.summary} badge={t('wiki.guide')}
        media={<WikiArtwork article={article} />}>
        {children}
      </FeatureBanner>
      {expanded && <article className="wiki-article" aria-labelledby={`${id}-title`}>
        {article.fallback && <p className="hint">{t('wiki.fallback', { language: article.locale.toUpperCase() })}</p>}
        <ArticleBoundary key={`${article.id}:${article.locale}`} articleId={article.id} fallback={t('wiki.unavailable')}>
        <Markdown remarkPlugins={[remarkGfm]} skipHtml
          urlTransform={(url, key) => key === 'src' ? wikiImage(article.id, url) : defaultUrlTransform(url)}
          components={{
            h2: ({ children }) => <h2 id={`${id}-${headingId(textContent(children))}`}>{children}</h2>,
            h3: ({ children }) => <h3 id={`${id}-${headingId(textContent(children))}`}>{children}</h3>,
            a: ({ href, children }) => {
              const wikiArticle = href?.match(/^#wiki\/([a-z0-9]+(?:-[a-z0-9]+)*)$/)
              if (wikiArticle) return <a href={href} onClick={(event) => {
                event.preventDefault()
                onOpenWikiArticle?.(wikiArticle[1])
              }}>{children}</a>
              if (href?.startsWith('#')) return <a href={`#${id}-${href.slice(1)}`}>{children}</a>
              return <a href={href} onClick={(event) => {
                event.preventDefault()
                if (href && /^(https?:|mailto:)/i.test(href)) {
                  logUiEvent({ level: 'DEBUG', source: 'wiki', operation: 'Open article link',
                    subject: article.id, message: `Opening ${href}` })
                  postRequest('open-url', { url: href })
                }
              }}>{children}</a>
            },
          }}>{article.body}</Markdown>
        </ArticleBoundary>
      </article>}
    </>
  )
}
