import type { Settings } from './loader-settings'
import { postRequest } from '../../transport/ui/send-host-command'
import { useEffect, useState } from 'react'
import { Locale, TranslationKey, useI18n } from '../../ui-resources/i18n'
import type { PageId, Snapshot } from '../../modloader/ui/ui-snapshot'
import type { CommandResult } from '../../notifications/ui/notification-types'
import { startPageOptions } from '../../modloader/ui/ui-snapshot'
import { Card } from '../../ui-resources/show-card'
import { Field, SettingsRow, Toggle } from '../../ui-resources/edit-setting'
import { Flag } from '../../ui-resources/show-flag'
import { PageHeader, usePageZoom } from '../../ui-resources/show-page-header'
import { Segmented } from '../../ui-resources/choose-segment'
import { GamefileBackupManagement } from '../../game-files/ui/show-game-file-backups'
import { StoragePathsEditor } from './show-storage-paths'
import { ModulePreferencesEditor } from './show-module-settings'
import { GameProfileDiscoverySettings } from '../../game-profile-discovery/ui/settings'

export function SettingsPage({
  data,
  settings,
  setSettings,
  notifyCommandResult,
}: {
  data: Snapshot
  settings: Settings
  setSettings(v: Settings): void
  notifyCommandResult(result: CommandResult): void
}) {
  const { t, locale, locales, setLocale } = useI18n()
  const { setZoomPercent } = usePageZoom()
  const [tab, setTab] = useState('general')
  const [messageNameDraft, setMessageNameDraft] = useState(
    settings.messageNameFallback || 'Flameborn',
  )
  useEffect(
    () => setMessageNameDraft(settings.messageNameFallback || 'Flameborn'),
    [settings.messageNameFallback],
  )
  const modulePreferences = settings.modulePreferences || {}
  const modloaderPreferences = modulePreferences.modloaderUi || {}
  const configuredStartPage = startPageOptions.includes(
    modloaderPreferences.startPage,
  )
    ? modloaderPreferences.startPage
    : 'news'
  const save = () => postRequest('save-settings', { settings })
  const reload = () => {
    setSettings(data.settings)
    if (data.locale) setLocale(data.locale as Locale)
    postRequest('reload-settings')
  }
  const chooseLocale = (next: Locale) => {
    setLocale(next)
    postRequest('save-language', { locale: next })
  }
  const changeGeneral = (
    key: 'compactMode' | 'reducedMotion' | 'showNativeModMenu',
    value: boolean,
  ) => {
    setSettings({ ...settings, [key]: value })
    postRequest('save-setting', { scope: 'general', key, value })
  }
  const changeUsernameMode = (value: 'auto' | 'manual') => {
    setSettings({ ...settings, usernameMode: value })
    postRequest('save-setting', {
      scope: 'general',
      key: 'usernameMode',
      value,
    })
  }
  const changeMessageName = (value: string) => {
    const messageNameFallback = value.trim() || 'Flameborn'
    setSettings({ ...settings, messageNameFallback })
    postRequest('save-setting', {
      scope: 'general',
      key: 'messageNameFallback',
      value: messageNameFallback,
    })
  }
  const changeStartPage = (value: PageId) => {
    setSettings({
      ...settings,
      modulePreferences: {
        ...modulePreferences,
        modloaderUi: { ...modloaderPreferences, startPage: value },
      },
    })
    postRequest('save-setting', {
      scope: 'module',
      module: 'modloaderUi',
      key: 'startPage',
      value,
    })
  }
  const changeLogLevel = (value: string) => {
    setSettings({ ...settings, logLevel: value })
    postRequest('save-setting', {
      scope: 'logging',
      key: 'minimumLevel',
      value,
    })
  }
  const changeExportEnabled = (value: boolean) => {
    setSettings({ ...settings, exportEnabled: value })
    postRequest('save-setting', { scope: 'exports', key: 'enabled', value })
  }
  const changeRuntimeProfile = (value: string) => {
    const profileId = value || null
    setSettings({ ...settings, runtimeProfileId: profileId })
    postRequest('save-setting', {
      scope: 'runtime',
      key: 'profileId',
      value: profileId,
    })
  }
  const changeProfileDiscovery = (enabled: boolean) => {
    setSettings({
      ...settings,
      modulePreferences: {
        ...modulePreferences,
        gameProfileDiscovery: { ...modulePreferences.gameProfileDiscovery, enabled },
      },
    })
    postRequest('save-setting', {
      scope: 'module',
      module: 'gameProfileDiscovery',
      key: 'enabled',
      value: enabled,
    })
  }
  const currentLocale = locales.find((option) => option.locale === locale)!
  return (
    <>
      <PageHeader
        eyebrow={t('nav.modloader')}
        title={t('settings.title')}
        subtitle={t('settings.subtitle')}
        actions={
          <>
            <button className="button secondary" onClick={reload}>
              {t('common.reload')}
            </button>
            <button className="button" onClick={save}>
              {t('common.save')}
            </button>
          </>
        }
      />
      <Segmented
        value={tab}
        onChange={setTab}
        items={[
          ['general', t('settings.general')],
          ['modules', t('settings.modules')],
          ['paths', t('settings.paths.title')],
        ]}
      />
      {tab === 'general' && (
        <>
          <Card title={t('settings.username.title')}>
            <SettingsRow
              label={t('settings.username.mode')}
              detail={t('settings.username.mode.hint')}
              control={
                <select
                  value={settings.usernameMode || 'auto'}
                  onChange={(event) =>
                    changeUsernameMode(event.target.value as 'auto' | 'manual')
                  }
                >
                  <option value="auto">{t('settings.username.auto')}</option>
                  <option value="manual">
                    {t('settings.username.manual')}
                  </option>
                </select>
              }
            />
            <SettingsRow
              label={t('settings.username.effective')}
              detail={t(
                `settings.username.source.${settings.usernameSource || 'fallback'}` as TranslationKey,
              )}
              control={
                <strong className="settings-value">
                  {settings.usernameMode === 'manual'
                    ? messageNameDraft
                    : settings.effectiveUsername || messageNameDraft}
                </strong>
              }
            />
            <SettingsRow
              label={t('settings.username.fallback')}
              detail={t('settings.username.fallback.hint')}
              control={
                <input
                  className="message-name-input"
                  type="text"
                  maxLength={40}
                  value={messageNameDraft}
                  onChange={(event) => setMessageNameDraft(event.target.value)}
                  onBlur={(event) => {
                    const value =
                      event.currentTarget.value.trim() || 'Flameborn'
                    setMessageNameDraft(value)
                    changeMessageName(value)
                  }}
                />
              }
            />
          </Card>
          <Card title={t('settings.presentation')}>
            <Field label={t('settings.language')}>
              <span className="language-select">
                <Flag value={currentLocale.flag} />
                <select
                  value={locale}
                  onChange={(event) => chooseLocale(event.target.value)}
                >
                  {locales.map((option) => (
                    <option key={option.locale} value={option.locale}>
                      {option.name} ({option.code})
                    </option>
                  ))}
                </select>
              </span>
            </Field>
            <p className="hint">{t('settings.language.hint')}</p>
            <SettingsRow
              label={t('common.contentZoom')}
              detail={t('settings.zoom.hint')}
              control={
                <div className="zoom-setting-control">
                  <output className="zoom-setting-value">
                    {settings.zoomPercent ?? 100}%
                  </output>
                  <button
                    type="button"
                    className="button ghost"
                    disabled={(settings.zoomPercent ?? 100) === 100}
                    onClick={() => setZoomPercent(100)}
                  >
                    {t('settings.zoom.reset')}
                  </button>
                </div>
              }
            />
            <SettingsRow
              label={t('settings.startPage')}
              detail={t('settings.startPage.hint')}
              control={
                <select
                  value={configuredStartPage}
                  onChange={(event) =>
                    changeStartPage(event.target.value as PageId)
                  }
                >
                  {startPageOptions.map((option) => (
                    <option key={option} value={option}>
                      {t(`nav.${option}` as TranslationKey)}
                    </option>
                  ))}
                </select>
              }
            />
            <Toggle
              label={t('settings.compact')}
              checked={settings.compactMode}
              onChange={(value) => changeGeneral('compactMode', value)}
            />
            <Toggle
              label={t('settings.motion')}
              checked={settings.reducedMotion}
              onChange={(value) => changeGeneral('reducedMotion', value)}
            />
            <Toggle
              label={t('settings.nativeMenu')}
              checked={settings.showNativeModMenu}
              onChange={(value) => changeGeneral('showNativeModMenu', value)}
            />
          </Card>
          <Card title={t('settings.runtimeProfile.title')}>
            <p className="hint">
              {t('settings.runtimeProfile.active', {
                profile:
                  settings.activeRuntimeProfile ||
                  t('settings.runtimeProfile.notRunning'),
              })}
            </p>
            <SettingsRow
              label={t('settings.runtimeProfile.selection')}
              control={
                <select
                  value={settings.runtimeProfileId || ''}
                  onChange={(event) => changeRuntimeProfile(event.target.value)}
                >
                  <option value="">
                    {t('settings.runtimeProfile.automatic')}
                  </option>
                  {(settings.runtimeProfiles || []).map((id) => (
                    <option key={id} value={id}>
                      {id}
                    </option>
                  ))}
                </select>
              }
            />
            <p className="hint">
              {t('settings.runtimeProfile.hint', {
                directory:
                  settings.runtimeProfileDirectory ||
                  'shroudforge/runtime/profiles',
              })}
            </p>
            {settings.activeRuntimeProfile?.includes('build-diff-warning') && (
              <p className="hint">{t('settings.runtimeProfile.buildDiff')}</p>
            )}
            <GameProfileDiscoverySettings
              enabled={modulePreferences.gameProfileDiscovery?.enabled !== false}
              onChange={changeProfileDiscovery}
              wikiModuleSettings={modulePreferences.wiki}
            />
          </Card>
          <Card title={t('settings.logging.title')}>
            <Field label={t('settings.logging.minimumLevel')}>
              <select
                value={settings.logLevel}
                onChange={(event) => changeLogLevel(event.target.value)}
              >
                <option value="ERROR">
                  {t('settings.logging.level.error')}
                </option>
                <option value="WARN">{t('settings.logging.level.warn')}</option>
                <option value="INFO">{t('settings.logging.level.info')}</option>
                <option value="DEBUG">
                  {t('settings.logging.level.debug')}
                </option>
                <option value="TRACE">
                  {t('settings.logging.level.trace')}
                </option>
              </select>
            </Field>
            <p className="hint">{t('settings.logging.hint')}</p>
          </Card>
          <Card title={t('settings.exports.title')}>
            <Toggle
              label={t('settings.exports.enabled')}
              checked={settings.exportEnabled}
              onChange={changeExportEnabled}
            />
            <p className="hint">{t('settings.exports.hint')}</p>
          </Card>
          <GamefileBackupManagement data={data} />
        </>
      )}
      {tab === 'modules' && (
        <ModulePreferencesEditor
          data={data}
          settings={settings}
          setSettings={setSettings}
          notifyCommandResult={notifyCommandResult}
        />
      )}
      {tab === 'paths' && <StoragePathsEditor data={data} />}
    </>
  )
}
