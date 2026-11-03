import type { NewsItem } from './notification-types'
import type { LogLevel } from '../../logging/ui/log-entry'

const infoCategoryOrder = [
  'welcome',
  'support',
  'community',
  'updates',
  'modEvents',
  'includedMods',
  'configuration',
  'applySettings',
  'other',
] as const

type InfoCategory = (typeof infoCategoryOrder)[number]

const priority: Record<LogLevel, number> = {
  error: 0,
  warn: 1,
  info: 2,
  debug: 3,
  trace: 4,
}

const infoCategoryPriority = (item: NewsItem): InfoCategory => {
  switch (item.kind) {
    case 'notice.welcome':
      return 'welcome'
    case 'notice.support':
      return 'support'
    case 'notice.localization':
    case 'notice.feedback':
    case 'notice.discord':
    case 'notice.nexusMods':
      return 'community'
    case 'mod.update.success':
    case 'notice.creativeMode':
    case 'notice.multiplayerSupport':
      return 'updates'
    case 'mod.install.success':
    case 'mod.remove.success':
      return 'modEvents'
    case 'notice.modsGuide':
      return 'includedMods'
    case 'notice.configuration':
      return 'configuration'
    case 'notice.applySettings':
      return 'applySettings'
    default:
      return item.id.startsWith('system-') ? 'updates' : 'other'
  }
}

export function sortNewsItems(items: NewsItem[]) {
  return [...items].sort((a, b) => {
    const levelOrder = priority[a.level] - priority[b.level]
    if (levelOrder) return levelOrder

    if (a.level === 'info') {
      const categoryOrder =
        infoCategoryOrder.indexOf(infoCategoryPriority(a)) -
        infoCategoryOrder.indexOf(infoCategoryPriority(b))
      if (categoryOrder) return categoryOrder
    }

    return b.updatedAt - a.updatedAt
  })
}
