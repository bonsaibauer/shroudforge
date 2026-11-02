import type { Locale } from './i18n'

export const formatTime = (value: number, locale: Locale) => {
  return value < 1000000000
    ? new Date(value * 1000).toLocaleTimeString(locale, {
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
      })
    : new Intl.DateTimeFormat(locale, {
        dateStyle: 'short',
        timeStyle: 'short',
      }).format(new Date(value * 1000))
}
