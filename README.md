# GravityLancacheUI

GravityLancacheUI ist ein hochmodernes, schnelles und visuell ansprechendes Überwachungs-Dashboard für **LanCache (Monolithic)**. Es wurde in Rust (Axum + Tokio) und modernem Vanilla JS/CSS entwickelt und bietet Echtzeitstatistiken, historische Download-Analysen, detaillierte Speicherplatzberichte sowie eine Integration für Prefill-Tools (SteamPrefill, BattleNetPrefill, EpicPrefill).

---

## Features

- 📊 **Echtzeit-Statistiken:** Live-Netzwerkdurchsatz, aktive Downloads und Cache-Trefferrate (Hit/Miss-Rate).
- 💾 **Historische Daten:** Lokale SQLite-Datenbank (standardmäßig mit Write-Ahead Logging für hohe Performance) oder optionale PostgreSQL-Datenbank.
- 🔍 **Disk- & Cache-Analyse:** Detaillierte Berichte darüber, welche Spiele/Plattformen wie viel Platz im Cache belegen (vollständig anpassbares Scan-Intervall).
- 🎮 **Game Resolver:** Automatische Auflösung von Steam Depot-IDs in echte Spielnamen (lokales Mapping + optionale Steam Web API-Integration).
- 🚀 **Prefill-Management:** Integrierter CLI-Wrapper zum Vorwärmen des Caches über SteamPrefill, BattleNetPrefill oder EpicPrefill.
- ⚙️ **Settings & Setup Wizard:** Ein Einrichtungsassistent prüft beim ersten Start alle Pfade und Berechtigungen. Einstellungen (API-Keys, Ausschluss-IPs etc.) können im laufenden Betrieb über das Web-UI geändert werden.

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
      - '8080:8080' # Port für das Webinterface
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

Stelle sicher, dass die Host-Pfade auf der rechten Seite der Volumes (`/mnt/...`) mit deiner Unraid-Konfiguration übereinstimmen:
- `/mnt/user/appdata/lancache/logs` sollte auf den Ordner zeigen, in dem LanCache seine `access.log` ablegt.
- `/mnt/user/lancache` sollte das Hauptverzeichnis deines LanCaches sein (wo die Unterordner `cache` oder `installs` liegen).

### 3. Container starten

Klicke im Docker Compose Plugin auf **Up**, um den Container herunterzuladen und zu starten. Das Webinterface ist anschließend unter `http://<unraid-ip>:8080` erreichbar.

---

## Konfiguration (Umgebungsvariablen)

Folgende Variablen können über die Compose-Datei konfiguriert werden:

| Variable | Beschreibung | Standardwert |
|---|---|---|
| `TZ` | Zeitzone für korrekte Zeitstempel | `Europe/Berlin` |
| `LANCACHE_LOGS_DIR` | Container-Pfad zu den LanCache-Logs | `/data/logs` |
| `LANCACHE_CACHE_DIR` | Container-Pfad zum LanCache-Speicherverzeichnis | `/data/cache` |
| `DB_PATH` | Pfad zur SQLite-Datenbank im Container | `/data/gravitylancacheui/db.sqlite` |
| `CONFIG_FILE` | Pfad zur Einstellungsdatei im Container | `/data/gravitylancacheui/config.json` |
| `LISTEN_PORT` | Port, auf dem das Webinterface lauscht | `8080` |
| `CACHE_SCAN_INTERVAL_SECS` | Intervall für die Disk-Analyse in Sekunden | `300` (5 Min.) |
| `LOG_RETENTION_DAYS` | Aufbewahrungsfrist für Download-Events in Tagen | `90` |
| `STEAM_API_KEY` | Steam Web API Key für die Namensauflösung | *(Optional)* |
| `EXCLUDED_IPS` | Kommagetrennte IPs, die im Tracking ignoriert werden | *(Optional)* |

*Hinweis: Viele dieser Einstellungen (wie der Steam API-Key, Ausschluss-IPs und das Scan-Intervall) können nach dem ersten Start auch direkt im **Settings**-Bereich des Web-UIs geändert und gespeichert werden.*

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
