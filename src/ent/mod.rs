//! Accès à l'ENT (plateforme Edifice / ODE, marque « ONE »).

pub mod client;
pub mod error;
pub mod ids;
pub mod model;

pub use client::{Corps, EntClient, Identifiants, chemin};
