import type { Activity } from '../../logging/ui/log-entry'
import type {
  Release,
  UpdateQueue,
  Catalog,
  BackupManagement,
} from '../../updater/ui/update-types'
import type { Settings } from '../../loader-config/ui/loader-settings'
import type { ModInfo } from './mod-types'
import type { DiagnosticStatus } from '../../runtime-diagnostics/ui/diagnostic-status'
import type { Notice } from '../../notifications/ui/notification-types'

export type Snapshot = {
  diagnostics?: DiagnosticStatus
  dllCandidates: DllCandidate[]
  dllChainEntries: DllLoadEntry[]
  windows?: Record<
    string,
    {
      visible: boolean
      requestedVisible?: boolean
      focusTarget?: string
      focusRequestId?: number
    }
  >
  configuration?: {
    modStates?: Record<string, { state: string; code?: string; detail: string }>
    errors: string[]
    checks: Array<{
      id: string
      group: string
      state: 'ok' | 'warning' | 'neutral'
      detail: string
      phase?: string
    }>
    assets: { state: string; detail: string }
    lastUpdate?: { version: string; build: string; installedAt: number }
  }
  connected: boolean
  mode: string
  gameVersion: string
  version: string
  mods: ModInfo[]
  hotkeyConflicts: Array<{
    firstId: string
    secondId: string
    binding: string
  }>
  activity: Activity[]
  notices: Notice[]
  newsTemplates?: Record<
    string,
    { title: string; message: string; beta?: boolean; englishOnly?: boolean }
  >
  readNoticeIds: string[]
  release: Release
  releaseAnnouncement?: {
    updateAvailable: boolean
    latestVersion?: string
    releaseUrl?: string | null
    changelog?: string[]
  }
  updateQueue?: UpdateQueue
  backupManagement?: BackupManagement
  settings: Settings
  catalog?: Catalog
  locale: string
}

export type DllCandidate = {
  packageDirectory: string
  name: string
  shippedProfile: string | null
  shippedProfileFiles: string[]
  filteredDlls: string[]
  manifestState: 'valid' | 'missing' | 'invalid'
  hasLua: boolean
  capabilities: string[]
  target: 'client' | 'server' | 'both'
  manifestEnabled: boolean | null
  iniState: 'missing' | 'enabled' | 'disabled' | 'invalid' | 'filtered'
  declaredDll: string | null
  dlls: Array<{ path: string; size: number; mode?: DllLoadMode }>
  availableProfiles: Array<{ id: string; name: string }>
  selectedProfile: string | null
  source: string
  loadEntries: DllLoadEntry[]
  issues: string[]
}

export type DllLoadMode = 'library' | 'dinput8'

export type DllLoadEntry = {
  statusId: string
  path: string
  mode: DllLoadMode
  source: string
}

export type DllManagerProfile = {
  id: string
  packageDirectory: string
  name: string
  target: 'client' | 'server' | 'both'
  enabled: boolean
  approved: boolean
  compatibilityProfile?: string
  files: Array<{ path: string; mode?: DllLoadMode }>
}

export type PageId =
  | 'activity'
  | 'news'
  | 'discover'
  | 'installed'
  | 'updates'
  | 'recycle-bin'
  | 'compatibility'
  | 'settings'
  | 'wiki'

export type LinkDefinition = {
  id: string
  labelKey: string
  descriptionKey: string
  icon: string
  appearance: string
}

export const startPageOptions: PageId[] = [
  'news',
  'activity',
  'discover',
  'installed',
  'updates',
  'recycle-bin',
  'compatibility',
  'settings',
]

export const empty: Snapshot = {
  dllCandidates: [],
  dllChainEntries: [],
  connected: false,
  mode: 'DESKTOP',
  gameVersion: '',
  version: '1.0.0',
  mods: [],
  hotkeyConflicts: [],
  activity: [],
  notices: [],
  readNoticeIds: [],
  locale: 'en-GB',
  release: {
    currentVersion: '1.0.0',
    updaterAvailable: false,
    updateAvailable: false,
    state: 'idle',
    staged: false,
  },
  releaseAnnouncement: { updateAvailable: false },
  updateQueue: { items: [] },
  backupManagement: {
    target: 'client',
    originals: { status: 'missing' },
    workingBaseline: { kfc: false, resources: false, complete: false },
    transaction: { pending: false },
    rollback: { systemUpdates: 0, modPackages: 0 },
    restore: { status: 'idle' },
  },
  settings: {
    compactMode: false,
    reducedMotion: false,
    showNativeModMenu: true,
    zoomPercent: 100,
    usernameMode: 'auto',
    detectedUsername: null,
    effectiveUsername: 'Flameborn',
    usernameSource: 'fallback',
    messageNameFallback: 'Flameborn',
    logLevel: 'INFO',
    exportEnabled: true,
    runtimeProfileId: null,
    runtimeProfiles: [],
    diagnosticJobs: [],
    diagnosticJobErrors: [],
    diagnosticJobDirectory: '',
    diagnosticSelectedJobId: 'runtime.overview',
    paths: [],
    baseUrl: 'https://api.shroudedit.com',
    projectId: 'shroudforge',
  },
  catalog: { state: 'idle', query: '', items: [] },
}
