export type SchematicInfo = {
  name: string
  image: string | null
  source: 'local' | 'shroudedit' | string
  projectId: string | null
  versionId: string | null
  valid: boolean
  formatVersion: number | null
  convertible: boolean
  validationError: string | null
  size: number
  modifiedAt: number
}
