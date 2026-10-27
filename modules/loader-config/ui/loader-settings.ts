export type ModulePreferences = Record<string, Record<string, any>>

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
  action: 'block' | 'confirm' | 'cancel'
}

export type Settings = {
  configRevision?: string
  modulePreferences?: ModulePreferences
  protectedHotkeys?: ProtectedHotkey[]
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
