import { useI18n } from './i18n'

export function ModeBadge({
  mode,
  connected,
  remoteServerState,
}: {
  mode: string
  connected: boolean
  remoteServerState?: 'connecting' | 'connected' | 'disconnected'
}) {
  const { t } = useI18n()
  const normalized = mode === 'OFFLINE' ? 'DESKTOP' : mode
  const desktop = normalized === 'DESKTOP'
  const remoteServerStatus = remoteServerState !== undefined
  const statusConnected = remoteServerState === 'connected'
  const stateClass = remoteServerStatus
    ? remoteServerState === 'connected'
      ? 'online'
      : remoteServerState === 'disconnected'
        ? 'server-disconnected'
        : 'offline'
    : desktop ? 'desktop' : connected ? 'online' : 'offline'
  const title = remoteServerStatus
    ? t(`status.remoteServer.${remoteServerState}`)
    : desktop
      ? t('status.runtime.desktop')
      : connected
        ? t('status.runtime.connected')
        : t('status.runtime.disconnected')
  return (
    <div
      className={`mode-badge ${stateClass}`}
      role="status"
      aria-label={title}
      title={title}
    >
      <i />
      <span>{normalized}</span>
    </div>
  )
}
