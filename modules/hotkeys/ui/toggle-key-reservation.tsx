import { useI18n } from '../../ui-resources/i18n'

/** A key or the mouse wheel a mod may keep from the game during `condition`. */
export type KeyReservation = {
  condition: string
  reserved: boolean
  onToggle(reserved: boolean): void
}

/**
 * The padlock in front of a reservable binding: closed, the game does not
 * receive `keyLabel` during the mod's condition; open, it receives it too. Its
 * tooltip says which, and what a click does.
 */
export function KeyReservationLock({
  reservation,
  keyLabel,
  disabled = false,
}: {
  reservation: KeyReservation
  keyLabel: string
  disabled?: boolean
}) {
  const { t } = useI18n()
  const { reserved, condition } = reservation
  const key = `“${keyLabel}”`
  return (
    <span className={'key-lock' + (reserved ? ' reserved' : '')}>
      <button
        type="button"
        className="key-lock-button"
        aria-pressed={reserved}
        aria-label={t(reserved ? 'hotkeys.reservation.share' : 'hotkeys.reservation.reserve', {
          key: keyLabel,
        })}
        disabled={disabled}
        onClick={() => reservation.onToggle(!reserved)}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <rect x="5" y="11" width="14" height="10" rx="2" />
          <path d={reserved ? 'M8 11V7a4 4 0 0 1 8 0v4' : 'M8 11V7a4 4 0 0 1 7.5-2'} />
        </svg>
      </button>
      <span className="key-lock-tip" role="tooltip">
        <strong>
          {t(reserved ? 'hotkeys.reservation.reservedTitle' : 'hotkeys.reservation.sharedTitle')}
        </strong>
        <span>
          {t(reserved ? 'hotkeys.reservation.reservedText' : 'hotkeys.reservation.sharedText', {
            key,
            condition,
          })}
        </span>
      </span>
    </span>
  )
}
