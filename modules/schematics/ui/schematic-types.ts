export type SchematicInfo = {
  name: string
  image: string | null
  source: 'local' | 'shroudedit' | string
  projectId: string | null
  versionId: string | null
  valid: boolean
  size: number
  modifiedAt: number
}
