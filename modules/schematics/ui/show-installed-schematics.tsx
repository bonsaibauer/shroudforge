import { useRef, useState } from 'react'
import { postRequest } from '../../transport/ui/send-host-command'
import type { SchematicInfo } from './schematic-types'
import { useI18n } from '../../ui-resources/i18n'
import { Card } from '../../ui-resources/show-card'
import { Empty } from '../../ui-resources/show-empty-state'
import './styles.css'

function formatSize(size: number) {
  if (size < 1024 * 1024) return `${Math.max(1, Math.round(size / 1024))} KB`
  return `${(size / (1024 * 1024)).toFixed(1)} MB`
}

export function InstalledSchematics({
  schematics,
  path,
}: {
  schematics: SchematicInfo[]
  path: string
}) {
  const { t } = useI18n()
  const coverInput = useRef<HTMLInputElement>(null)
  const schematicInput = useRef<HTMLInputElement>(null)
  const [coverTarget, setCoverTarget] = useState<string | null>(null)
  const manage = (action: string, id: string, value?: string) =>
    postRequest('manage-schematic', { action, id, value })
  const rename = (schematic: SchematicInfo) => {
    const name = window.prompt(t('schematics.namePrompt'), schematic.name)?.trim()
    if (name) manage('rename', schematic.name, name)
  }
  const duplicate = (schematic: SchematicInfo) => {
    const name = window.prompt(t('schematics.copyNamePrompt'), `${schematic.name}_copy`)?.trim()
    if (name) manage('duplicate', schematic.name, name)
  }
  const chooseCover = (name: string) => {
    setCoverTarget(name)
    coverInput.current?.click()
  }
  const readCover = (file?: File) => {
    if (!file || !coverTarget) return
    if (file.size > 64 * 1024 * 1024) {
      window.alert(t('schematics.importTooLarge'))
      setCoverTarget(null)
      return
    }
    const reader = new FileReader()
    reader.onload = () => {
      if (typeof reader.result === 'string') {
        manage('set-cover', coverTarget, reader.result)
      }
      setCoverTarget(null)
    }
    reader.onerror = () => setCoverTarget(null)
    reader.readAsDataURL(file)
  }
  const importSchematic = (file?: File) => {
    if (!file) return
    if (file.size > 64 * 1024 * 1024) {
      window.alert(t('schematics.importTooLarge'))
      return
    }
    const suggestedName = file.name
      .replace(/\.sfbp$/i, '')
      .replace(/[^a-zA-Z0-9_-]/g, '_')
      .slice(0, 64) || 'schematic'
    const name = window.prompt(t('schematics.importNamePrompt'), suggestedName)?.trim()
    if (!name) return
    const reader = new FileReader()
    reader.onload = () => {
      if (typeof reader.result === 'string') manage('import', name, reader.result)
    }
    reader.onerror = () => window.alert(t('schematics.importFailed'))
    reader.readAsDataURL(file)
  }

  return (
    <Card
      title={t('installed.tab.schematics')}
      actions={
        <div className="schematic-library-actions">
          <button className="button ghost" onClick={() => schematicInput.current?.click()}>
            {t('schematics.import')}
          </button>
          <button className="button ghost" onClick={() => postRequest('open-path', { key: 'schematics' })}>
            {t('settings.paths.open')}
          </button>
        </div>
      }
    >
      <div className="schematic-library-path" title={path}>{path}</div>
      <input
        ref={coverInput}
        className="schematic-cover-input"
        type="file"
        accept="image/png,image/jpeg"
        onChange={(event) => {
          readCover(event.currentTarget.files?.[0])
          event.currentTarget.value = ''
        }}
      />
      <input
        ref={schematicInput}
        className="schematic-cover-input"
        type="file"
        accept=".sfbp,application/octet-stream"
        onChange={(event) => {
          importSchematic(event.currentTarget.files?.[0])
          event.currentTarget.value = ''
        }}
      />
      {schematics.length === 0 ? (
        <Empty
          title={t('schematics.empty.title')}
          text={t('schematics.empty.text')}
          icon="installed"
        />
      ) : (
        <div className="schematic-grid">
          {schematics.map((schematic) => (
            <article className="schematic-card" key={schematic.name}>
              <div className="schematic-cover">
                {schematic.image ? (
                  <img src={schematic.image} alt="" />
                ) : (
                  <span>SFBP</span>
                )}
              </div>
              <div className="schematic-card-content">
                <h3 title={schematic.name}>{schematic.name}</h3>
                <p className="schematic-origin">
                  <span>{t(schematic.source === 'shroudedit' ? 'schematics.source.shroudedit' : 'schematics.source.local')}</span>
                  {schematic.projectId && <small>{schematic.projectId}{schematic.versionId ? ` · ${schematic.versionId}` : ''}</small>}
                  <small>{formatSize(schematic.size)}</small>
                </p>
                {!schematic.valid && <p className="schematic-invalid">{t('schematics.invalid')}</p>}
                <div className="schematic-actions">
                  <button className="button ghost" onClick={() => rename(schematic)}>{t('schematics.rename')}</button>
                  <button className="button ghost" onClick={() => duplicate(schematic)}>{t('schematics.duplicate')}</button>
                  <button className="button ghost" onClick={() => chooseCover(schematic.name)}>{t('schematics.cover')}</button>
                  <button
                    className="button ghost danger"
                    onClick={() => window.confirm(t('schematics.deleteConfirm')) && manage('delete', schematic.name)}
                  >
                    {t('schematics.delete')}
                  </button>
                </div>
              </div>
            </article>
          ))}
        </div>
      )}
    </Card>
  )
}
