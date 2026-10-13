//! Real public/private HTTPS Git source lifecycle (phase 13).
//!
//! Policy validates URLs/refs and network destinations; checkout runs a fixed
//! Git binary as the application user with a hardened environment; credentials
//! are delivered transiently for private repositories.

pub mod checkout;
pub mod credentials;
pub mod policy;

pub use checkout::{CheckoutError, CheckoutOutcome, CheckoutRequest, GitRunner};
pub use credentials::{CredentialError, CredentialFile};
pub use policy::{
    GitPolicyError, GitRef, NetworkPolicy, RepositoryUrl, validate_ref, validate_repository_url,
};
