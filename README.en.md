# Nani Switch

English | [日本語](README.md)

Nani Switch is a lightweight, unofficial Windows tray utility for switching between multiple Nani accounts. It is written in Rust and does not require Electron or Node.js.

> [!CAUTION]
> This project is a proof of concept (PoC) intended exclusively for research and educational purposes. Do not use it for real account operations or everyday use.

> [!WARNING]
> Nani Switch is not an official product of Nani or Kioku LLC. Changes to Nani's data format or API may break this application.

## Requirements

- Windows 10 or Windows 11 (x64)
- The Microsoft Store version of Nani

## Installation

As this project is intended for research purposes, prebuilt releases are not distributed.
Please build and install it yourself.

## Usage

1. Sign in to the account you want to save in Nani.
2. Select `Save / update current login` from the tray menu.
3. Sign in to another account and repeat.
4. Select a saved account from the tray menu to switch to it.

Switching force-closes Nani, updates its authentication data, and restarts it without a confirmation dialog. Unsaved text will be lost.

## Features

- Save or update the current Nani login
- Preserve account order from the JSON store
- Mark the current account with a check
- Switch accounts and restart Nani
- Show remaining tokens, calls, and reset time for each account
- Refresh usage at startup, manually, or every five minutes
- Enable or disable periodic usage refresh
- Enable or disable startup with Windows

## Stored Data

Data is stored in `%USERPROFILE%\.nani-switch`.

| File | Contents |
| --- | --- |
| `accounts.json` | Saved accounts and authentication data |
| `settings.json` | Application settings |
| `nani-switch.log` | Application log; tokens are never logged |

> [!IMPORTANT]
> `accounts.json` contains decrypted access tokens required for account switching. Do not publish, share, or cloud-sync this file.

## Uninstall

Uninstall the installed version from Windows Settings under Installed apps. For the portable version, disable `Start with Windows`, exit Nani Switch, and delete the executable.

Neither method automatically deletes saved accounts. Delete `%USERPROFILE%\.nani-switch` if you also want to remove them.

## Development

Rust stable with the MSVC toolchain is required.

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

The executable is generated at `target/release/nani-switch.exe`.

## Releasing

Update the version in `Cargo.toml`, commit it, then push a matching `v` tag.

```powershell
git tag v0.1.0
git push origin v0.1.0
```

GitHub Actions builds the Windows x64 release and uploads the installer, portable EXE, ZIP, and SHA-256 checksums to a draft GitHub Release. Review and publish the draft on GitHub when it is ready.

To manually create or retry a release for an existing tag, open `Actions` → `Release` → `Run workflow` on GitHub and enter a tag such as `v0.1.0`. Running it again for the same tag replaces the assets in the draft release.

## License

[MIT License](LICENSE)
