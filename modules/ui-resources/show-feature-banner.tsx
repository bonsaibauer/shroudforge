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
}: {
  variant: 'promo' | 'warning' | 'info' | 'error'
  brand: string
  title: string
  titleId: string
  description: string
  media: ReactNode
  badge: string
  role?: 'alert' | 'status'
  children?: ReactNode
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
      <span className="feature-banner-badge">{badge}</span>
    </aside>
  )
}
