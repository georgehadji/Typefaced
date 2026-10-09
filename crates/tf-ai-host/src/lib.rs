//! AI egress host (adapter layer, ADR-0009, implementation plan §6.2): the Rust half of
//! the in-app AI. The OpenRouter API key lives only in the OS keychain
//! ([`CredentialVault`]). The webview's SDK `fetch` is forwarded over IPC to
//! [`EgressHost::fetch`], which checks the request against the [`EgressPolicy`], adds
//! `Authorization: Bearer <key>`, calls OpenRouter's Messages API and streams
//! the response back as [`ProxyEvent`]s. [`UsageLedger`] records one line per request.
//!
//! Nothing in this crate's public API returns the key.

mod body;
mod host;
mod ledger;
mod policy;
#[cfg(test)]
mod test_server;
mod vault;

pub use host::{EgressHost, HostError, ProxyEvent};
pub use ledger::UsageLedger;
pub use policy::{EgressPolicy, PolicyError, ProxyRequest};
pub use vault::{CredentialVault, VaultError};
