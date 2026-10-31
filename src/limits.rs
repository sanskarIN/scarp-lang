//! Deterministic per-call execution budgets, independent of wall-clock timing.

use crate::diagnostics::{Diagnostic, DiagnosticKind, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionLimits {
    /// Program instructions and expression tasks, including backward jumps.
    pub max_steps: u64,
    /// Number of selected output statements (including empty writes), also enforced during validation.
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
    /// Existing work and selected-output limits, shared by all invocations.
    pub limits: ExecutionLimits,
    /// Active user frames, excluding the file frame.
    pub max_call_depth: usize,
    /// Cumulative selected calls, including defaults and contracts.
    pub max_calls: u64,
    /// Slots in all live file/function frames, including saved loop state.
    pub max_live_slots: usize,
    /// Conservative cumulative storage of output snapshots, including overhead.
    pub max_capture_bytes: usize,
    /// Cumulative newly reserved numerical-vector element payload, in bytes.
    /// Sharing an existing immutable vector does not charge again.
    pub max_vector_bytes: usize,
}

impl Default for ExecutionOptions {
    fn default() -> Self {
        Self {
            limits: ExecutionLimits::default(),
            max_call_depth: 256,
            max_calls: 100_000,
            max_live_slots: 1_000_000,
            max_capture_bytes: 16 * 1024 * 1024,
            max_vector_bytes: 8 * 1024 * 1024,
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
    limit: u64,
    vector_remaining: usize,
    vector_limit: usize,
}

impl Budget {
    pub fn new(steps: u64) -> Self {
        Self {
            remaining: steps,
            limit: steps,
            vector_remaining: usize::MAX,
            vector_limit: usize::MAX,
        }
    }

    /// Configure a separate cumulative payload-allocation cap. This is not a
    /// physical process-memory cap or a live-heap byte count.
    pub fn with_vector_bytes(mut self, bytes: usize) -> Self {
        self.vector_remaining = bytes;
        self.vector_limit = bytes;
        self
    }

    pub fn charge_vector_allocation(
        &mut self,
        bytes: usize,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if bytes > self.vector_remaining {
            return Err(Diagnostic::new(
                DiagnosticKind::VectorBudget,
                span,
                format!(
                    "cumulative vector allocation limit of {} bytes exceeded",
                    self.vector_limit
                ),
            )
            .with_help("reduce vector materialization or raise --max-vector-bytes / ExecutionOptions.max_vector_bytes"));
        }
        self.vector_remaining -= bytes;
        Ok(())
    }

    pub fn charge(&mut self, amount: u64, span: Span) -> Result<(), Diagnostic> {
        if self.remaining < amount {
            return Err(Diagnostic::new(
                DiagnosticKind::ExecutionLimit,
                span,
                format!("execution step limit of {} exceeded", self.limit),
            )
            .with_help("check loop progress or raise --max-steps / ExecutionLimits.max_steps"));
        }
        self.remaining -= amount;
        Ok(())
    }
}
