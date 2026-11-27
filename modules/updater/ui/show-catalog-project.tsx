import { useEffect, useMemo, useState } from 'react'
import { CatalogMarkdown } from '../../modloader/ui/catalog-markdown'
import { useI18n } from '../../ui-resources/i18n'
import { Icon } from '../../ui-resources/show-icon'
import { EnvironmentBadge } from '../../ui-resources/show-environment-badge'
import { Loading } from '../../ui-resources/show-empty-state'
import { CatalogIcon } from './show-catalog-icon'
import { DiscoverFilter } from './show-discover-filter'
import type { CatalogGalleryImage, CatalogProject, CatalogProjectVersion } from './update-types'

type ProjectTab = 'description' | 'versions' | 'gallery'

type Props = {
  project: CatalogProject | null
  projectType: string
  tab: ProjectTab
  versionId: string
  versionPage: number
  channel: string[]
  gameVersion: string[]
  loading: boolean
  error?: string | null
  enabled: boolean
  installed: boolean
  onClose: () => void
  onTab: (tab: ProjectTab) => void
  onVersion: (versionId: string) => void
  onVersionList: () => void
  onVersionPage: (page: number) => void
  onVersionFilter: (channel: string[], gameVersion: string[]) => void
  onInstall: (versionId?: string) => void
  onOpenProject: (projectId: string) => void
  onOpenLink: (url: string) => void
}

function safeUrl(value: string | undefined): string | undefined {
  if (!value) return undefined
  try {
    const url = new URL(value)
    return url.protocol === 'https:' ? url.toString() : undefined
  } catch {
    return undefined
  }
}

function formatNumber(value: number | undefined, locale: string): string | undefined {
  if (typeof value !== 'number') return undefined
  return new Intl.NumberFormat(locale, { notation: 'compact', maximumFractionDigits: 2 }).format(value)
}

function formatDate(value: string | undefined, locale: string): string | undefined {
  if (!value) return undefined
  const date = new Date(value)
  if (!Number.isFinite(date.getTime())) return undefined
  return new Intl.DateTimeFormat(locale, { year: 'numeric', month: 'short', day: 'numeric' }).format(date)
}

function formatRelativeDate(value: string | undefined, locale: string): string | undefined {
  if (!value) return undefined
  const timestamp = Date.parse(value)
  if (!Number.isFinite(timestamp)) return undefined
  const delta = timestamp - Date.now()
  const units = [
    ['year', 31_536_000_000],
    ['month', 2_592_000_000],
    ['week', 604_800_000],
    ['day', 86_400_000],
    ['hour', 3_600_000],
    ['minute', 60_000],
  ] as const
  const formatter = new Intl.RelativeTimeFormat(locale, { numeric: 'auto' })
  for (const [unit, duration] of units) {
    if (Math.abs(delta) >= duration) return formatter.format(Math.round(delta / duration), unit)
  }
  return formatter.format(Math.round(delta / 1000), 'second')
}

function safeLinks(project: CatalogProject): Array<[string, string]> {
  const links: Array<[string, string]> = []
  for (const [name, value] of Object.entries(project.links || {})) {
    const url = safeUrl(typeof value === 'string' ? value : undefined)
    if (url) links.push([name, url])
  }
  return links
}

function serverFields(value: Record<string, unknown> | null | undefined) {
  if (!value) return []
  return Object.entries(value).filter(([, field]) => field !== null && field !== undefined && field !== '')
}

function versionChannel(version: CatalogProjectVersion) {
  return version.versionType || 'release'
}

function versionPageItems(current: number, total: number): Array<number | 'ellipsis'> {
  const pages = new Set([1, total, current - 1, current, current + 1])
  const ordered = [...pages].filter((page) => page >= 1 && page <= total).sort((left, right) => left - right)
  const items: Array<number | 'ellipsis'> = []
  ordered.forEach((page, index) => {
    if (index > 0 && page - ordered[index - 1] > 1) items.push('ellipsis')
    items.push(page)
  })
  return items
}

function galleryUrl(image: CatalogGalleryImage, raw = false) {
  return safeUrl(raw ? image.rawUrl || image.url : image.url)
}

export function CatalogProjectPage({
  project,
  projectType,
  tab,
  versionId,
  versionPage,
  channel,
  gameVersion,
  loading,
  error,
  enabled,
  installed,
  onClose,
  onTab,
  onVersion,
  onVersionList,
  onVersionPage,
  onVersionFilter,
  onInstall,
  onOpenProject,
  onOpenLink,
}: Props) {
  const { t, locale } = useI18n()
  const [viewerIndex, setViewerIndex] = useState<number | null>(null)
  const [zoom, setZoom] = useState(1)
  const [selectedPlatforms, setSelectedPlatforms] = useState<string[]>([])
  const versions = project?.versions || []
  const visibleGallery = (project?.gallery || []).filter((image) => Boolean(galleryUrl(image)))
  const filteredVersions = useMemo(() => versions.filter((version) => {
    const channelMatches = !channel.length || channel.includes(versionChannel(version))
    const gameMatches = !gameVersion.length || version.gameVersions.some((value) => gameVersion.includes(value))
    const platformMatches = !selectedPlatforms.length || version.loaders.some((loader) => selectedPlatforms.includes(loader))
    return channelMatches && gameMatches && platformMatches
  }), [versions, channel, gameVersion, selectedPlatforms])
  const pageSize = 20
  const pageCount = Math.max(1, Math.ceil(filteredVersions.length / pageSize))
  const boundedPage = Math.min(Math.max(1, versionPage), pageCount)
  const visibleVersions = filteredVersions.slice((boundedPage - 1) * pageSize, boundedPage * pageSize)
  const selectedVersion = versions.find((version) => version.id === versionId)
  const games = [...new Set(versions.flatMap((version) => version.gameVersions))]
  const channels = [...new Set(versions.map(versionChannel))]
  const platforms = [...new Set(versions.flatMap((version) => version.loaders))]
  const projectIsServer = projectType === 'server' || project?.projectTypes.includes('server')
  const projectLinks = project ? safeLinks(project) : []
  const hasSidebar = projectLinks.length > 0 || !projectIsServer || Boolean(project?.dependencies.length)
  const canInstall = projectType === 'mod' || projectType === 'schematic' || projectType === 'modpack'
  const latestVersion = versions[0]
  const latestVersionId = latestVersion?.id

  useEffect(() => {
    if (viewerIndex === null) return
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setViewerIndex(null)
      else if (event.key === 'ArrowLeft') setViewerIndex((index) => index === null ? null : (index + visibleGallery.length - 1) % visibleGallery.length)
      else if (event.key === 'ArrowRight') setViewerIndex((index) => index === null ? null : (index + 1) % visibleGallery.length)
      else if (event.key === '+' || event.key === '=') setZoom((value) => Math.min(3, value + 0.25))
      else if (event.key === '-') setZoom((value) => Math.max(0.5, value - 0.25))
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [viewerIndex, visibleGallery.length])

  useEffect(() => {
    if (viewerIndex === null) setZoom(1)
  }, [viewerIndex])

  useEffect(() => {
    setSelectedPlatforms([])
    setViewerIndex(null)
  }, [project?.id])

  const openLink = (url: string | undefined) => {
    const safe = safeUrl(url)
    if (safe) onOpenLink(safe)
  }
  const virusTotalLabel = (file: CatalogProjectVersion['files'][number]) => {
    if (!file.malware) return t('discover.virus.lookup')
    if (file.malware.failed) return t('discover.virus.failed')
    if (file.malware.status === 'passed') return t('discover.virus.passed')
    if (file.malware.status === 'manually_approved') return t('discover.virus.approved')
    if (file.malware.status === 'blocked') return t('discover.virus.blocked')
    return t('discover.virus.pending')
  }
  const renderVirusTotalButton = (file: CatalogProjectVersion['files'][number]) => {
    const hash = file.hashes?.sha256 || file.hashes?.sha1
    const url = safeUrl(file.malware?.analysis_url || undefined)
      || (hash && /^(?:[a-f0-9]{40}|[a-f0-9]{64})$/i.test(hash)
        ? `https://www.virustotal.com/gui/file/${hash}`
        : undefined)
    if (!url && !file.malware) return null
    const label = virusTotalLabel(file)
    return <button
      type="button"
      className={`discover-virustotal ${file.malware?.failed ? 'failed' : file.malware?.status || 'unknown'}`}
      onClick={() => openLink(url)}
      disabled={!url}
      title={label}
      aria-label={label}
    ><Icon name="virustotal" /><span>VirusTotal</span></button>
  }
  const renderInstallButton = (version?: string) => canInstall && (
    <button className="button discover-project-install" type="button" disabled={!enabled || !(version || latestVersionId)} onClick={() => onInstall(version || latestVersionId)}>
      <Icon name="download" />
      {projectType === 'schematic'
        ? t('schematics.import')
        : projectType === 'modpack'
          ? t('discover.detail.installPack')
          : installed ? t('discover.status.updateButton') : t('common.install')}
    </button>
  )

  return (
    <section className="discover-project-page card" aria-live="polite">
      {loading ? (
        <Loading text={t('discover.detail.loading')} />
      ) : error ? (
        <div className="discover-project-error">
          <p className="notice error" role="alert">{error || t('discover.detail.failed')}</p>
          <button className="button ghost" type="button" onClick={onClose}>{t('discover.detail.backToResults')}</button>
        </div>
      ) : project ? (
        <>
          <header className="discover-project-header">
            <CatalogIcon
              key={project.id}
              url={project.iconUrl}
              className="discover-project-icon"
            />
            <div className="discover-project-heading">
              <h2>{project.name}</h2>
              <p>{project.summary}</p>
              <div className="discover-project-stats">
                {typeof project.downloads === 'number' && <span><Icon name="download" /> {t(project.downloads === 1 ? 'discover.detail.download' : 'discover.detail.downloads', { count: formatNumber(project.downloads, locale) || '0' })}</span>}
                {typeof project.follows === 'number' && <span><Icon name="heart" /> {t(project.follows === 1 ? 'discover.detail.follower' : 'discover.detail.followers', { count: formatNumber(project.follows, locale) || '0' })}</span>}
                {project.author && <span>{project.author}</span>}
              </div>
              <div className="discover-badges">
                <EnvironmentBadge environment={versions[0]?.environment} catalog />
                {project.categories.map((category) => <span className="discover-badge" key={category}>{category}</span>)}
                {project.loaders.map((loader) => <span className="discover-badge loader" key={loader}>{loader}</span>)}
              </div>
            </div>
            <div className="discover-project-actions">
              <button className="button ghost" type="button" onClick={onClose}>{t('discover.detail.backToResults')}</button>
            </div>
          </header>

          {projectIsServer && project.enshroudedServer && (
            <section className="discover-project-data" aria-label={t('discover.detail.serverDetails')}>
              {serverFields(project.enshroudedServer).map(([key, value]) => (
                <div key={key}><span>{key.replaceAll('_', ' ')}</span><strong>{typeof value === 'object' ? JSON.stringify(value) : String(value)}</strong></div>
              ))}
            </section>
          )}
          {(projectType === 'schematic' || project.projectTypes.includes('schematic')) && project.schematic && (
            <section className="discover-project-data" aria-label={t('discover.detail.schematicDetails')}>
              {serverFields(project.schematic).map(([key, value]) => (
                <div key={key}><span>{key.replaceAll('_', ' ')}</span><strong>{typeof value === 'object' ? JSON.stringify(value) : String(value)}</strong></div>
              ))}
            </section>
          )}

          <nav className="discover-project-tabs" role="tablist" aria-label={project.name}>
            {([
              ['description', t('discover.detail.description')],
              ['versions', t('discover.detail.versions')],
              ['gallery', t('discover.detail.gallery')],
            ] as const).filter(([value]) => value !== 'gallery' || visibleGallery.length > 0).map(([value, label]) => (
              <button
                className={`discover-project-tab${tab === value ? ' active' : ''}`}
                type="button"
                role="tab"
                aria-selected={tab === value}
                key={value}
                onClick={() => onTab(value)}
              >
                {label}
              </button>
            ))}
          </nav>

          {tab === 'description' && (
            <div className={`discover-project-content${hasSidebar ? '' : ' no-sidebar'}`}>
              <section className="discover-project-description" aria-label={t('discover.detail.description')}>
                <CatalogMarkdown value={project.description || project.summary || ''} />
              </section>
              {hasSidebar && <aside className="discover-project-sidebar">
                {projectLinks.length > 0 && (
                  <section>
                    <h3>{t('discover.detail.links')}</h3>
                    <div className="discover-project-links">
                      {projectLinks.map(([name, url]) => (
                        <button type="button" key={name} onClick={() => openLink(url)}>{name.replaceAll('_', ' ')}</button>
                      ))}
                    </div>
                  </section>
                )}
                {!projectIsServer && (
                  <section>
                    <h3>{t('discover.detail.installVersion')}</h3>
                    {latestVersion ? <>
                      <strong className="discover-latest-version">{latestVersion.versionNumber || latestVersion.name}</strong>
                      <div className="discover-project-tags">
                        {latestVersion.gameVersions.map((value) => <span className="discover-badge" key={value}>{value}</span>)}
                        {latestVersion.loaders.map((value) => <span className="discover-badge loader" key={value}>{value}</span>)}
                      </div>
                      {renderInstallButton(latestVersionId)}
                      {latestVersion.published && <p className="discover-project-date">{t('discover.detail.published')} {formatDate(latestVersion.published, locale)}</p>}
                    </> : <p className="muted">{t('discover.detail.noVersions')}</p>}
                  </section>
                )}
                {project.dependencies.length > 0 && (
                  <section>
                    <h3>{t('mod.dependencies.title')}</h3>
                    <ul className="discover-dependencies">
                      {project.dependencies.map((dependency) => (
                        <li key={dependency.id}>
                          <button className="discover-dependency-link" type="button" onClick={() => onOpenProject(dependency.id)}>{dependency.name || dependency.id}</button>
                          {dependency.summary && <small>{dependency.summary}</small>}
                        </li>
                      ))}
                    </ul>
                  </section>
                )}
              </aside>}
            </div>
          )}

          {tab === 'versions' && !versionId && (
            <section className="discover-versions-page">
              <div className="discover-version-filters">
                <DiscoverFilter
                  name="discover-version-channel"
                  className="version-channel"
                  multiple
                  allValue="all"
                  label={t('discover.detail.channel')}
                  summary="label"
                  value={channel}
                  options={[
                    { value: 'all', label: t('common.all') },
                    ...channels.map((value) => ({ value, label: value })),
                  ]}
                  onChange={(nextChannel) => onVersionFilter(nextChannel, gameVersion)}
                />
                <DiscoverFilter
                  name="discover-version-game"
                  className="version-game"
                  multiple
                  allValue=""
                  label={t('discover.filter.gameVersion')}
                  summary="label"
                  value={gameVersion}
                  options={[
                    { value: '', label: t('common.all') },
                    ...games.map((value) => ({ value, label: value })),
                  ]}
                  onChange={(nextGameVersion) => onVersionFilter(channel, nextGameVersion)}
                />
                {platforms.length > 0 && (
                  <DiscoverFilter
                    name="discover-version-platform"
                    className="version-channel"
                    multiple
                    allValue=""
                    label={t('discover.detail.platform')}
                    summary="label"
                    value={selectedPlatforms}
                    options={[
                      { value: '', label: t('common.all') },
                      ...platforms.map((value) => ({ value, label: value })),
                    ]}
                    onChange={(nextPlatforms) => {
                      setSelectedPlatforms(nextPlatforms)
                      onVersionPage(1)
                    }}
                  />
                )}
                {(selectedPlatforms.length > 0 || channel.length > 0 || gameVersion.length > 0) && (
                  <button className="discover-clear-filters" type="button" onClick={() => {
                    setSelectedPlatforms([])
                    onVersionFilter([], [])
                  }}>{t('discover.detail.clearFilters')}</button>
                )}
                <span className="discover-result-count">{t(filteredVersions.length === 1 ? 'discover.detail.versionCount' : 'discover.detail.versionsCount', { count: filteredVersions.length })}</span>
                <div className="discover-version-pagination">
                  <button className="discover-page-next" type="button" aria-label={t('wiki.previousPage')} disabled={boundedPage <= 1} onClick={() => onVersionPage(boundedPage - 1)}><Icon name="chevron" /></button>
                  <div className="discover-page-numbers">
                    {versionPageItems(boundedPage, pageCount).map((value, index) => value === 'ellipsis'
                      ? <span className="discover-page-ellipsis" key={`version-ellipsis-${index}`} aria-hidden="true">…</span>
                      : <button
                          className={`discover-page-number${boundedPage === value ? ' active' : ''}`}
                          type="button"
                          key={value}
                          aria-label={t('wiki.pageIndicator', { current: value, total: pageCount })}
                          aria-current={boundedPage === value ? 'page' : undefined}
                          onClick={() => onVersionPage(value)}
                        >{value}</button>)}
                  </div>
                  <button className="discover-page-next" type="button" aria-label={t('wiki.nextPage')} disabled={boundedPage >= pageCount} onClick={() => onVersionPage(boundedPage + 1)}><Icon name="chevron" /></button>
                </div>
              </div>
              <div className="discover-versions-table-wrap">
                <table className="discover-versions-table">
                  <thead><tr><th>{t('discover.detail.version')}</th><th>{t('discover.filter.gameVersion')}</th><th>{t('discover.detail.platform')}</th><th>{t('discover.detail.published')}</th><th>{t('discover.sort.downloads')}</th><th /></tr></thead>
                  <tbody>
                    {visibleVersions.map((version) => {
                      const file = version.files.find((item) => item.primary) || version.files[0]
                      return (
                      <tr key={version.id} onClick={(event) => {
                        if (!(event.target as HTMLElement).closest('button, a, input, select')) onVersion(version.id)
                      }}>
                        <td>
                          <button className="discover-version-row-link" type="button" onClick={() => onVersion(version.id)}>
                            <span className={`discover-channel-mark ${versionChannel(version)}`}>{versionChannel(version)}</span>
                            <span><strong>{version.versionNumber || version.name || version.id}</strong><small>{version.name && version.name !== version.versionNumber ? version.name : ''}</small></span>
                          </button>
                        </td>
                        <td>{version.gameVersions.map((value) => <span className="discover-badge" key={value}>{value}</span>)}</td>
                        <td>{version.loaders.map((value) => <span className="discover-badge loader" key={value}>{value}</span>)}<EnvironmentBadge environment={version.environment} catalog /></td>
                        <td>{formatRelativeDate(version.published, locale) || formatDate(version.published, locale) || '—'}</td>
                        <td>{formatNumber(version.downloads, locale) || '—'}</td>
                        <td className="discover-version-actions">
                          {file && renderVirusTotalButton(file)}
                          {canInstall && <button className="discover-version-install" type="button" disabled={!enabled} aria-label={t('common.install')} title={t('common.install')} onClick={() => onInstall(version.id)}><Icon name="download" /></button>}
                        </td>
                      </tr>
                    )})}
                    {!visibleVersions.length && <tr><td className="discover-empty-cell" colSpan={6}>{t('discover.detail.noVersions')}</td></tr>}
                  </tbody>
                </table>
              </div>
            </section>
          )}

          {tab === 'versions' && versionId && (
            <section className="discover-version-detail">
              <button className="discover-back-to-versions" type="button" onClick={onVersionList}>← {t('discover.detail.versions')}</button>
              {selectedVersion ? (
                <>
                  <div className="discover-version-detail-heading">
                    <div>
                      <h3>{selectedVersion.versionNumber || selectedVersion.name}</h3>
                      <p>{selectedVersion.name}</p>
                    </div>
                    {renderInstallButton(selectedVersion.id)}
                  </div>
                  <div className="discover-version-detail-meta">
                    <EnvironmentBadge environment={selectedVersion.environment} catalog />
                    <span>{versionChannel(selectedVersion)}</span>
                    {selectedVersion.gameVersions.map((value) => <span className="discover-badge" key={value}>{value}</span>)}
                    {selectedVersion.loaders.map((value) => <span className="discover-badge loader" key={value}>{value}</span>)}
                    <span>{formatDate(selectedVersion.published, locale)}</span>
                    {typeof selectedVersion.downloads === 'number' && <span>{t(selectedVersion.downloads === 1 ? 'discover.detail.download' : 'discover.detail.downloads', { count: formatNumber(selectedVersion.downloads, locale) || '0' })}</span>}
                  </div>
                  <div className="discover-detail-columns">
                    <section><h3>{t('discover.detail.changelog')}</h3><CatalogMarkdown value={selectedVersion.changelog || t('discover.detail.noChangelog')} /></section>
                    <section>
                      <h3>{t('discover.detail.files')}</h3>
                      <div className="discover-version-files">
                        {selectedVersion.files.map((file, index) => (
                          <div key={file.id || file.filename || index}>
                            <div><strong>{file.filename || file.id || t('discover.detail.projectFile')}</strong><small>{typeof file.size === 'number' ? `${(file.size / 1024 / 1024).toFixed(2)} MB` : ''}</small></div>
                            <span className="discover-file-actions">
                              {renderVirusTotalButton(file)}
                              {file.url && <button type="button" onClick={() => openLink(file.url)}><Icon name="download" /> {t('discover.detail.downloadFile')}</button>}
                            </span>
                          </div>
                        ))}
                        {!selectedVersion.files.length && <p className="muted">{t('discover.detail.noFiles')}</p>}
                      </div>
                    </section>
                  </div>
                  {selectedVersion.dependencies.length > 0 && (
                    <section className="discover-version-dependencies">
                      <h3>{t('mod.dependencies.title')}</h3>
                      {selectedVersion.dependencies.map((dependency, index) => {
                        const id = String(dependency.project_id || dependency.projectId || '')
                        return <button type="button" key={String(dependency.version_id || id || index)} onClick={() => id && onOpenProject(id)}>{id || String(dependency.dependency_type || t('discover.detail.dependency'))}</button>
                      })}
                    </section>
                  )}
                </>
              ) : <p className="muted">{t('discover.detail.noVersions')}</p>}
            </section>
          )}

          {tab === 'gallery' && (
            <div className="discover-gallery-grid">
              {visibleGallery.map((image, index) => (
                <article className="discover-gallery-card" key={`${image.url}-${index}`}>
                  <button className="discover-gallery-image-button" type="button" onClick={() => setViewerIndex(index)}>
                    <img src={galleryUrl(image)} alt={image.title || project.name} loading="lazy" />
                  </button>
                  <div className="discover-gallery-copy">
                    <h3>{image.title || project.name}</h3>
                    {image.description && <p>{image.description}</p>}
                    {image.created && <time>{formatDate(image.created, locale)}</time>}
                  </div>
                </article>
              ))}
            </div>
          )}
        </>
      ) : null}

      {project && viewerIndex !== null && visibleGallery[viewerIndex] && (
        <div className="discover-image-viewer" role="dialog" aria-modal="true" aria-label={t('discover.detail.gallery')} onMouseDown={(event) => { if (event.target === event.currentTarget) setViewerIndex(null) }}>
          <header>
            <div><strong>{visibleGallery[viewerIndex].title || project.name}</strong><span>{viewerIndex + 1} / {visibleGallery.length}</span></div>
            <div className="discover-image-viewer-actions">
              <button type="button" aria-label={t('common.zoomOut')} onClick={() => setZoom((value) => Math.max(0.5, value - 0.25))}>−</button>
              <span>{Math.round(zoom * 100)}%</span>
              <button type="button" aria-label={t('common.zoomIn')} onClick={() => setZoom((value) => Math.min(3, value + 0.25))}>+</button>
              <button type="button" aria-label={t('discover.detail.openImage')} onClick={() => openLink(galleryUrl(visibleGallery[viewerIndex], true))}>↗</button>
              <button type="button" aria-label={t('discover.detail.closeGallery')} onClick={() => setViewerIndex(null)}>×</button>
            </div>
          </header>
          <button className="discover-viewer-previous" type="button" aria-label={t('wiki.previousPage')} onClick={() => setViewerIndex((index) => index === null ? null : (index + visibleGallery.length - 1) % visibleGallery.length)}>‹</button>
          <figure>
            <img src={galleryUrl(visibleGallery[viewerIndex], true) || galleryUrl(visibleGallery[viewerIndex])} alt={visibleGallery[viewerIndex].title || project.name} style={{ transform: `scale(${zoom})` }} />
            {visibleGallery[viewerIndex].description && <figcaption>{visibleGallery[viewerIndex].description}</figcaption>}
          </figure>
          <button className="discover-viewer-next" type="button" aria-label={t('wiki.nextPage')} onClick={() => setViewerIndex((index) => index === null ? null : (index + 1) % visibleGallery.length)}>›</button>
        </div>
      )}
    </section>
  )
}
