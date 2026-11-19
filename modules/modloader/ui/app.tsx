import type { Settings } from '../../loader-config/ui/loader-settings'
import {
  isTargetUiVisible,
  resolveUiTargetSettings,
} from '../../loader-config/ui/module-visibility'
import { post, postRequest } from '../../transport/ui/send-host-command'
import { useEffect, useMemo, useRef, useState } from 'react'
import { I18nProvider, useI18n } from '../../ui-resources/i18n'
import { PageId, ServerView, Snapshot, empty } from './ui-snapshot'
import type { ServerViewDetails } from './ui-snapshot'
import type { NoticeAction } from '../../notifications/ui/notification-types'
import type { RuntimeProblemCode } from './ui-snapshot'
import type { ModInfo } from './mod-types'
import { logUiEvent } from '../../logging/ui/log-ui-event'
import { Chirper } from '../../notifications/ui/show-notice-popup'
import { useNotifications } from '../../notifications/ui/read-notifications'
import { Toasts, useCommandToasts } from '../../notifications/ui/show-toasts'
import { Nav, NavGroup } from '../../ui-resources/show-navigation'
import { ModIcon } from './show-mod-icon'
import { ModPage } from './show-mod'
import { useUiUpdates } from './receive-ui-updates'
import { RemoteTargetSwitch, ServerAdminVisual, TopBar } from './show-top-bar'
import { Page } from './show-page'
import { UiErrorBoundary } from './show-ui-error'
import { compatibilityNavigationStatus } from './show-compatibility'
import type { CompatibilityTab } from './show-compatibility'
import { PageZoomProvider } from '../../ui-resources/show-page-header'
import { FeatureBanner } from '../../ui-resources/show-feature-banner'
import { Icon } from '../../ui-resources/show-icon'
import type { TranslationKey } from '../../ui-resources/i18n'
import { DEFAULT_WIKI_CATEGORY } from '../../wiki/mediawiki-api'
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
  const [wikiCategory, setWikiCategory] = useState(DEFAULT_WIKI_CATEGORY)
  const [compatibilityTab, setCompatibilityTab] = useState<CompatibilityTab>('mod-detection')
  const [dismissedBannerKey, setDismissedBannerKey] = useState<string | null>(null)
  const [chirper, setChirper] = useState(false)
  const [settings, setSettings] = useState<Settings>(empty.settings)
  const [remoteTarget, setRemoteTarget] = useState<'client' | 'server'>('client')
  const [serverView, setServerView] = useState<ServerView | null>(null)
  const [serverAdminAuthenticated, setServerAdminAuthenticated] = useState(false)
  const [serverAdminGroupName, setServerAdminGroupName] = useState<string | null>(null)
  const [serverAdminLoginPending, setServerAdminLoginPending] = useState(false)
  const [serverAdminLoginError, setServerAdminLoginError] = useState<string | null>(null)
  const [confirmServerVanilla, setConfirmServerVanilla] = useState(false)
  const pendingServerLogin = useRef<string | null>(null)
  const [read, setRead] = useState<Record<'client' | 'server', string[]>>({
    client: [],
    server: [],
  })
  const previousBannerKey = useRef<string | null>(null)
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
  useEffect(() => {
    setRemoteTarget(
      settings.modulePreferences?.modloaderUi?.remoteTarget === 'server'
        ? 'server'
        : 'client',
    )
  }, [settings.modulePreferences?.modloaderUi?.remoteTarget])
  useEffect(() => {
    const onCommandResult = (event: Event) => {
      const result = (event as CustomEvent<{
        requestId: string
        success: boolean
        message?: string
        groupName?: string
      }>).detail
      if (result?.requestId === pendingServerLogin.current) {
        pendingServerLogin.current = null
        setServerAdminLoginPending(false)
        setServerAdminAuthenticated(result.success)
        setServerAdminGroupName(
          result.success ? result.groupName ?? 'Server admin' : null,
        )
        setServerAdminLoginError(result.success ? null : result.message ?? 'Admin authentication failed')
      } else if (
        remoteTarget === 'server' &&
        !result?.success &&
        (result?.message?.startsWith('Server admin access expired') ||
          result?.message?.startsWith('Sign in with a server admin group'))
      ) {
        setServerAdminAuthenticated(false)
        setServerAdminGroupName(null)
        setServerView(null)
        setServerAdminLoginError(result.message ?? 'Server admin access expired')
      }
    }
    window.addEventListener('shroudforge-command-result', onCommandResult)
    return () => window.removeEventListener('shroudforge-command-result', onCommandResult)
  }, [remoteTarget])

  useEffect(() => {
    const onRemoteModData = (event: Event) => {
      const detail = (event as CustomEvent<{
        mods?: ModInfo[]
        mod?: ModInfo
        serverView?: ServerViewDetails
        serverRights?: ServerView['serverRights']
      }>).detail
      if (Array.isArray(detail?.mods)) {
        const view = detail.serverView
        if (view) {
          setServerView({
            ...view,
            mode: 'SERVER',
            version: view.version ?? '',
            mods: detail.mods,
            dllCandidates: view.dllCandidates ?? [],
            dllChainEntries: view.dllChainEntries ?? [],
            hotkeyConflicts: view.hotkeyConflicts ?? [],
            notices: view.notices ?? [],
          })
        }
      } else if (detail?.mod && typeof detail.mod.id === 'string') {
        const updated = detail.mod
        setServerView((current) =>
          current
            ? { ...current, mods: current.mods.map((mod) => (mod.id === updated.id ? updated : mod)) }
            : current,
        )
      } else if (detail?.serverRights) {
        setServerView((current) =>
          current
            ? { ...current, serverRights: detail.serverRights }
            : {
                mode: 'SERVER',
                gameVersion: '',
                version: '',
                dllCandidates: [],
                dllChainEntries: [],
                hotkeyConflicts: [],
                notices: [],
                mods: [],
                serverRights: detail.serverRights,
              },
        )
      }
    }
    window.addEventListener('shroudforge-remote-mod-data', onRemoteModData)
    return () =>
      window.removeEventListener('shroudforge-remote-mod-data', onRemoteModData)
  }, [])

  useEffect(() => {
    if (remoteTarget !== 'server' || data.vanillaSession || data.installationTarget === 'server') {
      setServerView(null)
      return
    }
    if (!serverAdminAuthenticated) return
    postRequest('fetch-server-mods', { locale: data.locale })
  }, [remoteTarget, serverAdminAuthenticated, data.locale, data.vanillaSession, data.installationTarget])

  const activeData: Snapshot = remoteTarget === 'server' && data.installationTarget !== 'server'
    ? {
        ...data,
        ...(serverView ?? {}),
        uiVanilla: data.vanillaSession === true,
        vanillaSession: serverView ? false : null,
        mode: 'SERVER',
        gameVersion: serverView?.gameVersion ?? '',
        mods: serverAdminAuthenticated ? serverView?.mods ?? [] : [],
        notices: serverAdminAuthenticated ? serverView?.notices ?? [] : [],
        newsTemplates: serverAdminAuthenticated ? serverView?.newsTemplates : {},
        configuration: serverAdminAuthenticated ? serverView?.configuration : undefined,
        dllCandidates: serverAdminAuthenticated ? serverView?.dllCandidates ?? [] : [],
        dllChainEntries: serverAdminAuthenticated ? serverView?.dllChainEntries ?? [] : [],
        hotkeyConflicts: serverAdminAuthenticated ? serverView?.hotkeyConflicts ?? [] : [],
        release: remoteTarget === 'server'
          ? { ...data.release, updateAvailable: false, staged: false }
          : data.release,
        releaseAnnouncement: remoteTarget === 'server'
          ? { updateAvailable: false }
          : data.releaseAnnouncement,
        readNoticeIds: read.server,
      }
    : { ...data, uiVanilla: data.vanillaSession === true, mode: data.installationTarget === 'server' ? 'SERVER' : data.mode, readNoticeIds: read.client }
  const displayedMods = activeData.mods
  const vanillaKey = remoteTarget === 'server' ? 'vanillaServer' : 'vanillaClient'
  const vanillaRequested = data.settings.modulePreferences?.modloaderUi?.[vanillaKey] === true
  useEffect(() => {
    if (!confirmServerVanilla) return
    const cancel = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      event.stopImmediatePropagation()
      setConfirmServerVanilla(false)
    }
    window.addEventListener('keydown', cancel, true)
    return () => window.removeEventListener('keydown', cancel, true)
  }, [confirmServerVanilla])

  useEffect(() => {
    if (remoteTarget !== 'server' || vanillaRequested ||
        (data.installationTarget !== 'server' && (!serverAdminAuthenticated || data.vanillaSession))) {
      setConfirmServerVanilla(false)
    }
  }, [remoteTarget, vanillaRequested, data.installationTarget, serverAdminAuthenticated, data.vanillaSession])

  const newsSource = {
    ...activeData,
    updatesEnabled: activeData.settings.modulePreferences?.updates?.enabled === true,
  }
  const {
    news,
    unread,
    chirperSeverity,
    chirperTitle,
    chirperItems,
    chirperMotion,
    markRead,
  } = useNotifications(
    newsSource,
    settings,
    remoteTarget,
    read[remoteTarget],
    setRead,
    remoteTarget === 'client'
      ? data.newsTemplates !== undefined
      : serverAdminAuthenticated && serverView !== null,
  )
  const activeMod = displayedMods.find((mod) => mod.id === selectedMod)
  const compatibilityStatus = compatibilityNavigationStatus(activeData)
  const runtimeCheck = activeData.configuration?.checks.find((check) => check.id === 'runtime')
  const clientRuntimeCheck = data.configuration?.checks.find((check) => check.id === 'runtime')
  const targetUi = useMemo(
    () => resolveUiTargetSettings(settings.targetUi),
    [settings.targetUi],
  )
  const pageVisible = (id: PageId, target: 'client' | 'server' = remoteTarget) =>
    (id === 'server-management'
      ? target === 'server'
      : isTargetUiVisible('pages', id, target, targetUi)) &&
    (id !== 'compatibility' ||
      ['modDetection', 'dllManager', 'modloaderUi', 'runtimeDiagnostics'].some((module) =>
        isTargetUiVisible('modules', module, target, targetUi),
      ))
  const firstVisiblePage = (target: 'client' | 'server'): PageId => {
    const pages: PageId[] = [
      'news',
      'activity',
      'installed',
      'compatibility',
      'discover',
      'updates',
      'recycle-bin',
      'settings',
      'server-management',
      'wiki',
    ]
    return pages.find((id) => pageVisible(id, target)) ?? 'settings'
  }
  useEffect(() => {
    if (!pageVisible(page)) {
      setPage(firstVisiblePage(remoteTarget))
      setSelectedMod(null)
      setSelectedWikiArticle(null)
    } else if (!pageVisible('installed') && selectedMod) {
      setSelectedMod(null)
    }
  }, [page, remoteTarget, selectedMod, targetUi])
  const serverAdminLoginRequiresWorld = data.mode === 'DESKTOP' ||
    clientRuntimeCheck?.phase === 'main-menu' ||
    clientRuntimeCheck?.phase === 'waiting-world'
  const serverAdminLoginAvailable = data.mode === 'CLIENT' &&
    clientRuntimeCheck?.phase === 'normal' &&
    data.remoteServerState === 'connected'
  const serverAdminLoginSearching = data.mode === 'CLIENT' &&
    clientRuntimeCheck?.phase === 'normal' &&
    data.remoteServerState === 'connecting'
  const profileDiscovery = isTargetUiVisible('modules', 'gameProfileDiscovery', remoteTarget, targetUi)
    ? activeData.configuration?.gameProfileDiscovery
    : undefined
  const profileSearching = profileDiscovery?.state === 'searching'
  const profileWaitingForWorld = profileDiscovery?.state === 'waiting-for-world'
  const profileBetaActive = profileDiscovery?.state === 'beta-active'
  const profileDiscoveryPending = profileSearching || profileWaitingForWorld
  const runtimeBannerKey = isTargetUiVisible('modules', 'runtimeDiagnostics', remoteTarget, targetUi) &&
    runtimeCheck?.state === 'warning'
    ? `runtime:${runtimeCheck.code ?? 'generic'}`
    : null
  const profileBannerKey = runtimeCheck?.state !== 'warning' &&
    profileDiscovery &&
    (profileDiscoveryPending || (profileBetaActive && !runtimeCheck?.code))
    ? `profile:${profileDiscovery.state}:${profileDiscovery.profileId ?? ''}`
    : null
  const activeBannerKey = runtimeBannerKey ?? profileBannerKey
  useEffect(() => {
    setDismissedBannerKey(null)
    if (previousBannerKey.current !== activeBannerKey) {
      const previous = previousBannerKey.current
      previousBannerKey.current = activeBannerKey
      if (activeBannerKey || previous) {
        logUiEvent({
          level: 'TRACE',
          source: 'modloader',
          operation: 'status-banner-changed',
          subject: activeBannerKey ?? 'none',
          message: `Active status banner changed from ${previous ?? 'none'} to ${activeBannerKey ?? 'none'}.`,
        })
      }
    }
  }, [activeBannerKey])
  const showRuntimeBlocker = runtimeBannerKey !== null &&
    dismissedBannerKey !== runtimeBannerKey
  const showProfileDiscovery = profileBannerKey !== null &&
    dismissedBannerKey !== profileBannerKey
  const dismissBanner = () => {
    if (activeBannerKey) {
      logUiEvent({
        level: 'DEBUG',
        source: 'modloader',
        operation: 'dismiss-status-banner',
        subject: activeBannerKey,
        message: 'User dismissed the status banner.',
      })
      setDismissedBannerKey(activeBannerKey)
      notifyCommandResult({
        requestId: `dismiss-${activeBannerKey}`,
        success: true,
        message: 'settings.modules.windowClosed',
        level: 'info',
      })
    }
  }
  const bannerDismiss = {
    label: t('common.dismiss'),
    onClick: dismissBanner,
  }
  const changeRemoteTarget = (target: 'client' | 'server') => {
    if (target === remoteTarget) return
    setRemoteTarget(target)
    setServerView(null)
    setSelectedMod(null)
    setServerAdminLoginError(null)
    if (!pageVisible(page, target)) {
      setSelectedWikiArticle(null)
      setPage(firstVisiblePage(target))
    }
    postRequest('save-setting', {
      scope: 'module',
      module: 'modloaderUi',
      key: 'remoteTarget',
      value: target,
    })
  }
  const wikiTarget = () => {
    const configured = targetUi.wikiLinkTarget
    const requested = configured === 'current' ? remoteTarget : configured
    if (pageVisible('wiki', requested)) return requested
    if (pageVisible('wiki', remoteTarget)) return remoteTarget
    const other = remoteTarget === 'client' ? 'server' : 'client'
    return pageVisible('wiki', other) ? other : requested
  }
  const navigateToWiki = (
    destination:
      | { type: 'article'; articleId: string }
      | { type: 'category'; category: string },
  ) => {
    const target = wikiTarget()
    if (!pageVisible('wiki', target)) return
    if (target !== remoteTarget) changeRemoteTarget(target)
    setSelectedMod(null)
    if (destination.type === 'category') {
      setWikiCategory(destination.category)
      setSelectedWikiArticle(null)
    } else {
      setSelectedWikiArticle(destination.articleId)
    }
    setPage('wiki')
  }
  const navigate = (next: PageId) => {
    if (next === 'wiki') {
      navigateToWiki({ type: 'category', category: wikiCategory || DEFAULT_WIKI_CATEGORY })
      return
    }
    if (!pageVisible(next)) return
    setSelectedMod(null)
    setSelectedWikiArticle(null)
    setPage(next)
  }
  const openWikiArticle = (articleId: string) =>
    navigateToWiki({ type: 'article', articleId })
  const openWikiCategory = (category: string) =>
    navigateToWiki({ type: 'category', category })
  const openCompatibility = (tab: CompatibilityTab) => {
    if (!pageVisible('compatibility')) return
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
    else if (action.kind === 'open-wiki') navigate('wiki')
    else if (action.kind === 'open-mod' && action.target) {
      if (activeData.mods.some((mod) => mod.id === action.target) && pageVisible('installed')) {
        setSelectedWikiArticle(null)
        setPage('installed')
        setSelectedMod(action.target)
      } else navigate('installed')
    } else if (
      action.kind === 'open-debug-console' &&
      isTargetUiVisible('modules', 'debugConsole', remoteTarget, targetUi)
    )
      postRequest('set-window-visibility', {
        module: 'debugConsole',
        visible: true,
      })
    else if (
      action.kind === 'check-updates' &&
      pageVisible('updates')
    ) postRequest('check-updates')
    else if (action.kind === 'open-updates' && pageVisible('updates')) navigate('updates')
    else if (
      action.kind === 'queue-system-update' &&
      pageVisible('updates')
    )
      postRequest('queue-system-update')
  }

  return (
    <main className={`shell ${remoteTarget === 'server' ? 'server-target-active' : ''}`}>
      <Toasts
        errors={activeData.configuration?.errors}
        message={saveMessage}
        onDismissMessage={dismissCommandToast}
      />
      <TopBar
        data={activeData}
        remoteTarget={remoteTarget}
        serverAdminAuthenticated={serverAdminAuthenticated}
        serverAdminGroupName={serverAdminGroupName}
        serverAdminLoginAvailable={serverAdminLoginAvailable}
        serverAdminLoginRequiresWorld={serverAdminLoginRequiresWorld}
        serverAdminLoginSearching={serverAdminLoginSearching}
        serverAdminLoginPending={serverAdminLoginPending}
        serverAdminLoginError={serverAdminLoginError}
        onRemoteAdminLogin={(password) => {
          if (!serverAdminLoginAvailable) return
          setServerAdminLoginError(null)
          const requestId = postRequest('remote-admin-login', { password })
          pendingServerLogin.current = requestId
          setServerAdminLoginPending(true)
        }}
        onRemoteTargetChange={changeRemoteTarget}
        onOpenWikiCategory={openWikiCategory}
        showChirper={pageVisible('news')}
        isWikiPage={page === 'wiki'}
        unread={unread.length}
        severity={chirperSeverity}
        crowded={unread.length >= 5}
        motion={chirperMotion}
        title={chirperTitle}
        onChirper={() => setChirper(true)}
      />
      <PageZoomProvider
        zoomPercent={settings.zoomPercent ?? 100}
        onChange={savePageZoom}
      >
        <div className="workspace">
          <aside className="sidebar">
            <div className="sidebar-target">
              <button
                className="button vanilla-mode-button"
                disabled={remoteTarget === 'server' && data.installationTarget !== 'server' && (!serverAdminAuthenticated || data.vanillaSession === true)}
                onClick={() => {
                  if (remoteTarget === 'server' && !vanillaRequested) {
                    setConfirmServerVanilla(true)
                    return
                  }
                  postRequest('save-setting', {
                    scope: 'module', module: 'modloaderUi', key: vanillaKey, value: !vanillaRequested,
                  })
                }}
              >
                {t(vanillaRequested ? 'vanilla.enable' : 'vanilla.button')}
              </button>
              <RemoteTargetSwitch
                remoteTarget={remoteTarget}
                onChange={changeRemoteTarget}
              />
            </div>
            <NavGroup
              title={t('nav.modloader')}
              version={activeData.version}
              className="nav-group-modloader"
            >
              {pageVisible('activity') && <Nav
                icon="activity"
                label={t('nav.activity')}
                active={!activeMod && page === 'activity'}
                onClick={() => navigate('activity')}
              />}
              {pageVisible('news') && <Nav
                icon="news"
                label={t('nav.news')}
                badge={unread.length || undefined}
                active={!activeMod && page === 'news'}
                onClick={() => navigate('news')}
              />}
              {pageVisible('compatibility') && <Nav
                icon="compatibility"
                label={t('nav.compatibility')}
                status={compatibilityStatus}
                statusLabel={compatibilityStatus === 'restart-required'
                  ? t('compatibility.status.restartRequired')
                  : undefined}
                active={!activeMod && page === 'compatibility'}
                onClick={() => openCompatibility('mod-detection')}
              />}
              {pageVisible('settings') && <Nav
                icon="settings"
                label={t('nav.settings')}
                active={!activeMod && page === 'settings'}
                onClick={() => navigate('settings')}
              />}
              {pageVisible('server-management') && <Nav
                icon="server"
                label={t('nav.serverManagement')}
                active={!activeMod && page === 'server-management'}
                onClick={() => navigate('server-management')}
              />}
              {pageVisible('wiki') && (
                <Nav
                  icon="wiki"
                  label={t('nav.wiki')}
                  active={!activeMod && page === 'wiki'}
                  onClick={() => navigate('wiki')}
                />
              )}
            </NavGroup>
            <NavGroup title={t('nav.modsManagement')}>
              {pageVisible('discover') && <Nav
                icon="discover"
                label={t('nav.discover')}
                active={!activeMod && page === 'discover'}
                onClick={() => navigate('discover')}
              />}
              {pageVisible('installed') && <Nav
                icon="installed"
                label={t('nav.installed')}
                badge={displayedMods.length || undefined}
                active={!activeMod && page === 'installed'}
                onClick={() => navigate('installed')}
              />}
              {pageVisible('updates') && <Nav
                icon="updates"
                label={t('nav.updates')}
                badge={activeData.release.updateAvailable ? 1 : undefined}
                active={!activeMod && page === 'updates'}
                onClick={() => navigate('updates')}
              />}
              {pageVisible('recycle-bin') && <Nav
                icon="trash"
                label={t('nav.recycleBin')}
                active={!activeMod && page === 'recycle-bin'}
                onClick={() => navigate('recycle-bin')}
              />}
            </NavGroup>
            {pageVisible('installed') && <NavGroup className="nav-group-mods" title={t('nav.mods')}>
              <div className="sidebar-mods">
                {displayedMods.map((mod) => (
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
            </NavGroup>}
          </aside>
          <section className="content">
            {showProfileDiscovery && (
              <FeatureBanner
                variant={profileBetaActive ? 'warning' : 'info'}
                brand="ShroudForge"
                title={t(profileWaitingForWorld
                  ? 'runtime.profileDiscovery.waitingWorldTitle'
                  : profileBetaActive
                    ? 'runtime.profileDiscovery.betaTitle'
                    : 'runtime.profileDiscovery.searchingTitle')}
                titleId="runtime-profile-discovery-title"
                description={profileWaitingForWorld
                  ? t('runtime.profileDiscovery.waitingWorldDescription')
                  : profileBetaActive
                    ? t('runtime.profileDiscovery.betaDescription')
                    : t('runtime.profileDiscovery.searchingDescription')}
                media={<Icon name={profileBetaActive ? 'warning' : 'refresh'} />}
                badge={t(profileWaitingForWorld
                  ? 'runtime.profileDiscovery.waitingWorldBadge'
                  : profileBetaActive
                    ? 'runtime.profileDiscovery.betaBadge'
                    : 'runtime.profileDiscovery.searchingBadge')}
                role="status"
                dismiss={bannerDismiss}
              >
                <div className="feature-banner-actions">
                  <button className="button ghost" onClick={() => openWikiArticle('game-profile-discovery')}>
                    {t('runtime.profileDiscovery.openGuide')}
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
                dismiss={bannerDismiss}
              >
                <div className="feature-banner-actions runtime-blocker-actions">
                  <button
                    className="button ghost"
                    onClick={() => openCompatibility('engine')}
                  >
                    {t('common.details')}
                  </button>
                  {pageVisible('updates') && (
                    <button
                      className="button ghost"
                      onClick={() => {
                        navigate('updates')
                        window.setTimeout(() => postRequest('check-updates'), 2000)
                      }}
                    >
                      {activeData.release.updateAvailable && activeData.release.latestVersion
                        ? t('runtime.blocker.updateTo', {
                            version: activeData.release.latestVersion,
                          })
                        : t('runtime.blocker.checkUpdates')}
                    </button>
                  )}
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
                </div>
              </FeatureBanner>
            )}
            {activeMod ? (
              <ModPage
                key={activeMod.id}
                mod={activeMod}
                status={activeData.configuration?.modStates?.[activeMod.id]}
                dependencyStates={activeData.configuration?.modStates || {}}
                remoteTarget={remoteTarget}
              />
            ) : (
              <Page
                id={page}
                data={activeData}
                settings={settings}
                setSettings={setSettings}
                notifyCommandResult={notifyCommandResult}
                news={news}
                wikiArticleId={selectedWikiArticle}
                wikiCategory={wikiCategory}
                selectWikiArticle={setSelectedWikiArticle}
                openWikiArticle={openWikiArticle}
                openWikiCategory={openWikiCategory}
                unread={unread}
                read={read[remoteTarget]}
                markRead={markRead}
                runNoticeAction={runNoticeAction}
                openNews={() => navigate('news')}
                selectMod={setSelectedMod}
                onUpdaterCommand={postRequest}
                compatibilityTab={compatibilityTab}
                setCompatibilityTab={setCompatibilityTab}
                serverAdminAuthenticated={serverAdminAuthenticated}
              />
            )}
          </section>
        </div>
      </PageZoomProvider>
      {confirmServerVanilla && (
        <div className="overlay server-admin-overlay" role="presentation">
          <section className="modal server-admin-modal" role="alertdialog" aria-modal="true"
            aria-labelledby="server-vanilla-title" aria-describedby="server-vanilla-impact">
            <header>
              <div>
                <small>SHROUDFORGE · SERVER</small>
                <h2 id="server-vanilla-title">{t('vanilla.serverWarning.title')}</h2>
              </div>
              <button className="window-control window-close" type="button"
                onClick={() => setConfirmServerVanilla(false)} aria-label={t('common.close')}
                title={t('common.close')}>×</button>
            </header>
            <div className="modal-body">
              <ServerAdminVisual warning />
              <p>{t('vanilla.serverWarning.prompt')}</p>
              <p id="server-vanilla-impact">{t('vanilla.serverWarning.impact')}</p>
              <p>{t('vanilla.serverWarning.recover')}</p>
              <code className="vanilla-server-code">shroudforge/config/modloader-config.json</code>
              <p>{t('vanilla.serverWarning.setting')}</p>
              <code className="vanilla-server-code">"vanillaServer": false</code>
              <p>{t('vanilla.serverWarning.restart')}</p>
              <div className="remote-admin-actions">
                <button className="button ghost" type="button" autoFocus
                  onClick={() => setConfirmServerVanilla(false)}>{t('common.cancel')}</button>
                <button className="button" type="button" onClick={() => {
                  setConfirmServerVanilla(false)
                  postRequest('save-setting', {
                    scope: 'module', module: 'modloaderUi', key: 'vanillaServer', value: true,
                  })
                }}>{t('vanilla.serverWarning.confirm')}</button>
              </div>
            </div>
          </section>
        </div>
      )}
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
