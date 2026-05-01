pub mod account_list;
pub mod confirm;
pub mod export;
pub mod notification;
pub mod password;
pub mod vault_list;

#[derive(Debug)]
pub enum Screen {
    VaultList(vault_list::VaultList),
    AccountList(account_list::AccountList),
    Exit,
}
impl Default for Screen {
    fn default() -> Self {
        Self::VaultList(vault_list::VaultList::new())
    }
}
