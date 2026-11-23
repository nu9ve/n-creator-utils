//! `nu9ve mcp`: one MCP server for the suite (docs/suite-mcp.md). It reads the engine
//! registry (nu9ve-registry-v1), shows each live engine's tools under its app prefix with
//! the levels, session grants and `_untrusted` rules of nu9ve-mcp-v1, and aggregates the
//! data catalogs of nu9ve-data-v1. It knows no app: engines publish their own tools.

pub mod agent;
pub mod catalog;
pub mod confirm;
pub mod engines;
pub mod export;
mod server;
pub mod text;

pub use server::{Options, run};
