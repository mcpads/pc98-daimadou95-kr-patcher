use expected_write::{MachineCodeVerifierError, WritePlan, WritePlanError};
use v30::{AssembledProgram, ExpectedWriteVerifier};

/// Applies a patch plan with independent source assembly and full V30 verification.
pub fn apply_v30_patch_plan<ResolveSource>(
    baseline: &[u8],
    plan: &WritePlan,
    resolve_source: ResolveSource,
) -> Result<Vec<u8>, WritePlanError>
where
    ResolveSource: Fn(&str) -> Result<AssembledProgram, MachineCodeVerifierError> + Send + Sync,
{
    let verifier = ExpectedWriteVerifier::new(resolve_source);
    plan.apply(baseline, Some(&verifier))
}
