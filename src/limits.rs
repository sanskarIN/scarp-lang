//! Deterministic per-call execution budgets, independent of wall-clock timing.

use crate::diagnostics::{Diagnostic, DiagnosticKind, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionLimits {
    /// Program instructions and expression tasks, including backward jumps.
    pub max_steps: u64,
    /// Number of selected println statements, also enforced during validation.
    pub max_prints: usize,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            max_steps: 1_000_000,
            max_prints: 100_000,
        }
    }
}

pub(crate) struct Budget {
    remaining: u64,
}

impl Budget {
    pub fn new(steps: u64) -> Self {
        Self { remaining: steps }
    }

    pub fn charge(&mut self, amount: u64, span: Span) -> Result<(), Diagnostic> {
        if self.remaining < amount {
            return Err(Diagnostic::new(
                DiagnosticKind::ExecutionLimit,
                span,
                "execution step limit exceeded",
            )
            .with_help("check loop progress or select a larger execution budget"));
        }
        self.remaining -= amount;
        Ok(())
    }
}
