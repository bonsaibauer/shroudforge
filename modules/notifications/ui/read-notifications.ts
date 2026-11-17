import { postRequest } from '../../transport/ui/send-host-command'
import { useMemo } from 'react'
import type { Dispatch, SetStateAction } from 'react'
import { useI18n } from '../../ui-resources/i18n'

import type {
  NewsItem,
  NewsDisplaySource,
  NewsPersonalization,
} from './notification-types'
import { buildNewsItems } from './build-news-items'
import { sortNewsItems } from './sort-news-items'

export function useNotifications(
  data: NewsDisplaySource,
  settings: NewsPersonalization,
  target: 'client' | 'server',
  read: string[],
  setRead: Dispatch<SetStateAction<Record<'client' | 'server', string[]>>>,
) {
  const { t } = useI18n()
  const news = useMemo<NewsItem[]>(
    () => buildNewsItems(data, settings, t),
    [data, t, settings.effectiveUsername, settings.messageNameFallback],
  )
  const unread = news.filter((item) => !read.includes(item.id))
  const errorCount = unread.filter((item) => item.level === 'error').length
  const warningCount = unread.filter((item) => item.level === 'warn').length
  const updateCount = unread.filter(
    (item) =>
      item.id.startsWith('system-') || item.kind === 'mod.update.success',
  ).length
  const chirperSeverity = errorCount
    ? 'error'
    : warningCount
      ? 'warning'
      : unread.length >= 5
        ? 'many'
        : updateCount
          ? 'update'
          : unread.length
            ? 'info'
            : 'quiet'
  const chirperTitle = errorCount
    ? t('header.chirper.errors', { count: errorCount })
    : warningCount
      ? t('header.chirper.warnings', { count: warningCount })
      : t('header.chirper.unread', { count: unread.length })
  const chirperItems = sortNewsItems(unread)

  const markRead = (ids: string[], isRead = true) => {
    const next = isRead
      ? Array.from(new Set([...read, ...ids]))
      : read.filter((id) => !ids.includes(id))
    setRead((current) => ({ ...current, [target]: next }))
    postRequest('set-news-read', { ids, read: isRead, target })
  }
  return { news, unread, chirperSeverity, chirperTitle, chirperItems, markRead }
}
