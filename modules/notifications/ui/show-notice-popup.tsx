import { useI18n } from '../../ui-resources/i18n'
import type { NewsItem, NoticeAction } from './notification-types'
import { Empty } from '../../ui-resources/show-empty-state'
import { NoticeCard } from './show-notice-card'
export function Chirper({
  items,
  close,
  all,
  markRead,
  runNoticeAction,
  openWikiArticle,
}: {
  items: NewsItem[]
  close(): void
  all(): void
  markRead(ids: string[]): void
  runNoticeAction(action: NoticeAction): void
  openWikiArticle(articleId: string): void
}) {
  const { t } = useI18n()
  const errors = items.filter((item) => item.level === 'error').length
  const warnings = items.filter((item) => item.level === 'warn').length
  return (
    <div
      className="overlay"
      onMouseDown={(e) => e.target === e.currentTarget && close()}
    >
      <section className="modal chirper-modal">
        <header>
          <div className="chirper-heading">
            <small>CHIRPER</small>
            <h2>{t('news.chirper.title')}</h2>
            <div className="chirper-summary">
              <span>
                {t('news.chirper.summary.unread', { count: items.length })}
              </span>
              {errors > 0 && (
                <b className="error">
                  🚨 {t('news.chirper.summary.errors', { count: errors })}
                </b>
              )}
              {warnings > 0 && (
                <b className="warning">
                  ⚠️ {t('news.chirper.summary.warnings', { count: warnings })}
                </b>
              )}
            </div>
          </div>
          <button onClick={close}>×</button>
        </header>
        <div className="modal-body">
          {items.length ? (
            <div className="compact-news">
              {items.map((item) => (
                <NoticeCard
                  key={item.id}
                  item={item}
                  compact
                  unread
                  onAction={runNoticeAction}
                  onRead={() => markRead([item.id])}
                  onWikiArticle={openWikiArticle}
                />
              ))}
            </div>
          ) : (
            <Empty
              title={t('news.noneUnread.title')}
              text={t('news.noneUnread.text')}
              icon="chirper"
            />
          )}
        </div>
        <footer>
          {items.length ? (
            <button
              className="button ghost"
              onClick={() => markRead(items.map((item) => item.id))}
            >
              {t('news.markAllRead')}
            </button>
          ) : (
            <span />
          )}
          <button className="button" onClick={all}>
            {t('news.title')}
          </button>
        </footer>
      </section>
    </div>
  )
}
