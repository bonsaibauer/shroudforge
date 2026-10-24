import { useEffect, useRef, useState } from 'react'
import { logUiEvent } from '../../logging/ui/log-ui-event'
import { useI18n } from '../../ui-resources/i18n'
import { Icon } from '../../ui-resources/show-icon'
import { PageHeader } from '../../ui-resources/show-page-header'
import { readWikiArticle, readWikiArticles } from './read-wiki-articles'
import { WikiArticle, WikiArtwork } from './show-wiki-article'

export function WikiPage({
  selectedArticleId,
  onSelectArticle,
}: {
  selectedArticleId: string | null
  onSelectArticle(articleId: string | null): void
}) {
  const { t, locale } = useI18n()
  const [query, setQuery] = useState('')
  const selected = selectedArticleId
  const article = selected ? readWikiArticle(selected, locale) : undefined
  const filter = query.trim().toLocaleLowerCase()
  const articles = readWikiArticles(locale).filter((item) =>
    `${item.title} ${item.summary} ${item.body}`.toLocaleLowerCase().includes(filter),
  )
  const opened = useRef(false)
  useEffect(() => {
    if (opened.current) return
    opened.current = true
    logUiEvent({ level: 'DEBUG', source: 'wiki', operation: 'Open wiki', subject: 'article-list',
      message: `Wiki opened in ${locale}.` })
  }, [locale])
  const lastSearch = useRef('')
  useEffect(() => {
    if (selected) return
    const search = `${locale}:${filter}:${articles.length}`
    if (lastSearch.current === search) return
    lastSearch.current = search
    logUiEvent({ level: 'TRACE', source: 'wiki', operation: 'Find articles', subject: 'article-list',
      message: `${articles.length} articles shown. Search length: ${filter.length}. Language: ${locale}.` })
  }, [locale, filter, articles.length, selected])
  const missingArticle = useRef('')
  useEffect(() => {
    if (!selected || article || missingArticle.current === `${selected}:${locale}`) return
    missingArticle.current = `${selected}:${locale}`
    logUiEvent({ level: 'ERROR', source: 'wiki', operation: 'Load article', subject: selected,
      message: t('wiki.unavailable'), notify: true })
  }, [selected, locale, Boolean(article), t])
  function selectArticle(id: string | null) {
    logUiEvent({ level: 'DEBUG', source: 'wiki', operation: id ? 'Open article' : 'Return to wiki',
      subject: id || selected || 'article-list', message: id ? 'Article selected.' : 'Article list opened.' })
    onSelectArticle(id)
  }
  return <>
    <PageHeader eyebrow={t('nav.modloader')} title={t('nav.wiki')} subtitle={t('wiki.subtitle')} />
    {article ? <WikiArticle article={article} onOpenWikiArticle={(id) => selectArticle(id)}>
      <div className="feature-banner-actions"><button type="button" className="button ghost"
        onClick={() => selectArticle(null)}>{t('wiki.back')}</button></div>
    </WikiArticle> : <>
      {selected && <p className="hint" role="alert">{t('wiki.unavailable')}</p>}
      <label className="searchbar wiki-search"><Icon name="search" />
        <input type="search" aria-label={t('wiki.search')} placeholder={t('wiki.search')}
          value={query} onChange={(event) => setQuery(event.currentTarget.value)} />
      </label>
      <div className="wiki-article-list">
        {articles.map((item) => <button type="button" className="wiki-article-link" key={item.id}
          onClick={() => selectArticle(item.id)}>
          <WikiArtwork article={item} />
          <span><strong>{item.title}</strong><small>{item.summary}</small></span>
          <Icon name="wiki" />
        </button>)}
      </div>
      {articles.length === 0 && <p className="hint">{t('wiki.empty')}</p>}
    </>}
  </>
}
