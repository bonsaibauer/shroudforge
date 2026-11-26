import { useI18n } from '../../ui-resources/i18n'
import { useState } from 'react'
import { Empty } from '../../ui-resources/show-empty-state'
import { Icon } from '../../ui-resources/show-icon'
import { PageHeader } from '../../ui-resources/show-page-header'
import { Segmented } from '../../ui-resources/choose-segment'
import { SystemUpdate } from './show-system-update'
import { UpdateQueuePanel } from './show-update-queue'
import type { UpdaterCommand, UpdaterViewData } from './update-types'
import { UpdaterAvailability } from './show-updater-availability'
import { useSystemInstallationActions } from './show-system-installation-actions'

export function Updates({
  data,
  onCommand,
  commandError,
}: {
  data: UpdaterViewData
  onCommand: UpdaterCommand
  commandError?: string
}) {
  const { t } = useI18n()
  const [tab, setTab] = useState('all')
  const queue = data.updateQueue || { items: [] }
  const modUpdates = data.modUpdates || []
  const modCheckResult = data.modCheckResult
  const checking =
    tab === 'mods'
      ? data.modCheckRunning === true
      : data.release.state === 'checking'
  const updaterAvailable = data.release.updaterAvailable === true
  const enabled = data.enabled === true
  const actionsEnabled = updaterAvailable && enabled
  const installerQueuePending = queue.items.some(
    (item) =>
      item.kind === 'installer' &&
      !['complete', 'cancelled', 'error'].includes(item.state),
  )
  const queueActionsEnabled = updaterAvailable && (enabled || installerQueuePending)
  const moduleUnavailable =
    !!commandError &&
    /disabled in Settings|is disabled in module settings/i.test(
      commandError,
    )
  const queueFailureMessages = [
    queue.error,
    queue.run?.error,
    ...queue.items
      .filter((item) => item.state === 'error')
      .map((item) => item.message),
  ].filter((message): message is string => Boolean(message))
  const duplicateQueueError =
    data.release.state === 'error' &&
    queueFailureMessages.some(
      (message) =>
        data.release.message === message || data.release.message?.includes(message),
    )
  const commandErrorAlreadyShown =
    !!commandError &&
    ((tab !== 'history' && queueFailureMessages.includes(commandError)) ||
      (tab !== 'mods' &&
        tab !== 'history' &&
        data.release.state === 'error' &&
        data.release.message === commandError))
  const systemRelease =
    duplicateQueueError && !data.release.updateAvailable
      ? { ...data.release, state: 'ready', message: undefined }
      : data.release
  const queuedSystem = queue.items.some(
    (item) =>
      item.kind === 'system' &&
      item.version === data.release.latestVersion &&
      !['complete', 'cancelled', 'error'].includes(item.state),
  )
  const installationActions = useSystemInstallationActions({
    selectedRoot: data.installationRoot || '',
    rootSelected: data.installationRootSelected !== false,
    updaterAvailable,
    installationDetected: data.release.installationDetected === true,
    latestVersion: data.release.latestVersion,
    backupManagement: data.backupManagement,
    queue,
    onCommand,
  })
  return (
    <>
      <PageHeader
        eyebrow={t('nav.modsManagement')}
        title={t('updates.title')}
        subtitle={t('updates.subtitle')}
        actions={
          <>
            {installationActions.repairAction}
            <button
              className="button"
              onClick={() =>
                onCommand(tab === 'mods' ? 'check-mod-updates' : 'check-updates')
              }
              disabled={!actionsEnabled || checking}
            >
              <Icon name="refresh" />
              {checking ? t('updates.checking') : t('updates.check')}
            </button>
            {installationActions.uninstallAction}
          </>
        }
      />
      {installationActions.options}
      {commandError && !commandErrorAlreadyShown && (
        <p
          className={moduleUnavailable ? 'notice warning' : 'notice error'}
          role={moduleUnavailable ? 'status' : 'alert'}
        >
          {moduleUnavailable ? t('settings.moduleUnavailable') : commandError}
        </p>
      )}
      <UpdaterAvailability
        updaterAvailable={updaterAvailable}
        enabled={enabled}
        onCommand={onCommand}
      />
      <Segmented
        value={tab}
        onChange={setTab}
        items={[
          ['all', t('common.all')],
          ['system', t('updates.system')],
          ['mods', t('updates.mods')],
          ['history', t('updates.history')],
        ]}
      />
      {tab !== 'history' && (
        <UpdateQueuePanel
          queue={queue}
          mode={data.mode}
          release={data.release}
          disabled={!queueActionsEnabled}
          onCommand={onCommand}
        />
      )}
      {tab === 'mods' ? (
        <>
          {modUpdates.length === 0 ? (
            <Empty
              title={t('updates.mods.empty.title')}
              text={t('updates.mods.empty.text')}
              icon="updates"
            />
          ) : (
            <div className="settings-module-list">
              {modUpdates.map((update) => {
                const queued = queue.items.some(
                  (item) =>
                    item.id === `mod-update:${update.projectId}:${update.modId}` &&
                    !['complete', 'cancelled', 'error'].includes(item.state),
                )
                return (
                  <section className="card" key={update.projectId}>
                    <div className="card-head">
                      <div>
                        <h2>{update.name || update.modId}</h2>
                        <p>
                          {update.currentVersion || update.modId}
                          {' → '}
                          {update.latestVersion || t('updates.mods.available')}
                        </p>
                      </div>
                      <button
                        className="button"
                        disabled={!actionsEnabled || queued}
                        onClick={() =>
                          onCommand('update-mod', {
                            modId: update.modId,
                            title: update.name || update.modId,
                          })
                        }
                      >
                        {t(queued ? 'updates.queue.added' : 'updates.queue.addMod')}
                      </button>
                    </div>
                  </section>
                )
              })}
            </div>
          )}
          {modCheckResult?.checkedAt && (
            <p className="hint settings-module-hint">
              {t('updates.mods.lastChecked', {
                checked: modCheckResult.checked,
                failed: modCheckResult.failed,
                time: new Date(modCheckResult.checkedAt * 1000).toLocaleString(),
              })}
            </p>
          )}
        </>
      ) : tab === 'history' ? (
        <Empty
          title={t('updates.history.empty.title')}
          text={t('updates.history.empty.text')}
          icon="activity"
        />
      ) : (
        <>
          <SystemUpdate
            release={systemRelease}
            queued={queuedSystem}
            disabled={!actionsEnabled}
            onCommand={onCommand}
          />
        </>
      )}
    </>
  )
}
