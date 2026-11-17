export type ModulePreferences = Record<string, Record<string, any>>

export type TargetUiVisibility = {
  client: {
    pages: Record<string, boolean>
    modules: Record<string, boolean>
  }
  server: {
    pages: Record<string, boolean>
    modules: Record<string, boolean>
  }
}

export type UiTargetSettings = {
  visibility: TargetUiVisibility
  wikiLinkTarget: 'current' | 'client' | 'server'
}

export type StoragePath = { id: string; path: string; defaultPath: string }
export type DiagnosticJob = {
  id: string
  name: string
  description: string
  capabilities: string[]
}
export type ProtectedHotkey = {
  scancode: number
  label: string
  emoji: string
  text: string
}
export type ProtectedHotkeyGroups = {
  confirm: ProtectedHotkey[]
  cancel: ProtectedHotkey[]
}

export type Settings = {
  configRevision?: string
  modulePreferences?: ModulePreferences
  targetUi?: UiTargetSettings
  protectedHotkeys?: ProtectedHotkeyGroups
  compactMode: boolean
  reducedMotion: boolean
  showNativeModMenu: boolean
  zoomPercent: number
  usernameMode: 'auto' | 'manual'
  detectedUsername?: string | null
  effectiveUsername: string
  usernameSource: 'live' | 'saved' | 'fallback'
  messageNameFallback: string
  logLevel: string
  exportEnabled: boolean
  runtimeProfileId?: string | null
  activeRuntimeProfile?: string
  runtimeProfiles?: string[]
  runtimeProfileDirectory?: string
  diagnosticJobs?: DiagnosticJob[]
  diagnosticJobErrors?: string[]
  diagnosticJobDirectory?: string
  diagnosticSelectedJobId?: string
  paths?: StoragePath[]
  baseUrl: string
  projectId: string
}
