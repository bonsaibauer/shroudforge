import type React from 'react'
import { createContext, useContext, useEffect, useState } from 'react'
import { useI18n } from './i18n'

const minimumZoom = 70
const maximumZoom = 150
const zoomStep = 10

function normalizeZoom(value: number): number {
  return Math.min(maximumZoom, Math.max(minimumZoom, Math.round(value)))
}

type PageZoomContextValue = {
  zoomPercent: number
  setZoomPercent(value: number): void
}

const PageZoomContext = createContext<PageZoomContextValue | null>(null)

export function PageZoomProvider({
  zoomPercent,
  onChange,
  children,
}: {
  zoomPercent: number
  onChange(value: number): void
  children: React.ReactNode
}) {
  const normalizedZoom = normalizeZoom(zoomPercent)
  const setZoomPercent = (value: number) => onChange(normalizeZoom(value))

  useEffect(() => {
    document.documentElement.style.zoom = `${normalizedZoom}%`
  }, [normalizedZoom])

  return (
    <PageZoomContext.Provider
      value={{ zoomPercent: normalizedZoom, setZoomPercent }}
    >
      {children}
    </PageZoomContext.Provider>
  )
}

export function usePageZoom(): PageZoomContextValue {
  const context = useContext(PageZoomContext)
  if (!context)
    throw new Error('usePageZoom must be used inside PageZoomProvider')
  return context
}

export function PageHeader({
  eyebrow,
  title,
  subtitle,
  links,
  actions,
}: {
  eyebrow: string
  title: React.ReactNode
  subtitle: string
  links?: React.ReactNode
  actions?: React.ReactNode
}) {
  const { t } = useI18n()
  const pageZoom = useContext(PageZoomContext)
  const [standaloneZoom, setStandaloneZoom] = useState(100)
  const zoom = pageZoom?.zoomPercent ?? standaloneZoom
  const changeZoom = (delta: number) => {
    const next = normalizeZoom(zoom + delta)
    if (pageZoom) pageZoom.setZoomPercent(next)
    else {
      document.documentElement.style.zoom = `${next}%`
      setStandaloneZoom(next)
    }
  }

  return (
    <header className="page-header">
      <div>
        <small>{eyebrow}</small>
        <h1>{title}</h1>
        {links}
        <p>{subtitle}</p>
      </div>
      <div className="page-actions">
        {actions}
        <div
          className="page-zoom-controls"
          role="group"
          aria-label={t('common.contentZoom')}
        >
          <button
            type="button"
            className="button ghost page-zoom-step"
            aria-label={t('common.zoomOut')}
            title={t('common.zoomOut')}
            disabled={zoom <= minimumZoom}
            onClick={() => changeZoom(-zoomStep)}
          >
            −
          </button>
          <output className="page-zoom-value" aria-live="polite">
            {zoom}%
          </output>
          <button
            type="button"
            className="button ghost page-zoom-step"
            aria-label={t('common.zoomIn')}
            title={t('common.zoomIn')}
            disabled={zoom >= maximumZoom}
            onClick={() => changeZoom(zoomStep)}
          >
            +
          </button>
        </div>
      </div>
    </header>
  )
}
