import { postRequest } from '../../transport/ui/send-host-command'
import { useEffect, useMemo, useRef, useState } from 'react'
import type { Dispatch, SetStateAction } from 'react'
import { useI18n } from '../../ui-resources/i18n'

import type {
  ChirperMotion,
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
  notificationReady: boolean,
) {
  const { t } = useI18n()
  const news = useMemo<NewsItem[]>(
    () => buildNewsItems(data, settings, t),
    [data, t, settings.effectiveUsername, settings.messageNameFallback],
  )
  const unread = useMemo(
    () => news.filter((item) => !read.includes(item.id)),
    [news, read],
  )
  const [chirperMotion, setChirperMotion] = useState<ChirperMotion | null>(null)
  const previousUnread = useRef<{
    target: 'client' | 'server'
    entries: Map<string, string>
  } | null>(null)
  const motionRevision = useRef(0)
  const unreadSignature = useMemo(
    () =>
      JSON.stringify(
        unread
          .map((item) => [item.id, item.updatedAt, item.level, item.kind ?? ''] as const)
          .sort(([leftId], [rightId]) => leftId.localeCompare(rightId)),
      ),
    [unread],
  )
  useEffect(() => {
    if (!notificationReady) {
      previousUnread.current = null
      setChirperMotion(null)
      return
    }

    const motionItems = JSON.parse(unreadSignature) as Array<
      [string, number, string, string]
    >
    const entries = new Map(
      motionItems.map(([id, updatedAt, level, kind]) => [
        id,
        JSON.stringify([updatedAt, level, kind]),
      ]),
    )
    const previous = previousUnread.current
    if (!previous || previous.target !== target) {
      previousUnread.current = { target, entries }
      setChirperMotion(null)
      return
    }

    const arriving = motionItems.filter(
      (item) => previous.entries.get(item[0]) !== entries.get(item[0]),
    )
    const cleared = arriving.length === 0 && entries.size < previous.entries.size
    previousUnread.current = { target, entries }
    if (!arriving.length && !cleared) return

    const kind: ChirperMotion['kind'] = cleared
      ? 'cleared'
      : arriving.some((item) => item[2] === 'error')
        ? 'error'
        : arriving.some((item) => item[2] === 'warn')
          ? 'warning'
          : arriving.some(
                ([id, , , itemKind]) =>
                  id.startsWith('system-') || itemKind === 'mod.update.success',
              )
            ? 'update'
            : 'arrival'
    const revision = ++motionRevision.current
    setChirperMotion({ kind, revision })
    const duration =
      kind === 'error'
        ? 1500
        : kind === 'warning' || kind === 'update'
          ? 1250
          : kind === 'cleared'
            ? 650
            : 950
    const timeout = window.setTimeout(() => {
      setChirperMotion((current) =>
        current?.revision === revision ? null : current,
      )
    }, duration)
    return () => window.clearTimeout(timeout)
  }, [notificationReady, target, unreadSignature])
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
  return {
    news,
    unread,
    chirperSeverity,
    chirperTitle,
    chirperItems,
    chirperMotion,
    markRead,
  }
}
