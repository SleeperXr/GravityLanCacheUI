# GravityLancacheUI

GravityLancacheUI ist ein hochmodernes, schnelles und visuell ansprechendes Überwachungs-Dashboard für **LanCache (Monolithic)**. Es wurde in Rust (Axum + Tokio) und modernem Vanilla JS/CSS entwickelt und bietet Echtzeitstatistiken, historische Download-Analysen, detaillierte Speicherplatzberichte sowie eine Integration für Prefill-Tools (SteamPrefill, BattleNetPrefill, EpicPrefill).

---

## Features

- 📊 **Echtzeit-Statistiken:** Live-Netzwerkdurchsatz, aktive Downloads und Cache-Trefferrate (Hit/Miss-Rate).
- 💾 **Historische Daten:** Lokale SQLite-Datenbank (standardmäßig mit Write-Ahead Logging für hohe Performance) oder optionale PostgreSQL-Datenbank.
- 🔍 **Disk- & Cache-Analyse:** Detaillierte Berichte darüber, welche Spiele/Plattformen wie viel Platz im Cache belegen (vollständig anpassbares Scan-Intervall + manueller Sofort-Scan per Button).
- 🎮 **Game Resolver:** Automatische Auflösung von Steam Depot-IDs in echte Spielnamen (lokales Mapping + optionale Steam Web API-Integration + Schutz für geteilte Hilfsdepots wie Steamworks).
- 🚀 **Prefill-Management:** Integrierter CLI-Wrapper zum Vorwärmen des Caches über SteamPrefill, BattleNetPrefill oder EpicPrefill.
- ⚙️ **Settings & Setup Wizard:** Ein Einrichtungsassistent prüft beim ersten Start alle Pfade und Berechtigungen. Einstellungen (API-Keys, Ausschluss-IPs, Prefill-Verzeichnis etc.) können im laufenden Betrieb über das Web-UI geändert werden.

---

## Installation auf Unraid (via Docker Compose)

Wenn du das **Docker Compose Plugin** auf Unraid verwendest, kannst du GravityLancacheUI ganz einfach über folgendes Compose-Setup einrichten:

### 1. `docker-compose.yml` vorbereiten

Erstelle ein neues Compose-Projekt in deinem Unraid-Manager und füge folgenden Inhalt ein:

```yaml
version: '3.8'

services:
  gravitylancacheui:
    image: sleeperxr/gravitylancacheui:latest
    container_name: gravitylancacheui
    restart: unless-stopped
    ports:
      - '5005:8080' # Port für das Webinterface
    environment:
      - TZ=Europe/Berlin
      - LANCACHE_LOGS_DIR=/lancache/logs
      - LANCACHE_CACHE_DIR=/lancache/cache
      - CACHE_SCAN_INTERVAL_SECS=300
      - LOG_RETENTION_DAYS=90
      # - STEAM_API_KEY=dein_steam_api_key (Optional)
      # - EXCLUDED_IPS=192.168.1.100 (Optional)
    volumes:
      # Verzeichnis für die SQLite-Datenbank und Konfigurationsdatei
      - /mnt/user/appdata/gravitylancacheui:/data/gravitylancacheui

      # Logs deines LanCache-Monolithic-Containers (schreibgeschützt einbinden)
      - /mnt/user/appdata/lancache/logs:/lancache/logs:ro

      # Cache-Verzeichnis deines LanCache-Containers für die Speicheranalyse (schreibgeschützt einbinden)
      - /mnt/user/lancache:/lancache/cache:ro
```

### 2. Pfade anpassen

Stelle sicher, dass die Host-Pfade auf der linken Seite der Volumes (`/mnt/...`) mit deiner Unraid-Konfiguration übereinstimmen:

- `/mnt/user/appdata/lancache/logs` sollte auf den Ordner zeigen, in dem LanCache seine `access.log` ablegt.
- `/mnt/user/lancache` sollte das Hauptverzeichnis deines LanCaches sein (wo die Unterordner `cache` oder `installs` liegen).

### 3. Container starten

Klicke im Docker Compose Plugin auf **Up**, um den Container herunterzuladen und zu starten. Das Webinterface ist anschließend unter `http://<unraid-ip>:5005` erreichbar.

---

## 🚀 Prefill-Tools einrichten

Die Prefill-Binärdateien (SteamPrefill, BattleNetPrefill, EpicPrefill) sind **nicht im Container vorinstalliert**. Dies liegt daran, dass sie unabhängig geupdated werden müssen und Schreibrechte in ihrem eigenen Verzeichnis benötigen, um Login-Tokens (z. B. verschlüsselte Steam Guard Credentials) und Konfigurationsdateien permanent auf deinem Server zu speichern.

*(Ein großes Dankeschön geht an den Entwickler **[tpill90](https://github.com/tpill90)** für die Bereitstellung dieser fantastischen Prefill-Tools!)*

Um die Prefill-Integration zu nutzen, befolge diese Schritte:

1. Navigiere auf deinem Host-Server in das gemountete Appdata-Verzeichnis deiner UI (z. B. `/mnt/user/appdata/gravitylancacheui/`).
2. Erstelle darin Unterordner für die jeweiligen Plattformen:
   - `SteamPrefill`
   - `BattleNetPrefill`
   - `EpicPrefill`
3. Lade die **Linux-x64**-Releases der Tools von GitHub herunter:
   - 🎮 [SteamPrefill Releases](https://github.com/tpill90/steam-lancache-prefill/releases)
   - 🌀 [BattleNetPrefill Releases](https://github.com/tpill90/battlenet-lancache-prefill/releases)
   - 🌌 [EpicPrefill Releases](https://github.com/tpill90/epic-lancache-prefill/releases)
4. Entpacke die jeweilige ausführbare Datei in den entsprechenden Ordner. Deine Ordnerstruktur sollte so aussehen:
   ```text
   /mnt/user/appdata/gravitylancacheui/
   ├── config.json
   ├── db.sqlite
   ├── SteamPrefill/
   │   └── SteamPrefill (ausführbare Datei)
   ├── BattleNetPrefill/
   │   └── BattleNetPrefill (ausführbare Datei)
   └── EpicPrefill/
       └── EpicPrefill (ausführbare Datei)
   ```
5. Setze die Ausführungsrechte für die heruntergeladenen Binärdateien auf deinem Server:
   ```bash
   chmod +x /mnt/user/appdata/gravitylancacheui/SteamPrefill/SteamPrefill
   chmod +x /mnt/user/appdata/gravitylancacheui/BattleNetPrefill/BattleNetPrefill
   chmod +x /mnt/user/appdata/gravitylancacheui/EpicPrefill/EpicPrefill
   ```
6. Stelle im Web-UI unter **Settings** den Pfad für das **Prefill-Verzeichnis** auf `/data/gravitylancacheui` (dies entspricht deinem Appdata-Pfad im Container). Nun kannst du über die Interactive Setup Console direkt Logins durchführen und Spiele auswählen!

---

## Konfiguration (Umgebungsvariablen)

Folgende Variablen können über die Compose-Datei konfiguriert werden:

| Variable | Beschreibung | Standardwert |
| --- | --- | --- |
| `TZ` | Zeitzone für korrekte Zeitstempel | `Europe/Berlin` |
| `LANCACHE_LOGS_DIR` | Container-Pfad zu den LanCache-Logs | `/data/logs` |
| `LANCACHE_CACHE_DIR` | Container-Pfad zum LanCache-Speicherverzeichnis | `/data/cache` |
| `DB_PATH` | Pfad zur SQLite-Datenbank im Container | `/data/gravitylancacheui/db.sqlite` |
| `PREFILL_DIR` | Verzeichnis mit den Prefill-Ordnern (SteamPrefill etc.) | `/data/gravitylancacheui` |
| `CONFIG_FILE` | Pfad zur Einstellungsdatei im Container | `/data/gravitylancacheui/config.json` |
| `LISTEN_PORT` | Port, auf dem das Webinterface lauscht | `8080` |
| `CACHE_SCAN_INTERVAL_SECS` | Intervall für die Disk-Analyse in Sekunden | `300` (5 Min.) |
| `LOG_RETENTION_DAYS` | Aufbewahrungsfrist für Download-Events in Tagen | `90` |
| `STEAM_API_KEY` | Steam Web API Key für die Namensauflösung | *(Optional)* |
| `EXCLUDED_IPS` | Kommagetrennte IPs, die im Tracking ignoriert werden | *(Optional)* |

*Hinweis: Viele dieser Einstellungen (wie der Steam API-Key, Ausschluss-IPs, das Scan-Intervall und das Prefill-Verzeichnis) können nach dem ersten Start auch direkt im **Settings**-Bereich des Web-UIs geändert und gespeichert werden.*

---

## Setup Wizard (Ersteinrichtung)

Beim ersten Aufruf des Dashboards prüft der integrierte Assistent automatisch:

1. Ob das Log-Verzeichnis existiert und lesbar ist.
2. Ob die `access.log` gefunden wurde.
3. Ob das Cache-Verzeichnis für Speicherplatzberichte erreichbar ist.
4. Ob die SQLite-Datenbank im Appdata-Ordner schreibbar ist.

Sollten Pfade falsch gemountet sein, zeigt dir der Wizard genau an, was korrigiert werden muss.

---

## Lizenz

Dieses Projekt steht unter der MIT-Lizenz.
