import type { ModInfo } from './mod-types'
import type { SchematicInfo } from '../../schematics/ui/schematic-types'
import { useI18n } from '../../ui-resources/i18n'

import { Card } from '../../ui-resources/show-card'
import { EcosystemBadge } from '../../ui-resources/show-ecosystem-badge'
import { Empty } from '../../ui-resources/show-empty-state'
import { PageHeader } from '../../ui-resources/show-page-header'
import { ModIcon } from './show-mod-icon'
import { Segmented } from '../../ui-resources/choose-segment'
import { InstalledSchematics } from '../../schematics/ui/show-installed-schematics'

export function Installed({
  mods,
  schematics,
  schematicsPath,
  schematicsEnabled,
  select,
  tab,
  setTab,
}: {
  mods: ModInfo[]
  schematics: SchematicInfo[]
  schematicsPath: string
  schematicsEnabled: boolean
  select(id: string): void
  tab: 'mods' | 'schematics'
  setTab(tab: 'mods' | 'schematics'): void
}) {
  const { t } = useI18n()
  const visibleTab = tab === 'schematics' && !schematicsEnabled ? 'mods' : tab
  return (
    <>
      <PageHeader
        eyebrow={t('nav.modsManagement')}
        title={t('installed.title')}
        subtitle={t('installed.subtitle')}
      />
      <Segmented
        value={visibleTab}
        onChange={(value) => setTab(value as 'mods' | 'schematics')}
        items={[
          ['mods', t('installed.tab.mods')],
          ...(schematicsEnabled
            ? ([['schematics', t('installed.tab.schematics')]] as [string, string][])
            : []),
        ]}
      />
      {visibleTab === 'schematics' ? (
        <InstalledSchematics
          schematics={schematics}
          path={schematicsPath}
        />
      ) : (
        <Card flush>
          {mods.length ? (
            <div className="mod-list">
              {mods.map((mod) => (
                <button
                  className="mod-row"
                  key={mod.id}
                  onClick={() => select(mod.id)}
                >
                  <ModIcon mod={mod} variant="list" />
                  <span className="mod-name">
                    <strong title={mod.name}>
                      {mod.name}
                      <EcosystemBadge ecosystem={mod.ecosystem} />
                    </strong>
                    <small title={mod.description || mod.id}>
                      {mod.description || mod.id}
                    </small>
                  </span>
                  <span className="mod-version">{mod.version}</span>
                  <span className="chevron">›</span>
                </button>
              ))}
            </div>
          ) : (
            <Empty
              title={t('installed.empty.title')}
              text={t('installed.empty.text')}
              icon="installed"
            />
          )}
        </Card>
      )}
    </>
  )
}
