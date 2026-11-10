import type { ReactNode } from 'react'

export function FeatureBanner({
  variant,
  brand,
  title,
  titleId,
  description,
  media,
  badge,
  role,
  children,
  dismiss,
}: {
  variant: 'promo' | 'warning' | 'info' | 'error'
  brand: string
  title: string
  titleId: string
  description: string
  media: ReactNode
  badge: ReactNode
  role?: 'alert' | 'status'
  children?: ReactNode
  dismiss?: { label: string; onClick: () => void }
}) {
  return (
    <aside
      className={`feature-banner feature-banner--${variant}`}
      role={role}
      aria-labelledby={titleId}
    >
      <div className="feature-banner-art">{media}</div>
      <div className="feature-banner-copy">
        <span className="feature-banner-brand">{brand}</span>
        <h2 id={titleId}>{title}</h2>
        <p>{description}</p>
        {children}
      </div>
      {(badge || dismiss) && <div className="feature-banner-meta">
        {typeof badge === 'string' ? <span className="feature-banner-badge">{badge}</span> : badge}
        {dismiss && <button type="button" className="button ghost feature-banner-dismiss"
          aria-label={dismiss.label} title={dismiss.label} onClick={dismiss.onClick}>×</button>}
      </div>}
    </aside>
  )
}
