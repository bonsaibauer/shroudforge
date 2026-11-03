import { useEffect, useState } from 'react'
import { postRequest } from '../../transport/ui/send-host-command'
import { useI18n } from '../../ui-resources/i18n'
import { Icon } from '../../ui-resources/show-icon'
import { BrandMark } from '../../ui-resources/show-brand'
import { Updates } from './show-updates'
import type {
  BackupManagement,
  ModUpdate,
  ModUpdateCheckResult,
  Release,
  UpdateQueue,
  UpdaterViewData,
} from './update-types'

type UpdaterSnapshot = {
  root: string
  suggestedRoot?: string
  installationRootSelected?: boolean
  version: string
  release: Release
  queue: UpdateQueue
  backupManagement?: BackupManagement
  modUpdates?: ModUpdate[]
  modCheckResult?: ModUpdateCheckResult | null
  modCheckRunning?: boolean
  locale?: string
  updatesEnabled?: boolean
}

type UpdaterWindow = Window & {
  __shroudforgeUpdaterSnapshot?: (snapshot: UpdaterSnapshot) => void
  __shroudforgeUpdaterResult?: (result: {
    error?: string
  }) => void
}

const initialSnapshot: UpdaterSnapshot = {
  root: '',
  version: '',
  release: {
    currentVersion: '',
    updaterAvailable: true,
    updateAvailable: false,
    state: 'idle',
    staged: false,
  },
  queue: { items: [] },
}

export function UpdaterApp() {
  const { t, setLocale } = useI18n()
  const [snapshot, setSnapshot] = useState(initialSnapshot)
  const [message, setMessage] = useState('')

  useEffect(() => {
    const updaterWindow = window as UpdaterWindow
    updaterWindow.__shroudforgeUpdaterSnapshot = setSnapshot
    updaterWindow.__shroudforgeUpdaterResult = (result) =>
      setMessage(result.error || '')
    return () => {
      delete updaterWindow.__shroudforgeUpdaterSnapshot
      delete updaterWindow.__shroudforgeUpdaterResult
    }
  }, [])

  useEffect(() => {
    if (snapshot.locale) setLocale(snapshot.locale)
  }, [snapshot.locale, setLocale])

  const data: UpdaterViewData = {
    mode: 'DESKTOP',
    installationRoot: snapshot.root,
    release: { ...snapshot.release, updaterAvailable: true },
    installationRootSelected: snapshot.installationRootSelected ?? Boolean(snapshot.root),
    enabled: snapshot.updatesEnabled === true,
    updateQueue: snapshot.queue,
    backupManagement: snapshot.backupManagement,
    modUpdates: snapshot.modUpdates,
    modCheckResult: snapshot.modCheckResult,
    modCheckRunning: snapshot.modCheckRunning,
  }
  const command = (name: string, payload?: Record<string, unknown>) => {
    setMessage('')
    postRequest(name, payload)
  }

  return (
    <main className="shell updater-standalone-shell">
      <header className="topbar">
        <div className="brand">
          <BrandMark />
          <span>
            <strong>SHROUDFORGE</strong>
            <small>UPDATER</small>
          </span>
        </div>
        <div className="updater-installation">
          <div className="updater-installation-path">
            <span title={snapshot.root || snapshot.suggestedRoot}>
              {snapshot.root || t('updater.root.missing')}
            </span>
            {!snapshot.root && snapshot.suggestedRoot && (
              <small>
                {t('updater.root.suggested', { path: snapshot.suggestedRoot })}
              </small>
            )}
          </div>
          <button
            className="header-link"
            onClick={() => command('choose-root')}
            title={t('updater.root.choose')}
          >
            <Icon name="folder" />
            <span>{t('updater.root.choose')}</span>
          </button>
        </div>
      </header>
      <div className="updater-standalone-workspace">
        <section className="content updater-standalone-content">
          <Updates data={data} onCommand={command} commandError={message} />
        </section>
        <footer className="updater-standalone-status">
          <span>{t('updater.installedVersion', { version: snapshot.version || '—' })}</span>
        </footer>
      </div>
    </main>
  )
}
