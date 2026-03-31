use crate::screen::{
    account_list::AccountList, export::ExportTotp, password::PasswordPrompt, vault_list::VaultList,
};

pub mod account_list;
pub mod export;
pub mod password;
pub mod vault_list;

#[derive(Debug)]
pub enum Screen {
    VaultList(VaultList),
    PasswordPrompt(PasswordPrompt),
    AccountList(AccountList),
    ExportPopUp(ExportTotp),
    Exit,
}
impl Default for Screen {
    fn default() -> Self {
        Self::VaultList(VaultList::new())
    }
}

impl Screen {
    pub fn password_prompt(vault_name: String) -> Self {
        Self::PasswordPrompt(PasswordPrompt::new(vault_name))
    }

    pub fn password_error(vault_name: String, error: &'static str) -> Self {
        Self::PasswordPrompt(PasswordPrompt::with_error(vault_name, Some(error)))
    }
}
