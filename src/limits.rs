//! Deterministic per-call execution budgets, independent of wall-clock timing.

use crate::diagnostics::{Diagnostic, DiagnosticKind, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionLimits {
    /// Instructions, expression tasks, and additional output-argument separators.
    pub max_steps: u64,
    /// Selected output statements on either stream, including empty raw writes.
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

/// Additive interpreter options. Existing two-field ExecutionLimits literals
/// remain valid and old APIs use these defaults for the additional safeguards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionOptions {
    pub limits: ExecutionLimits,
    pub max_call_depth: usize,
    pub max_calls: u64,
    pub max_live_slots: usize,
    /// Conservative cumulative storage of output snapshots, including overhead.
    pub max_capture_bytes: usize,
}

impl Default for ExecutionOptions {
    fn default() -> Self {
        Self {
            limits: ExecutionLimits::default(),
            max_call_depth: 256,
            max_calls: 100_000,
            max_live_slots: 1_000_000,
            max_capture_bytes: 16 * 1024 * 1024,
        }
    }
}

impl From<ExecutionLimits> for ExecutionOptions {
    fn from(limits: ExecutionLimits) -> Self {
        Self {
            limits,
            ..Self::default()
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
