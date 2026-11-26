export type Release = {
  currentVersion: string
  installationDetected?: boolean
  updaterAvailable?: boolean
  latestVersion?: string
  updateAvailable: boolean
  mandatory?: boolean
  prerelease?: boolean
  state: string
  message?: string
  releaseUrl?: string
  staged: boolean
  downloadedBytes?: number
  totalBytes?: number
  bytesPerSecond?: number
  completedFiles?: number
  totalFiles?: number
  currentFile?: string
  currentFileBytes?: number
  currentFileTotalBytes?: number
}

export type UpdateQueueItem = {
  id: string
  kind: 'system' | 'mod' | 'maintenance' | 'installer'
  operation?: string
  title: string
  version?: string
  prerelease?: boolean
  state: string
  selected: boolean
  required?: boolean
  message?: string
  releaseUrl?: string
  payload?: {
    removeMods?: boolean
    resetUserSettings?: boolean
    purgeData?: boolean
    restoreGamefiles?: boolean
  }
}

export type BackupManagement = {
  target: string
  originals?: {
    status: string
    capturedAt?: number
    source?: string
    detail?: string
    files?: Array<{ name: string; valid: boolean }>
  }
  workingBaseline?: { kfc: boolean; resources: boolean; complete: boolean }
  transaction?: { pending: boolean }
  rollback?: { systemUpdates: number; modPackages: number }
  restore?: { status: string; message?: string }
}

export type UpdateQueue = {
  items: UpdateQueueItem[]
  error?: string
  run?: {
    state: string
    waitForGame?: boolean
    installing?: boolean
    cancelable?: boolean
    cancelRequested?: boolean
    activeItemId?: string | null
    itemIds?: string[]
    error?: string
  } | null
}

export type CatalogItem = {
  id: string
  slug: string
  projectType: 'mod' | 'schematic' | 'modpack' | 'server'
  projectTypes: string[]
  name: string
  summary: string
  iconUrl?: string
  author?: string | null
  downloads?: number | null
  follows?: number | null
  version?: string | null
  environment?: string | null
  dateModified?: string | null
  latestVersionId?: string | null
  loaders: string[]
  categories: string[]
  gameVersions: string[]
  gallery: string[]
  installedModId?: string
}

export type Catalog = {
  state: string
  query: string
  projectType?: string
  loader?: string
  category?: string
  gameVersion?: string
  sort?: string
  page?: number
  limit?: number
  total?: number
  message?: string
  items: CatalogItem[]
  categories?: string[]
  gameVersions?: string[]
  projectTypes?: string[]
  detail?: CatalogProject | null
  detailState?: string
  detailError?: string | null
}

export type CatalogProject = {
  id: string
  slug: string
  projectTypes: string[]
  name: string
  summary: string
  description: string
  downloads?: number
  follows?: number
  author?: string
  dateModified?: string
  dateCreated?: string
  iconUrl?: string
  links: Record<string, string | null | undefined>
  loaders: string[]
  categories: string[]
  gameVersions: string[]
  gallery: CatalogGalleryImage[]
  versionsCount: number
  versions: CatalogProjectVersion[]
  dependencies: Array<{ id: string; name: string; summary: string }>
  schematic?: Record<string, unknown> | null
  enshroudedMod?: Record<string, unknown> | null
  enshroudedServer?: Record<string, unknown> | null
  installedModId?: string
}

export type CatalogGalleryImage = {
  url: string
  rawUrl: string
  title: string
  description: string
  created: string
}

export type CatalogProjectVersion = {
  id: string
  name: string
  versionNumber: string
  published: string
  changelog: string
  versionType: string
  environment?: string
  downloads?: number
  loaders: string[]
  gameVersions: string[]
  projectTypes: string[]
  files: Array<{
    id?: string
    filename?: string
    url?: string
    primary?: boolean
    size?: number
    hashes?: Record<string, string>
  }>
  dependencies: Array<Record<string, unknown>>
}

export type ModUpdate = {
  projectId: string
  modId: string
  name: string
  currentVersion?: string
  latestVersion?: string
  latestVersionId: string
}

export type ModUpdateCheckResult = {
  checked: number
  failed: number
  checkedAt?: number
}

export type UpdaterViewData = {
  mode: string
  release: Release
  installationRoot?: string
  installationRootSelected?: boolean
  enabled?: boolean
  updateQueue?: UpdateQueue
  backupManagement?: BackupManagement
  modUpdates?: ModUpdate[]
  modCheckResult?: ModUpdateCheckResult | null
  modCheckRunning?: boolean
}

export type UpdaterCommand = (
  command: string,
  payload?: Record<string, unknown>,
) => string | void
