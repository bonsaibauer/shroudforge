# Cloudflare-Schutz für ShroudForge

Diese Einstellungen werden in der Cloudflare-Zone `shroudforge.net` vorgenommen. Sie ergänzen das Deployment der beiden Worker für `shroudforge.net` und `dev.shroudforge.net`. Der Websitebuild und Wrangler aktivieren keine WAF-Regeln oder Bot-Einstellungen. Die folgenden Regeln sind konkrete Vorschläge und wurden nicht im Livekonto eingeschaltet.

## Ausgangslage

Die Downloadseite liefert ausschließlich statische Dateien aus. Sie hat keine Formulare, Kommentare, Anmeldung oder schreibende API. Releaseinformationen und Sicherheitsberichte werden während des GitHub-Actions-Builds gelesen. Ein Seitenaufruf fragt weder GitHub noch VirusTotal ab. ZIP-Downloads werden direkt von GitHub ausgeliefert.

Dadurch betreffen Spamversuche hier vor allem massenhafte HTTP-Anfragen und automatisiertes Abrufen von Dateien. Ein CAPTCHA auf jedem Downloadbutton ist für diesen Aufbau nicht erforderlich.

## Empfohlene Grundeinstellungen

1. Beide Hostnamen als Worker Custom Domains innerhalb der aktiven Cloudflare-Zone verwenden. Wrangler übernimmt die Domainzuordnung. Die Konfiguration deaktiviert direkte `workers.dev`- und Deployment-Vorschauadressen für beide Worker.
2. Den automatischen DDoS-Schutz von Cloudflare verwenden. Er ist auf allen Plänen verfügbar und läuft ohne eigenen Schutzcode in der Website.
3. Unter **Security → Settings → Bot traffic** den **Bot Fight Mode** für den Free-Plan einschalten und anschließend **Security → Analytics → Events** beobachten. Auf Plänen mit **Super Bot Fight Mode** lassen sich die Botkategorien gezielter behandeln. Verifizierte Suchmaschinen und gewünschte Linkvorschauen sollen die Seite weiter abrufen können.
4. Keine pauschale Challenge für sämtliche Besucher, CSS-Dateien, Bilder oder Schriftdateien einschalten. Eine Prüfung der tatsächlichen Blockierungen verhindert, dass normale Nutzer und Vorschauen unnötig abgefangen werden.

Bot Fight Mode ist ein Schalter für die gesamte Domain. Er lässt sich auf Free nicht über eigene WAF-Ausnahmen umgehen. Falls legitime automatisierte Zugriffe Probleme bekommen, die Ereignisse prüfen und diese Einstellung entsprechend anpassen. Super Bot Fight Mode unterstützt gezieltere Ausnahmen.

## Eigene Regel für unnötige Schreibanfragen

Unter **Security → Security rules → Create rule → Custom rules** kann eine Regel mit dieser Bedingung angelegt werden:

```text
(http.host in {"shroudforge.net" "dev.shroudforge.net"}
 and not starts_with(http.request.uri.path, "/cdn-cgi/")
 and not http.request.method in {"GET" "HEAD"})
```

Aktion: **Block**.

Die Regel passt zur aktuellen rein lesenden Website. Cloudflares eigene `/cdn-cgi/`-Pfade bleiben ausgenommen. Falls später ein Formular, eine API oder ein anderer schreibender Dienst hinzukommt, muss die Regel vorher angepasst werden.

## Rate Limit für übermäßiges Neuladen

Unter **Security → Security rules → Create rule → Rate limiting rules** bietet sich für den Einstieg diese Bedingung an:

```text
(http.request.uri.path in {"/" "/index.html"} and not cf.client.bot)
```

Vorgeschlagener Startwert:

| Einstellung | Wert |
| --- | --- |
| Zählung | Pro Quell-IP |
| Grenze | 60 Anfragen in 10 Sekunden |
| Aktion | Block |
| Sperrdauer | 10 Sekunden |

Dieser Wert ist eine Ausgangseinstellung. In den Ereignissen beobachten, ob normale Besucher hinter gemeinsam genutzten IP-Adressen betroffen sind, und die Grenze bei Bedarf anpassen. Die Bedingung zählt nur die HTML-Einstiegsseiten. Bilder, CSS, JavaScript, Schriftdateien, `robots.txt` und die Sitemap werden von dieser Regel nicht erfasst. Verifizierte Bots sind ebenfalls ausgenommen.

Der Free-Plan stellt derzeit eine Rate-Limiting-Regel mit einem Zählfenster und einer Sperrdauer von jeweils 10 Sekunden bereit. In diesem Plan sind Pfad und Verified Bot als Bedingungsfelder verfügbar. Eine Hostnamenbedingung ist erst ab Pro verfügbar. Deshalb gilt das obige Free-Beispiel für passende Pfade in der gesamten Zone. Falls später weitere Websites auf Subdomains derselben Zone betrieben werden, die Wirkung auf diese Seiten erneut prüfen.

Ab Pro kann die Bedingung auf die beiden Hostnamen eingegrenzt werden:

```text
(http.host in {"shroudforge.net" "dev.shroudforge.net"}
 and http.request.uri.path in {"/" "/index.html"}
 and not cf.client.bot)
```

Ein einzelnes IP-Limit ergänzt den automatischen DDoS-Schutz. Es ersetzt ihn bei verteilten Anfragen aus vielen Netzen nicht.

## Google, Bing und die Devseite

Die Hauptdomain ist zur Indexierung freigegeben. Die Devdomain liefert `noindex, nofollow`. Suchmaschinen müssen die Devseite abrufen können, um diese Kennzeichnung zu lesen. Deshalb bleibt `robots.txt` dort lesbar und erlaubt den Abruf. `noindex` ist kein Zugriffsschutz.

Googlebot und Bingbot bei eigenen Bot- und Rate-Limit-Regeln anhand von `cf.client.bot` ausnehmen. Ein frei gewählter User-Agent-Text ist dafür kein verlässlicher Ersatz. Linkvorschauen in Discord nach dem Einschalten der Schutzfunktionen ebenfalls prüfen.

Falls die Devseite später privat werden soll, kann **Cloudflare Access** vor `dev.shroudforge.net` eine Anmeldung verlangen. Das ist eine eigene Entscheidung und für die derzeit vorgesehene öffentliche Devseite nicht aktiviert.

## Bei einem laufenden Angriff

Zuerst unter **Security → Analytics → Events** Hostname, Pfad und auslösenden Dienst ansehen. Konkrete missbräuchliche Muster können mit gezielten Regeln behandelt werden. **Under Attack Mode** eignet sich als vorübergehende zusätzliche Maßnahme bei einem tatsächlichen Angriff. Der Modus zeigt Besuchern eine vorgeschaltete Browserprüfung und ist für den normalen Dauerbetrieb dieser Downloadseite nicht vorgesehen.

Der Schutz des Cloudflare-Kontos gehört ebenfalls dazu. Zwei-Faktor-Anmeldung verwenden und das CI-Token auf den vorgesehenen Account und die erforderlichen Worker- und Domainrechte begrenzen.

## Quellen

- [Automatischer DDoS-Schutz](https://developers.cloudflare.com/ddos-protection/)
- [Bot Fight Mode und seine Grenzen](https://developers.cloudflare.com/bots/get-started/bot-fight-mode/)
- [Rate-Limiting-Regeln und Planverfügbarkeit](https://developers.cloudflare.com/waf/rate-limiting-rules/)
- [Statische Ressourcen und legitime Bots](https://developers.cloudflare.com/bots/additional-configurations/static-resources/)
- [Under Attack Mode](https://developers.cloudflare.com/fundamentals/reference/under-attack-mode/)
