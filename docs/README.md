<div align="center">
  <img src="../assets/icon.png" />
  <h1 align="center">Koko</h1>
  <h4 align="center">Self-hosted media server</h4>
</div>

<div align="center">
  <a href="https://github.com/LizardByte/Koko"><img src="https://img.shields.io/github/stars/lizardbyte/koko.svg?logo=github&style=for-the-badge" alt="GitHub stars"></a>
<!-- disabled for now
  <a href="https://github.com/LizardByte/Koko/releases/latest"><img src="https://img.shields.io/github/downloads/lizardbyte/koko/total.svg?style=for-the-badge&logo=github" alt="GitHub Releases"></a>
  <a href="https://hub.docker.com/r/lizardbyte/koko"><img src="https://img.shields.io/docker/pulls/lizardbyte/koko.svg?style=for-the-badge&logo=docker" alt="Docker"></a>
  <a href="https://github.com/LizardByte/Koko/pkgs/container/koko"><img src="https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fipitio.github.io%2Fbackage%2FLizardByte%Koko%2Fkoko.json&query=%24.downloads&label=ghcr%20pulls&style=for-the-badge&logo=github" alt="GHCR"></a>
  <a href="https://flathub.org/apps/dev.lizardbyte.app.Koko"><img src="https://img.shields.io/flathub/downloads/dev.lizardbyte.app.Koko?style=for-the-badge&logo=flathub" alt="Flathub installs"></a>
  <a href="https://flathub.org/apps/dev.lizardbyte.app.Koko"><img src="https://img.shields.io/flathub/v/dev.lizardbyte.app.Koko?style=for-the-badge&logo=flathub" alt="Flathub Version"></a>
  <a href="https://github.com/microsoft/winget-pkgs/tree/master/manifests/l/LizardByte/Koko"><img src="https://img.shields.io/winget/v/LizardByte.Koko?style=for-the-badge&logo=data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAAAAXNSR0IArs4c6QAAAARnQU1BAACxjwv8YQUAAAAJcEhZcwAADsMAAA7DAcdvqGQAAAHuSURBVFhH7ZfNTtRQGIYZiMDwN/IrCAqIhMSNKxcmymVwG+5dcDVsWHgDrtxwCYQVl+BChzDEwSnPY+eQ0sxoOz1mQuBNnpyvTdvz9jun5/SrjfxnJUkyQbMEz2ELduF1l0YUA3QyTrMAa2AnPtyOXsELeAYNyKtV2EC3k3lYgTOwg09ghy/BTp7CKBRV844BOpmmMV2+ySb4BmInG7AKY7AHH+EYqqhZo9PPBG/BVDlOizAD/XQFmnoPXzxRQX8M/CCYS48L6RIc4ygGHK9WGg9HZSZMUNRPVwNJGg5Hg2Qgqh4N3FsDsb6EmgYm07iwwvUxstdxJTwgmILf4CfZ6bb5OHANX8GN5x20IVxnG8ge94pt2xpwU3GnCwayF4Q2G2vgFLzHndFzQdk4q77nNfCdwL28qNyMtmEf3A1/QV5FjDiPWo5jrwf8TWZChTlgJvL4F9QL50/A43qVidTvLcuoM2wDQ1+IkgefgUpLcYwMVBqCKNJA2b0gKNocOIITOIef8C/F/CdMbh/GklynsSawKLHS8d9/B1x2LUqsfFyy3TMsWj5A1cLkotDbYO4JjWWZlZEGv8EbOIR1CAVN2eG8W5oNKgxaeC6DmTJjZs7ixUxpznLPLT+v4sXpoMLcLI3mzFSonDXIEI/M3QCIO4YuimBJ/gAAAABJRU5ErkJggg==" alt="Winget Version"></a>
  <a href="https://gurubase.io/g/koko"><img src="https://img.shields.io/badge/Gurubase-Ask%20Guru-ef1a1b?style=for-the-badge&logo=data:image/jpeg;base64,/9j/2wCEAAgGBgcGBQgHBwcJCQgKDBQNDAsLDBkSEw8UHRofHh0aHBwgJC4nICIsIxwcKDcpLDAxNDQ0Hyc5PTgyPC4zNDIBCQkJDAsMGA0NGDIhHCEyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMv/AABEIABgAGAMBIgACEQEDEQH/xAGiAAABBQEBAQEBAQAAAAAAAAAAAQIDBAUGBwgJCgsQAAIBAwMCBAMFBQQEAAABfQECAwAEEQUSITFBBhNRYQcicRQygZGhCCNCscEVUtHwJDNicoIJChYXGBkaJSYnKCkqNDU2Nzg5OkNERUZHSElKU1RVVldYWVpjZGVmZ2hpanN0dXZ3eHl6g4SFhoeIiYqSk5SVlpeYmZqio6Slpqeoqaqys7S1tre4ubrCw8TFxsfIycrS09TV1tfY2drh4uPk5ebn6Onq8fLz9PX29/j5+gEAAwEBAQEBAQEBAQAAAAAAAAECAwQFBgcICQoLEQACAQIEBAMEBwUEBAABAncAAQIDEQQFITEGEkFRB2FxEyIygQgUQpGhscEJIzNS8BVictEKFiQ04SXxFxgZGiYnKCkqNTY3ODk6Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGlqc3R1dnd4eXqCg4SFhoeIiYqSk5SVlpeYmZqio6Slpqeoqaqys7S1tre4ubrCw8TFxsfIycrS09TV1tfY2dri4+Tl5ufo6ery8/T19vf4+fr/2gAMAwEAAhEDEQA/AOLqSO3mlilljido4QGkYDIQEgAn05IH41seFo7aS+uRKlrJci2Y2cd2QImlyOGyQPu7sA8ZxXapAlvpThbPRkv7nTQWhDoIZZRc/XaSAOmcZGOnFfP06XMr3P17F5iqE+Tl1uuvf9Lde55dRW74pit4r61EcdtFdG2U3kVqQY0lyeBgkD5duQOASawqykuV2O6jV9rTU0rXLNjf3Om3QubSXy5QCudoYEEYIIOQR7GnahqV3qk6zXk3mOqhFAUKqqOyqAAByeAKqUUXdrFezhz89lfv1+8KKKKRZ//Z" alt="Gurubase"></a>
-->
  <a href="https://github.com/LizardByte/Koko/actions/workflows/ci.yml?query=branch%3Amaster"><img src="https://img.shields.io/github/actions/workflow/status/lizardbyte/koko/ci.yml.svg?branch=master&label=CI%20build&logo=github&style=for-the-badge" alt="GitHub Workflow Status (CI)"></a>
<!-- disabled for now
  <a href="https://github.com/LizardByte/Koko/actions/workflows/localize.yml?query=branch%3Amaster"><img src="https://img.shields.io/github/actions/workflow/status/lizardbyte/koko/localize.yml.svg?branch=master&label=localize%20build&logo=github&style=for-the-badge" alt="GitHub Workflow Status (localize)"></a>
-->
  <a href="https://docs.lizardbyte.dev/projects/koko"><img src="https://img.shields.io/readthedocs/koko.svg?label=Docs&style=for-the-badge&logo=readthedocs" alt="Read the Docs"></a>
  <a href="https://codecov.io/gh/LizardByte/Koko"><img src="https://img.shields.io/codecov/c/gh/LizardByte/Koko?token=wkbk5nOLAr&style=for-the-badge&logo=codecov&label=codecov" alt="Codecov"></a>
</div>

## ℹ️ About
Koko is a (WIP) self-hosted media server written in Rust. At this point in time this is a learning project,
and you **SHOULD NOT** use this for any purpose. I don't know what I am doing and the code is probably terrible.
This is also **NOT** a functioning media server yet. Once it is, I will update this README.

If you are interested in this project, please leave a star and watch the repository for updates.

If you would like to contribute, please reach out on our [discord](https://app.lizardbyte.dev/discord) server.

## Configuration

Koko uses a YAML configuration file to set up the server.

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
