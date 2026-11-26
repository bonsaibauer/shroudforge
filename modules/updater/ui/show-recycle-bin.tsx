import { useEffect, useRef, useState } from 'react'
import { postRequest } from '../../transport/ui/send-host-command'
import { Empty } from '../../ui-resources/show-empty-state'
import { Icon } from '../../ui-resources/show-icon'
import { PageHeader } from '../../ui-resources/show-page-header'
import { useI18n } from '../../ui-resources/i18n'
import { UpdaterAvailability } from './show-updater-availability'
import type { CommandResult } from '../../notifications/ui/notification-types'
import type { UpdaterCommand } from './update-types'
import './recycle-bin.css'

type Entry = {
  id: string
  reason: string
  createdAt: number
  fileCount: number
  sizeBytes: number
  canRestore: boolean
  restoreModId?: string
}

type RecycleBinList = {
  requestId: string
  entries: Entry[]
}

export function RecycleBin({
  retentionDays,
  automaticCleanup,
  updaterAvailable,
  enabled,
}: {
  retentionDays: number
  automaticCleanup: boolean
  updaterAvailable: boolean
  enabled: boolean
}) {
  const { t } = useI18n()
  const [entries, setEntries] = useState<Entry[] | null>(null)
  const [loading, setLoading] = useState(false)
  const [busy, setBusy] = useState<string | null>(null)
  const pendingListRequestId = useRef<string | null>(null)
  const pendingActionRequestId = useRef<string | null>(null)
  const actionsEnabled = updaterAvailable && enabled
  const onCommand: UpdaterCommand = (command, payload) =>
    postRequest(command, payload)

  const refresh = (reportResult = true) => {
    if (!actionsEnabled) return
    setLoading(true)
    pendingListRequestId.current = postRequest('updater-request', {
      action: 'recycle-bin-list',
      reportResult,
    })
  }

  useEffect(() => {
    const receiveList = (event: Event) => {
      const result = (event as CustomEvent<RecycleBinList>).detail
      if (result.requestId !== pendingListRequestId.current) return
      pendingListRequestId.current = null
      setLoading(false)
      setEntries(result.entries)
    }
    const receiveCommandResult = (event: Event) => {
      const result = (event as CustomEvent<CommandResult>).detail
      if (result.requestId === pendingListRequestId.current) {
        pendingListRequestId.current = null
        setLoading(false)
        return
      }
      if (result.requestId !== pendingActionRequestId.current) return
      pendingActionRequestId.current = null
      setBusy(null)
      if (result.success) refresh(false)
    }
    window.addEventListener('shroudforge-recycle-bin-list', receiveList)
    window.addEventListener('shroudforge-command-result', receiveCommandResult)
    if (actionsEnabled) refresh()
    return () => {
      window.removeEventListener('shroudforge-recycle-bin-list', receiveList)
      window.removeEventListener(
        'shroudforge-command-result',
        receiveCommandResult,
      )
    }
  }, [actionsEnabled])

  const restore = (entry: Entry) => {
    if (!actionsEnabled || loading || busy !== null) return
    setBusy(entry.id)
    pendingActionRequestId.current = postRequest('updater-request', {
      action: 'recycle-bin-restore',
      id: entry.id,
    })
  }

  const empty = () => {
    if (!actionsEnabled || loading || busy !== null) return
    if (!window.confirm(t('recycleBin.emptyConfirm'))) return
    setBusy('empty')
    pendingActionRequestId.current = postRequest('updater-request', {
      action: 'recycle-bin-empty',
    })
  }

  const remove = (entry: Entry) => {
    if (!actionsEnabled || loading || busy !== null) return
    const name = entry.restoreModId || entry.reason
    if (!window.confirm(t('recycleBin.deleteConfirm', { name }))) return
    setBusy(`delete-${entry.id}`)
    pendingActionRequestId.current = postRequest('updater-request', {
      action: 'recycle-bin-delete',
      id: entry.id,
    })
  }

  return (
    <>
      <PageHeader
        eyebrow={t('nav.modsManagement')}
        title={t('recycleBin.title')}
        subtitle={
          automaticCleanup
            ? t('recycleBin.subtitle', { days: retentionDays })
            : t('recycleBin.subtitle.manual')
        }
        actions={
          <>
            <button
              className="button secondary"
              onClick={() => refresh()}
              disabled={!actionsEnabled || loading || busy !== null}
            >
              <Icon name="refresh" /> {t('common.refresh')}
            </button>
            <button
              className="button danger"
              onClick={empty}
              disabled={!actionsEnabled || !entries?.length || loading || busy !== null}
            >
              <Icon name="trash" /> {busy === 'empty' ? t('recycleBin.emptying') : t('recycleBin.empty')}
            </button>
          </>
        }
      />
      <UpdaterAvailability
        updaterAvailable={updaterAvailable}
        enabled={enabled}
        onCommand={onCommand}
      />
      {!actionsEnabled ? null : (
        <>
          {entries === null ? null : entries.length === 0 ? (
            <Empty
              title={t('recycleBin.empty.title')}
              text={t('recycleBin.empty.text')}
              icon="trash"
            />
          ) : (
            <div
              className="card flush recycle-bin-list"
              role="list"
              aria-label={t('recycleBin.title')}
            >
              {entries.map((entry) => (
                <article className="recycle-bin-row" key={entry.id} role="listitem">
                  <div className="recycle-bin-item">
                    <span className="recycle-bin-item-icon" aria-hidden="true">
                      <Icon name={entry.restoreModId ? 'module' : 'updates'} />
                    </span>
                    <span className="recycle-bin-item-copy">
                      <strong>{entry.restoreModId || entry.reason}</strong>
                      {entry.restoreModId && <small>{entry.reason}</small>}
                    </span>
                  </div>
                  <time
                    className="recycle-bin-date"
                    dateTime={new Date(entry.createdAt * 1000).toISOString()}
                  >
                    {new Date(entry.createdAt * 1000).toLocaleString()}
                  </time>
                  <div className="recycle-bin-details">
                    <span>
                      <Icon name="installed" />
                      {t('recycleBin.files', { count: entry.fileCount })}
                    </span>
                    <span>
                      <Icon name="version" />
                      {formatSize(entry.sizeBytes)}
                    </span>
                  </div>
                  <div className="recycle-bin-actions">
                    {entry.canRestore ? (
                      <button
                        className="button secondary"
                        onClick={() => restore(entry)}
                        disabled={loading || busy !== null}
                      >
                        {busy === entry.id
                          ? t('recycleBin.restoring')
                          : t('recycleBin.restore')}
                      </button>
                    ) : (
                      <span className="badge">
                        {t(
                          automaticCleanup
                            ? 'recycleBin.automaticCleanup'
                            : 'recycleBin.manualCleanup',
                        )}
                      </span>
                    )}
                    <button
                      className="button danger"
                      onClick={() => remove(entry)}
                      disabled={loading || busy !== null}
                    >
                      {busy === `delete-${entry.id}`
                        ? t('recycleBin.deleting')
                        : t('recycleBin.delete')}
                    </button>
                  </div>
                </article>
              ))}
            </div>
          )}
        </>
      )}
    </>
  )
}

function formatSize(size: number): string {
  if (size < 1024) return `${size} B`
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`
  if (size < 1024 * 1024 * 1024)
    return `${(size / (1024 * 1024)).toFixed(1)} MB`
  return `${(size / (1024 * 1024 * 1024)).toFixed(1)} GB`
}
