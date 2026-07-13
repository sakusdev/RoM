# RoM release policy

RoM publishes native build outputs to GitHub Releases.

## Numbering

The workspace version in the root `Cargo.toml` is the release base version.

- A successful build of `master` is published as the next prerelease: `vMAJOR.MINOR.PATCH-alpha.N`.
- `N` is one greater than the highest existing prerelease number for the same base version.
- A pushed semantic-version tag such as `v0.1.0` publishes that exact version as a stable release.
- A manually dispatched release may use an explicit semantic-version tag or leave the tag empty to use the next automatic prerelease number.

Every archive contains a `VERSION` file and a `BUILD_INFO` file containing the release tag, source commit, target triple, Minecraft version, and protocol version.

## Published artifacts

Each release includes `ferrum-server` and `rom-bootstrap` for:

- Windows x86_64
- Linux x86_64
- Linux AArch64 using glibc
- Android/Termux AArch64 using Android Bionic
- macOS x86_64
- macOS AArch64

Raw executables, platform archives, per-file SHA-256 checksums, and a combined `SHA256SUMS` file are attached to each release.

The release also contains:

- `install-termux.sh` for native Android AArch64 Termux installations
- `install-pixel-terminal.sh` for the Linux environment provided by Pixel Terminal

Set `ROM_VERSION` to install a specific release or run the release-attached script directly to install the version it was published with.
