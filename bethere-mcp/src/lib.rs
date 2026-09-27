//! bethere-mcp: an MCP server that makes an AI agent a BeThere attendee.
//!
//! The agent finds an event, registers, and pays the attendance deposit into
//! the Solana devnet escrow from its **own** wallet. The key stays in this
//! process on the agent's machine; BeThere only ever sees signatures.

pub mod api;
pub mod config;
pub mod error;
pub mod server;
pub mod tools;
