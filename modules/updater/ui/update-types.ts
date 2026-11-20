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
  name: string
  summary: string
  iconUrl?: string
  author?: string
  downloads?: number
  version?: string
}

export type Catalog = {
  state: string
  query: string
  message?: string
  items: CatalogItem[]
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
