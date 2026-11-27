import type React from 'react'
import { useEffect, useRef, useState } from 'react'
import { useI18n } from '../../ui-resources/i18n'
import type {
  Catalog,
  CatalogItem,
  ModUpdate,
  UpdaterCommand,
} from './update-types'
import { Empty, Loading } from '../../ui-resources/show-empty-state'
import { Icon } from '../../ui-resources/show-icon'
import { EnvironmentBadge } from '../../ui-resources/show-environment-badge'
import { PageHeader } from '../../ui-resources/show-page-header'
import { postRequest } from '../../transport/ui/send-host-command'
import { UpdaterAvailability } from './show-updater-availability'
import { CatalogProjectPage } from './show-catalog-project'
import { CatalogIcon } from './show-catalog-icon'
import { DiscoverFilter } from './show-discover-filter'
import type { ModInfo } from '../../modloader/ui/mod-types'
import type { Snapshot } from '../../modloader/ui/ui-snapshot'
import './discover.css'

type SearchFilters = {
  query: string
  projectType: 'mod' | 'schematic' | 'modpack' | 'server'
  loader: string[]
  category: string[]
  gameVersion: string[]
  environment: string[]
  sort: string
  limit: number
}

const emptyCatalog: Catalog = {
  state: 'idle',
  query: '',
  projectType: 'mod',
  projectTypes: [],
  loader: [],
  category: [],
  gameVersion: [],
  sort: 'relevance',
  limit: 20,
  page: 1,
  total: 0,
  items: [],
  categories: [],
  gameVersions: [],
}

function filtersFromUrl(catalog?: Catalog): SearchFilters {
  const params = new URLSearchParams(window.location.search)
  const requestedType = params.get('type')
  const projectType = ['mod', 'schematic', 'modpack', 'server'].includes(requestedType || '')
    ? (requestedType as SearchFilters['projectType'])
    : (['mod', 'schematic', 'modpack', 'server'].includes(catalog?.projectType || '')
        ? catalog?.projectType as SearchFilters['projectType']
        : 'mod')
  return {
    query: params.get('q') ?? catalog?.query ?? '',
    projectType,
    loader: filterValues(params.get('loader') ?? catalog?.loader, 'all'),
    category: filterValues(params.get('category') ?? catalog?.category),
    gameVersion: filterValues(params.get('gameVersion') ?? catalog?.gameVersion),
    environment: (params.get('environment') || '').split(',').filter((value) => value === 'client' || value === 'server'),
    sort: params.get('sort') ?? catalog?.sort ?? 'relevance',
    limit: pageSize(params.get('limit') ?? catalog?.limit),
  }
}

const discoverProjectTypes = [
  { value: 'mod', labelKey: 'discover.type.mods', singularKey: 'discover.type.mod' },
  { value: 'schematic', labelKey: 'discover.type.schematics', singularKey: 'discover.type.schematic' },
  { value: 'modpack', labelKey: 'discover.type.modpacks', singularKey: 'discover.type.modpack' },
  { value: 'server', labelKey: 'discover.type.servers', singularKey: 'discover.type.server' },
] as const

type ProjectTab = 'description' | 'versions' | 'gallery'
const CATALOG_SEARCH_DEBOUNCE_MS = 300

function readProjectTab(value: string | null): ProjectTab {
  return value === 'versions' || value === 'gallery' ? value : 'description'
}

function strings(values: string[] | undefined) {
  return (values || []).filter(Boolean)
}

function filterValues(value: string | string[] | null | undefined, allValue = '') {
  const values = Array.isArray(value) ? value : (value || '').split(',')
  return [...new Set(values.map((entry) => entry.trim()).filter((entry) => entry && entry !== allValue))]
}

function pageSize(value: string | number | undefined, fallback = 20) {
  const parsed = Number(value)
  return [20, 40, 60].includes(parsed) ? parsed : fallback
}

function paginationItems(current: number, total: number): Array<number | 'ellipsis'> {
  const pages = new Set([1, total])
  if (current <= 2) {
    pages.add(2)
    pages.add(3)
  } else if (current >= total - 1) {
    pages.add(total - 2)
    pages.add(total - 1)
  } else {
    pages.add(current)
    pages.add(current + 1)
  }

  const sorted = [...pages].filter((value) => value >= 1 && value <= total).sort((a, b) => a - b)
  const items: Array<number | 'ellipsis'> = []
  sorted.forEach((value, index) => {
    if (index > 0 && value - sorted[index - 1] > 1) items.push('ellipsis')
    items.push(value)
  })
  return items
}

function relativeDate(value: string | null | undefined, locale: string) {
  if (!value) return undefined
  const timestamp = Date.parse(value)
  if (!Number.isFinite(timestamp)) return undefined

  const delta = timestamp - Date.now()
  const formatter = new Intl.RelativeTimeFormat(locale, { numeric: 'auto' })
  const units = [
    ['year', 31_536_000_000],
    ['month', 2_592_000_000],
    ['week', 604_800_000],
    ['day', 86_400_000],
    ['hour', 3_600_000],
    ['minute', 60_000],
  ] as const

  for (const [unit, milliseconds] of units) {
    if (Math.abs(delta) >= milliseconds) {
      return formatter.format(Math.round(delta / milliseconds), unit)
    }
  }
  return formatter.format(Math.round(delta / 1000), 'second')
}

function compactNumber(value: number, locale: string) {
  return new Intl.NumberFormat(locale, {
    notation: 'compact',
    maximumFractionDigits: 2,
  }).format(value)
}

export function Discover({
  catalog: currentCatalog,
  updaterAvailable,
  enabled,
  installedMods,
  installedPacks,
  modUpdates,
  gameVersion,
  onCommand,
}: {
  catalog?: Catalog
  updaterAvailable: boolean
  enabled: boolean
  installedMods: Pick<ModInfo, 'id' | 'version'>[]
  installedPacks: Snapshot['modpacks']
  modUpdates: ModUpdate[]
  gameVersion: string
  onCommand: UpdaterCommand
}) {
  const { t, locale } = useI18n()
  const catalog = currentCatalog || emptyCatalog
  const [filters, setFilters] = useState(() => filtersFromUrl(currentCatalog))
  const [page, setPage] = useState(() =>
    Math.max(1, Number(new URLSearchParams(window.location.search).get('page')) || catalog.page || 1),
  )
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(() =>
    new URLSearchParams(window.location.search).get('project'),
  )
  const [projectTab, setProjectTab] = useState<ProjectTab>(() =>
    readProjectTab(new URLSearchParams(window.location.search).get('tab')),
  )
  const [versionsPage, setVersionsPage] = useState(() =>
    Math.max(1, Number(new URLSearchParams(window.location.search).get('vpage')) || 1),
  )
  const [versionChannel, setVersionChannel] = useState(
    () => filterValues(new URLSearchParams(window.location.search).get('channel'), 'all'),
  )
  const [versionGame, setVersionGame] = useState(
    () => filterValues(new URLSearchParams(window.location.search).get('versionGame')),
  )
  const [selectedVersionId, setSelectedVersionId] = useState(
    () => new URLSearchParams(window.location.search).get('version') || '',
  )
  const discoverPageRef = useRef<HTMLDivElement>(null)
  const searchTimer = useRef<number | undefined>()
  const [detailLoading, setDetailLoading] = useState(false)
  const [pendingPack, setPendingPack] = useState<{ projectId: string; slug: string; versionId: string; title: string } | null>(null)
  const [selectedOptionals, setSelectedOptionals] = useState<string[]>([])
  const initializedPreview = useRef('')
  const started = useRef(false)
  const actionsEnabled = updaterAvailable && enabled
  const detail = catalog.detail?.id === selectedProjectId ? catalog.detail : null
  const installedById = new Map(installedMods.map((mod) => [mod.id, mod]))
  const preview = pendingPack && catalog.packPreview?.id === pendingPack.slug && catalog.packPreview?.catalogVersionId === pendingPack.versionId ? catalog.packPreview : null
  const previewMemberId = (item: { id?: string; path?: string }) => item.id || item.path?.split('/').at(-1) || ''
  const requestPackPreview = (projectId: string, slug: string, versionId: string, title: string) => {
    initializedPreview.current = ''
    setPendingPack({ projectId, slug, versionId, title })
    setSelectedOptionals([])
    onCommand('preview-modpack', { projectId, versionId })
  }
  useEffect(() => {
    if (preview && catalog.packPreviewState === 'ready') {
      const key = `${pendingPack?.projectId}:${pendingPack?.versionId}`
      if (initializedPreview.current === key) return
      const existing = installedPacks.find((pack) => pack.id === preview.id)
      setSelectedOptionals(preview.mods.filter((item) => item.optional && (!existing || existing.mods.some((member) => member.id === previewMemberId(item) && member.selected))).map(previewMemberId))
      initializedPreview.current = key
    }
  }, [preview, catalog.packPreviewState, installedPacks, pendingPack])
  const updateById = new Map(modUpdates.map((update) => [update.modId, update]))
  const updateByProjectId = new Map(modUpdates.map((update) => [update.projectId, update]))

  const writeUrl = (
    next: SearchFilters,
    nextPage: number,
    projectId: string | null,
    nextTab: ProjectTab = 'description',
    versionId?: string | null,
    nextVersionsPage = 1,
    nextVersionChannel = versionChannel,
    nextVersionGame = versionGame,
  ) => {
    const params = new URLSearchParams(window.location.search)
    const values: Record<string, string> = {
      q: next.query,
      type: next.projectType,
      loader: next.loader.join(','),
      category: next.category.join(','),
      gameVersion: next.gameVersion.join(','),
      environment: next.environment.join(','),
      sort: next.sort,
      limit: String(next.limit),
      page: String(nextPage),
    }
    for (const [key, value] of Object.entries(values)) {
      if (value) params.set(key, value)
      else params.delete(key)
    }
    if (projectId) {
      params.set('project', projectId)
      params.set('tab', nextTab)
      if (versionId) params.set('version', versionId)
      else params.delete('version')
      if (nextVersionsPage > 1) params.set('vpage', String(nextVersionsPage))
      else params.delete('vpage')
      if (nextVersionChannel.length) params.set('channel', nextVersionChannel.join(','))
      else params.delete('channel')
      if (nextVersionGame.length) params.set('versionGame', nextVersionGame.join(','))
      else params.delete('versionGame')
    } else {
      params.delete('project')
      params.delete('tab')
      params.delete('version')
      params.delete('vpage')
      params.delete('channel')
      params.delete('versionGame')
    }
    const suffix = params.toString()
    window.history.pushState(
      { shroudforgeDiscover: true },
      '',
      `${window.location.pathname}${suffix ? `?${suffix}` : ''}${window.location.hash}`,
    )
  }

  const searchCatalog = (nextFilters: SearchFilters, nextPage = 1) => {
    setFilters(nextFilters)
    setPage(nextPage)
    setSelectedProjectId(null)
    setSelectedVersionId('')
    setProjectTab('description')
    setVersionsPage(1)
    setVersionChannel([])
    setVersionGame([])
    setDetailLoading(false)
    writeUrl(nextFilters, nextPage, null)
    if (searchTimer.current !== undefined) window.clearTimeout(searchTimer.current)
    searchTimer.current = window.setTimeout(() => {
      searchTimer.current = undefined
      onCommand('search-catalog', { ...nextFilters, page: nextPage })
    }, CATALOG_SEARCH_DEBOUNCE_MS)
  }

  const openProjectById = (projectId: string) => {
    setSelectedProjectId(projectId)
    setSelectedVersionId('')
    setProjectTab('description')
    setVersionsPage(1)
    setVersionChannel([])
    setVersionGame([])
    setDetailLoading(true)
    writeUrl(filters, page, projectId, 'description', null, 1, [], [])
    onCommand('load-catalog-project', { projectId })
  }

  const openProject = (item: CatalogItem) => {
    openProjectById(item.id)
  }

  const navigateProject = (
    nextTab: ProjectTab,
    versionId: string | null = null,
    nextVersionsPage = 1,
    nextVersionChannel = versionChannel,
    nextVersionGame = versionGame,
  ) => {
    if (!selectedProjectId) return
    setProjectTab(nextTab)
    setSelectedVersionId(versionId || '')
    setVersionsPage(nextVersionsPage)
    setVersionChannel(nextVersionChannel)
    setVersionGame(nextVersionGame)
    writeUrl(
      filters,
      page,
      selectedProjectId,
      nextTab,
      versionId,
      nextVersionsPage,
      nextVersionChannel,
      nextVersionGame,
    )
  }

  const closeProject = () => {
    setSelectedProjectId(null)
    setSelectedVersionId('')
    setProjectTab('description')
    setVersionsPage(1)
    setVersionChannel([])
    setVersionGame([])
    setDetailLoading(false)
    writeUrl(filters, page, null)
  }

  useEffect(() => {
    if (!started.current) {
      started.current = true
      if (catalog.state === 'idle' && actionsEnabled) {
        onCommand('search-catalog', { ...filters, page })
      }
      if (selectedProjectId) {
        setDetailLoading(true)
        onCommand('load-catalog-project', { projectId: selectedProjectId })
      }
    }
  }, [])

  useEffect(() => () => {
    if (searchTimer.current !== undefined) window.clearTimeout(searchTimer.current)
  }, [])

  useEffect(() => {
    const closeFiltersOutside = (event: PointerEvent) => {
      const page = discoverPageRef.current
      const target = event.target
      if (!page || !(target instanceof Node)) return

      page
        .querySelectorAll<HTMLDetailsElement>(
          '.discover-option-filter[open], .discover-environment-filter[open]',
        )
        .forEach((filter) => {
          if (!filter.contains(target)) filter.open = false
        })
    }
    const closeFiltersOnEscape = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      discoverPageRef.current
        ?.querySelectorAll<HTMLDetailsElement>(
          '.discover-option-filter[open], .discover-environment-filter[open]',
        )
        .forEach((filter) => {
          filter.open = false
        })
    }

    document.addEventListener('pointerdown', closeFiltersOutside)
    document.addEventListener('keydown', closeFiltersOnEscape)
    return () => {
      document.removeEventListener('pointerdown', closeFiltersOutside)
      document.removeEventListener('keydown', closeFiltersOnEscape)
    }
  }, [])

  useEffect(() => {
    if (detail?.id === selectedProjectId) {
      setDetailLoading(false)
    } else if (selectedProjectId && catalog.detailState === 'error') {
      setDetailLoading(false)
    }
  }, [catalog.detailState, detail, selectedProjectId])

  useEffect(() => {
    const onPopState = () => {
      const params = new URLSearchParams(window.location.search)
      const nextFilters = filtersFromUrl(currentCatalog)
      const nextPage = Math.max(1, Number(params.get('page')) || 1)
      const nextProjectId = params.get('project')
      const nextTab = readProjectTab(params.get('tab'))
      const nextVersionId = params.get('version') || ''
      const nextVersionsPage = Math.max(1, Number(params.get('vpage')) || 1)
      const nextVersionChannel = filterValues(params.get('channel'), 'all')
      const nextVersionGame = filterValues(params.get('versionGame'))
      setFilters(nextFilters)
      setPage(nextPage)
      setProjectTab(nextTab)
      setVersionsPage(nextVersionsPage)
      setVersionChannel(nextVersionChannel)
      setVersionGame(nextVersionGame)
      setSelectedVersionId(nextVersionId)
      setSelectedProjectId(nextProjectId)
      if (nextProjectId) {
        if (catalog.detail?.id !== nextProjectId) {
          setDetailLoading(true)
          onCommand('load-catalog-project', { projectId: nextProjectId })
        }
      } else if (actionsEnabled) {
        setDetailLoading(false)
        onCommand('search-catalog', { ...nextFilters, page: nextPage })
      }
    }
    window.addEventListener('popstate', onPopState)
    return () => window.removeEventListener('popstate', onPopState)
  }, [actionsEnabled, catalog.detail?.id, currentCatalog, onCommand])

  const submitSearch = (event: React.FormEvent) => {
    event.preventDefault()
    if (actionsEnabled) searchCatalog(filters, 1)
  }

  const totalPages = Math.max(
    1,
    Math.ceil((catalog.total || 0) / (catalog.limit || filters.limit || 20)),
  )
  const activeType = discoverProjectTypes.find((type) => type.value === filters.projectType) || discoverProjectTypes[0]
  const activeTypeLabel = t(activeType.labelKey)
  const searchPlaceholder = filters.projectType === 'mod'
    ? t('discover.placeholder')
    : t('discover.searchByType', { type: activeTypeLabel.toLowerCase() })
  const resultCount = catalog.total || catalog.items.length
  const installSelectedVersion = (requestedVersionId?: string) => {
    if (!detail || !actionsEnabled) return
    if (filters.projectType === 'server') return
    const versionId = requestedVersionId || selectedVersionId || detail.versions[0]?.id
    if (!versionId) return
    if (filters.projectType === 'modpack') {
      requestPackPreview(detail.id, detail.slug, versionId, detail.name)
      return
    }
    const installed = filters.projectType === 'mod'
      ? installedById.get(detail.installedModId || detail.slug)
      : undefined
    const payload: Record<string, unknown> = {
      projectId: detail.id,
      title: detail.name,
      versionId,
      contentType: filters.projectType,
    }
    onCommand(installed ? 'update-mod' : 'install-mod', {
      ...payload,
      ...(installed ? { modId: installed.id } : {}),
    })
  }

  return (
    <div
      ref={discoverPageRef}
      className={`discover-page${selectedProjectId ? ' project-open' : ''}`}
    >
      <PageHeader
        eyebrow={t('nav.modsManagement')}
        title={t('discover.title')}
        subtitle={t('discover.subtitle')}
        actions={
          <a
            className="button ghost discover-explore-link"
            href="https://shroudedit.com"
            target="_blank"
            rel="noopener noreferrer"
            onClick={(event) => {
              event.preventDefault()
              postRequest('open-url', { url: 'https://shroudedit.com' })
            }}
          >
            {t('discover.exploreShroudEdit')}
          </a>
        }
      />
      {pendingPack && <section className="card" aria-label={t('modpack.chooseMods')}>
        <h2>{t('modpack.chooseMods')} · {pendingPack.title}</h2>
        {catalog.packPreviewState === 'loading' && <p>{t('discover.detail.loading')}</p>}
        {catalog.packPreviewState === 'error' && <p className="notice warning">{catalog.packPreviewError}</p>}
        {preview?.mods.map((item) => {
          const id = previewMemberId(item)
          return <label className="settings-option" key={id}>
            <input type="checkbox" checked={!item.optional || selectedOptionals.includes(id)}
              disabled={!item.optional}
              onChange={(event) => setSelectedOptionals((current) => event.target.checked
                ? [...current, id]
                : current.filter((selected) => selected !== id))} />
            <span>{id} · {item.optional ? t('modpack.member.optional') : t('modpack.member.required')}</span>
          </label>
        })}
        <div className="settings-actions">
          <button className="button ghost" onClick={() => setPendingPack(null)}>{t('common.cancel')}</button>
          <button className="button" disabled={!preview} onClick={() => {
            if (!pendingPack) return
            onCommand('install-mod', { projectId: pendingPack.projectId, title: pendingPack.title,
              versionId: pendingPack.versionId, contentType: 'modpack', optionalMods: selectedOptionals })
            setPendingPack(null)
          }}>{t('discover.detail.installPack')}</button>
        </div>
      </section>}
      {!selectedProjectId && <>
      <UpdaterAvailability
        updaterAvailable={updaterAvailable}
        enabled={enabled}
        onCommand={onCommand}
      />
      <div className="discover-type-tabs" role="tablist" aria-label={t('discover.title')}>
        {discoverProjectTypes.map((tab) => (
          <button
            role="tab"
            type="button"
            key={tab.value}
            aria-selected={filters.projectType === tab.value}
            tabIndex={filters.projectType === tab.value ? 0 : -1}
            className={`discover-type-tab${filters.projectType === tab.value ? ' active' : ''}`}
            disabled={!actionsEnabled}
            onClick={() =>
              searchCatalog(
                {
                  ...filters,
                  projectType: tab.value,
                  loader: [],
                  category: [],
                  gameVersion: [],
                  environment: [],
                },
                1,
              )
            }
          >
            {t(tab.labelKey)}
          </button>
        ))}
      </div>
      <form className="discover-search" onSubmit={submitSearch}>
        <label className="discover-query">
          <Icon name="search" />
          <input
            value={filters.query}
            disabled={!actionsEnabled}
            onChange={(event) =>
              setFilters((current) => ({ ...current, query: event.target.value }))
            }
            onKeyDown={(event) => {
              if (event.key === 'Enter' && !event.nativeEvent.isComposing) {
                event.preventDefault()
                if (actionsEnabled) searchCatalog(filters, 1)
              }
            }}
            placeholder={searchPlaceholder}
            aria-label={searchPlaceholder}
          />
        </label>
      </form>
      <div className="discover-toolbar">
        <div className="discover-toolbar-filters">
          {filters.projectType === 'mod' && (
            <DiscoverFilter
              name="discover-loader"
              className="loader"
              multiple
              allValue="all"
              label={t('discover.filter.loader')}
              summary="label"
              value={filters.loader}
              options={[
                { value: 'all', label: t('common.all') },
                { value: 'shroudforge', label: 'ShroudForge' },
                { value: 'eml', label: 'EML' },
              ]}
              disabled={!actionsEnabled}
              onChange={(loader) => searchCatalog({ ...filters, loader }, 1)}
            />
          )}
          {(filters.projectType === 'mod' || filters.projectType === 'modpack') && (
            <details
              className="discover-environment-filter"
              onMouseLeave={(event) => {
                event.currentTarget.open = false
              }}
            >
              <summary>{t('discover.filter.environment')}</summary>
              <div>
                {(['client', 'server'] as const).map((side) => (
                  <label key={side}>
                    <input
                      type="checkbox"
                      disabled={!actionsEnabled}
                      checked={filters.environment.includes(side)}
                      onChange={(event) => searchCatalog({
                        ...filters,
                        environment: event.target.checked
                          ? [...filters.environment, side]
                          : filters.environment.filter((value) => value !== side),
                      }, 1)}
                    />
                    <Icon name={side} />
                    {t(side === 'client' ? 'mod.environment.client' : 'mod.environment.server')}
                  </label>
                ))}
              </div>
            </details>
          )}
          <DiscoverFilter
            name="discover-sort"
            className="sort"
            label={t('discover.filter.sort')}
            summary="value"
            value={filters.sort}
            options={[
              { value: 'relevance', label: t('discover.sort.relevance') },
              { value: 'downloads', label: t('discover.sort.downloads') },
              { value: 'newest', label: t('discover.sort.newest') },
              { value: 'updated', label: t('discover.sort.updated') },
            ]}
            disabled={!actionsEnabled}
            onChange={(sort) => searchCatalog({ ...filters, sort }, 1)}
          />
          <DiscoverFilter
            name="discover-page-size"
            className="page-size"
            label={t('wiki.itemsPerPage')}
            value={String(filters.limit)}
            options={[20, 40, 60].map((limit) => ({ value: String(limit), label: String(limit) }))}
            disabled={!actionsEnabled}
            onChange={(limit) => searchCatalog({ ...filters, limit: pageSize(limit) }, 1)}
          />
          <DiscoverFilter
            name="discover-category"
            className="category"
            multiple
            allValue=""
            label={t('wiki.filterCategory')}
            summary="label"
            value={filters.category}
            options={[
              { value: '', label: t('wiki.allCategories') },
              ...strings(catalog.categories).map((category) => ({ value: category, label: category })),
            ]}
            disabled={!actionsEnabled}
            onChange={(category) => searchCatalog({ ...filters, category }, 1)}
          />
          <DiscoverFilter
            name="discover-game-version"
            className="game-version"
            multiple
            allValue=""
            label={t('discover.filter.gameVersion')}
            summary="label"
            value={filters.gameVersion}
            options={[
              { value: '', label: t('common.all') },
              ...strings(catalog.gameVersions).map((version) => ({ value: version, label: version })),
            ]}
            disabled={!actionsEnabled}
            onChange={(gameVersion) => searchCatalog({ ...filters, gameVersion }, 1)}
          />
        </div>
        <span className="discover-result-count">
          {filters.projectType === 'mod'
            ? t(resultCount === 1 ? 'discover.result' : 'discover.results', { count: resultCount })
            : t('discover.resultsByType', {
                count: new Intl.NumberFormat(locale).format(resultCount),
                type: t(resultCount === 1 ? activeType.singularKey : activeType.labelKey),
              })}
        </span>
        {totalPages > 1 && (
          <nav className="discover-pagination" aria-label={t('discover.title')}>
            <div className="discover-page-numbers">
              {paginationItems(page, totalPages).map((value, index) =>
                value === 'ellipsis' ? (
                  <span className="discover-page-ellipsis" key={`ellipsis-${index}`} aria-hidden="true">…</span>
                ) : (
                  <button
                    className={`discover-page-number${page === value ? ' active' : ''}`}
                    type="button"
                    key={value}
                    aria-label={t('wiki.pageIndicator', { current: value, total: totalPages })}
                    aria-current={page === value ? 'page' : undefined}
                    disabled={!actionsEnabled}
                    onClick={() => searchCatalog(filters, value)}
                  >
                    {value}
                  </button>
                ),
              )}
            </div>
            <button
              className="discover-page-next"
              type="button"
              aria-label={t('wiki.nextPage')}
              disabled={!actionsEnabled || page >= totalPages}
              onClick={() => searchCatalog(filters, page + 1)}
            >
              <Icon name="chevron" />
            </button>
          </nav>
        )}
      </div>
      </>}

      {selectedProjectId && (
        <CatalogProjectPage
          project={detail || null}
          projectType={filters.projectType}
          tab={projectTab}
          versionId={selectedVersionId}
          versionPage={versionsPage}
          channel={versionChannel}
          gameVersion={versionGame}
          loading={detailLoading}
          error={catalog.detailState === 'error' ? catalog.detailError : null}
          enabled={actionsEnabled}
          installed={Boolean(detail?.installedModId && installedById.has(detail.installedModId))}
          onClose={closeProject}
          onTab={(tab) => navigateProject(tab)}
          onVersion={(versionId) => navigateProject('versions', versionId)}
          onVersionList={() => navigateProject('versions')}
          onVersionPage={(nextPage) => navigateProject('versions', null, nextPage)}
          onVersionFilter={(channel, gameVersion) =>
            navigateProject('versions', null, 1, channel, gameVersion)
          }
          onInstall={installSelectedVersion}
          onOpenProject={openProjectById}
          onOpenLink={(url) => onCommand('open-url', { url })}
          onClearCache={() => onCommand('clear-catalog-cache')}
        />
      )}
      {!selectedProjectId && (catalog.state === 'loading' ? (
        <Loading text={t('discover.catalog.loading')} />
      ) : catalog.state === 'error' ? (
        <>
          <Empty
            title={t('discover.catalog.unavailable')}
            text={catalog.message || t('discover.empty.text')}
            icon="search"
          />
          <div className="discover-retry">
            <button
              className="button"
              disabled={!actionsEnabled}
              onClick={() => searchCatalog(filters, page)}
            >
              {t('discover.catalog.retry')}
            </button>
          </div>
        </>
      ) : catalog.items.length ? (
        <>
          <div className="discover-list">
            {catalog.items.map((item) => {
              const installedId = item.installedModId || item.slug
              const installed = item.projectType === 'mod' ? installedById.get(installedId) : undefined
              const update = item.projectType === 'mod'
                ? updateById.get(installedId) || updateByProjectId.get(item.id)
                : undefined
              const canInstall = Boolean(item.latestVersionId) && !installed && item.projectType !== 'server'
              const updated = relativeDate(item.dateModified, locale)
              return (
                <article className="discover-row" key={item.id} onClick={(event) => {
                  if (!(event.target as HTMLElement).closest('button, a, input, select')) openProject(item)
                }}>
                  <button className="discover-row-icon" type="button" onClick={() => openProject(item)} aria-label={item.name}>
                    <CatalogIcon
                      url={item.iconUrl}
                      className="discover-row-icon-image"
                    />
                  </button>
                  <div className="discover-row-content">
                    <div className="discover-row-title">
                      <h2><button type="button" onClick={() => openProject(item)}>{item.name}</button></h2>
                      <span>{t('discover.byAuthor', { author: item.author || t('discover.community') })}</span>
                    </div>
                    <p className="discover-row-summary">{item.summary}</p>
                    <div className="discover-badges">
                      <EnvironmentBadge environment={item.environment} catalog />
                      {strings(item.categories).map((category) => (
                        <span className="discover-badge" key={category}>{category}</span>
                      ))}
                      {strings(item.loaders).map((loader) => (
                        <span className="discover-badge loader" key={loader}>{loader}</span>
                      ))}
                      {installed && <span className="discover-badge installed">{t('mod.installed')} · {installed.version}</span>}
                      {update && <span className="discover-badge update">{t('discover.status.update', { version: update.latestVersion || '' })}</span>}
                    </div>
                  </div>
                  <div className="discover-row-aside">
                    {item.projectType !== 'server' && <button
                        className="button discover-install-button"
                        type="button"
                        disabled={!actionsEnabled || (!canInstall && !update)}
                        onClick={() => {
                          if (installed && update) {
                            onCommand('update-mod', {
                              modId: installed?.id || installedId,
                              title: item.name,
                              versionId: update.latestVersionId || item.latestVersionId,
                              contentType: 'mod',
                            })
                          } else if (item.projectType === 'modpack' && item.latestVersionId) {
                            requestPackPreview(item.id, item.slug, item.latestVersionId, item.name)
                          } else if (!installed) {
                            onCommand('install-mod', {
                              projectId: item.id,
                              title: item.name,
                              versionId: item.latestVersionId,
                              contentType: item.projectType,
                            })
                          }
                        }}
                      >
                        {item.projectType === 'schematic'
                          ? t('schematics.import')
                          : item.projectType === 'modpack'
                            ? t('discover.detail.installPack')
                            : installed ? (update ? t('discover.status.updateButton') : t('mod.installed')) : t('common.install')}
                    </button>}
                    <div className="discover-row-meta">
                      {typeof item.downloads === 'number' && (
                        <span>
                          <Icon name="download" />
                          {compactNumber(item.downloads, locale)}
                        </span>
                      )}
                      {typeof item.follows === 'number' && (
                        <span className="discover-followers">
                          <Icon name="heart" />
                          {item.follows.toLocaleString(locale)}
                        </span>
                      )}
                      {updated && (
                        <span title={item.dateModified || undefined}>
                          <Icon name="history" />
                          {updated}
                        </span>
                      )}
                    </div>
                  </div>
                </article>
              )
            })}
          </div>
        </>
      ) : (
        <Empty
          title={filters.projectType === 'mod' ? t('discover.empty.title') : activeTypeLabel}
          text={catalog.state === 'ready'
            ? filters.projectType === 'mod'
              ? t('discover.empty.noResults')
              : t('discover.empty.noResultsOther')
            : catalog.message || t('discover.empty.text')}
          icon="search"
        />
      ))}
    </div>
  )
}
