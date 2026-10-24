import { useState } from 'react'
import { useI18n } from '../../ui-resources/i18n'
import type { NewsItem, NoticeAction } from './notification-types'
import { Empty } from '../../ui-resources/show-empty-state'
import { PageHeader } from '../../ui-resources/show-page-header'
import { Segmented } from '../../ui-resources/choose-segment'
import { NoticeCard } from './show-notice-card'
import { sortNewsItems } from './sort-news-items'
export function NewsPage({
  items,
  unread,
  read,
  markRead,
  runNoticeAction,
  openWikiArticle,
}: {
  items: NewsItem[]
  unread: NewsItem[]
  read: string[]
  markRead(ids: string[], isRead?: boolean): void
  runNoticeAction(action: NoticeAction): void
  openWikiArticle(articleId: string): void
}) {
  const { t } = useI18n()
  const [tab, setTab] = useState('unread')
  const shown =
    tab === 'unread'
      ? unread
      : tab === 'read'
        ? items.filter((item) => read.includes(item.id))
        : items
  const ordered = sortNewsItems(shown)
  return (
    <>
      <PageHeader
        eyebrow={t('nav.modloader')}
        title={t('news.title')}
        subtitle={t('news.subtitle')}
        actions={
          unread.length ? (
            <button
              className="button ghost"
              onClick={() => markRead(unread.map((item) => item.id))}
            >
              {t('news.markAllRead')}
            </button>
          ) : undefined
        }
      />
      <Segmented
        value={tab}
        onChange={setTab}
        items={[
          ['unread', t('news.unread', { count: unread.length })],
          ['all', t('news.all')],
          ['read', t('news.read')],
        ]}
      />
      <div className="news-list">
        {ordered.length ? (
          ordered.map((item) => (
            <NoticeCard
              key={item.id}
              item={item}
              unread={!read.includes(item.id)}
              onRead={() => markRead([item.id], !read.includes(item.id))}
              onAction={runNoticeAction}
              onWikiArticle={openWikiArticle}
            />
          ))
        ) : (
          <Empty
            title={t('news.empty.title')}
            text={t('news.empty.text')}
            icon="chirper"
          />
        )}
      </div>
    </>
  )
}
