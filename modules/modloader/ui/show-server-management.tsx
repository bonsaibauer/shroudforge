import { useI18n } from '../../ui-resources/i18n'
import { FeatureBanner } from '../../ui-resources/show-feature-banner'
import { PageHeader } from '../../ui-resources/show-page-header'
import type { UpdaterCommand } from '../../updater/ui/update-types'
import shroudEditHammer from '../../../assets/shroudedit-hammer.png'

const SHROUDFORGE_REPOSITORY = 'https://github.com/bonsaibauer/shroudforge'

export function ServerManagement({ onCommand }: { onCommand: UpdaterCommand }) {
  const { t } = useI18n()

  return (
    <>
      <PageHeader
        eyebrow={t('nav.modloader')}
        title={t('nav.serverManagement')}
        subtitle={t('serverManagement.subtitle')}
      />
      <FeatureBanner
        variant="promo"
        brand="SHROUDEDIT"
        title={t('serverManagement.bannerTitle')}
        titleId="server-management-banner-title"
        description={t('serverManagement.description')}
        media={<img src={shroudEditHammer} alt="" />}
        badge={t('serverManagement.badge')}
      >
        <p className="feature-banner-contact">
          {t('serverManagement.invitation')}
        </p>
        <div className="feature-banner-actions">
          <button
            className="button ghost"
            type="button"
            onClick={() => onCommand('open-url', { url: SHROUDFORGE_REPOSITORY })}
          >
            {t('serverManagement.openRepository')}
          </button>
        </div>
      </FeatureBanner>
    </>
  )
}
