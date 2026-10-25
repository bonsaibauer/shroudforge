import type { LogLevel } from '../../logging/ui/log-entry'

export type NoticeAction = { id: string; kind: string; target?: string }

export type NoticeLink = {
  labelKey?: string
  label?: string
  url?: string
  wikiArticleId?: string
}

export type NoticeBanner = {
  imageUrl: string
  linkUrl: string
  alt: string
}

export type GitHubContributor = {
  login: string
  html_url: string
  contributions: number
}

export type Notice = {
  id: string
  modId: string
  ecosystem?: 'EML' | 'SF'
  title: string
  message: string
  level: LogLevel
  beta?: boolean
  englishOnly?: boolean
  actionUrl?: string
  actionLabelKey?: string
  links?: NoticeLink[]
  banner?: NoticeBanner
  actions?: NoticeAction[]
  updatedAt: number
  kind?: string
  values?: Record<string, string | number>
  changelog?: string[]
}

export type NewsItem = {
  id: string
  modId?: string
  ecosystem?: 'EML' | 'SF'
  level: LogLevel
  title: string
  message: string
  beta?: boolean
  englishOnly?: boolean
  actionUrl?: string
  actionLabelKey?: string
  links?: NoticeLink[]
  banner?: NoticeBanner
  actions?: NoticeAction[]
  updatedAt: number
  kind?: string
  values?: Record<string, string | number>
  changelog?: string[]
}

export type NewsPersonalization = {
  usernameMode: 'auto' | 'manual'
  detectedUsername?: string | null
  effectiveUsername?: string
  messageNameFallback: string
}
export type NewsDisplaySource = {
  notices: Notice[]
  newsTemplates?: Record<
    string,
    { title: string; message: string; beta?: boolean; englishOnly?: boolean }
  >
  mods: Array<{ id: string; ecosystem: 'EML' | 'SF' }>
  updatesEnabled: boolean
  releaseAnnouncement?: {
    updateAvailable: boolean
    latestVersion?: string
    releaseUrl?: string | null
    changelog?: string[]
  }
  release: {
    updaterAvailable?: boolean
    updateAvailable: boolean
    latestVersion?: string
    message?: string
    releaseUrl?: string
    staged: boolean
  }
}
export type CommandResult = {
  requestId: string
  success: boolean
  message: string
  level: LogLevel
  batchKey?: string
  restoreSettings?: boolean
  ecosystem?: 'EML' | 'SF' | null
  /** New revision of a saved mod state: the next save uses it. */
  revision?: string | null
}
