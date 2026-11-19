import { postRequest } from '../../transport/ui/send-host-command'
import { useI18n } from '../../ui-resources/i18n'
import { Brand } from '../../ui-resources/show-brand'
import { HeaderLink } from '../../ui-resources/show-header-link'
import { LanguageMenu } from '../../ui-resources/choose-language'
import { useEffect, useState } from 'react'
import { Snapshot } from './ui-snapshot'
import { ModeBadge } from '../../ui-resources/show-mode-badge'
import { WikiHelp } from '../../wiki/ui/show-wiki-help'
import type { WikiModuleSettings } from '../../wiki/mediawiki-api'
import type { ChirperMotion } from '../../notifications/ui/notification-types'

export function TopBar({
  data,
  remoteTarget,
  serverAdminAuthenticated,
  serverAdminGroupName,
  serverAdminLoginAvailable,
  serverAdminLoginRequiresWorld,
  serverAdminLoginSearching,
  serverAdminLoginPending,
  serverAdminLoginError,
  onRemoteAdminLogin,
  onRemoteTargetChange,
  onOpenWikiCategory,
  showChirper,
  isWikiPage,
  unread,
  severity,
  crowded,
  motion,
  title,
  onChirper,
}: {
  data: Snapshot
  remoteTarget: 'client' | 'server'
  serverAdminAuthenticated: boolean
  serverAdminGroupName: string | null
  serverAdminLoginAvailable: boolean
  serverAdminLoginRequiresWorld: boolean
  serverAdminLoginSearching: boolean
  serverAdminLoginPending: boolean
  serverAdminLoginError: string | null
  onRemoteAdminLogin(password: string): void
  onRemoteTargetChange(target: 'client' | 'server'): void
  onOpenWikiCategory(category: string): void
  showChirper: boolean
  isWikiPage: boolean
  unread: number
  severity: string
  crowded: boolean
  motion: ChirperMotion | null
  title: string
  onChirper(): void
}) {
  const { t } = useI18n()
  const [password, setPassword] = useState('')
  const passwordValid = new TextEncoder().encode(password).length <= 1024
  const serverAdminWikiSettings: WikiModuleSettings = {
    ...(data.settings.modulePreferences?.wiki as WikiModuleSettings | undefined),
    enabled: !data.uiVanilla,
  }
  const serverAdminWikiHelp = (
    <WikiHelp
      articleId="modloader-p2p-connection"
      compact
      moduleSettings={serverAdminWikiSettings}
      onOpenCategory={onOpenWikiCategory}
    />
  )
  useEffect(() => {
    if (remoteTarget !== 'server' || serverAdminAuthenticated || !serverAdminLoginAvailable) {
      setPassword('')
    }
  }, [remoteTarget, serverAdminAuthenticated, serverAdminLoginAvailable])
  const external = (url: string) => postRequest('open-url', { url })
  return (
    <>
    <header className="topbar" data-drag-region>
      <Brand />
      <nav className="top-actions">
        <HeaderLink
          label={t('header.github')}
          icon="github"
          collapseOnHover
          tone="github"
          onClick={() => external('https://github.com/bonsaibauer/shroudforge')}
        />
        <HeaderLink
          label={t('header.reportIssue')}
          icon="issue"
          collapseOnHover
          tone="danger"
          onClick={() =>
            external(
              'https://github.com/bonsaibauer/shroudforge/issues/new/choose',
            )
          }
        />
        <HeaderLink
          label={t('header.discord')}
          icon="discord"
          useInlineIcon
          collapseOnHover
          tone="discord"
          onClick={() => external('https://discord.com/invite/jH2gNJYnt2')}
        />
        <HeaderLink
          label={t('header.youtube')}
          icon="video-youtube"
          collapseOnHover
          tone="youtube"
          onClick={() => external('https://www.youtube.com/@shroudforge')}
        />
        <HeaderLink
          label={t('header.support')}
          icon="coffee"
          collapseOnHover
          tone="support"
          onClick={() => external('https://buymeacoffee.com/bonsaibauer')}
        />
        {showChirper && <HeaderLink
          label={t('header.chirper')}
          icon="chirper"
          tone="chirper"
          badge={unread || undefined}
          severity={severity}
          crowded={crowded}
          motion={motion}
          title={title}
          onClick={onChirper}
        />}
      </nav>
      <LanguageMenu />
      <ModeBadge
        mode={data.mode}
        connected={data.connected}
        remoteServerState={remoteTarget === 'server' && data.mode !== 'DESKTOP'
          ? data.remoteServerState
          : undefined}
      />
      {remoteTarget === 'server' && data.installationTarget !== 'server' && serverAdminAuthenticated && (
        <span className="remote-admin-status" role="status">
          {serverAdminGroupName ?? 'Server admin'}
        </span>
      )}
      <div className="window-controls">
        <button
          className="window-control"
          onClick={() => postRequest('minimize-window')}
          aria-label={t('common.minimize')}
          title={t('common.minimize')}
        >
          −
        </button>
        <button
          className="window-control"
          onClick={() => postRequest('toggle-maximize-window')}
          aria-label={t('common.maximize')}
          title={t('common.maximize')}
        >
          □
        </button>
        <button
          className="window-control window-close"
          onClick={() => postRequest('hide')}
          aria-label={t('common.close')}
          title={t('common.close')}
        >
          ×
        </button>
      </div>
    </header>
    {remoteTarget === 'server' && data.installationTarget !== 'server' && !serverAdminAuthenticated && !isWikiPage && (
      <div className="overlay server-admin-overlay" role="presentation">
        <section
          className="modal server-admin-modal"
          role="dialog"
          aria-modal="true"
          aria-labelledby="server-admin-title"
        >
          <header>
            <div>
              <small>SHROUDFORGE · SERVER ACCESS</small>
              <h2 id="server-admin-title">
                {serverAdminLoginAvailable
                  ? t('serverAdmin.title')
                  : serverAdminLoginSearching
                    ? t('serverAdmin.searchingTitle')
                    : t('serverAdmin.unavailableTitle')}
              </h2>
            </div>
            <button
              className="window-control window-close"
              type="button"
              onClick={() => onRemoteTargetChange('client')}
              aria-label={t('common.close')}
              title={t('common.close')}
            >
              ×
            </button>
          </header>
          <div className="modal-body">
            <ServerAdminVisual
              serverAdminLoginAvailable={serverAdminLoginAvailable}
              serverAdminLoginSearching={serverAdminLoginSearching}
            />
            {serverAdminLoginAvailable ? (
              <>
                <p>{t('serverAdmin.prompt')}</p>
                <form
                  className="remote-admin-login"
                  onSubmit={(event) => {
                    event.preventDefault()
                    if (!password || !passwordValid || serverAdminLoginPending || !serverAdminLoginAvailable) return
                    onRemoteAdminLogin(password)
                    setPassword('')
                  }}
                >
                  <label htmlFor="server-admin-password">{t('serverAdmin.password')}</label>
                  <input
                    id="server-admin-password"
                    type="password"
                    value={password}
                    autoComplete="off"
                    autoFocus
                    aria-label={t('serverAdmin.passwordAria')}
                    maxLength={1024}
                    onChange={(event) => setPassword(event.currentTarget.value)}
                  />
                  {serverAdminLoginError && (
                    <p className="remote-admin-error" role="alert">{serverAdminLoginError}</p>
                  )}
                  <div className="remote-admin-actions">
                    {serverAdminWikiHelp}
                    <button
                      className="button"
                      type="submit"
                      disabled={!password || !passwordValid || serverAdminLoginPending}
                    >
                      {serverAdminLoginPending ? '…' : t('serverAdmin.signIn')}
                    </button>
                  </div>
                </form>
              </>
            ) : (
              <div
                className="server-admin-unavailable"
                role="status"
                aria-live="polite"
                aria-busy={serverAdminLoginSearching}
              >
                {serverAdminLoginSearching ? (
                  <div className="server-admin-searching">
                    <span className="server-admin-spinner" aria-hidden="true" />
                    <div>
                      <strong>{t('serverAdmin.searchingTitle')}</strong>
                      <p>{t('serverAdmin.searchingMessage')}</p>
                    </div>
                  </div>
                ) : (
                  <p>{t(serverAdminLoginRequiresWorld
                    ? 'serverAdmin.enterWorld'
                    : 'serverAdmin.serverNotConnected')}</p>
                )}
                {serverAdminWikiHelp}
              </div>
            )}
          </div>
        </section>
      </div>
    )}
    </>
  )
}

export function ServerAdminVisual({
  serverAdminLoginAvailable = false,
  serverAdminLoginSearching = false,
  warning = false,
}: {
  serverAdminLoginAvailable?: boolean
  serverAdminLoginSearching?: boolean
  warning?: boolean
}) {
  const { t } = useI18n()
  return (
    <div className="server-admin-visual" aria-hidden="true">
      <svg viewBox="0 0 420 146" fill="none" xmlns="http://www.w3.org/2000/svg">
        <defs>
          <linearGradient id="server-admin-panel" x1="28" y1="22" x2="383" y2="133" gradientUnits="userSpaceOnUse">
            <stop stopColor="#182a2f" />
            <stop offset="1" stopColor="#0b1418" />
          </linearGradient>
          <linearGradient id="server-admin-screen" x1="42" y1="38" x2="120" y2="91" gradientUnits="userSpaceOnUse">
            <stop stopColor="#1b343a" />
            <stop offset="1" stopColor="#0d1a1e" />
          </linearGradient>
          <radialGradient id="server-admin-glow" cx="0" cy="0" r="1" gradientTransform="matrix(0 71 -145 0 215 59)" gradientUnits="userSpaceOnUse">
            <stop stopColor="#d8b56c" stopOpacity=".2" />
            <stop offset="1" stopColor="#d8b56c" stopOpacity="0" />
          </radialGradient>
          <filter id="server-admin-light" x="184" y="34" width="64" height="64" filterUnits="userSpaceOnUse" colorInterpolationFilters="sRGB">
            <feGaussianBlur stdDeviation="7" />
          </filter>
        </defs>
        <rect x=".5" y=".5" width="419" height="145" rx="14" fill="url(#server-admin-panel)" />
        <rect x=".5" y=".5" width="419" height="145" rx="14" fill="url(#server-admin-glow)" />
        {serverAdminLoginAvailable ? (
          <>
            <path d="M112 61h188" stroke="#d8b56c" strokeOpacity=".42" strokeDasharray="3 6" />
            <path d="M112 61h188" stroke="#d8b56c" strokeOpacity=".3" strokeWidth="5" filter="url(#server-admin-light)" />
          </>
        ) : (
          <>
            <path d="M112 61h67m52 0h69" stroke="#789197" strokeOpacity=".48" strokeDasharray="3 6" />
            <path d="M205 41v7m0 26v7" stroke="#d8b56c" strokeOpacity=".55" strokeDasharray="2 3" />
          </>
        )}
        <circle cx="205" cy="61" r="20" stroke={serverAdminLoginAvailable ? '#d8b56c' : '#789197'} strokeOpacity=".35" strokeDasharray="2 5" />
        <circle cx="205" cy="61" r="11" fill="#142226" stroke={serverAdminLoginAvailable ? '#e2cd98' : '#9aabad'} strokeOpacity=".8" />
        <path d="M201 62.5v-3a4 4 0 1 1 8 0v3m-9 0h10v7h-10z" stroke="#e2cd98" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
        <rect x="34" y="29" width="80" height="59" rx="7" fill="#0d181b" stroke="#789197" strokeOpacity=".52" />
        <rect x="41" y="36" width="66" height="45" rx="3" fill="url(#server-admin-screen)" />
        <circle cx="52" cy="49" r="3" fill={serverAdminLoginAvailable ? '#59c398' : '#d8b56c'} />
        <path d="M60 49h34M49 59h48M49 66h31" stroke="#9eb1ad" strokeOpacity=".42" strokeWidth="2" strokeLinecap="round" />
        <path d="m27 95 8-5h78l8 5a2 2 0 0 1-1 4H28a2 2 0 0 1-1-4Z" fill="#27383c" stroke="#789197" strokeOpacity=".5" />
        <path d="M60 94h28" stroke="#d8b56c" strokeOpacity=".7" strokeWidth="2" strokeLinecap="round" />
        <rect x="300" y="25" width="84" height="76" rx="8" fill="#101c20" stroke="#789197" strokeOpacity=".55" />
        <rect x="307" y="32" width="70" height="18" rx="4" fill="#1b2a2e" stroke="#53696d" strokeOpacity=".5" />
        <rect x="307" y="54" width="70" height="18" rx="4" fill="#1b2a2e" stroke="#53696d" strokeOpacity=".5" />
        <rect x="307" y="76" width="70" height="18" rx="4" fill="#1b2a2e" stroke="#53696d" strokeOpacity=".5" />
        <circle cx="316" cy="41" r="2.5" fill="#d8b56c" />
        <circle cx="316" cy="63" r="2.5" fill="#59c398" />
        <circle cx="316" cy="85" r="2.5" fill="#59c398" />
        <path d="M324 41h39m-39 22h39m-39 22h39" stroke="#8fa3a3" strokeOpacity=".5" strokeWidth="2" strokeLinecap="round" />
        <text x="74" y="123" fill="#9aabad" fontSize="8" fontWeight="700" letterSpacing="2" textAnchor="middle">CLIENT UI</text>
        <text x="205" y="104" fill={serverAdminLoginAvailable ? '#cbb478' : '#9aabad'} fontSize="7" fontWeight="700" letterSpacing="1.5" textAnchor="middle">
          {t(warning ? 'vanilla.serverWarning.link' : serverAdminLoginAvailable
            ? 'serverAdmin.activeLink'
            : serverAdminLoginSearching
              ? 'serverAdmin.searchingLink'
              : 'serverAdmin.waitingLink')}
        </text>
        <text x="342" y="123" fill="#9aabad" fontSize="8" fontWeight="700" letterSpacing="2" textAnchor="middle">SERVER</text>
        <rect x=".5" y=".5" width="419" height="145" rx="14" stroke="#d7e2df" strokeOpacity=".1" />
      </svg>
    </div>
  )
}

export function RemoteTargetSwitch({
  remoteTarget,
  onChange,
}: {
  remoteTarget: 'client' | 'server'
  onChange(target: 'client' | 'server'): void
}) {
  return (
    <div
      className={`remote-target ${remoteTarget === 'server' ? 'server-selected' : ''}`}
      role="group"
      aria-label="Modloader target"
    >
      <button
        className={remoteTarget === 'client' ? 'active' : ''}
        aria-pressed={remoteTarget === 'client'}
        onClick={() => onChange('client')}
      >
        Client
      </button>
      <button
        className={`server-target ${remoteTarget === 'server' ? 'active' : ''}`}
        aria-pressed={remoteTarget === 'server'}
        onClick={() => onChange('server')}
      >
        Server
      </button>
    </div>
  )
}
