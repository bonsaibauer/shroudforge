import { Icon } from './show-icon'

export function HeaderLink({
  label,
  icon,
  iconOnly = false,
  tone = '',
  badge,
  severity,
  crowded = false,
  title,
  onClick,
}: {
  label: string
  icon: string
  iconOnly?: boolean
  tone?: string
  badge?: number
  severity?: string
  crowded?: boolean
  title?: string
  onClick(): void
}) {
  const asset = linkAssetIcons[icon]
  const marker =
    severity === 'error'
      ? '🚨'
      : severity === 'warning'
        ? '⚠️'
        : severity === 'update'
          ? '✨'
          : severity === 'many'
            ? '📨'
            : severity === 'info'
              ? '•'
              : ''
  return (
    <button
      className={`header-link ${tone}${iconOnly ? ' icon-only' : ''}${severity ? ` chirper--${severity}` : ''}${crowded ? ' chirper-crowded' : ''}`}
      aria-label={iconOnly ? label : undefined}
      title={title ?? (iconOnly ? label : undefined)}
      onClick={onClick}
    >
      {asset ? (
        <img className="icon" src={asset} aria-hidden="true" />
      ) : (
        <Icon name={icon} />
      )}
      {!iconOnly && <span>{label}</span>}
      {marker && (
        <i className="chirper-severity-mark" aria-hidden="true">
          {marker}
        </i>
      )}
      {badge !== undefined && <b>{badge > 99 ? '99+' : badge}</b>}
    </button>
  )
}

export const linkAssetIcons = Object.fromEntries(
  Object.entries(
    import.meta.glob('../modloader/ui/links/*.svg', {
      eager: true,
      import: 'default',
      query: '?url',
    }),
  ).map(([path, value]) => [
    path.split('/').pop()?.replace('.svg', ''),
    value as string,
  ]),
) as Record<string, string>
