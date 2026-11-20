import { useEffect, useRef, useState } from 'react'
import { useI18n } from '../../ui-resources/i18n'
import type { TranslationKey } from '../../ui-resources/i18n'
import { FeatureBanner } from '../../ui-resources/show-feature-banner'
import { PageHeader } from '../../ui-resources/show-page-header'
import type { UpdaterCommand } from '../../updater/ui/update-types'
import type {
  ServerPermissionGroup,
  ServerPermissionPlayer,
  ServerPermissionValues,
  ServerRights,
} from './ui-snapshot'
import shroudEditHammer from '../../../assets/shroudedit-hammer.png'

const SHROUDFORGE_REPOSITORY = 'https://github.com/bonsaibauer/shroudforge'

const permissionColumns: Array<{
  key: keyof ServerPermissionValues
  label: TranslationKey
}> = [
  { key: 'canKickBan', label: 'serverRights.canKickBan' },
  { key: 'canAccessInventories', label: 'serverRights.canAccessInventories' },
  { key: 'canEditWorld', label: 'serverRights.canEditWorld' },
  { key: 'canEditBase', label: 'serverRights.canEditBase' },
  { key: 'canExtendBase', label: 'serverRights.canExtendBase' },
]

type EditableServerPermissionGroup = ServerPermissionValues & {
  name: string
  syncServerLogs: boolean
}

function groupDraft(
  groups: Array<ServerPermissionValues & { name: string; syncServerLogs: boolean }>,
): EditableServerPermissionGroup[] {
  return groups.map((group) => {
    const isAdmin = permissionColumns.every(({ key }) => group[key])
    return {
      name: group.name,
      canKickBan: group.canKickBan,
      canAccessInventories: group.canAccessInventories,
      canEditWorld: group.canEditWorld,
      canEditBase: group.canEditBase,
      canExtendBase: group.canExtendBase,
      syncServerLogs: isAdmin && group.syncServerLogs,
    }
  })
}

function hasAllAdminRights(player: ServerPermissionValues) {
  return permissionColumns.every(({ key }) => player[key])
}

export function ServerManagement({
  onCommand,
  serverRights,
  authenticated,
}: {
  onCommand: UpdaterCommand
  serverRights?: ServerRights
  authenticated: boolean
}) {
  const { t } = useI18n()
  const [draft, setDraft] = useState<EditableServerPermissionGroup[]>([])
  const [saving, setSaving] = useState(false)
  const pendingSave = useRef<string | null>(null)

  useEffect(() => {
    setDraft(serverRights ? groupDraft(serverRights.groups) : [])
    pendingSave.current = null
    setSaving(false)
  }, [serverRights])

  useEffect(() => {
    const onResult = (event: Event) => {
      const result = (event as CustomEvent<{ requestId?: string }>).detail
      if (result?.requestId && result.requestId === pendingSave.current) {
        pendingSave.current = null
        setSaving(false)
      }
    }
    window.addEventListener('shroudforge-command-result', onResult)
    return () => window.removeEventListener('shroudforge-command-result', onResult)
  }, [])

  const dirty = Boolean(
    serverRights &&
      JSON.stringify(groupDraft(serverRights.groups)) !== JSON.stringify(groupDraft(draft)),
  )

  const changeGroup = (
    groupName: string,
    key: keyof ServerPermissionValues | 'syncServerLogs',
    value: boolean,
  ) => {
    setDraft((current) =>
      current.map((group) =>
        group.name === groupName ? { ...group, [key]: value } : group,
      ),
    )
  }

  const save = () => {
    if (!dirty || saving || !authenticated) return
    const requestId = onCommand('save-server-rights', {
      values: {
        expectedGroups: groupDraft(serverRights?.groups ?? []),
        groups: groupDraft(draft),
      },
    })
    if (typeof requestId === 'string') {
      pendingSave.current = requestId
      setSaving(true)
    }
  }

  const refreshPlayers = () => {
    if (authenticated) onCommand('fetch-server-rights')
  }

  const renderPermission = (
    player: ServerPermissionValues,
    key: keyof ServerPermissionValues,
    known = true,
  ) => (
    <span
      className={`server-rights-value${known && player[key] ? ' is-enabled' : ''}`}
      aria-label={t(!known ? 'serverRights.unknown' : player[key] ? 'serverRights.allowed' : 'serverRights.denied')}
    >
      {!known ? '?' : player[key] ? '✓' : '—'}
    </span>
  )

  const renderPlayer = (player: ServerPermissionPlayer) => (
    <tr key={player.steamId}>
      <th scope="row" className="server-rights-player-name">
        <span>{player.playerName?.trim() || player.steamId}</span>
        <small>{player.steamId}</small>
      </th>
      <td>
        {player.isShroudForgeAdmin && (
          <span className="server-rights-admin">{t('serverRights.shroudforgeAdmin')}</span>
        )}
        <span>
          {player.permissionsKnown
            ? player.groupName ?? t('serverRights.noUniqueGroup')
            : t('serverRights.unknown')}
        </span>
      </td>
      {permissionColumns.map(({ key }) => (
        <td key={key}>{renderPermission(player, key, player.permissionsKnown)}</td>
      ))}
    </tr>
  )

  return (
    <>
      <PageHeader
        eyebrow={t('nav.modloader')}
        title={t('nav.serverManagement')}
        subtitle={t('serverManagement.subtitle')}
      />
      <FeatureBanner
        variant="promo"
        brand="SHROUDEDIT"
        title={t('serverManagement.bannerTitle')}
        titleId="server-management-banner-title"
        description={t('serverManagement.description')}
        media={<img src={shroudEditHammer} alt="" />}
        badge={t('serverManagement.badge')}
      >
        <p className="feature-banner-contact">
          {t('serverManagement.invitation')}
        </p>
        <div className="feature-banner-actions">
          <button
            className="button ghost"
            type="button"
            onClick={() => onCommand('open-url', { url: SHROUDFORGE_REPOSITORY })}
          >
            {t('serverManagement.openRepository')}
          </button>
        </div>
      </FeatureBanner>

      <div className="server-management-tabs" role="tablist" aria-label={t('nav.serverManagement')}>
        <button className="server-management-tab" type="button" role="tab" aria-selected="true">
          {t('serverRights.tab')}
        </button>
      </div>

      <section className="server-rights-page" role="tabpanel">
        <div className="server-rights-heading">
          <div>
            <h2>{t('serverRights.tab')}</h2>
            <p>{t('serverRights.description')}</p>
          </div>
          <button
            className="button ghost"
            type="button"
            disabled={!authenticated}
            onClick={refreshPlayers}
          >
            {t('serverRights.refresh')}
          </button>
        </div>

        {!authenticated || !serverRights ? (
          <p className="server-rights-empty">{t('serverRights.waiting')}</p>
        ) : (
          <>
            <div className="server-rights-heading server-rights-section-heading">
              <h3>{t('serverRights.groupsTitle')}</h3>
              <button
                className="button primary"
                type="button"
                disabled={!dirty || saving}
                onClick={save}
              >
                {saving ? t('serverRights.saving') : t('serverRights.save')}
              </button>
            </div>
            <div className="server-rights-table-wrap">
              <table className="server-rights-table">
                <thead>
                  <tr>
                    <th scope="col">{t('serverRights.group')}</th>
                    {permissionColumns.map(({ key, label }) => (
                      <th scope="col" key={key}>{t(label)}</th>
                    ))}
                    <th scope="col">{t('serverRights.syncServerLogs')}</th>
                  </tr>
                </thead>
                <tbody>
                  {draft.map((group) => (
                    <tr key={group.name}>
                      <th scope="row">
                        <span>{group.name}</span>
                        {hasAllAdminRights(group) && (
                          <span className="server-rights-admin">
                            {t('serverRights.shroudforgeAdmin')}
                          </span>
                        )}
                      </th>
                      {permissionColumns.map(({ key, label }) => (
                        <td key={key}>
                          <input
                            aria-label={`${group.name}: ${t(label)}`}
                            type="checkbox"
                            checked={group[key]}
                            onChange={(event) => changeGroup(group.name, key, event.currentTarget.checked)}
                          />
                        </td>
                      ))}
                      <td>
                        <input
                          aria-label={`${group.name}: ${t('serverRights.syncServerLogs')}`}
                          type="checkbox"
                          checked={group.syncServerLogs}
                          disabled={!hasAllAdminRights(group)}
                          onChange={(event) => changeGroup(group.name, 'syncServerLogs', event.currentTarget.checked)}
                        />
                      </td>
                    </tr>
                  ))}
                  {draft.length === 0 && (
                    <tr><td colSpan={7}>{t('serverRights.noGroups')}</td></tr>
                  )}
                </tbody>
              </table>
            </div>

            <div className="server-rights-heading server-rights-section-heading">
              <h3>{t('serverRights.playersTitle')}</h3>
              <span>{serverRights.players.length}</span>
            </div>
            <div className="server-rights-table-wrap">
              <table className="server-rights-table server-rights-player-table">
                <thead>
                  <tr>
                    <th scope="col">{t('serverRights.player')}</th>
                    <th scope="col">{t('serverRights.group')}</th>
                    {permissionColumns.map(({ key, label }) => (
                      <th scope="col" key={key}>{t(label)}</th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {serverRights.players.map(renderPlayer)}
                  {serverRights.players.length === 0 && (
                    <tr><td colSpan={7}>{t('serverRights.noPlayers')}</td></tr>
                  )}
                </tbody>
              </table>
            </div>
          </>
        )}
      </section>
    </>
  )
}
