import { useState } from 'react'

export function CatalogIcon({ url, className }: { url?: string; className: string }) {
  const [failed, setFailed] = useState(false)
  if (!url || failed) return null
  return <img className={className} src={url} alt="" onError={() => setFailed(true)} />
}
