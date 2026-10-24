import type { Settings } from '../../loader-config/ui/loader-settings'
import { PageId, Snapshot } from './ui-snapshot'
import type {
  NewsItem,
  NoticeAction,
} from '../../notifications/ui/notification-types'
import { NewsPage } from '../../notifications/ui/show-news'
import { Updates } from '../../updater/ui/show-updates'
import { Discover } from '../../updater/ui/show-mod-catalog'
import { Installed } from './show-installed-mods'
import { SettingsPage } from '../../loader-config/ui/show-loader-settings'
import { ActivityPage } from '../../logging/ui/show-activity'
import { Compatibility } from './show-compatibility'
import { RecycleBin } from '../../updater/ui/show-recycle-bin'
import type { UpdaterCommand } from '../../updater/ui/update-types'
import { WikiPage } from '../../wiki/ui/show-wiki'

export function Page({
  id,
  data,
  settings,
  setSettings,
  news,
  wikiArticleId,
  selectWikiArticle,
  openWikiArticle,
  unread,
  read,
  markRead,
  runNoticeAction,
  openNews,
  selectMod,
  onUpdaterCommand,
}: {
  id: PageId
  data: Snapshot
  settings: Settings
  setSettings(v: Settings): void
  news: NewsItem[]
  wikiArticleId: string | null
  selectWikiArticle(articleId: string | null): void
  openWikiArticle(articleId: string): void
  unread: NewsItem[]
  read: string[]
  markRead(ids: string[], isRead?: boolean): void
  runNoticeAction(action: NoticeAction): void
  openNews(): void
  selectMod(id: string): void
  onUpdaterCommand: UpdaterCommand
}) {
  if (id === 'activity')
    return <ActivityPage activity={data.activity} mods={data.mods} />
  if (id === 'news')
    return (
      <NewsPage
        items={news}
        unread={unread}
        read={read}
        markRead={markRead}
        runNoticeAction={runNoticeAction}
        openWikiArticle={openWikiArticle}
      />
    )
  if (id === 'discover')
    return (
      <Discover
        catalog={data.catalog}
        updaterAvailable={data.release.updaterAvailable === true}
        enabled={data.settings.modulePreferences?.updates?.enabled === true}
        onCommand={onUpdaterCommand}
      />
    )
  if (id === 'installed')
    return <Installed mods={data.mods} select={selectMod} />
  if (id === 'updates')
    return (
      <Updates
        data={{
          mode: data.mode,
          release: data.release,
          installationRootSelected: true,
          enabled: data.settings.modulePreferences?.updates?.enabled === true,
          updateQueue: data.updateQueue,
          backupManagement: data.backupManagement,
          modUpdates:
            data.settings.modulePreferences?.updates?.mods?.latestUpdates,
          modCheckResult:
            data.settings.modulePreferences?.updates?.mods?.lastCheckResult,
        }}
        onCommand={onUpdaterCommand}
      />
    )
  if (id === 'recycle-bin') {
    const recycleBin = settings.modulePreferences?.updates?.recycleBin
    return (
      <RecycleBin
        retentionDays={recycleBin?.retentionDays ?? 30}
        automaticCleanup={recycleBin?.automaticCleanup !== false}
        updaterAvailable={data.release.updaterAvailable === true}
        enabled={data.settings.modulePreferences?.updates?.enabled === true}
      />
    )
  }
  if (id === 'compatibility')
    return (
      <Compatibility
        data={data}
        news={news}
        openNews={openNews}
        selectMod={selectMod}
        openLog={() =>
          runNoticeAction({
            id: 'open-debug-console',
            kind: 'open-debug-console',
          })
        }
      />
    )
  if (id === 'wiki')
    return (
      <WikiPage
        selectedArticleId={wikiArticleId}
        onSelectArticle={selectWikiArticle}
      />
    )
  return (
    <SettingsPage data={data} settings={settings} setSettings={setSettings} />
  )
}
