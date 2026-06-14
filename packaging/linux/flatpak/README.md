# Koko Flatpak

This directory contains the Flatpak manifest and desktop metadata for Koko.

The CI workflow generates npm and Cargo source manifests before invoking
`flatpak-builder`, so the build runs without network access inside the sandbox.
