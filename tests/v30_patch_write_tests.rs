//! Project-level V30 Expected Write verification.

use ds8_daimadou_builder::apply_v30_patch_plan;
use expected_write::{
    ExpectedWrite, ImageRegion, MachineCodeProvenance, RegionKind, WriteIntent, WritePlan,
    WritePlanError,
};
use v30::{Assembler, CodeLocation, Instruction, PROFILE_ID};

const BASELINE: [u8; 2] = [0x90, 0x90];
const SOURCE_ID: &str = "test-return-hook";

#[test]
fn applies_machine_code_produced_by_resolved_v30_source() {
    let source = test_source();
    let replacement = source.bytes().to_vec();
    let plan = machine_code_plan(replacement.clone());

    let output = apply_v30_patch_plan(&BASELINE, &plan, move |id| {
        if id == SOURCE_ID {
            Ok(source.clone())
        } else {
            Err(expected_write::MachineCodeVerifierError::new(format!(
                "unknown V30 assembly source {id}"
            )))
        }
    })
    .unwrap();

    assert_eq!(output, replacement);
}

#[test]
fn rejects_machine_code_not_produced_by_resolved_v30_source() {
    let source = test_source();
    let plan = machine_code_plan(vec![0x90, 0x90]);

    let error = apply_v30_patch_plan(&BASELINE, &plan, move |_| Ok(source.clone())).unwrap_err();

    assert_eq!(
        error,
        WritePlanError::AssembledSourceMismatch {
            write_id: "test-machine-code-write".into(),
            assembly_source_id: SOURCE_ID.into(),
        }
    );
}

fn test_source() -> v30::AssembledProgram {
    let mut assembler = Assembler::new();
    assembler
        .emit(Instruction::Nop)
        .emit(Instruction::Ret { pop: 0 });
    assembler
        .assemble(CodeLocation {
            seg: 0x1000,
            off: 0x0100,
        })
        .unwrap()
}

fn machine_code_plan(replacement: Vec<u8>) -> WritePlan {
    WritePlan::new()
        .region(ImageRegion {
            id: "test-machine-code-region".into(),
            range: 0..BASELINE.len(),
            kind: RegionKind::MachineCode,
            reason: "exercise the project's V30 patch verification boundary".into(),
        })
        .write(ExpectedWrite {
            id: "test-machine-code-write".into(),
            owner: "test-hook-producer".into(),
            purpose: "replace the fixture body with typed V30 instructions".into(),
            offset: 0,
            expected_original: BASELINE.to_vec(),
            replacement,
            intent: WriteIntent::MachineCode(MachineCodeProvenance {
                assembly_source_id: SOURCE_ID.into(),
                isa_profile_id: PROFILE_ID.into(),
            }),
        })
}
