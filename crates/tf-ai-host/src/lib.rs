//! AI egress host (adapter layer, ADR-0009, implementation plan §6.2): the Rust half of
//! the in-app AI. The Claude API key lives only in the OS keychain ([`CredentialVault`]).
//! The webview's SDK `fetch` is forwarded over IPC to [`EgressHost::fetch`], which checks
//! the request against the [`EgressPolicy`], adds `x-api-key`, calls the API and streams
//! the response back as [`ProxyEvent`]s. [`UsageLedger`] records one line per request.
//!
//! Nothing in this crate's public API returns the key.

mod host;
mod ledger;
mod policy;
#[cfg(test)]
mod test_server;
mod vault;

pub use host::{EgressHost, HostError, ProxyEvent};
pub use ledger::{UsageLedger, UsageRecord};
pub use policy::{
    ANTHROPIC_ORIGIN, EgressPolicy, MAX_BODY_BYTES, PolicyError, PreparedRequest, ProxyRequest,
};
pub use vault::{CredentialVault, PRODUCTION_ACCOUNT, PRODUCTION_SERVICE, VaultError};
