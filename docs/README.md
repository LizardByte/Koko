<div align="center">
  <img
    src="../assets/Koko.svg"
    alt="Koko icon"
    width="256"
  />
  <h1 align="center">Koko</h1>
  <h4 align="center">Self-hosted media server.</h4>
</div>

<div align="center">
  <a href="https://github.com/LizardByte/Koko"><img src="https://img.shields.io/github/stars/lizardbyte/koko.svg?logo=github&style=for-the-badge" alt="GitHub stars"></a>
  <a href="https://github.com/LizardByte/Koko/actions/workflows/ci.yml?query=branch%3Amaster"><img src="https://img.shields.io/github/actions/workflow/status/lizardbyte/koko/ci.yml.svg?branch=master&label=CI%20build&logo=github&style=for-the-badge" alt="GitHub Workflow Status (CI)"></a>
  <a href="https://codecov.io/gh/LizardByte/Koko"><img src="https://img.shields.io/endpoint.svg?url=https%3A%2F%2Fapp.lizardbyte.dev%2Fdashboard%2Fshields%2Fcodecov%2FKoko.json&style=for-the-badge&logo=codecov" alt="Codecov"></a>
  <a href="https://sonarcloud.io/project/overview?id=LizardByte_Koko"><img src="https://img.shields.io/sonar/quality_gate/LizardByte_Koko.svg?server=https%3A%2F%2Fsonarcloud.io&style=for-the-badge&logo=sonarqubecloud&label=sonarcloud" alt="SonarCloud"></a>
</div>

## ℹ️ About
Koko is a (WIP) self-hosted media server written in Rust. At this point in time this is a learning project,
and you **SHOULD NOT** use this for any purpose. I don't know what I am doing and the code is probably terrible.
This is also **NOT** a functioning media server yet. Once it is, I will update this README.

Current product direction highlights:

- The browser UI is targeting a Kodi/Plex-style media-first experience.
- TheMovieDB is the first planned online metadata source for movies and TV.
- Metadata providers should stay pluggable so Koko can add more sources over time.
- Koko currently assumes external `ffmpeg` and `ffprobe` executables by default to keep the licensing path clearer for source-available distribution.

If you are interested in this project, please leave a star and watch the repository for updates.

If you would like to contribute, please reach out on our [discord](https://app.lizardbyte.dev/discord) server.

## ⚙️ Configuration

Koko uses a YAML configuration file for core server settings.

Media libraries are stored in the application database instead of the YAML file. The browser settings UI edits
server settings in YAML and library definitions in the database.

Database schema changes are managed only through Diesel SQL migrations in `crates/server/sql/migrations`.
Koko has not had a release yet, so the current database starts from one consolidated initial schema migration.
Migration directories use an opaque hash-like revision prefix, and runtime execution order is defined by
`SQLITE_MIGRATION_ORDER` in `crates/server/src/db/mod.rs` instead of hash lexicographic order. Future schema or data
changes should be added as new migration files instead of Rust startup repair code.

Create a new migration from the repository root with:

```console
cargo new-migration add_media_flags
```

If the name is omitted, the command prompts for it. The command generates a unique revision, creates `up.sql` and
`down.sql`, appends the revision to `SQLITE_MIGRATION_ORDER`, and runs `cargo +nightly fmt`.

For movie library naming guidance, see [Movie naming guidelines](./MOVIE_NAMING.md).

The file must be named `settings.yml` and be placed in the following location, depending on your OS.

| OS      | Location                                 |
|---------|------------------------------------------|
| Linux   | `$XDG_CONFIG_HOME/Koko`                  |
| macOS   | `$HOME/Library/Application Support/Koko` |
| Windows | `%LOCALAPPDATA%\Koko`                    |

Only the non default values need to be set in the configuration file.
An example with all the default values is shown below.

```yml
---
general:
  data_dir: 'data'

server:
  use_https: true
  address: '127.0.0.1'
  port: 9191
  cert_path: 'cert.pem'
  key_path: 'key.pem'
  use_custom_certs: false

ffmpeg:
  strategy: 'external_binaries'
  ffmpeg_path: 'ffmpeg'
  ffprobe_path: 'ffprobe'

metadata:
  providers:
    - id: 'tmdb'
      enabled: true
      api_key: ''
      language: 'en-US'
```

## 📝 TODO
This list is not all-inclusive, and just meant to be a very high level for the initial design.

- [ ] Branding
  - [x] Koko logo
  - [ ] Koko banner
  - [ ] Tray icons for different states/activity
- [ ] Publishing (enabling readme badges as required)
  - [ ] GitHub Releases
  - [ ] Docker/GHCR
  - [ ] Flathub
  - [ ] Winget
  - [ ] LizardByte/Homebrew
  - [ ] LizardByte/pacman-repo
- [ ] Localization and CrowdIn integration
- [x] Unit Testing
  - [ ] doc tests
  - [x] Coverage
- [x] Settings/Config
- [ ] Notification System
  - [ ] System Notifications
  - [ ] Discord
  - [ ] Webhooks
- [x] Database
- [ ] Backend
  - [x] Authentication
  - [ ] API
  - [x] Certs/SSL
  - [ ] Media Scanner
  - [ ] Media Player
  - [x] Legal/Licensing info on dependencies
- [ ] Frontend
  - [ ] Home
  - [ ] Media
  - [ ] Settings
  - [ ] Dashboard
    - [ ] System Info
    - [ ] CPU Usage
    - [ ] Memory Usage
    - [ ] Disk Usage
    - [ ] Network Usage
    - [ ] GPU Usage
    - [ ] Play history
  - [ ] Media Player
  - [ ] User Management
  - [ ] Legal/Licensing info on dependencies
- [ ] Plugin system
- [ ] User Documentation
  - [x] Publish docs to ReadTheDocs
  - [ ] Create Gurubase and enable readme badge
- [ ] Media
  - [ ] Live TV
    - [ ] DVR/Tuner
  - [ ] Video
    - [ ] Movies
    - [ ] TV Shows
    - [ ] Videos
  - [ ] Audio
    - [ ] Albums/Music
    - [ ] Podcasts
    - [ ] Audiobooks
  - [ ] Images
    - [ ] Photos
  - [ ] Books
    - [ ] Ebooks
    - [ ] PDFs
    - [ ] Comics
  - [ ] Games (Pipe Dream)
    - [ ] Spin up on-demand game servers (containers or VMs)
