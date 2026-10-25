import { postRequest } from '../../transport/ui/send-host-command'
import { formatTime } from '../../ui-resources/format-time'
import { TranslationKey, useI18n } from '../../ui-resources/i18n'
import type { NewsItem, NoticeAction } from './notification-types'

import { EcosystemBadge } from '../../ui-resources/show-ecosystem-badge'
import { Icon } from '../../ui-resources/show-icon'
import { ContributorList } from './show-contributors'
import { CrowdinProgressPanel } from './show-crowdin-progress'
import buyMeACoffeeButton from './assets/buymeacoffee-button.png'
import modSupportFeedbackDe from './assets/mod-support-feedback.de.svg'
import modSupportFeedbackEn from './assets/mod-support-feedback.en.svg'
import modSupportFeedbackEs from './assets/mod-support-feedback.es.svg'
import modSupportFeedbackFr from './assets/mod-support-feedback.fr.svg'
import multiplayerSupportBanner from './assets/multiplayer-support.svg'
import creativeModeBanner from './assets/creative-mode-banner.png'
import nexusModsBanner from '../../../assets/nexusmods-enshrouded-banner.jpg'

const modSupportFeedbackBanners: Record<string, string> = {
  de: modSupportFeedbackDe,
  en: modSupportFeedbackEn,
  es: modSupportFeedbackEs,
  fr: modSupportFeedbackFr,
}

export function noticeIcon(item: NewsItem) {
  if (item.level === 'error') return 'issue'
  if (item.level === 'warn') return 'warning'
  if (
    item.id.startsWith('system-staged-') ||
    item.kind === 'mod.install.success'
  )
    return 'check'
  if (item.id.startsWith('system-') || item.kind === 'mod.update.success')
    return 'updates'
  switch (item.kind) {
    case 'notice.welcome':
      return 'heart'
    case 'notice.modsGuide':
      return 'installed'
    case 'notice.configuration':
      return 'settings'
    case 'notice.discord':
      return 'discord'
    case 'notice.localization':
      return 'discover'
    case 'notice.multiplayerSupport':
      return 'server'
    case 'notice.nexusMods':
      return 'discover'
    case 'notice.applySettings':
      return 'refresh'
    case 'notice.feedback':
      return 'issue'
    case 'notice.support':
      return 'coffee'
    case 'mod.remove.success':
      return 'trash'
    default:
      return 'news'
  }
}

export function NoticeCard({
  item,
  compact = false,
  unread = false,
  onRead,
  onAction,
  onWikiArticle,
}: {
  item: NewsItem
  compact?: boolean
  unread?: boolean
  onRead?(): void
  onAction?(action: NoticeAction): void
  onWikiArticle?(articleId: string): void
}) {
  const { t, locale } = useI18n()
  const cardT = (key: TranslationKey) =>
    t(key, {}, item.englishOnly ? 'en-GB' : undefined)
  const actions = item.actions || []
  const links = item.links || []
  const supportLink =
    item.kind === 'notice.support'
      ? links.find((link) => link.labelKey === 'news.link.buyCoffee')
      : undefined
  const modSupportLink = links.find(
    (link) => link.labelKey === 'news.link.modCompatibility',
  )
  const creativeModeDownloadLink =
    item.kind === 'notice.creativeMode'
      ? links.find((link) => link.labelKey === 'news.link.creativeModeDownload')
      : undefined
  const footerLinks =
    !compact && supportLink
      ? links.filter((link) => link !== supportLink)
      : links
  const title = item.title
  const message = item.message
  const tone =
    item.level === 'warn'
      ? 'warning'
      : item.id.startsWith('system-staged-') ||
          item.kind === 'mod.install.success'
        ? 'success'
        : item.id.startsWith('system-') || item.kind === 'mod.update.success'
          ? 'update'
          : item.level
  const localizedUrl = (url: string) =>
    url.replace('{locale}', locale === 'de' ? 'de' : 'en')
  return (
    <article
      data-kind={item.kind}
      className={`notice ${tone} ${compact ? 'compact' : ''} ${unread ? 'unread' : ''}`}
    >
      <span className="notice-icon">
        <Icon name={noticeIcon(item)} />
      </span>
      <div>
        <div className="notice-title">
          <h3>{title}</h3>
          <span className="state-pill neutral">{item.level.toUpperCase()}</span>
          {item.beta && <span className="state-pill neutral">BETA</span>}
          <EcosystemBadge ecosystem={item.ecosystem} />
          {unread && <i />}
        </div>
        {!compact && item.banner && (
          <button
            className="notice-banner"
            type="button"
            aria-label={item.banner.alt}
            onClick={() =>
              postRequest('open-url', { url: localizedUrl(item.banner!.linkUrl) })
            }
          >
            <img src={item.banner.imageUrl} alt="" loading="lazy" />
          </button>
        )}
        {!compact &&
          item.kind === 'notice.creativeMode' &&
          creativeModeDownloadLink?.url && (
            <button
              className="notice-banner"
              type="button"
              aria-label={cardT('news.link.creativeModeDownload')}
              onClick={() =>
                postRequest('open-url', {
                  url: localizedUrl(creativeModeDownloadLink.url!),
                })
              }
            >
              <img src={creativeModeBanner} alt="" loading="lazy" />
            </button>
          )}
        {!compact && item.kind === 'notice.multiplayerSupport' && (
          <img
            className="notice-artwork"
            src={multiplayerSupportBanner}
            alt={cardT('news.notice.multiplayerSupport.bannerAlt')}
            loading="lazy"
          />
        )}
        {!compact && item.kind === 'notice.nexusMods' && item.actionUrl && (
          <button
            className="notice-banner"
            type="button"
            aria-label={cardT('news.notice.nexusMods.open')}
            onClick={() =>
              postRequest('open-url', { url: localizedUrl(item.actionUrl!) })
            }
          >
            <img src={nexusModsBanner} alt="" loading="lazy" />
          </button>
        )}
        {!compact && modSupportLink?.url && (
          <button
            className="notice-banner"
            type="button"
            aria-label={cardT('news.link.modCompatibility')}
            onClick={() =>
              postRequest('open-url', { url: localizedUrl(modSupportLink.url!) })
            }
          >
            <img
              src={
                modSupportFeedbackBanners[locale] ??
                modSupportFeedbackBanners.en
              }
              alt=""
              loading="lazy"
            />
          </button>
        )}
        {item.kind !== 'notice.multiplayerSupport' && <p>{message}</p>}
        {!compact && supportLink && (
          <button
            className="notice-banner"
            type="button"
            aria-label={cardT('news.link.buyCoffee')}
            onClick={() =>
              postRequest('open-url', { url: localizedUrl(supportLink.url!) })
            }
          >
            <img
              src={buyMeACoffeeButton}
              alt=""
              width={174}
              height={41}
            />
          </button>
        )}
        {!compact && item.kind === 'notice.localization' && (
          <CrowdinProgressPanel />
        )}
        {!compact && item.kind === 'notice.welcome' && <ContributorList />}
        {!compact && item.changelog?.length ? (
          <ul className="notice-changelog" tabIndex={0}>
            {item.changelog.map((entry, index) => (
              <li key={`${index}-${entry}`}>{entry}</li>
            ))}
          </ul>
        ) : null}
        {(!compact ||
          actions.length > 0 ||
          item.actionUrl ||
          footerLinks.length > 0 ||
          onRead) && (
          <footer>
            {item.updatedAt > 0 && (
              <time>{formatTime(item.updatedAt, locale)}</time>
            )}
            <span />
            {item.actionUrl && (
              <button
                onClick={() =>
                  postRequest('open-url', {
                    url: localizedUrl(item.actionUrl!),
                  })
                }
              >
                {item.actionLabelKey
                  ? cardT(item.actionLabelKey as TranslationKey)
                  : cardT('news.more')}
              </button>
            )}
            {footerLinks.map((link, index) => (
              <button
                key={`${link.labelKey || link.label}-${index}`}
                onClick={() => {
                  if (link.wikiArticleId)
                    onWikiArticle?.(link.wikiArticleId)
                  else if (link.url)
                    postRequest('open-url', { url: localizedUrl(link.url) })
                }}
              >
                {link.label || cardT(link.labelKey as TranslationKey)}
              </button>
            ))}
            {actions.map((action) => (
              <button key={action.id} onClick={() => onAction?.(action)}>
                {cardT(`news.action.${action.kind}` as TranslationKey)}
              </button>
            ))}
            {onRead && (
              <button onClick={onRead}>
                {cardT(unread ? 'news.markRead' : 'news.markUnread')}
              </button>
            )}
          </footer>
        )}
      </div>
    </article>
  )
}
