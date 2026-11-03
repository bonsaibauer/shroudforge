---
name: add-news-notice
description: Add a bundled ShroudForge news notice or event-notification kind using the existing schemas, localization and display pipeline. Use for News and Chirper content, not for offline Wiki articles or command-result toasts.
---

# Add a News Notice

Read the shared repository rules at `.agents/skills/repository/repository-working-rules.md`. Paths below are relative to the repository root.

Read `modules/notifications/news.json`, `news-schema.json`, `read_news.rs`, `read_notifications.rs`, and the relevant UI projection in `ui/build-news-items.ts` and `show-notice-card.tsx`.

For a bundled article-like notice, add one stable message ID to `news.json`. Choose its existing severity, optional kind, text, links and repeat interval. HTTPS links and offline Wiki article links are alternatives in each link object. Retain existing IDs when only wording changes so read state remains meaningful. `read_news.rs` uses the embedded source, not a new remote feed.

For a generated event, use the separate `event-schema.json` and `write_notification_event.rs`. Reuse `write_mod_event.rs` and templates for mod lifecycle notices. Event state and read state use their existing sections in `state.json`. Do not store generated events by editing bundled news or create a second event store.

Translation keys follow `news.<kind>.title` and `.message`, with the current fallback/englishOnly behavior in `build-news-items.ts`. Add keys consistently to the shared UI catalogs and preserve interpolation placeholders. Use an existing presentation kind unless new rendering is actually required. New actions must be accepted by the right schema, reader, frontend contract and action dispatcher.

Preserve severity-based sorting, repeat/read timestamps and Chirper behavior. Verify schema acceptance, localized text, link/action routing and compact/full display with the relevant requested checks. Command-result toasts use a separate existing protocol and do not need a news entry.
