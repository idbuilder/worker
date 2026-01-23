//! Domain models for IDBuilder.
//!
//! This module contains the core domain types representing ID configurations,
//! sequences, and API contracts.

pub mod config;
pub mod dto;
pub mod sequence;

pub use config::{
    FormattedConfig, IdConfig, IdType, IncrementConfig, SequenceReset, SnowflakeConfig,
};
pub use dto::{
    ApiResponse, ConfigResponse, FormattedIdResponse, GenerateRequest, IdResponse,
    IncrementIdResponse, SnowflakeIdResponse, TokenRequest, TokenResponse,
};
pub use sequence::{SequenceRange, SequenceState};
