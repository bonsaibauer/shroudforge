import { ModuleEnabledRow } from '../../ui-resources/edit-setting'
import { useI18n } from '../../ui-resources/i18n'
import { WikiHelp } from '../../wiki/ui/show-wiki-help'
import type { WikiModuleSettings } from '../../wiki/mediawiki-api'

export function GameProfileDiscoverySettings({
  enabled,
  onChange,
  wikiModuleSettings,
}: {
  enabled: boolean
  onChange(enabled: boolean): void
  wikiModuleSettings?: WikiModuleSettings
}) {
  const { t } = useI18n()
  return (
    <>
      <p className="hint settings-module-description">
        {t('settings.modules.gameProfileDiscovery.description')}
      </p>
      <div className="settings-options">
        <ModuleEnabledRow
          label={t('settings.modules.gameProfileDiscovery.title')}
          enabled={enabled}
          onChange={onChange}
        />
      </div>
      <p className="hint settings-module-hint">
        {t('settings.modules.gameProfileDiscovery.restartHint')}
      </p>
      <WikiHelp articleId="game-profile-discovery" compact moduleSettings={wikiModuleSettings} />
    </>
  )
}
