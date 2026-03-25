# tufa-rs

A terminal-based TOTP authenticator written in Rust.

> **Warning**: This project is still in development.

## Features

- TOTP code generation
- Multiple password-protected vaults
- AES-256-GCM encryption with Argon2id key derivation
- Interactive TUI and CLI modes
- Import from `otpauth://` and Google Authenticator migration format
- Export to `otpauth://` URIs
- Clipboard support (wl-copy, xclip, xsel, pbcopy)

## Installation

### From crates.io

```bash
cargo install tufa-rs
```

### From source

```bash
cargo build --release
```

The binary will be at `target/release/tufa`.

## Usage

### Interactive Mode

Run without arguments to start the interactive TUI:

```bash
tufa
```

Navigate vaults with arrow keys or `j`/`k`, press Enter to open. Select an account and press Enter to copy the current TOTP code to your clipboard.

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

## Security

Vaults are encrypted with AES-256-GCM using a key derived from your password via Argon2id. The encryption format includes a random salt and nonce per file, ensuring identical inputs produce different outputs.

## License

MIT
