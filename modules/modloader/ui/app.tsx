import type { Settings } from '../../loader-config/ui/loader-settings'
import { post, postRequest } from '../../transport/ui/send-host-command'
import { useEffect, useState } from 'react'
import { I18nProvider, useI18n } from '../../ui-resources/i18n'
import { PageId, Snapshot, empty } from './ui-snapshot'
import type { NoticeAction } from '../../notifications/ui/notification-types'
import type { RuntimeProblemCode } from './ui-snapshot'
import { Chirper } from '../../notifications/ui/show-notice-popup'
import { useNotifications } from '../../notifications/ui/read-notifications'
import { Toasts, useCommandToasts } from '../../notifications/ui/show-toasts'
import { Nav, NavGroup } from '../../ui-resources/show-navigation'
import { ModIcon } from './show-mod-icon'
import { ModPage } from './show-mod'
import { useUiUpdates } from './receive-ui-updates'
import { TopBar } from './show-top-bar'
import { Page } from './show-page'
import { UiErrorBoundary } from './show-ui-error'
import { compatibilityNavigationStatus } from './show-compatibility'
import type { CompatibilityTab } from './show-compatibility'
import { PageZoomProvider } from '../../ui-resources/show-page-header'
import { FeatureBanner } from '../../ui-resources/show-feature-banner'
import { Icon } from '../../ui-resources/show-icon'
import type { TranslationKey } from '../../ui-resources/i18n'
import { createRoot } from 'react-dom/client'
import './styles.css'

function runtimeProblemMessage(code: RuntimeProblemCode | undefined): TranslationKey {
  switch (code) {
    case 'profile-unavailable':
      return 'runtime.blocker.profileUnavailable'
    case 'profile-selection-invalid':
      return 'runtime.blocker.profileSelectionInvalid'
    case 'profile-error':
      return 'runtime.blocker.profileError'
    case 'provider-unavailable':
      return 'runtime.blocker.providerUnavailable'
    case 'runtime-not-ready':
      return 'runtime.blocker.runtimeNotReady'
    default:
      return 'runtime.blocker.generic'
  }
}

export function App() {
  const { t, setLocale } = useI18n()
  const [data, setData] = useState<Snapshot>(empty)
  const [page, setPage] = useState<PageId>('news')
  const [selectedMod, setSelectedMod] = useState<string | null>(null)
  const [selectedWikiArticle, setSelectedWikiArticle] = useState<string | null>(null)
  const [compatibilityTab, setCompatibilityTab] = useState<CompatibilityTab>('mod-detection')
  const [runtimeBlockerDismissed, setRuntimeBlockerDismissed] = useState(false)
  const [chirper, setChirper] = useState(false)
  const [settings, setSettings] = useState<Settings>(empty.settings)
  const [read, setRead] = useState<string[]>([])
  const {
    message: saveMessage,
    notifyCommandResult,
    dismissCommandToast,
  } = useCommandToasts()
  useUiUpdates({
    data,
    setData,
    setSettings,
    setRead,
    setLocale,
    setPage,
    setSelectedMod,
    notifyCommandResult,
  })
  useEffect(() => {
    document.body.classList.toggle('compact', settings.compactMode)
    document.body.classList.toggle('reduced-motion', settings.reducedMotion)
  }, [settings.compactMode, settings.reducedMotion])

  const newsSource = {
    ...data,
    updatesEnabled: data.settings.modulePreferences?.updates?.enabled === true,
  }
  const {
    news,
    unread,
    chirperSeverity,
    chirperTitle,
    chirperItems,
    markRead,
  } = useNotifications(newsSource, settings, read, setRead)
  const activeMod = data.mods.find((mod) => mod.id === selectedMod)
  const runtimeCheck = data.configuration?.checks.find((check) => check.id === 'runtime')
  const profileDiscovery = data.configuration?.gameProfileDiscovery
  const profileSearching = profileDiscovery?.state === 'searching'
  const showRuntimeBlocker = runtimeCheck?.state === 'warning' && !profileSearching && !runtimeBlockerDismissed
  const showProfileDiscovery = profileSearching || (profileDiscovery?.state === 'beta-active' && !runtimeCheck?.code)
  const navigate = (next: PageId) => {
    setSelectedMod(null)
    setSelectedWikiArticle(null)
    setPage(next)
  }
  const openWikiArticle = (articleId: string) => {
    setSelectedMod(null)
    setSelectedWikiArticle(articleId)
    setPage('wiki')
  }
  const openCompatibility = (tab: CompatibilityTab) => {
    setSelectedMod(null)
    setSelectedWikiArticle(null)
    setCompatibilityTab(tab)
    setPage('compatibility')
  }
  const savePageZoom = (zoomPercent: number) => {
    setSettings((current) => ({ ...current, zoomPercent }))
    postRequest('save-setting', {
      scope: 'general',
      key: 'zoomPercent',
      value: zoomPercent,
    })
  }

  const runNoticeAction = (action: NoticeAction) => {
    if (action.kind === 'open-installed-mods') navigate('installed')
    else if (action.kind === 'open-mod' && action.target) {
      if (data.mods.some((mod) => mod.id === action.target)) {
        setPage('installed')
        setSelectedMod(action.target)
      } else navigate('installed')
    } else if (action.kind === 'open-debug-console')
      postRequest('set-window-visibility', {
        module: 'debugConsole',
        visible: true,
      })
    else if (action.kind === 'check-updates') postRequest('check-updates')
    else if (action.kind === 'open-updates') navigate('updates')
    else if (action.kind === 'queue-system-update')
      postRequest('queue-system-update')
  }

  return (
    <main className="shell">
      <Toasts
        errors={data.configuration?.errors}
        message={saveMessage}
        onDismissMessage={dismissCommandToast}
      />
      <TopBar
        data={data}
        unread={unread.length}
        severity={chirperSeverity}
        crowded={unread.length >= 5}
        title={chirperTitle}
        onChirper={() => setChirper(true)}
      />
      <PageZoomProvider
        zoomPercent={settings.zoomPercent ?? 100}
        onChange={savePageZoom}
      >
        <div className="workspace">
          <aside className="sidebar">
            <NavGroup title={t('nav.modloader')}>
              <Nav
                icon="activity"
                label={t('nav.activity')}
                active={!activeMod && page === 'activity'}
                onClick={() => navigate('activity')}
              />
              <Nav
                icon="news"
                label={t('nav.news')}
                badge={unread.length || undefined}
                active={!activeMod && page === 'news'}
                onClick={() => navigate('news')}
              />
              <Nav
                icon="compatibility"
                label={t('nav.compatibility')}
                status={compatibilityNavigationStatus(data)}
                active={!activeMod && page === 'compatibility'}
                onClick={() => openCompatibility('mod-detection')}
              />
              <Nav
                icon="settings"
                label={t('nav.settings')}
                active={!activeMod && page === 'settings'}
                onClick={() => navigate('settings')}
              />
              <Nav
                icon="wiki"
                label={t('nav.wiki')}
                active={!activeMod && page === 'wiki'}
                onClick={() => navigate('wiki')}
              />
            </NavGroup>
            <NavGroup title={t('nav.modsManagement')}>
              <Nav
                icon="discover"
                label={t('nav.discover')}
                active={!activeMod && page === 'discover'}
                onClick={() => navigate('discover')}
              />
              <Nav
                icon="installed"
                label={t('nav.installed')}
                badge={data.mods.length || undefined}
                active={!activeMod && page === 'installed'}
                onClick={() => navigate('installed')}
              />
              <Nav
                icon="updates"
                label={t('nav.updates')}
                badge={data.release.updateAvailable ? 1 : undefined}
                active={!activeMod && page === 'updates'}
                onClick={() => navigate('updates')}
              />
              <Nav
                icon="trash"
                label={t('nav.recycleBin')}
                active={!activeMod && page === 'recycle-bin'}
                onClick={() => navigate('recycle-bin')}
              />
            </NavGroup>
            <NavGroup className="nav-group-mods" title={t('nav.mods')}>
              <div className="sidebar-mods">
                {data.mods.map((mod) => (
                  <button
                    key={mod.id}
                    className={`nav-item ${activeMod?.id === mod.id ? 'active' : ''}`}
                    onClick={() => setSelectedMod(mod.id)}
                  >
                    <ModIcon mod={mod} variant="nav" />
                    <span>{mod.name}</span>
                  </button>
                ))}
              </div>
            </NavGroup>
          </aside>
          <section className="content">
            {showProfileDiscovery && (
              <FeatureBanner
                variant="warning"
                brand="ShroudForge"
                title={t(profileSearching ? 'runtime.profileDiscovery.searchingTitle' : 'runtime.profileDiscovery.betaTitle')}
                titleId="runtime-profile-discovery-title"
                description={profileSearching
                  ? t('runtime.profileDiscovery.searchingDescription')
                  : t('runtime.profileDiscovery.betaDescription', { profile: profileDiscovery?.profileId || '' })}
                media={<Icon name="warning" />}
                badge={t('compatibility.status.warning')}
                role="status"
              >
                <div className="feature-banner-actions">
                  <button className="button ghost" onClick={() => openWikiArticle('game-profile-discovery')}>
                    {t('runtime.profileDiscovery.openGuide')}
                  </button>
                  <button className="button ghost" onClick={() => navigate('settings')}>
                    {t('nav.settings')}
                  </button>
                </div>
              </FeatureBanner>
            )}
            {showRuntimeBlocker && (
              <FeatureBanner
                variant="error"
                brand="ShroudForge"
                title={t('runtime.blocker.title')}
                titleId="runtime-blocker-title"
                description={t(runtimeProblemMessage(runtimeCheck?.code))}
                media={<Icon name="warning" />}
                badge={t('compatibility.status.error')}
                role="alert"
              >
                <div className="feature-banner-actions runtime-blocker-actions">
                  <button
                    className="button ghost"
                    onClick={() => openCompatibility('engine')}
                  >
                    {t('common.details')}
                  </button>
                  <button className="button ghost" onClick={() => navigate('updates')}>
                    {data.release.updateAvailable && data.release.latestVersion
                      ? t('runtime.blocker.updateTo', {
                          version: data.release.latestVersion,
                        })
                      : t('runtime.blocker.checkUpdates')}
                  </button>
                  <button
                    className="button ghost"
                    onClick={() =>
                      postRequest('open-url', {
                        url: 'https://github.com/bonsaibauer/shroudforge/issues/new?template=version_mismatch.yml',
                      })
                    }
                  >
                    {t('runtime.blocker.reportVersion')}
                  </button>
                  <button
                    className="button ghost runtime-blocker-dismiss"
                    aria-label={t('common.dismiss')}
                    title={t('common.dismiss')}
                    onClick={() => setRuntimeBlockerDismissed(true)}
                  >
                    ×
                  </button>
                </div>
              </FeatureBanner>
            )}
            {activeMod ? (
              <ModPage
                key={activeMod.id}
                mod={activeMod}
                status={data.configuration?.modStates?.[activeMod.id]}
                dependencyStates={data.configuration?.modStates || {}}
              />
            ) : (
              <Page
                id={page}
                data={data}
                settings={settings}
                setSettings={setSettings}
                news={news}
                wikiArticleId={selectedWikiArticle}
                selectWikiArticle={setSelectedWikiArticle}
                openWikiArticle={openWikiArticle}
                unread={unread}
                read={read}
                markRead={markRead}
                runNoticeAction={runNoticeAction}
                openNews={() => navigate('news')}
                selectMod={setSelectedMod}
                onUpdaterCommand={postRequest}
                compatibilityTab={compatibilityTab}
                setCompatibilityTab={setCompatibilityTab}
              />
            )}
          </section>
        </div>
      </PageZoomProvider>
      {chirper && (
        <Chirper
          items={chirperItems}
          close={() => setChirper(false)}
          all={() => {
            setChirper(false)
            navigate('news')
          }}
          markRead={markRead}
          runNoticeAction={runNoticeAction}
          openWikiArticle={(articleId) => {
            setChirper(false)
            openWikiArticle(articleId)
          }}
        />
      )}
    </main>
  )
}

window.addEventListener('error', (event) =>
  post('ui-error', {
    message: `${event.message} (${event.filename}:${event.lineno}:${event.colno})`,
  }),
)

window.addEventListener('unhandledrejection', (event) =>
  post('ui-error', {
    message: `Unhandled promise rejection: ${String(event.reason)}`,
  }),
)

createRoot(document.getElementById('root')!).render(
  <I18nProvider>
    <UiErrorBoundary>
      <App />
    </UiErrorBoundary>
  </I18nProvider>,
)
