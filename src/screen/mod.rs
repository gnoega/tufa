use crate::screen::{account_list::AccountList, vault_list::VaultList};

pub mod account_list;
pub mod confirm;
pub mod export;
pub mod password;
pub mod vault_list;

#[derive(Debug)]
pub enum Screen {
    VaultList(VaultList),
    AccountList(AccountList),
    Exit,
}
impl Default for Screen {
    fn default() -> Self {
        Self::VaultList(VaultList::new())
    }
}
