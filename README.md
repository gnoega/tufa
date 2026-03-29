<div align="center">
  <img src="assets/tufa.svg" alt="tufa logo" width="300" height="300" />
  <h1>tufa-rs</h1>
  <p>A terminal-based TOTP authenticator written in Rust.</p>

  <p>
    <a href="https://crates.io/crates/tufa-rs"><img src="https://img.shields.io/crates/v/tufa-rs?style=flat-square&color=7C3AED" alt="crates.io" /></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-7C3AED?style=flat-square" alt="MIT License" /></a>
    <img src="https://img.shields.io/badge/status-in%20development-orange?style=flat-square" alt="Status" />
    <img src="https://img.shields.io/badge/made%20with-Rust-orange?style=flat-square&logo=rust" alt="Rust" />
  </p>
</div>

> **Warning**: This project is still in development.

---

## Overview

**tufa** is a fast, secure, offline TOTP authenticator for the terminal. Secrets live in encrypted vaults on your local machine.

---

## Features

- TOTP code generation
- Multiple password-protected vaults
- AES-256-GCM encryption with Argon2id key derivation
- Interactive TUI and CLI modes
- Import from `otpauth://` and Google Authenticator migration format
- Export to `otpauth://` URIs
- Clipboard support (`wl-copy`, `xclip`, `xsel`, `pbcopy`)

---

## Installation

### From crates.io

```bash
cargo install tufa-rs
```

### From source

```bash
git clone https://github.com/gnoega/tufa
cd tufa-rs
cargo build --release
# binary at: target/release/tufa
```

---

## Usage

### Interactive TUI

Launch without arguments to open the interactive interface:

```bash
tufa
```

Navigate vaults with arrow keys or `j`/`k`, press `Enter` to open. Select an account and press `Enter` to copy the current TOTP code to your clipboard.

### CLI Mode

```
tufa show <account>      # Display the current TOTP code
tufa list [vault]        # List all accounts in a vault
tufa add <name> <secret> # Add a new TOTP account
tufa del <name>          # Delete a TOTP account
tufa import <uri>        # Import from an otpauth:// URI
tufa export [account]    # Export accounts as otpauth:// URIs
```

Account names use the format `<issuer>:<name>` or `<vault>.<issuer>:<name>`.

---

## Security

Vaults are encrypted with AES-256-GCM using a key derived from your password via Argon2id. Each vault file includes a unique random salt and nonce, ensuring identical inputs produce different ciphertext.

All data stays on your machine

Vaults are stored in your system's config directory:

| Platform | Path                                  |
| -------- | ------------------------------------- |
| Linux    | `~/.config/tufa/`                     |
| macOS    | `~/Library/Application Support/tufa/` |
| Windows  | `%APPDATA%\tufa\`                     |

Each vault is a separate `.2fa` file.

---

## License

[MIT](LICENSE)
