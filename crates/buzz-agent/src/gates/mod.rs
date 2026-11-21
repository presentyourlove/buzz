//! Triple Gate Validation Module
//!
//! Implementation of the three-tier verification gates specified in
//! `PLANS/DESIGN_SPEC_B_TRIPLE_GATE.md`.
pub mod semantic_gate;
pub mod fingerprint_gate;
pub mod syntax_gate;
pub mod pipeline;

pub use pipeline::{TripleGateConfig, TripleGatePipeline, TripleGatePipelineResult};
pub use syntax_gate::{SyntaxGate, SyntaxGateResult, SyntaxVerdict};
