import type { ModInfo } from './mod-types'
import type { Snapshot } from './ui-snapshot'
import type { SchematicInfo } from '../../schematics/ui/schematic-types'
import { useI18n } from '../../ui-resources/i18n'

import { Card } from '../../ui-resources/show-card'
import { EcosystemBadge } from '../../ui-resources/show-ecosystem-badge'
import { Empty } from '../../ui-resources/show-empty-state'
import { PageHeader } from '../../ui-resources/show-page-header'
import { ModIcon } from './show-mod-icon'
import { Segmented } from '../../ui-resources/choose-segment'
import { InstalledSchematics } from '../../schematics/ui/show-installed-schematics'
import { postRequest } from '../../transport/ui/send-host-command'

export function Installed({
  mods,
  modpacks,
  removable,
  schematics,
  schematicsPath,
  schematicConversionMode,
  schematicsEnabled,
  select,
  tab,
  setTab,
}: {
  mods: ModInfo[]
  modpacks: Snapshot['modpacks']
  removable: boolean
  schematics: SchematicInfo[]
  schematicsPath: string
  schematicConversionMode: 'manual' | 'automatic'
  schematicsEnabled: boolean
  select(id: string): void
  tab: 'mods' | 'schematics' | 'modpacks'
  setTab(tab: 'mods' | 'schematics' | 'modpacks'): void
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
        onChange={(value) => setTab(value as 'mods' | 'schematics' | 'modpacks')}
        items={[
          ['mods', t('installed.tab.mods')],
          ['modpacks', t('installed.tab.modpacks')],
          ...(schematicsEnabled
            ? ([['schematics', t('installed.tab.schematics')]] as [string, string][])
            : []),
        ]}
      />
      {visibleTab === 'modpacks' ? (
        <Card flush>
          {modpacks.length ? <div className="mod-list">
            {modpacks.map((pack) => <section className="mod-row" key={pack.id}>
              <span className="mod-name">
                <strong>{pack.name}</strong>
                <small>{pack.id} · {pack.mods.filter((item) => item.installed).length}/{pack.mods.filter((item) => item.selected).length} Mods</small>
                {pack.mods.map((item) => <small key={item.id}>
                  {item.id} · {item.source}{item.optional ? ` · ${t('modpack.member.optional')}` : ''}
                  {!item.selected ? ` · ${t('modpack.member.notSelected')}` : !item.installed ? ` · ${t('modpack.member.missing')}` : ''}
                </small>)}
              </span>
              <span className="mod-version">{pack.version}</span>
              {removable && <button className="button danger" onClick={() => {
                if (window.confirm(t('mod.remove.confirm', { name: pack.name })))
                  postRequest('remove-modpack', { modId: pack.id })
              }}>{t('mod.remove')}</button>}
            </section>)}
          </div> : <Empty title={t('modpack.empty')} text={t('modpack.empty')} icon="installed" />}
        </Card>
      ) : visibleTab === 'schematics' ? (
        <InstalledSchematics
          schematics={schematics}
          path={schematicsPath}
          conversionMode={schematicConversionMode}
        />
      ) : (
        <Card flush>
          {mods.length ? (
            <div className="mod-list">
              {mods.map((mod) => (
                <button
                  className="mod-row"
                  key={mod.instanceKey ?? mod.id}
                  onClick={() => select(mod.instanceKey ?? mod.id)}
                >
                  <ModIcon mod={mod} variant="list" />
                  <span className="mod-name">
                    <strong title={mod.name}>
                      {mod.name}
                      <EcosystemBadge ecosystem={mod.ecosystem} />
                    </strong>
                    <small title={mod.description || mod.id}>
                      {mod.packId ? `${mod.packId} · ` : ''}{mod.description || mod.id}
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
