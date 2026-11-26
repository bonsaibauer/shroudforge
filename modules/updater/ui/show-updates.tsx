import { useI18n } from '../../ui-resources/i18n'
import { useState } from 'react'
import { Empty } from '../../ui-resources/show-empty-state'
import { Icon } from '../../ui-resources/show-icon'
import { PageHeader } from '../../ui-resources/show-page-header'
import { Segmented } from '../../ui-resources/choose-segment'
import { SystemUpdate } from './show-system-update'
import { UpdateQueuePanel } from './show-update-queue'
import type {
  ModUpdate,
  ModUpdateCheckResult,
  ModpackUpdate,
  SchematicUpdate,
  UpdaterCommand,
  UpdaterViewData,
} from './update-types'
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
    ['all', 'mods', 'schematics', 'modpacks'].includes(tab)
      ? data.modCheckRunning === true
      : data.release.state === 'checking'
  const checkCommand = {
    all: 'check-all-updates',
    mods: 'check-mod-updates',
    schematics: 'check-schematic-updates',
    modpacks: 'check-modpack-updates',
  }[tab] || 'check-updates'
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
              onClick={() => onCommand(checkCommand)}
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
          ['schematics', t('updates.schematics')],
          ['modpacks', t('updates.modpacks')],
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
      {tab === 'all' && (
        <AllUpdatesList
          release={systemRelease}
          queuedSystem={queuedSystem}
          modUpdates={modUpdates}
          modCheckResult={modCheckResult}
          schematicUpdates={data.schematicUpdates || []}
          schematicCheckResult={data.schematicCheckResult}
          modpackUpdates={data.modpackUpdates || []}
          modpackCheckResult={data.modpackCheckResult}
          modCheckRunning={data.modCheckRunning === true}
          queue={queue}
          disabled={!actionsEnabled}
          onCommand={onCommand}
        />
      )}
      {tab === 'mods' ? (
        <>
          {modUpdates.length === 0 && !modCheckResult?.failed ? (
            <Empty
              title={t('updates.mods.empty.title')}
              text={t('updates.mods.empty.text')}
              icon="updates"
            />
          ) : modUpdates.length > 0 ? (
            <div className="settings-module-list">
              <ModUpdateItems
                updates={modUpdates}
                queue={queue}
                disabled={!actionsEnabled}
                onCommand={onCommand}
              />
            </div>
          ) : null}
          <ModCheckStatus checkResult={modCheckResult} />
        </>
      ) : tab === 'schematics' || tab === 'modpacks' ? (
        <ContentUpdates
          kind={tab === 'schematics' ? 'schematic' : 'modpack'}
          updates={tab === 'schematics' ? data.schematicUpdates || [] : data.modpackUpdates || []}
          checkResult={tab === 'schematics' ? data.schematicCheckResult : data.modpackCheckResult}
          queue={queue}
          disabled={!actionsEnabled}
          onCommand={onCommand}
          showHeading={false}
        />
      ) : tab === 'history' ? (
        <Empty
          title={t('updates.history.empty.title')}
          text={t('updates.history.empty.text')}
          icon="activity"
        />
      ) : tab === 'system' ? (
        <>
          <SystemUpdate
            release={systemRelease}
            queued={queuedSystem}
            disabled={!actionsEnabled}
            onCommand={onCommand}
          />
          <ModCheckStatus checkResult={modCheckResult} />
        </>
      ) : null}
    </>
  )
}

function AllUpdatesList({
  release,
  queuedSystem,
  modUpdates,
  modCheckResult,
  schematicUpdates,
  schematicCheckResult,
  modpackUpdates,
  modpackCheckResult,
  modCheckRunning,
  queue,
  disabled,
  onCommand,
}: {
  release: UpdaterViewData['release']
  queuedSystem: boolean
  modUpdates: ModUpdate[]
  modCheckResult?: ModUpdateCheckResult | null
  schematicUpdates: SchematicUpdate[]
  schematicCheckResult?: ModUpdateCheckResult | null
  modpackUpdates: ModpackUpdate[]
  modpackCheckResult?: ModUpdateCheckResult | null
  modCheckRunning: boolean
  queue: NonNullable<UpdaterViewData['updateQueue']>
  disabled: boolean
  onCommand: UpdaterCommand
}) {
  const { t } = useI18n()
  const hasUpdates =
    release.updateAvailable ||
    modUpdates.length > 0 ||
    schematicUpdates.length > 0 ||
    modpackUpdates.length > 0
  const hasCheckFailures = [
    modCheckResult,
    schematicCheckResult,
    modpackCheckResult,
  ].some((result) => (result?.failed || 0) > 0)
  const emptyText =
    modCheckRunning || release.state === 'checking'
      ? t('updates.checking')
      : release.state === 'idle'
        ? t('updates.none.notChecked')
        : release.state === 'ready' && !release.latestVersion
          ? t('updates.none.notPublished')
          : release.state === 'error'
            ? release.message || t('updates.none.message')
            : t('updates.none.message')

  return (
    <>
      <section className="settings-module-list">
        {release.updateAvailable && (
          <SystemUpdate
            release={release}
            queued={queuedSystem}
            disabled={disabled}
            onCommand={onCommand}
          />
        )}
        {modUpdates.length > 0 && (
          <ModUpdateItems
            updates={modUpdates}
            queue={queue}
            disabled={disabled}
            onCommand={onCommand}
            showTypeLabel
          />
        )}
        <ContentUpdates
          kind="schematic"
          updates={schematicUpdates}
          checkResult={schematicCheckResult}
          queue={queue}
          disabled={disabled}
          onCommand={onCommand}
          showHeading={false}
          showEmpty={false}
          showCheckStatus={false}
          showTypeLabel
          wrap={false}
        />
        <ContentUpdates
          kind="modpack"
          updates={modpackUpdates}
          checkResult={modpackCheckResult}
          queue={queue}
          disabled={disabled}
          onCommand={onCommand}
          showHeading={false}
          showEmpty={false}
          showCheckStatus={false}
          showTypeLabel
          wrap={false}
        />
        {release.state === 'error' && release.message && (
          <p className="notice warning">{release.message}</p>
        )}
        {!hasUpdates && !hasCheckFailures && release.state !== 'error' && (
          <Empty
            title={t('updates.content.empty.title')}
            text={emptyText}
            icon="updates"
          />
        )}
      </section>
      <ModCheckStatus checkResult={modCheckResult} />
      <ContentUpdateStatus kind="schematic" checkResult={schematicCheckResult} />
      <ContentUpdateStatus kind="modpack" checkResult={modpackCheckResult} />
    </>
  )
}

function ModUpdateItems({
  updates,
  queue,
  disabled,
  onCommand,
  showTypeLabel = false,
}: {
  updates: ModUpdate[]
  queue: NonNullable<UpdaterViewData['updateQueue']>
  disabled: boolean
  onCommand: UpdaterCommand
  showTypeLabel?: boolean
}) {
  const { t } = useI18n()
  return (
    <>
      {updates.map((update) => {
        const queued = queue.items.some(
          (item) =>
            item.id === `mod-update:${update.projectId}:${update.modId}` &&
            !['complete', 'cancelled', 'error'].includes(item.state),
        )
        return (
          <section
            className="card"
            key={`mod:${update.projectId}:${update.modId}`}
          >
            <div className="card-head">
              <div>
                {showTypeLabel && (
                  <span className="release-channel-chip">{t('updates.mods')}</span>
                )}
                <h2>{update.name || update.modId}</h2>
                <p>
                  {update.currentVersion || update.modId}
                  {' → '}
                  {update.latestVersion || t('updates.mods.available')}
                </p>
              </div>
              <button
                className="button"
                disabled={disabled || queued}
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
    </>
  )
}

function ContentUpdates({
  kind,
  updates,
  checkResult,
  queue,
  disabled,
  onCommand,
  showHeading = true,
  showEmpty = true,
  showCheckStatus = true,
  showTypeLabel = false,
  wrap = true,
}: {
  kind: 'schematic' | 'modpack'
  updates: SchematicUpdate[] | ModpackUpdate[]
  checkResult?: ModUpdateCheckResult | null
  queue: NonNullable<UpdaterViewData['updateQueue']>
  disabled: boolean
  onCommand: UpdaterCommand
  showHeading?: boolean
  showEmpty?: boolean
  showCheckStatus?: boolean
  showTypeLabel?: boolean
  wrap?: boolean
}) {
  const { t } = useI18n()
  const content = (
    <>
      {showHeading && (
        <h2>{t(kind === 'schematic' ? 'updates.schematics' : 'updates.modpacks')}</h2>
      )}
      {showEmpty && updates.length === 0 && !checkResult?.failed && (
        <Empty
          title={t('updates.content.empty.title')}
          text={t('updates.content.empty.text')}
          icon="updates"
        />
      )}
      {updates.map((update) => {
        const queued = queue.items.some(
          (item) =>
            item.id === `${kind}-update:${update.projectId}` &&
            !['complete', 'cancelled', 'error'].includes(item.state),
        )
        const blocked = kind === 'schematic'
          ? (update as SchematicUpdate).modified
          : !(update as ModpackUpdate).canUpdate
        const reason = kind === 'schematic'
          ? blocked ? t('updates.schematics.modified') : ''
          : (update as ModpackUpdate).conflicts.join(' ')
        return (
          <section className="card" key={`${kind}:${update.projectId}`}>
            <div className="card-head">
              <div>
                {showTypeLabel && (
                  <span className="release-channel-chip">
                    {t(kind === 'schematic' ? 'updates.schematics' : 'updates.modpacks')}
                  </span>
                )}
                <h3>{update.name}</h3>
                <p>
                  {'currentVersion' in update ? update.currentVersion : update.currentVersionId}
                  {' → '}
                  {update.latestVersion || update.latestVersionId}
                </p>
                {kind === 'modpack' && (
                  <>
                    <p className="hint">
                      {t('updates.modpacks.changes', { count: (update as ModpackUpdate).changes.length })}
                    </p>
                    {(update as ModpackUpdate).changes.some((change) => change.kind === 'removed') && (
                      <p className="hint">{t('updates.modpacks.removedRetained')}</p>
                    )}
                  </>
                )}
                {reason && <p className="notice warning">{reason}</p>}
              </div>
              <button
                className="button"
                disabled={disabled || queued || blocked}
                onClick={() => onCommand(`update-${kind}`, {
                  projectId: update.projectId,
                  versionId: update.latestVersionId,
                  title: update.name,
                })}
              >
                {t(queued ? 'updates.queue.added' : 'updates.content.add')}
              </button>
            </div>
          </section>
        )
      })}
      {showCheckStatus && (
        <ContentUpdateStatus kind={kind} checkResult={checkResult} />
      )}
    </>
  )
  return (
    wrap ? <section className="settings-module-list">{content}</section> : content
  )
}

function ContentUpdateStatus({
  kind,
  checkResult,
}: {
  kind: 'schematic' | 'modpack'
  checkResult?: ModUpdateCheckResult | null
}) {
  const { t } = useI18n()
  return (
    <>
      {checkResult?.checkedAt && (
        <p className="hint settings-module-hint">
          {t('updates.content.lastChecked', {
            type: t(kind === 'schematic' ? 'updates.schematics' : 'updates.modpacks'),
            checked: checkResult.checked,
            failed: checkResult.failed,
            time: new Date(checkResult.checkedAt * 1000).toLocaleString(),
          })}
        </p>
      )}
      {checkResult?.errors?.map((error, index) => (
        <p className="notice warning" key={index}>{error}</p>
      ))}
    </>
  )
}

function ModCheckStatus({
  checkResult,
}: {
  checkResult?: ModUpdateCheckResult | null
}) {
  const { t } = useI18n()
  return (
    <>
      {checkResult?.checkedAt && (
        <p className="hint settings-module-hint">
          {t('updates.mods.lastChecked', {
            checked: checkResult.checked,
            failed: checkResult.failed,
            time: new Date(checkResult.checkedAt * 1000).toLocaleString(),
          })}
        </p>
      )}
      {checkResult?.errors?.map((error, index) => (
        <p className="notice warning" key={index}>{error}</p>
      ))}
    </>
  )
}
