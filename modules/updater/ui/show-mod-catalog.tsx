import type React from 'react'
import { useState } from 'react'
import { useI18n } from '../../ui-resources/i18n'
import type { Catalog, UpdaterCommand } from './update-types'
import { Empty, Loading } from '../../ui-resources/show-empty-state'
import { Icon } from '../../ui-resources/show-icon'
import { PageHeader } from '../../ui-resources/show-page-header'
import shroudEditHammer from '../../../assets/shroudedit-hammer.png'
import { UpdaterAvailability } from './show-updater-availability'
import { FeatureBanner } from '../../ui-resources/show-feature-banner'

const SHOW_SHROUDEDIT_COMING_SOON = true
const SHROUDEDIT_SUPPORT_EMAIL = 'support@shroudedit.com'

export function Discover({
  catalog: currentCatalog,
  updaterAvailable,
  enabled,
  onCommand,
}: {
  catalog?: Catalog
  updaterAvailable: boolean
  enabled: boolean
  onCommand: UpdaterCommand
}) {
  const { t } = useI18n()
  const [query, setQuery] = useState(currentCatalog?.query || '')
  const catalog = currentCatalog || { state: 'idle', query: '', items: [] }
  const actionsEnabled = updaterAvailable && enabled
  const search = (event: React.FormEvent) => {
    event.preventDefault()
    if (!actionsEnabled) return
    onCommand('search-catalog', { query })
  }
  return (
    <>
      <PageHeader
        eyebrow={t('nav.modsManagement')}
        title={t('discover.title')}
        subtitle={t('discover.subtitle')}
      />
      <UpdaterAvailability
        updaterAvailable={updaterAvailable}
        enabled={enabled}
        onCommand={onCommand}
      />
      {SHOW_SHROUDEDIT_COMING_SOON && (
        <FeatureBanner
          variant="promo"
          brand="SHROUDEDIT"
          title={t('discover.teaser.title')}
          titleId="shroudedit-teaser-title"
          description={t('discover.teaser.description')}
          media={<img src={shroudEditHammer} alt="" />}
          badge={t('discover.teaser.badge')}
        >
          <p className="feature-banner-contact">
            {t('discover.teaser.contact')}
          </p>
          <div className="feature-banner-actions">
            <button
              className="button ghost"
              type="button"
              onClick={() =>
                onCommand('open-url', {
                  url: `mailto:${SHROUDEDIT_SUPPORT_EMAIL}`,
                })
              }
            >
              {SHROUDEDIT_SUPPORT_EMAIL}
            </button>
          </div>
        </FeatureBanner>
      )}
      <form className="searchbar" onSubmit={search}>
        <Icon name="search" />
        <input
          value={query}
          disabled={!actionsEnabled}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t('discover.placeholder')}
        />
        <button className="button" type="submit" disabled={!actionsEnabled}>
          {t('discover.search')}
        </button>
      </form>
      {catalog.state === 'loading' ? (
        <Loading text={t('discover.catalog.loading')} />
      ) : catalog.state === 'error' ? (
        <>
          <Empty
            title={t('discover.catalog.unavailable')}
            text={catalog.message || t('discover.empty.text')}
            icon="search"
          />
          <div className="discover-retry">
            <button
              className="button"
              disabled={!actionsEnabled}
              onClick={() =>
                onCommand('search-catalog', { query: catalog.query })
              }
            >
              {t('discover.catalog.retry')}
            </button>
          </div>
        </>
      ) : catalog.items.length ? (
        <div className="catalog-grid">
          {catalog.items.map((item) => (
            <article className="catalog-card" key={item.id}>
              <div className="catalog-icon">
                {item.iconUrl ? (
                  <img src={item.iconUrl} alt="" />
                ) : (
                  item.name.slice(0, 2).toUpperCase()
                )}
              </div>
              <div>
                <h3>{item.name}</h3>
                <p>{item.summary}</p>
                <div className="meta">
                  <span>{item.author || t('discover.community')}</span>
                  {item.version && <span>{item.version}</span>}
                </div>
              </div>
              <button
                className="button"
                disabled={!actionsEnabled}
                onClick={() =>
                  onCommand('install-mod', {
                    projectId: item.id,
                    title: item.name,
                  })
                }
              >
                {t('updates.queue.addMod')}
              </button>
            </article>
          ))}
        </div>
      ) : (
        <Empty
          title={t('discover.empty.title')}
          text={
            catalog.state === 'ready'
              ? t('discover.empty.noResults')
              : catalog.message || t('discover.empty.text')
          }
          icon="search"
        />
      )}
    </>
  )
}
