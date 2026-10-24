import type { TranslationKey, useI18n } from '../../ui-resources/i18n'
import type {
  NewsItem,
  NewsPersonalization,
  NewsDisplaySource,
} from './notification-types'

function releaseChangelog(message?: string): string[] {
  if (!message) return []
  const lines = message.split(/\r?\n/).map((line) => line.trim())
  const isChangelogHeading = (line: string) =>
    line.replace(/^#+/, '').trim().toLowerCase() === 'changelog:'
  const hasChangelogHeading = lines.some(isChangelogHeading)
  let inChangelog = !hasChangelogHeading
  return lines
    .flatMap((line) => {
      if (isChangelogHeading(line)) {
        inChangelog = true
        return []
      }
      if (/^\**\s*full changelog/i.test(line)) {
        inChangelog = false
        return []
      }
      if (!inChangelog || !line) return []
      const entry = line.replace(/^[-*+]\s+/, '').trim()
      return entry ? [entry.slice(0, 2000)] : []
    })
    .slice(0, 50)
}

export function buildNewsItems(
  data: NewsDisplaySource,
  settings: NewsPersonalization,
  t: ReturnType<typeof useI18n>['t'],
): NewsItem[] {
  const items: NewsItem[] = []
  const username =
    (settings.effectiveUsername || settings.messageNameFallback).trim() ||
    'Flameborn'
  const personalize = (text: string) => text.replace(/\{username\}/g, username)
  const configured = (kind: string, values: Record<string, string> = {}) => {
    const template = data.newsTemplates?.[kind]
    if (!template) return {}
    const render = (text: string) =>
      text.replace(/\{(\w+)\}/g, (match, key) => values[key] ?? match)
    return { title: render(template.title), message: render(template.message) }
  }
  const updaterAvailable = data.release.updaterAvailable === true
  const updatesEnabled = data.updatesEnabled === true
  const standardUpdateAvailable =
    updaterAvailable && updatesEnabled && data.release.updateAvailable
  const fallbackUpdateAvailable =
    !standardUpdateAvailable &&
    (data.releaseAnnouncement?.updateAvailable === true ||
      data.release.updateAvailable)
  if (standardUpdateAvailable)
    items.push({
      id: `system-${data.release.latestVersion}`,
      level: 'info',
      title: t('news.system.update.title', {
        version: data.release.latestVersion || '',
      }),
      message: data.release.message || t('news.system.update.message'),
      actionUrl: data.release.releaseUrl,
      actions: [{ id: 'queue-system-update', kind: 'queue-system-update' }],
      updatedAt: 0,
      changelog: releaseChangelog(data.release.message),
    })
  else if (fallbackUpdateAvailable) {
    const version =
      data.releaseAnnouncement?.latestVersion || data.release.latestVersion || ''
    items.push({
      id: `system-${version}`,
      level: 'warn',
      title: t('news.system.updaterMissing.title', { version }),
      message: !updaterAvailable
        ? t('news.system.updaterMissing.message', { version })
        : !updatesEnabled
          ? t('updates.availability.disabled.description')
          : t('news.system.update.message'),
      actions: [{ id: 'open-updates', kind: 'open-updates' }],
      updatedAt: 0,
      changelog:
        data.releaseAnnouncement?.changelog?.length
          ? data.releaseAnnouncement.changelog
          : releaseChangelog(data.release.message),
    })
  }
  if (standardUpdateAvailable && data.release.staged)
    items.push({
      id: `system-staged-${data.release.latestVersion}`,
      level: 'info',
      title: t('news.system.staged.title'),
      message: t('news.system.staged.message'),
      updatedAt: 0,
    })
  for (const item of items) {
    if (item.actions?.some((action) => action.kind === 'open-updates')) continue
    const kind = item.id.startsWith('system-staged-')
      ? 'system.staged'
      : 'system.update'
    item.title = t(`news.${kind}.title` as TranslationKey, {
      version: data.release.latestVersion || '',
    })
    item.message = t(`news.${kind}.message` as TranslationKey, {
      version: data.release.latestVersion || '',
    })
  }
  items.push(
    ...data.notices.map((item) => {
      if (!item.kind)
        return {
          ...item,
          title: personalize(item.title),
          message: personalize(item.message),
        }
      const values = { ...(item.values || {}), username }
      if (item.kind.startsWith('notice.'))
        return {
          ...item,
          title: t(`news.${item.kind}.title` as TranslationKey, values),
          message: t(`news.${item.kind}.message` as TranslationKey, values),
        }
      if (item.kind === 'mod.install.success')
        return {
          ...item,
          title: t('news.mod.install.success.title', values),
          message: t('news.mod.install.success.message', values),
          changelog: item.changelog,
        }
      if (item.kind === 'mod.update.success')
        return {
          ...item,
          title: t('news.mod.update.success.title', values),
          message: t('news.mod.update.success.message', values),
          changelog: item.changelog,
        }
      if (item.kind === 'mod.remove.success')
        return {
          ...item,
          title: t('news.mod.remove.success.title', values),
          message: t('news.mod.remove.success.message', values),
          changelog: item.changelog,
        }
      if (data.newsTemplates?.[item.kind])
        return {
          ...item,
          ...configured(item.kind, values as Record<string, string>),
        }
      return { ...item }
    }),
  )
  return items
    .map((item) => ({
      ...item,
      ecosystem:
        item.ecosystem ||
        data.mods.find((mod) => mod.id === item.modId)?.ecosystem,
      beta:
        item.beta ??
        (item.kind ? data.newsTemplates?.[item.kind]?.beta : undefined) ??
        false,
    }))
    .sort((a, b) => b.updatedAt - a.updatedAt)
}
