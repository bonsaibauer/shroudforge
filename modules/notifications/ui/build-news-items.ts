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
  const render = (
    text: string,
    values: Record<string, string | number>,
  ) =>
    text.replace(/\{(\w+)\}/g, (match, key) => String(values[key] ?? match))
  const englishOnlyFor = (kind: string, itemValue?: boolean) =>
    itemValue ?? data.newsTemplates?.[kind]?.englishOnly ?? false
  const translateText = (
    key: string,
    sourceText: string,
    values: Record<string, string | number>,
    englishOnly: boolean,
  ) => {
    const translationKey = `news.${key}` as TranslationKey
    const translated = t(
      translationKey,
      values,
      englishOnly ? 'en-GB' : undefined,
    )
    return translated === translationKey ? render(sourceText, values) : translated
  }
  const translateCard = (
    kind: string,
    sourceTitle: string,
    sourceMessage: string,
    values: Record<string, string | number>,
    englishOnly: boolean,
  ) => ({
    title: translateText(`${kind}.title`, sourceTitle, values, englishOnly),
    message: translateText(`${kind}.message`, sourceMessage, values, englishOnly),
  })
  const updaterAvailable = data.release.updaterAvailable === true
  const updatesEnabled = data.updatesEnabled === true
  const standardUpdateAvailable =
    updaterAvailable && updatesEnabled && data.release.updateAvailable
  const fallbackUpdateAvailable =
    !standardUpdateAvailable &&
    (data.releaseAnnouncement?.updateAvailable === true ||
      data.release.updateAvailable)
  if (standardUpdateAvailable) {
    const kind = 'system.update'
    const template = data.newsTemplates?.[kind]
    const englishOnly = englishOnlyFor(kind)
    const translated = translateCard(
      kind,
      template?.title || '',
      template?.message || '',
      { version: data.release.latestVersion || '' },
      englishOnly,
    )
    items.push({
      id: `system-${data.release.latestVersion}`,
      kind,
      englishOnly,
      level: 'info',
      ...translated,
      actionUrl: data.release.releaseUrl,
      actions: [{ id: 'queue-system-update', kind: 'queue-system-update' }],
      updatedAt: 0,
      changelog: releaseChangelog(data.release.message),
    })
  } else if (fallbackUpdateAvailable) {
    const version =
      data.releaseAnnouncement?.latestVersion || data.release.latestVersion || ''
    const kind = 'system.updaterMissing'
    const template = data.newsTemplates?.[kind]
    const englishOnly = englishOnlyFor(kind)
    const values = { version }
    const title = translateText(
      `${kind}.title`,
      template?.title || '',
      values,
      englishOnly,
    )
    const message = !updaterAvailable
      ? translateText(
          `${kind}.message`,
          template?.message || '',
          values,
          englishOnly,
        )
      : !updatesEnabled
        ? t(
            'updates.availability.disabled.description',
            {},
            englishOnly ? 'en-GB' : undefined,
          )
        : translateText(
            'system.update.message',
            data.newsTemplates?.['system.update']?.message || '',
            values,
            englishOnly,
          )
    items.push({
      id: `system-${version}`,
      kind,
      englishOnly,
      level: 'warn',
      title,
      message,
      actions: [{ id: 'open-updates', kind: 'open-updates' }],
      updatedAt: 0,
      changelog:
        data.releaseAnnouncement?.changelog?.length
          ? data.releaseAnnouncement.changelog
          : releaseChangelog(data.release.message),
    })
  }
  if (standardUpdateAvailable && data.release.staged) {
    const kind = 'system.staged'
    const template = data.newsTemplates?.[kind]
    const englishOnly = englishOnlyFor(kind)
    const translated = translateCard(
      kind,
      template?.title || '',
      template?.message || '',
      {},
      englishOnly,
    )
    items.push({
      id: `system-staged-${data.release.latestVersion}`,
      kind,
      englishOnly,
      level: 'info',
      ...translated,
      updatedAt: 0,
    })
  }
  items.push(
    ...data.notices.map((item) => {
      const kind = item.kind
      const template = kind ? data.newsTemplates?.[kind] : undefined
      const englishOnly = kind
        ? englishOnlyFor(kind, item.englishOnly)
        : item.englishOnly ?? false
      const values = { ...(item.values || {}), username }
      const sourceTitle = item.title || template?.title || ''
      const sourceMessage = item.message || template?.message || ''
      if (!kind)
        return {
          ...item,
          englishOnly,
          title: render(item.title, values),
          message: render(item.message, values),
        }
      return {
        ...item,
        englishOnly,
        ...translateCard(kind, sourceTitle, sourceMessage, values, englishOnly),
      }
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
      englishOnly: item.englishOnly ?? false,
    }))
    .sort((a, b) => b.updatedAt - a.updatedAt)
}
