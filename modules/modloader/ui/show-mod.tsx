import type { ModInfo, SettingDefinition } from './mod-types'
import { postRequest } from '../../transport/ui/send-host-command'
import { TranslationKey, useI18n } from '../../ui-resources/i18n'
import { LinkDefinition } from './ui-snapshot'
import { Card } from '../../ui-resources/show-card'
import { HeaderLink } from '../../ui-resources/show-header-link'
import { Icon } from '../../ui-resources/show-icon'
import { PageHeader } from '../../ui-resources/show-page-header'
import { SettingSwitch } from '../../ui-resources/edit-setting'
import { ModIcon } from './show-mod-icon'
import { DependencyList } from './show-mod-dependencies'
import { ModActionButton } from './run-mod-action'
import { ModSetting, settingIsEnabled } from './edit-mod-setting'
import { useModSettings } from './save-mod-settings'
import linkOrder from '../mod_link_order.json'

export function ModPage({
  mod,
  status,
  dependencyStates,
}: {
  mod: ModInfo
  status?: { state: string; code?: string; detail: string }
  dependencyStates: Record<string, { state: string; detail: string }>
}) {
  const { t } = useI18n()
  const { enabled, values, save, updateSetting, setEnabled } =
    useModSettings(mod)
  const groupedKeys = new Set(mod.groups.flatMap((group) => group.settings))
  const groups = [
    ...mod.groups,
    ...(mod.settings.some((item) => !groupedKeys.has(item.key))
      ? [
          {
            label: t('mod.settings'),
            settings: mod.settings
              .filter((item) => !groupedKeys.has(item.key))
              .map((item) => item.key),
            actions: [],
          },
        ]
      : []),
  ]
  const settings = mod.settings
  const remove = () => {
    if (window.confirm(t('mod.remove.confirm', { name: mod.name })))
      postRequest('remove-mod', { modId: mod.id })
  }
  const disablePending = status?.code === 'native-dll-disable-pending'
  const stateCode =
    status?.state === 'blocked'
      ? 'blocked'
      : status?.state === 'restart-required'
        ? 'restart'
        : !enabled
          ? 'disabled'
          : enabled !== mod.enabled
            ? 'unknown'
            : status?.state === 'failed'
              ? 'error'
              : status?.state === 'ready'
                ? 'ready'
                : status?.state === 'waiting-world'
                  ? 'waiting'
                  : status?.state === 'not-running'
                    ? 'notRunning'
                    : !status || status?.state === 'unconfirmed'
                      ? 'unknown'
                      : 'running'
  const stateLabel = t(
    disablePending
      ? 'mod.mode.disablePending'
      : stateCode === 'restart'
        ? 'mod.mode.nextStart'
        : stateCode === 'waiting'
          ? 'mod.mode.waitingWorld'
          : `mod.mode.${stateCode}`,
  )
  const target =
    mod.target === 'client'
      ? 'Client'
      : mod.target === 'server'
        ? 'Server'
        : 'Client, Server'
  const visibleLinks = linkOrder.flatMap((key) => {
    const definition = linkDefinitions[key],
      url = mod.links?.[key]
    const appearance = definition?.appearance || ''
    const tone = appearance.startsWith('support-')
      ? `support ${appearance}`
      : appearance
    return definition && url
      ? [
          {
            key,
            url,
            label: t(definition.labelKey as TranslationKey),
            description: t(definition.descriptionKey as TranslationKey),
            icon: definition.icon,
            tone,
          },
        ]
      : []
  })
  const title = (
    <span className="mod-title-lockup">
      <ModIcon mod={mod} variant="page" />
      <span className="mod-title-copy">
        <span>{mod.name}</span>
        {mod.authors.length > 0 && (
          <span className="mod-author-byline">
            <em>by</em>
            <span>{formatAuthors(mod.authors)}</span>
          </span>
        )}
      </span>
    </span>
  )
  return (
    <>
      <PageHeader
        eyebrow="MOD"
        title={title}
        subtitle={mod.description || mod.id}
        actions={
          <>
            {mod.source === 'shroudedit' && (
              <button
                className="button ghost"
                onClick={() =>
                  postRequest('update-mod', { modId: mod.id, title: mod.name })
                }
              >
                {t('updates.queue.addMod')}
              </button>
            )}
            <button className="button danger" onClick={remove}>
              {t('mod.remove')}
            </button>
            <button
              className="button"
              onClick={() => {
                save()
                postRequest('refresh-mod', { modId: mod.id })
              }}
            >
              {t('common.refresh')}
            </button>
            <button className="button" onClick={save}>
              {t('common.save')}
            </button>
          </>
        }
      />
      <section className="mod-status-row">
        <strong
          className={'mod-state-chip ' + stateCode}
          title={
            status?.code === 'native-dll-disable-pending'
              ? t('mod.status.disablePending')
              : status?.detail
          }
        >
          {stateLabel}
        </strong>
        <div className="mod-enable-group">
          <span>{t('common.active')}</span>
          <SettingSwitch
            label={enabled ? t('mod.disable') : t('mod.enable')}
            checked={enabled}
            onChange={setEnabled}
          />
        </div>
      </section>
      <Card title={t('mod.settings')}>
        <div className="settings-module-list">
          {groups.map((group, index) => (
            <section
              className="settings-module mod-settings-section"
              key={`${group.label}-${index}`}
            >
              <header>
                <div>
                  <h3>{group.label}</h3>
                  {group.description && (
                    <p className="hint">{group.description}</p>
                  )}
                </div>
              </header>
              <div className="settings-options">
                {group.settings
                  .map((key) => settings.find((item) => item.key === key))
                  .filter((definition): definition is SettingDefinition =>
                    Boolean(definition),
                  )
                  .map((definition) => (
                    <ModSetting
                      key={definition.key}
                      definition={definition}
                      value={values[definition.key]}
                      assessmentTarget={{
                        scope: 'mod',
                        modId: mod.id,
                        key: definition.key,
                        revision: mod.revision,
                        values,
                      }}
                      disabled={!settingIsEnabled(definition, settings, values)}
                      onChange={(value) => updateSetting(definition.key, value)}
                      contexts={mod.inputContexts}
                      onReserve={(device, reserved) =>
                        postRequest('save-key-reservation', {
                          modId: mod.id,
                          key: definition.key,
                          device,
                          enabled: reserved,
                        })
                      }
                    />
                  ))}
              </div>
              {group.actions.length > 0 && (
                <div className="mod-actions">
                  {group.actions.map((action, index) => (
                    <ModActionButton
                      key={`${action.id}-${index}`}
                      mod={mod}
                      action={action}
                    />
                  ))}
                </div>
              )}
            </section>
          ))}
          {groups.length === 0 && (
            <p className="hint">{t('mod.settings.empty')}</p>
          )}
        </div>
      </Card>
      <Card title={t('mod.details')}>
        <div className="mod-meta">
          <span
            className={`mod-info-badge ecosystem ${mod.ecosystem.toLowerCase()}`}
          >
            {t(
              mod.ecosystem === 'SF' ? 'mod.ecosystem.sf' : 'mod.ecosystem.eml',
            )}
          </span>
          <span className="mod-info-badge version">
            <Icon name="version" />
            {mod.version}
          </span>
          <span className="mod-info-badge target">
            <Icon name={mod.target === 'server' ? 'server' : 'client'} />
            {target}
          </span>
          {mod.asset && (
            <span className="mod-info-badge asset">
              <Icon name="blueprint" />
              {t('mod.type.asset')}
            </span>
          )}
          {mod.runtime && (
            <span className="mod-info-badge runtime">
              <Icon name="bolt" />
              {t('mod.type.runtime')}
            </span>
          )}
          {mod.license && (
            <span className="mod-info-badge license">{mod.license}</span>
          )}
        </div>
        {visibleLinks.length > 0 && (
          <nav className="mod-links" aria-label={t('mod.links')}>
            {visibleLinks.map((link) => (
              <HeaderLink
                key={link.key}
                label={link.label}
                icon={link.icon}
                tone={link.tone}
                title={link.description}
                onClick={() => postRequest('open-url', { url: link.url })}
              />
            ))}
          </nav>
        )}
        {mod.changelog.length > 0 && (
          <div className="mod-changelog">
            <h4>{t('mod.versions')}</h4>
            <ul>
              {mod.changelog.map((item, index) => (
                <li key={index}>{item}</li>
              ))}
            </ul>
          </div>
        )}
      </Card>
      <Card title={t('mod.dependencies.title')}>
        <DependencyList
          dependencies={mod.dependencies}
          dependencyStates={dependencyStates}
        />
      </Card>
    </>
  )
}

export function formatAuthors(authors: string[]) {
  return authors.length < 2
    ? authors[0] || ''
    : `${authors.slice(0, -1).join(', ')} & ${authors.at(-1)}`
}

export const linkDefinitions = Object.fromEntries(
  Object.entries(
    import.meta.glob('./links/*.json', { eager: true, import: 'default' }),
  )
    .filter(([path]) => !path.endsWith('/order.json'))
    .map(([, value]) => [
      (value as LinkDefinition).id,
      value as LinkDefinition,
    ]),
) as Record<string, LinkDefinition>
