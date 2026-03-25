use clap::{Parser, Subcommand};
use std::io::{self, Write};
use std::path::Path;

use crate::totp::{self, TotpEntry, TotpError};
use crate::vault::{Vault, VaultError};

#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub(crate) command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Display the current TOTP code for an account
    Show {
        /// Account identifier in format "<issuer>:<name>" or "<vault>.<issuer>:<name>"
        name: String,

        /// Display the remaining time before the code expires
        #[arg(short = 't', long, action = clap::ArgAction::SetTrue)]
        ttl: bool,
        /// Display the issuer associated with the account
        #[arg(short = 'i', long, action = clap::ArgAction::SetTrue)]
        issuer: bool,
    },
    /// List all TOTP accounts in a vault
    List {
        /// Name of the vault to list (defaults to "vault")
        vault: Option<String>,
    },
    /// Add a new TOTP account to a vault
    Add {
        /// Account identifier in format "<issuer>:<name>" or "<vault>.<issuer>:<name>"
        name: String,
        /// TOTP secret key
        secret: String,
    },
    /// Delete a TOTP account in the vault
    Del {
        /// Account identifier in format "<issuer>:<name>" or "<vault>.<issuer>:<name>"
        name: String,
    },
    /// Import a TOTP account from an otpauth:// URI
    Import {
        /// otpauth:// URI to import
        uri: String,
        /// Vault to import into (defaults to "vault")
        #[arg(long)]
        vault: Option<String>,
    },
    /// Export TOTP accounts as otpauth:// URIs
    Export {
        /// Account identifier in format "<issuer>:<name>" or "<vault>.<issuer>:<name>"
        name: Option<String>,

        #[arg(short = 'q', long = "qr", action = clap::ArgAction::SetTrue)]
        qrcode: bool,
    },
}

pub enum CLIError {
    TotpError(TotpError),
    VaultError(VaultError),
}

impl std::fmt::Display for CLIError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CLIError::TotpError(e) => write!(f, "TOTP error: {e}"),
            CLIError::VaultError(e) => write!(f, "Vault error: {e}"),
        }
    }
}

impl From<TotpError> for CLIError {
    fn from(e: TotpError) -> Self {
        CLIError::TotpError(e)
    }
}

impl From<VaultError> for CLIError {
    fn from(e: VaultError) -> Self {
        CLIError::VaultError(e)
    }
}

impl From<std::io::Error> for CLIError {
    fn from(e: std::io::Error) -> Self {
        CLIError::VaultError(VaultError::Io(e))
    }
}

fn prompt_password() -> Result<String, CLIError> {
    print!("Enter vault password: ");
    io::stdout().flush()?;
    let password = rpassword::read_password().map_err(|e| {
        CLIError::VaultError(VaultError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            e.to_string(),
        )))
    })?;
    print!("\r\x1b[2K\r\x1b[A\r\x1b[2K");
    io::stdout().flush()?;
    if password.is_empty() {
        return Err(CLIError::VaultError(VaultError::CryptoError(
            "password cannot be empty".to_string(),
        )));
    }
    Ok(password)
}

fn prompt_password_confirm() -> Result<String, CLIError> {
    loop {
        print!("Enter vault password: ");
        io::stdout().flush()?;
        let password = rpassword::read_password().map_err(|e| {
            CLIError::VaultError(VaultError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )))
        })?;
        if password.is_empty() {
            println!("Password cannot be empty. Try again.");
            continue;
        }

        print!("Confirm password: ");
        io::stdout().flush()?;
        let confirm = rpassword::read_password().map_err(|e| {
            CLIError::VaultError(VaultError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )))
        })?;

        if password == confirm {
            return Ok(password);
        }
        println!("Passwords do not match. Try again.");
    }
}

pub fn handle_command(cmd: Command) -> Result<(), CLIError> {
    match cmd {
        Command::Show { name, ttl, issuer } => {
            let (vault_name, account_name) = parse_account_name(&name);
            let vault = Vault::new(&vault_name);

            if !vault.exists() {
                return Err(CLIError::VaultError(VaultError::NotFound));
            }

            let password = prompt_password()?;
            let accounts = vault.load(password.as_bytes())?;

            let account = accounts
                .iter()
                .find(|acc| acc.display_name().to_lowercase() == account_name.to_lowercase())
                .ok_or_else(|| CLIError::VaultError(VaultError::AccountNotFound(account_name)))?;

            let code = account.generate_otp()?;

            let mut output = vec![code];
            if issuer {
                if let Some(iss) = &account.issuer {
                    output.push(format!("issuer: {iss}"));
                }
            }
            if ttl {
                output.push(format!("ttl: {}s", totp::totp_ttl()));
            }

            println!("{}", output.join(" "));
            Ok(())
        }
        Command::List { vault } => {
            match vault {
                Some(name) => {
                    let vault = Vault::new(name);
                    if !vault.exists() {
                        return Err(CLIError::VaultError(VaultError::NotFound));
                    }

                    let password = prompt_password()?;
                    let accounts = vault.load(password.as_bytes())?;
                    let ttl = totp::totp_ttl();

                    for account in accounts {
                        let code = account.generate_otp()?;
                        println!("{:40} {} {:2}s", account.display_name(), code, ttl);
                    }
                }
                None => {
                    let mut vaults = Vault::list_all();
                    vaults.sort();
                    for vault in vaults {
                        let name = Path::new(&vault)
                            .file_stem()
                            .and_then(|f| f.to_str())
                            .unwrap_or(&vault);
                        println!("{}", name);
                    }
                }
            }

            Ok(())
        }
        Command::Add { name, secret } => {
            let (vault_name, display_name) = parse_account_name(&name);
            let vault = Vault::new(vault_name);

            let (issuer, account_name) = match display_name.split_once(':') {
                Some((i, n)) => (Some(i.trim().to_string()), n.trim().to_string()),
                None => (None, display_name),
            };

            let mut totp = TotpEntry::new(secret, account_name)?;
            if let Some(issuer) = issuer {
                totp = totp.with_issuer(issuer);
            }

            let (mut accounts, password) = if vault.exists() {
                let password = prompt_password()?;
                let accounts = vault.load(password.as_bytes())?;

                (accounts, password)
            } else {
                println!("No existing vault found. Creating new vault...");
                let password = prompt_password_confirm()?;

                (Vec::new(), password)
            };

            if accounts.iter().any(|acc| acc.secret == totp.secret) {
                return Err(CLIError::TotpError(TotpError::DuplicatedSecret));
            }

            accounts.push(totp);
            vault.save(&accounts, password.as_bytes())?;

            Ok(())
        }
        Command::Del { name } => {
            let (vault_name, account_name) = parse_account_name(&name);
            let vault = Vault::new(vault_name);
            if !vault.exists() {
                return Err(CLIError::VaultError(VaultError::NotFound));
            }
            let password = prompt_password()?;
            let mut accounts = vault.load(password.as_bytes())?;
            let account = accounts
                .iter()
                .find(|acc| acc.display_name().to_lowercase() == account_name.to_lowercase());

            let account = match account {
                Some(acc) => acc,
                None => {
                    return Err(CLIError::VaultError(VaultError::AccountNotFound(
                        account_name,
                    )));
                }
            };
            if !prompt_confirm(&format!(
                "Are you sure you want to remove {}?",
                account.display_name()
            ))? {
                return Ok(());
            };

            accounts.retain(|acc| acc.display_name().to_lowercase() != account_name);
            vault.save(&accounts, password.as_bytes())?;
            Ok(())
        }
        Command::Import { uri, vault } => {
            let vault_name = vault.unwrap_or_else(|| "vault".to_string());
            let vault = Vault::new(vault_name);

            let entries = totp::parse_uri(&uri)?;
            let (mut accounts, password) = if vault.exists() {
                let password = prompt_password()?;
                let accounts = vault.load(password.as_bytes())?;
                (accounts, password)
            } else {
                println!("No existing vault found. Creating new vault...");
                let password = prompt_password_confirm()?;
                (Vec::new(), password)
            };
            let count = entries.len();
            accounts.extend(entries);
            vault.save(&accounts, password.as_bytes())?;
            println!("Imported {count} account");

            Ok(())
        }
        Command::Export { name, qrcode } => {
            let (vault_name, account_name): (String, Option<String>) = match name {
                Some(name) => match name.split_once('.') {
                    Some((vault, acc)) => (vault.to_string(), Some(acc.to_string())),
                    None => ("vault".to_string(), Some(name)),
                },
                None => ("vault".to_string(), None),
            };

            let vault = Vault::new(vault_name);

            if !vault.exists() {
                return Err(CLIError::VaultError(VaultError::NotFound));
            }

            let password = prompt_password()?;
            let mut accounts = vault.load(password.as_bytes())?;

            if let Some(acc) = account_name {
                accounts.retain(|a| a.display_name().to_lowercase() == acc.to_lowercase());
                if accounts.is_empty() {
                    return Err(CLIError::VaultError(VaultError::AccountNotFound(acc)));
                }
            }

            for account in accounts {
                let uri = account.to_uri();

                if qrcode {
                    match uri.to_qrcode_string() {
                        Ok(qr) => {
                            println!("\n  {}", account.display_name());
                            println!("{}", qr);
                            println!("  {}\n", uri);
                        }
                        Err(e) => {
                            eprintln!("failed to render QR for {}: {e}", account.display_name())
                        }
                    }
                } else {
                    println!("{}", uri)
                }
            }

            Ok(())
        }
    }
}

fn parse_account_name(input: &str) -> (String, String) {
    match input.split_once(".") {
        Some((vault, account)) => (vault.to_string(), account.to_string()),
        None => ("vault".to_string(), input.to_string()),
    }
}

fn prompt_confirm(prompt: &str) -> Result<bool, io::Error> {
    print!("{} [y/N]: ", prompt);

    io::stdout().flush()?;
    let mut input = String::new();

    io::stdin().read_line(&mut input)?;
    Ok(matches!(
        input.trim_end().to_lowercase().as_str(),
        "y" | "yes"
    ))
}
