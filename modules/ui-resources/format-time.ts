import type { Locale } from './i18n'
import { intlLocaleFor } from './intl-locale'

export const formatTime = (value: number, locale: Locale) => {
  const formatLocale = intlLocaleFor(locale)
  return value < 1000000000
    ? new Date(value * 1000).toLocaleTimeString(formatLocale, {
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
      })
    : new Intl.DateTimeFormat(formatLocale, {
        dateStyle: 'short',
        timeStyle: 'short',
      }).format(new Date(value * 1000))
}
