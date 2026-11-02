import { Flag } from '../../ui-resources/show-flag'
import { useI18n } from '../../ui-resources/i18n'
import crowdinProgressSnapshot from '@generated-crowdin-progress'

type LanguageProgress = {
  locale: string
  translationProgress: number | null
  approvalProgress: number | null
  translatedWords: number
  approvedWords: number
  totalWords: number
}
type CrowdinProgress = {
  branch: string
  updatedAt: string | null
  languages: LanguageProgress[]
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value)
}

function percentage(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value)
    ? Math.min(100, Math.max(0, Math.round(value)))
    : null
}

function count(value: unknown): number {
  return typeof value === 'number' && Number.isFinite(value)
    ? Math.max(0, Math.floor(value))
    : 0
}

function parseProgress(value: unknown): CrowdinProgress | null {
  if (
    !isRecord(value) ||
    value.schemaVersion !== 1 ||
    typeof value.branch !== 'string' ||
    (value.updatedAt !== null &&
      (typeof value.updatedAt !== 'string' ||
        !Number.isFinite(Date.parse(value.updatedAt)))) ||
    !Array.isArray(value.languages)
  )
    return null

  const byLocale = new Map<string, LanguageProgress>()
  for (const item of value.languages) {
    if (!isRecord(item) || typeof item.locale !== 'string') continue
    const locale = item.locale.trim()
    if (!locale) continue
    byLocale.set(locale, {
      locale,
      translationProgress: percentage(item.translationProgress),
      approvalProgress: percentage(item.approvalProgress),
      translatedWords: count(item.translatedWords),
      approvedWords: count(item.approvedWords),
      totalWords: count(item.totalWords),
    })
  }

  return {
    branch: value.branch,
    updatedAt: typeof value.updatedAt === 'string' ? value.updatedAt : null,
    languages: [...byLocale.values()],
  }
}

const buildProgress = parseProgress(crowdinProgressSnapshot)

export function CrowdinProgressPanel() {
  const { locale, locales, t } = useI18n()
  const progress = buildProgress
  const targetLocales = locales.filter((language) => language.locale !== 'en')
  const number = new Intl.NumberFormat(locale)
  const date =
    progress?.updatedAt &&
    new Intl.DateTimeFormat(locale, {
      dateStyle: 'medium',
      timeStyle: 'short',
    }).format(new Date(progress.updatedAt))
  const languageProgress = new Map(
    progress?.languages.map((language) => [language.locale, language]) ?? [],
  )

  return (
    <section
      className="crowdin-progress"
      aria-label={t('news.localization.progress')}
    >
      <header className="crowdin-progress__header">
        <div>
          <span className="crowdin-progress__eyebrow">CROWDIN</span>
          <h4>{t('news.localization.progress')}</h4>
        </div>
        {progress?.branch && (
          <span className="crowdin-progress__branch">{progress.branch}</span>
        )}
      </header>
      {progress && targetLocales.length > 0 && (
        <div className="crowdin-progress__languages">
          {targetLocales.map((display) => {
            const language = languageProgress.get(display.locale)
            const value = language?.translationProgress ?? null
            const progressText =
              value === null ? '—' : number.format(value) + '%'
            const wordText = !progress.updatedAt
              ? ''
              : !language
                ? t('news.localization.unavailable')
                : language.totalWords > 0
                  ? t('news.localization.words', {
                      translated: number.format(language.translatedWords),
                      total: number.format(language.totalWords),
                    })
                  : t('news.localization.noStrings')
            return (
              <div className="crowdin-progress__row" key={display.locale}>
                <div className="crowdin-progress__language">
                  <Flag value={display.flag} />
                  <strong>{display.name}</strong>
                </div>
                <div
                  className="crowdin-progress__track"
                  role="progressbar"
                  aria-label={t('news.localization.barLabel', {
                    language: display.name,
                  })}
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-valuenow={value ?? undefined}
                  aria-valuetext={
                    value === null
                      ? t(
                          progress.updatedAt && language
                            ? 'news.localization.noStrings'
                            : 'news.localization.unavailable',
                        )
                      : progressText
                  }
                >
                  <span style={{ width: (value ?? 0) + '%' }} />
                </div>
                <strong className="crowdin-progress__percent">
                  {progressText}
                </strong>
                <div className="crowdin-progress__details">
                  {wordText && <span>{wordText}</span>}
                  {language?.approvalProgress !== null &&
                    language?.approvalProgress !== undefined && (
                      <span>
                        {t('news.localization.approval', {
                          percent: number.format(language.approvalProgress),
                        })}
                      </span>
                    )}
                </div>
              </div>
            )
          })}
        </div>
      )}
      {(!progress || !progress.updatedAt) && (
        <div className="crowdin-progress__unavailable" aria-live="polite">
          {t('news.localization.unavailable')}
        </div>
      )}
      {date && (
        <footer className="crowdin-progress__updated">
          {t('news.localization.updated', { date })}
        </footer>
      )}
    </section>
  )
}
