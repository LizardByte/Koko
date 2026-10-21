<div align="center">
  <img
    src="https://raw.githubusercontent.com/LizardByte/Koko/refs/heads/master/assets/Koko.svg"
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
- Packaged builds include `ffmpeg` and `ffprobe` from LizardByte/build-deps. Explicit executable paths remain configurable.
- macOS packages require macOS 15.0 or later.

If you are interested in this project, please leave a star and watch the repository for updates.

If you would like to contribute, please reach out on our [discord](https://app.lizardbyte.dev/discord) server.
Contributor references are available in [contributing](../contributing/README.md).

## ⚙️ Configuration

Koko uses a YAML configuration file for core server settings.

Media libraries are stored in the application database instead of the YAML file. The browser settings UI edits
server settings in YAML and library definitions in the database.

To move a library, move its files, update the existing library's paths in **Settings → Libraries**,
and rescan it. Keep the same library to retain matched metadata, reading and playback progress, and
playlists. Check that the new paths are accessible to the server before scanning. Deleting the library
and creating a new one starts a separate catalog.

For movie library naming guidance, see [Movie naming guidelines](./MOVIE_NAMING.md).
For folder layouts and embedded tags, see [Music naming](./MUSIC_NAMING.md).
For library setup, metadata providers, and playback, see [Music libraries](./MUSIC.md).
For ebook formats, reader controls, comic libraries, and optional conversion, see [Books and comics](./BOOKS.md).

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

metadata:
  providers:
    - id: 'tmdb'
      enabled: true
      api_key: ''
      language: 'en-US'
```

Local `cargo build`, `cargo run`, and `cargo test` prepare `ffmpeg` and `ffprobe` automatically from
the release tag matching the current `third-party/build-deps` submodule commit. Initialize that submodule
with `git submodule update --init third-party/build-deps`. Git, curl, and tar must be available on `PATH`.
Downloads are cached in Cargo's build output directory; CI bundles tools from that same package.
To update the dependency, check out a different published release tag in the submodule.

Packaged builds resolve omitted FFmpeg paths from `ffmpeg/bin` beside the Koko executable. Local builds
also search the package prepared by Cargo. Explicit `ffmpeg_path` and `ffprobe_path` settings take precedence.

Set `KOKO_FFMPEG_ROOT` to an extracted build-deps package's `ffmpeg` directory to use locally built tools.
For an offline build, use that override or set `CARGO_NET_OFFLINE=true` with an already cached package.
Set `KOKO_SKIP_FFMPEG_DOWNLOAD=1` to use tools supplied separately through settings or `PATH`.

Flatpak uses `packaging/linux/flatpak/modules/ffmpeg.json`, matching Sunshine's downloads and extraction
into `/app/ffmpeg`. Its launcher adds `/app/ffmpeg/bin` to `PATH`, and its offline build skips Cargo's download.
The shared update workflow advances the build-deps submodule and refreshes the Flatpak module together.

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
    - [x] Albums/Music
    - [ ] Podcasts
    - [x] [Audiobooks](./AUDIOBOOKS.md)
  - [ ] Images
    - [ ] Photos
  - [ ] Books
    - [x] Ebooks
    - [x] PDFs
    - [x] Comics
  - [ ] Games (Pipe Dream)
    - [ ] Spin up on-demand game servers (containers or VMs)

Audiobook library setup, file naming, chapter behavior, and free test sources are documented in [Audiobooks](./AUDIOBOOKS.md).
