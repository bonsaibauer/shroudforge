# ShroudForge Downloadwebsite

`website/` enthält die statische Downloadwebsite für `https://shroudforge.net` und die öffentliche Devseite `https://dev.shroudforge.net`. Die vorhandene API-Dokumentation unter `docs/website/` und ihr GitHub-Pages-Workflow bleiben eigenständig. Die Downloadwebsite wird nach `build/download-website/` erzeugt und mit Cloudflare Workers Static Assets ausgeliefert. Ein eigener Worker-Entrypoint und ein Anwendungsserver sind nicht erforderlich.

## Releasequelle und lokaler Build

Die GitHub Releases von `bonsaibauer/shroudforge` sind die Datenquelle. Der Build übernimmt die veröffentlichten Releaseinformationen und verlinkt Downloads direkt auf die GitHub-Assets. Ein Browserbesuch löst keinen GitHub-API-Aufruf aus. Tokens gelangen nicht in die veröffentlichten Dateien.

Node.js 24 verwenden. Aus dem Repositoryverzeichnis wird die Website einschließlich Ausgabeprüfung so erzeugt:

```powershell
npm ci
$env:WEBSITE_ORIGIN = 'https://shroudforge.net'
$env:GH_TOKEN = gh auth token
npm run downloads:check
```

`GH_TOKEN` ist bei öffentlichen Releases optional, erhöht aber das verfügbare API-Kontingent. Den Token ausschließlich als Umgebungsvariable oder GitHub-Actions-Secret verwenden. Für einen reinen Build ohne anschließende Ausgabeprüfung steht `npm run downloads:build` zur Verfügung. Die Releasequelle und die übrige Websitekonfiguration stehen in `website/config.mjs`.

`npm ci` installiert die im Lockfile festgelegten Buildabhängigkeiten einschließlich der Dev-Abhängigkeiten. `fflate` wird ausschließlich beim Websitebuild zum Lesen der veröffentlichten ZIP-Berichte verwendet. Auf Cloudflare werden anschließend nur die erzeugten statischen Dateien veröffentlicht.

Die fertige Ausgabe kann anschließend ohne weitere Installation lokal geöffnet werden:

```powershell
npm run downloads:preview
```

Die Vorschau läuft standardmäßig unter `http://127.0.0.1:4173`. Mit `Strg+C` wird der Server beendet.

Die Website startet standardmäßig mit weißem Seitenhintergrund. Der Schalter **Light background** im Footer wechselt zwischen weißem und dunklem Seitenhintergrund. Downloadkarten, Versionsleiste, Releasearchiv, Filter und Sicherheitsanzeigen behalten in beiden Varianten ihre dunklen Originalfarben mit deckender Grundfläche. Schrift, Links und Trennlinien direkt auf dem weißen Hintergrund werden für die Lesbarkeit angepasst. Eine gespeicherte Auswahl wird bei späteren Besuchen übernommen und vor dem Laden der Darstellung angewendet. Bei gesperrtem Browserspeicher funktioniert der Schalter für den aktuellen Besuch weiterhin.

Die Webvarianten von Hammer und Favicon unter `public/assets/` sind aus `assets/shroudedit-hammer.png` abgeleitet. Der Hammer wird als 256-Pixel-PNG und das Favicon in 16, 32 und 48 Pixeln ausgeliefert. Die lokale variable Inter-Schrift stammt aus [Inter 4.1](https://github.com/rsms/inter/releases/tag/v4.1), ihre Lizenz liegt unter `public/licenses/Inter-OFL.txt`.

## Veröffentlichung im bestehenden Releaseablauf

Der vorhandene Workflow `.github/workflows/build.yml` wird weiterhin manuell auf einem Versions-Branch wie `1.0.0` gestartet. Nach dem vollständig erfolgreichen Job `publish` ruft der zusätzliche Job `website` den wiederverwendbaren Workflow `website-deploy.yml` auf. Build-, Scan-, Paket- und Releasefreigaben bleiben Bestandteil des bisherigen Ablaufs.

Der Websiteworkflow checkt den veröffentlichten Release-Tag aus und prüft dessen Commit gegen den Commit des aufrufenden Releaseworkflows. Er liest die aktuellen GitHub-Releasedaten, erzeugt und validiert die statische Ausgabe und deployt sie auf Cloudflare. Jeder Kanal hat eine eigene Warteschlange. Eine zweite Prüfung unmittelbar vor dem Deploy verwirft einen inzwischen überholten Stand.

| Auslöser nach erfolgreichem `publish` | Quelle | Ziel | Worker |
| --- | --- | --- | --- |
| Manueller Stable-Release in `build.yml` | `v<VERSION>-build.<RUN_ID>` | `shroudforge.net` | `shroudforge-net` |
| Dev-Prerelease nach Versions-Branch-Push in `dev_build.yml` | `v<VERSION>-dev.<RUN_ID>` | `dev.shroudforge.net` | `shroudforge-net-development` |

Stable verwendet das auf GitHub als neuestes Stable-Release markierte Release. Development wählt aus den vollständig veröffentlichten Dev-Prereleases die höchste semantische Version und anschließend die höchste Buildnummer. Ein bereits überholter Aufruf überschreibt die aktuellere Website nicht. Die Zielkonfiguration wird vor dem Build geprüft, damit ein Dev-Aufruf niemals auf den Produktionsworker zurückfällt.

Der direkte Aufruf nach `publish` ist beabsichtigt. Ein durch den normalen `GITHUB_TOKEN` erzeugtes Release startet üblicherweise keinen weiteren `release`-Workflow. Die Verknüpfung über `needs: publish` benötigt weder ein zusätzliches persönliches GitHub-Token noch ein GitHub-Webhook.

Die bisherigen Build-, Scan- und Publishfreigaben von `dev_build.yml` bleiben erhalten. Nur ein erfolgreich veröffentlichter Dev-Prerelease startet das Dev-Deployment. Pull Requests und fehlgeschlagene Builds veröffentlichen keine Website. Neue Dev-Releases erscheinen sofort auf der Devseite und beim nächsten Stable-Deploy oder manuellen Stable-Rebuild auch im Produktionsarchiv. Websitecode aus einem Dev-Release wird ausschließlich auf der Devdomain veröffentlicht.

## Sicherheitsberichte auf der Downloadseite

Der Websitebuild liest die bereits veröffentlichten Assets `security-report-<VERSION>-<BUILD>.zip` und die zugehörige Datei `.zip.sha256` aus den GitHub Releases. Vor dem Lesen des ZIP-Inhalts wird die SHA-256-Prüfsumme des Archivs mit der veröffentlichten Sidecardatei verglichen. Die Website zeigt die überlieferten Ergebnisse im statischen HTML an, sie startet keine neuen Scans und lädt keine Dateien zu VirusTotal oder anderen Scannern hoch.

Die Grafik unterscheidet fünf Prüfbereiche: Trivy für Quellcode und Abhängigkeiten, Microsoft Defender, Trivy für das Releasepaket, ClamAV und VirusTotal. Zwei davon sind unterschiedliche Trivy-Prüfungen. Die Darstellung benennt den jeweiligen Status und bietet vorhandene VirusTotal-Detailverweise sowie den vollständigen Bericht zum Nachlesen an.

Bei VirusTotal werden schädliche und verdächtige Erkennungen von nicht abgeschlossenen oder nicht unterstützten Prüfungen getrennt ausgewiesen. Zeitüberschreitungen und Fehler zählen damit nicht als unauffällige Scannergebnisse. Die Anzahl fehlender Abschlüsse bleibt neben den Erkennungszahlen sichtbar.

Ältere Releases ohne veröffentlichten Bericht bleiben als Downloads sichtbar und werden ausdrücklich als ohne verfügbaren Sicherheitsbericht gekennzeichnet. Ein fehlender oder nicht auswertbarer Bericht wird nicht als bestanden dargestellt. Die vorhandenen Scan-, Security- und Veröffentlichungsregeln des Modloader-Releaseworkflows bleiben unverändert.

## SEO und Suchmaschinen

Die Website liefert Überschrift, Releasearchiv und echte Downloadlinks als fertiges HTML aus. JavaScript ergänzt nur die Filter. Titel und Beschreibung benennen ShroudForge, Enshrouded und die Downloads. Eine Canonical-URL legt die Hauptadresse fest. Open-Graph- und Twitter-Metadaten enthalten Titel, Beschreibung und Hammerbild für Linkvorschauen. Die Hammericons und Inter werden lokal ausgeliefert.

Der Build erzeugt `robots.txt` und `sitemap.xml`. JSON-LD beschreibt den Websitenamen als `WebSite` und den kostenlosen Modloader als `SoftwareApplication`, einschließlich der aktuellen stabilen Version. Es werden keine Bewertungen oder Bewertungssterne erfunden. Die Ausgabeprüfung kontrolliert diese Metadaten und die Downloadlinks.

Nur die konfigurierte Produktionsadresse wird zur Indexierung freigegeben. Bei einer abweichenden `WEBSITE_ORIGIN` erhält die Seite `noindex, nofollow`. Die 404-Seite trägt ebenfalls `noindex`. Öffentliche Worker- und Vorschauadressen sind in Wrangler deaktiviert.

Der Development-Build setzt `WEBSITE_ORIGIN=https://dev.shroudforge.net` und ist damit nicht zur Indexierung freigegeben. `robots.txt` erlaubt dort weiterhin den Abruf, damit Suchmaschinen die `noindex`-Kennzeichnung lesen können. Die Devseite ist öffentlich erreichbar, `noindex` ist kein Zugriffsschutz.

Nach dem ersten Live-Deployment die Domain in Google Search Console und Bing Webmaster Tools verifizieren und `https://shroudforge.net/sitemap.xml` einreichen. Dort lassen sich Erreichbarkeit, Indexierung und Suchanfragen prüfen. Die Verifikation erfolgt im jeweiligen Konto und ist kein Bestandteil des Releasebuilds. Bei Bedarf lässt sich `www.shroudforge.net` mit einer dauerhaften Cloudflare-Weiterleitung auf die Hauptdomain vereinheitlichen.

Die Metadaten schaffen die technische Grundlage. Ob und wie Google oder andere Suchmaschinen die Website anzeigen, entscheiden deren Systeme. Für Google-Software-Rich-Results wären zusätzlich echte, öffentlich sichtbare Bewertungen erforderlich.

Referenzen: [Google zum Websitenamen](https://developers.google.com/search/docs/appearance/site-names), [Software-Strukturdaten](https://developers.google.com/search/docs/appearance/structured-data/software-app) und [Sitemaps](https://developers.google.com/search/docs/crawling-indexing/sitemaps/overview).

## Einmalige Cloudflare- und GitHub-Einrichtung

1. `shroudforge.net` als aktive Zone im vorgesehenen Cloudflare-Account einrichten. Die Domain muss diesem Account gehören. Einen bereits vorhandenen Dienst auf dieser Domain vor dem ersten Deploy bewusst ablösen. Ein kollidierender CNAME muss vor dem Anlegen der Custom Domain aufgelöst werden.
2. Im Cloudflare-Dashboard unter den API-Tokens ein Token mit der Vorlage **Edit Cloudflare Workers** anlegen und auf den vorgesehenen Account sowie die Zone `shroudforge.net` begrenzen. Die Vorlage muss das Veröffentlichen des Workers und Verwalten seiner Domainzuordnung erlauben. Die Account-ID aus der Accountübersicht übernehmen.
3. In `bonsaibauer/shroudforge` unter **Settings → Secrets and variables → Actions → Repository secrets** die Secrets `CF_API_TOKEN` und `CF_ACCOUNT_ID` anlegen. Der Workflow verwendet den vorhandenen `GITHUB_TOKEN` mit `contents: read` für den Releaseabruf.
4. Die Websiteänderungen einschließlich der Workflowdateien in den Versions-Branch aufnehmen. Für die manuelle Startschaltfläche muss `website-deploy.yml` zusätzlich im Default-Branch vorhanden sein. Ein Dev-Prerelease mit diesen Änderungen veröffentlicht zuerst die Devseite. Der erste manuelle Stable-Build mit diesen Änderungen veröffentlicht die Hauptdomain.
5. Nach erfolgreichem `publish` erzeugt Wrangler den zum Kanal gehörenden Worker und dessen Custom Domain aus `website/wrangler.jsonc`. Cloudflare verwaltet die DNS-Zuordnung und das Zertifikat. Beide Hostnamen gehören zur Zone `shroudforge.net`. Ein gewünschter Redirect von `www.shroudforge.net` auf die Hauptdomain kann separat als Cloudflare-Redirectregel eingerichtet werden.

Alternativ lassen sich die beiden Repository-Secrets mit der GitHub CLI interaktiv setzen. Die Befehle fragen die Werte ab, ohne sie als Kommandozeilenargument zu speichern:

```powershell
gh secret set CF_API_TOKEN --repo bonsaibauer/shroudforge
gh secret set CF_ACCOUNT_ID --repo bonsaibauer/shroudforge
```

Ein zusätzliches Cloudflare-Git-Deployment ist nicht erforderlich. Das Deployment übernimmt GitHub Actions mit der eingecheckten Wrangler-Konfiguration.

Konkrete Dashboard-Einstellungen gegen Bots und übermäßige Anfragen stehen in der [Cloudflare-Schutzanleitung](cloudflare-protection.de.md). Diese Konto- und Zonenregeln werden durch Wrangler nicht automatisch eingeschaltet.

## Releasedaten manuell aktualisieren

Nach dem ersten Release mit Websitecode unter **Actions → Deploy ShroudForge download website → Run workflow** starten und den Kanal `stable` oder `development` wählen. Die Auswahl eines Branches für den Workflow ersetzt nicht die Quellfreigabe. Der Workflow verwendet den aktuellen veröffentlichten Release-Tag dieses Kanals als Websitequelle und liest die Releasedaten neu ein. Damit lassen sich neue Dev-Releases und Änderungen an Releasebeschreibungen anzeigen, ohne einen weiteren Modloader-Build auszulösen.

Auch per GitHub CLI kann der Default-Branch-Workflow gestartet werden:

```powershell
gh workflow run website-deploy.yml --repo bonsaibauer/shroudforge -f channel=stable
gh workflow run website-deploy.yml --repo bonsaibauer/shroudforge -f channel=development
```

Vor dem ersten Release mit passendem Websitecode und passender Zielkonfiguration bricht der Rebuild mit einem Hinweis ab. Stable-Websiteänderungen werden durch einen neuen manuell gestarteten Stable-Build freigegeben, Dev-Websiteänderungen durch einen veröffentlichten Dev-Prerelease. Es gibt für diese Website keine zusätzlichen `dev`- oder `prod`-Branches und keine aus ShroudEdit übernommene PR-Freigabekette.

## Lokale Cloudflare-Vorschau und bewusster Direktdeploy

Nach einem erfolgreichen lokalen Websitebuild lässt sich die statische Ausgabe über Wrangler ansehen:

```powershell
npx --yes wrangler@4.115.0 dev --config website/wrangler.jsonc --env=""
```

Für einen Development-Build vor dem Build `WEBSITE_ORIGIN=https://dev.shroudforge.net` setzen. Wrangler wird für dieses Ziel mit `--env development` aufgerufen. `--env=""` wählt ausdrücklich die Produktionskonfiguration.

Der reguläre Veröffentlichungsweg ist der Releaseworkflow. Für einen ausdrücklich gewünschten Direktdeploy eines geprüften lokalen Standes dienen die folgenden Befehle. `wrangler deploy` verändert die Livewebsite und ihre Domainzuordnung:

```powershell
npx --yes wrangler@4.115.0 login
npx --yes wrangler@4.115.0 deploy --config website/wrangler.jsonc --env=""
```

Referenzen: [Workers Static Assets](https://developers.cloudflare.com/workers/static-assets/get-started/), [Custom Domains](https://developers.cloudflare.com/workers/configuration/routing/custom-domains/) und [Cloudflare-Deployment über GitHub Actions](https://developers.cloudflare.com/workers/ci-cd/external-cicd/github-actions/).
