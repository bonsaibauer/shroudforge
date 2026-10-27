import React, {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react'
import enGB from './locales/en-GB.json'

export type Locale = string
export type TranslationKey = keyof typeof enGB
export type LocaleOption = {
  locale: Locale
  code: string
  name: string
  flag: string
}
type Parameters = Record<string, string | number>
type Catalog = Record<TranslationKey, string>
type I18nValue = {
  locale: Locale
  locales: LocaleOption[]
  setLocale(locale: Locale): void
  t(key: TranslationKey, parameters?: Parameters, locale?: Locale): string
}

const modules = import.meta.glob('./locales/*.json', {
  eager: true,
  import: 'default',
}) as Record<string, Catalog>
const catalogs = Object.fromEntries(
  Object.entries(modules)
    .map(([path, catalog]) => {
      const locale = path.match(/\/([^/]+)\.json$/)?.[1]
      return [locale, catalog]
    })
    .filter((entry): entry is [string, Catalog] => Boolean(entry[0])),
)
const defaultLocale = catalogs['en-GB']
  ? 'en-GB'
  : catalogs.de
    ? 'de'
    : Object.keys(catalogs)[0]
const locales = Object.entries(catalogs)
  .map(([locale, catalog]) => ({
    locale,
    code: locale.toUpperCase(),
    name: catalog['locale.name'],
    flag: catalog['locale.flag'],
  }))
  .sort((left, right) => left.name.localeCompare(right.name))
const I18nContext = createContext<I18nValue | null>(null)

function resolveLocale(locale: string | null): Locale {
  const requested = locale === 'en' && catalogs['en-GB'] ? 'en-GB' : locale
  return requested && catalogs[requested] ? requested : defaultLocale
}

function storedLocale(): Locale {
  try {
    return resolveLocale(window.localStorage.getItem('sf-language'))
  } catch {
    return defaultLocale
  }
}

function saveLocale(locale: Locale) {
  try {
    window.localStorage.setItem('sf-language', locale)
  } catch {
    /* Opaque WebView origins may not expose storage. */
  }
}

export function I18nProvider({ children }: { children: React.ReactNode }) {
  const [locale, updateLocale] = useState<Locale>(storedLocale)
  const value = useMemo<I18nValue>(
    () => ({
      locale,
      locales,
      setLocale(next) {
        const resolved = resolveLocale(next)
        updateLocale(resolved)
        saveLocale(resolved)
      },
      t(key, parameters = {}, targetLocale = locale) {
        const template =
          catalogs[targetLocale]?.[key] ??
          catalogs['en-GB']?.[key] ??
          String(key)
        return template.replace(/\{([A-Za-z0-9_]+)\}/g, (match, name: string) =>
          String(parameters[name] ?? match),
        )
      },
    }),
    [locale],
  )

  useEffect(() => {
    document.documentElement.lang = locale
  }, [locale])
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>
}

export function useI18n(): I18nValue {
  const value = useContext(I18nContext)
  if (!value) throw new Error('useI18n must be used inside I18nProvider')
  return value
}
