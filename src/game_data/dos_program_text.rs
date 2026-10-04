use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use v30::{Instruction, decode_bytes};

use super::dos_text;

const COM_ORIGIN: usize = 0x100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DisplaySiteKind {
    Ah09Subroutine,
    DirectAh09,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TextReferenceKind {
    Immediate,
    PointerTable,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ProgramTextCatalog<'a> {
    pub programs: Vec<ProgramCatalog<'a>>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ProgramCatalog<'a> {
    pub filename: &'static str,
    pub bytes: &'a [u8],
    pub display_sites: Vec<DisplaySiteEvidence>,
    pub records: Vec<ProgramTextRecord<'a>>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct DisplaySiteEvidence {
    pub kind: DisplaySiteKind,
    pub file_offset: usize,
    pub runtime_address: usize,
    pub byte_len: usize,
    pub decoded_instruction_count: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ProgramTextRecord<'a> {
    pub id: &'static str,
    pub runtime_address: usize,
    pub file_offset: usize,
    pub bytes: &'a [u8],
    pub tokens: Vec<dos_text::DosTextToken>,
    pub references: Vec<TextReferenceEvidence>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct TextReferenceEvidence {
    pub kind: TextReferenceKind,
    pub file_offset: usize,
    pub output_file_offset: usize,
}

#[derive(Clone, Copy)]
struct ProgramSpec {
    filename: &'static str,
    display_sites: &'static [DisplaySiteSpec],
    records: &'static [RecordSpec],
}

#[derive(Clone, Copy)]
struct DisplaySiteSpec {
    kind: DisplaySiteKind,
    file_offset: usize,
    bytes: &'static [u8],
}

#[derive(Clone, Copy)]
struct RecordSpec {
    id: &'static str,
    runtime_address: usize,
    references: &'static [ReferenceSpec],
}

#[derive(Clone, Copy)]
struct ReferenceSpec {
    kind: TextReferenceKind,
    file_offset: usize,
    output_file_offset: usize,
}

const fn immediate(file_offset: usize, output_file_offset: usize) -> ReferenceSpec {
    ReferenceSpec {
        kind: TextReferenceKind::Immediate,
        file_offset,
        output_file_offset,
    }
}

const fn table(file_offset: usize, output_file_offset: usize) -> ReferenceSpec {
    ReferenceSpec {
        kind: TextReferenceKind::PointerTable,
        file_offset,
        output_file_offset,
    }
}

const FPLAY_PRINTER: &[DisplaySiteSpec] = &[DisplaySiteSpec {
    kind: DisplaySiteKind::Ah09Subroutine,
    file_offset: 0x244f,
    bytes: &[
        0xf6, 0x06, 0x18, 0x01, 0x01, 0x74, 0x01, 0xc3, 0xb4, 0x09, 0xcd, 0x21, 0xc3,
    ],
}];

const BPLAY_PRINTER: &[DisplaySiteSpec] = &[DisplaySiteSpec {
    kind: DisplaySiteKind::Ah09Subroutine,
    file_offset: 0x1c00,
    bytes: &[
        0xf6, 0x06, 0x18, 0x01, 0x01, 0x74, 0x01, 0xc3, 0xb4, 0x09, 0xcd, 0x21, 0xc3,
    ],
}];

const NMOUSE_PRINTERS: &[DisplaySiteSpec] = &[
    DisplaySiteSpec {
        kind: DisplaySiteKind::Ah09Subroutine,
        file_offset: 0x0d54,
        bytes: &[
            0xf7, 0x06, 0x8c, 0x08, 0x02, 0x00, 0x75, 0x04, 0xb4, 0x09, 0xcd, 0x21, 0xc3,
        ],
    },
    DisplaySiteSpec {
        kind: DisplaySiteKind::DirectAh09,
        file_offset: 0x0c79,
        bytes: &[0xb4, 0x09, 0xcd, 0x21],
    },
];

const BSAMP_PRINTERS: &[DisplaySiteSpec] = &[
    DisplaySiteSpec {
        kind: DisplaySiteKind::DirectAh09,
        file_offset: 0x127c,
        bytes: &[0xb4, 0x09, 0xcd, 0x21],
    },
    DisplaySiteSpec {
        kind: DisplaySiteKind::DirectAh09,
        file_offset: 0x12bb,
        bytes: &[0xb4, 0x09, 0xcd, 0x21],
    },
    DisplaySiteSpec {
        kind: DisplaySiteKind::DirectAh09,
        file_offset: 0x12eb,
        bytes: &[0xb4, 0x09, 0xcd, 0x21],
    },
];

const MAD_PRINTERS: &[DisplaySiteSpec] = &[DisplaySiteSpec {
    kind: DisplaySiteKind::DirectAh09,
    file_offset: 0xb393,
    bytes: &[0xb4, 0x09, 0xcd, 0x21],
}];

const MAD_RECORDS: &[RecordSpec] = &[
    RecordSpec {
        id: "disk-write-protected",
        runtime_address: 0xb4ad,
        references: &[immediate(0xb383, 0xb393)],
    },
    RecordSpec {
        id: "drive-not-ready",
        runtime_address: 0xb4da,
        references: &[immediate(0xb390, 0xb393)],
    },
    RecordSpec {
        id: "disk-error",
        runtime_address: 0xb501,
        references: &[immediate(0xb370, 0xb393)],
    },
];

const FPLAY_RECORDS: &[RecordSpec] = &[
    RecordSpec {
        id: "invalid-option",
        runtime_address: 0x255c,
        references: &[],
    },
    RecordSpec {
        id: "buffer-size-label",
        runtime_address: 0x257c,
        references: &[],
    },
    RecordSpec {
        id: "invalid-buffer-size",
        runtime_address: 0x2594,
        references: &[immediate(0x2383, 0x2386)],
    },
    RecordSpec {
        id: "resident-address-label",
        runtime_address: 0x25e2,
        references: &[],
    },
    RecordSpec {
        id: "resident-address-range",
        runtime_address: 0x25f2,
        references: &[],
    },
    RecordSpec {
        id: "interrupt-label",
        runtime_address: 0x25fa,
        references: &[],
    },
    RecordSpec {
        id: "interrupt-suffix",
        runtime_address: 0x260d,
        references: &[],
    },
    RecordSpec {
        id: "program-banner",
        runtime_address: 0x2a5b,
        references: &[immediate(0x271a, 0x271d)],
    },
    RecordSpec {
        id: "checking-sound-board",
        runtime_address: 0x2b4c,
        references: &[immediate(0x273c, 0x273f)],
    },
    RecordSpec {
        id: "sound-board-not-found",
        runtime_address: 0x2b76,
        references: &[immediate(0x2758, 0x275b)],
    },
    RecordSpec {
        id: "change-interrupt-to-five",
        runtime_address: 0x2bab,
        references: &[immediate(0x276f, 0x2772)],
    },
    RecordSpec {
        id: "sound-board-found",
        runtime_address: 0x2be0,
        references: &[immediate(0x2791, 0x2794)],
    },
    RecordSpec {
        id: "interrupt-number-prefix",
        runtime_address: 0x2c0d,
        references: &[immediate(0x2842, 0x2845)],
    },
    RecordSpec {
        id: "resident-failure",
        runtime_address: 0x2c1b,
        references: &[
            immediate(0x2769, 0x276c),
            immediate(0x2780, 0x2783),
            immediate(0x27df, 0x27e2),
        ],
    },
    RecordSpec {
        id: "release-success",
        runtime_address: 0x2c3f,
        references: &[immediate(0x293f, 0x2942)],
    },
    RecordSpec {
        id: "release-failure",
        runtime_address: 0x2c60,
        references: &[immediate(0x28ef, 0x28f2)],
    },
    RecordSpec {
        id: "bplay-already-resident",
        runtime_address: 0x2c88,
        references: &[immediate(0x294a, 0x294d)],
    },
    RecordSpec {
        id: "song-data-loaded",
        runtime_address: 0x2cb7,
        references: &[immediate(0x2869, 0x286c)],
    },
    RecordSpec {
        id: "interrupt-zero",
        runtime_address: 0x2cf8,
        references: &[immediate(0x2852, 0x2855)],
    },
    RecordSpec {
        id: "interrupt-five",
        runtime_address: 0x2cfb,
        references: &[immediate(0x2848, 0x2855)],
    },
    RecordSpec {
        id: "resident-success-suffix",
        runtime_address: 0x2cfe,
        references: &[immediate(0x2858, 0x285b)],
    },
    RecordSpec {
        id: "late-night-sleepy",
        runtime_address: 0x2e0a,
        references: &[table(0x2ce4, 0x2c9b)],
    },
    RecordSpec {
        id: "late-night-bad-feeling",
        runtime_address: 0x2e20,
        references: &[table(0x2ce8, 0x2c9b)],
    },
    RecordSpec {
        id: "late-night-do-not-wake",
        runtime_address: 0x2e3c,
        references: &[table(0x2cf0, 0x2c9b)],
    },
    RecordSpec {
        id: "late-night-insomnia",
        runtime_address: 0x2e6a,
        references: &[table(0x2cf4, 0x2c9b)],
    },
    RecordSpec {
        id: "late-night-bored",
        runtime_address: 0x2e8c,
        references: &[table(0x2cf8, 0x2c9b)],
    },
    RecordSpec {
        id: "morning-breakfast",
        runtime_address: 0x2eac,
        references: &[table(0x2cfc, 0x2c9b)],
    },
    RecordSpec {
        id: "color-reset",
        runtime_address: 0x2eea,
        references: &[
            table(0x2cec, 0x2c9b),
            table(0x2d00, 0x2c9b),
            immediate(0x2c9e, 0x2ca1),
            immediate(0x2cd5, 0x2cd8),
        ],
    },
    RecordSpec {
        id: "night-rest",
        runtime_address: 0x2ef1,
        references: &[table(0x2d04, 0x2c9b)],
    },
    RecordSpec {
        id: "almost-tomorrow",
        runtime_address: 0x2f1b,
        references: &[table(0x2d08, 0x2c9b)],
    },
    RecordSpec {
        id: "scream",
        runtime_address: 0x2f43,
        references: &[immediate(0x2cb3, 0x2cb6)],
    },
    RecordSpec {
        id: "asterisk-line",
        runtime_address: 0x2f59,
        references: &[],
    },
];

const BPLAY_RECORDS: &[RecordSpec] = &[
    RecordSpec {
        id: "invalid-option",
        runtime_address: 0x1d0d,
        references: &[],
    },
    RecordSpec {
        id: "buffer-size-label",
        runtime_address: 0x1d2d,
        references: &[],
    },
    RecordSpec {
        id: "invalid-buffer-size",
        runtime_address: 0x1d45,
        references: &[immediate(0x1b34, 0x1b37)],
    },
    RecordSpec {
        id: "resident-address-label",
        runtime_address: 0x1d93,
        references: &[],
    },
    RecordSpec {
        id: "resident-address-range",
        runtime_address: 0x1da3,
        references: &[],
    },
    RecordSpec {
        id: "interrupt-label",
        runtime_address: 0x1dab,
        references: &[],
    },
    RecordSpec {
        id: "interrupt-suffix",
        runtime_address: 0x1dbe,
        references: &[],
    },
    RecordSpec {
        id: "program-banner",
        runtime_address: 0x29d6,
        references: &[immediate(0x270e, 0x2711)],
    },
    RecordSpec {
        id: "resident-high",
        runtime_address: 0x2b1f,
        references: &[immediate(0x27d9, 0x27e6)],
    },
    RecordSpec {
        id: "resident-slow",
        runtime_address: 0x2b7a,
        references: &[immediate(0x27e3, 0x27e6)],
    },
    RecordSpec {
        id: "song-data-loaded",
        runtime_address: 0x2bd5,
        references: &[immediate(0x27f4, 0x27f7)],
    },
    RecordSpec {
        id: "resident-failure",
        runtime_address: 0x2c05,
        references: &[immediate(0x281d, 0x2820)],
    },
    RecordSpec {
        id: "fplay-already-resident",
        runtime_address: 0x2c29,
        references: &[immediate(0x282e, 0x2831)],
    },
    RecordSpec {
        id: "exit-without-action",
        runtime_address: 0x2c66,
        references: &[immediate(0x283a, 0x283d)],
    },
    RecordSpec {
        id: "release-success",
        runtime_address: 0x2c98,
        references: &[immediate(0x28a5, 0x28a8)],
    },
    RecordSpec {
        id: "release-failure",
        runtime_address: 0x2ce0,
        references: &[immediate(0x28b6, 0x28b9)],
    },
    RecordSpec {
        id: "force-release",
        runtime_address: 0x2dba,
        references: &[immediate(0x28c5, 0x28c8)],
    },
    RecordSpec {
        id: "night-has-arrived",
        runtime_address: 0x2ec4,
        references: &[table(0x2d9e, 0x2d55)],
    },
    RecordSpec {
        id: "someone-behind-you",
        runtime_address: 0x2eee,
        references: &[table(0x2da2, 0x2d55)],
    },
    RecordSpec {
        id: "cannot-sleep",
        runtime_address: 0x2f1c,
        references: &[table(0x2daa, 0x2d55)],
    },
    RecordSpec {
        id: "newspaper-delivery",
        runtime_address: 0x2f40,
        references: &[table(0x2dae, 0x2d55)],
    },
    RecordSpec {
        id: "sleep-before-sunrise",
        runtime_address: 0x2f70,
        references: &[table(0x2db2, 0x2d55)],
    },
    RecordSpec {
        id: "turn-to-ash",
        runtime_address: 0x2fa4,
        references: &[table(0x2db6, 0x2d55)],
    },
    RecordSpec {
        id: "color-reset",
        runtime_address: 0x2fd6,
        references: &[
            table(0x2da6, 0x2d55),
            table(0x2dba, 0x2d55),
            immediate(0x2d58, 0x2d5b),
            immediate(0x2d8f, 0x2d92),
        ],
    },
    RecordSpec {
        id: "good-evening",
        runtime_address: 0x2fdd,
        references: &[table(0x2dbe, 0x2d55)],
    },
    RecordSpec {
        id: "shooting-star",
        runtime_address: 0x2ffb,
        references: &[table(0x2dc2, 0x2d55)],
    },
    RecordSpec {
        id: "give-me-blood",
        runtime_address: 0x3021,
        references: &[immediate(0x2d6d, 0x2d70)],
    },
];

const BSAMP_RECORDS: &[RecordSpec] = &[
    RecordSpec {
        id: "program-banner",
        runtime_address: 0x166b,
        references: &[immediate(0x1279, 0x127c)],
    },
    RecordSpec {
        id: "resident-success",
        runtime_address: 0x16bb,
        references: &[immediate(0x12e8, 0x12eb)],
    },
    RecordSpec {
        id: "release-success",
        runtime_address: 0x16da,
        references: &[immediate(0x12b8, 0x12bb)],
    },
];

const NMOUSE_RECORDS: &[RecordSpec] = &[
    RecordSpec {
        id: "program-banner",
        runtime_address: 0x0fbc,
        references: &[immediate(0x0c3e, 0x0c41)],
    },
    RecordSpec {
        id: "resident-success",
        runtime_address: 0x1003,
        references: &[immediate(0x0c44, 0x0c47)],
    },
    RecordSpec {
        id: "release-success",
        runtime_address: 0x1053,
        references: &[immediate(0x0c26, 0x0c29)],
    },
    RecordSpec {
        id: "gmouse-already-resident",
        runtime_address: 0x1074,
        references: &[],
    },
    RecordSpec {
        id: "command-mode-label",
        runtime_address: 0x10b7,
        references: &[immediate(0x0c76, 0x0c79)],
    },
    RecordSpec {
        id: "frequency-120hz",
        runtime_address: 0x10ce,
        references: &[table(0x0eb0, 0x0c73)],
    },
    RecordSpec {
        id: "frequency-60hz",
        runtime_address: 0x10d9,
        references: &[table(0x0eb2, 0x0c73)],
    },
    RecordSpec {
        id: "frequency-30hz",
        runtime_address: 0x10e4,
        references: &[table(0x0eb4, 0x0c73)],
    },
    RecordSpec {
        id: "frequency-15hz",
        runtime_address: 0x10ef,
        references: &[table(0x0eb6, 0x0c73)],
    },
    RecordSpec {
        id: "ascii-mode",
        runtime_address: 0x10fa,
        references: &[table(0x0eba, 0x0c8c)],
    },
    RecordSpec {
        id: "nec-mode",
        runtime_address: 0x1107,
        references: &[table(0x0eb8, 0x0c8c)],
    },
    RecordSpec {
        id: "local-stack",
        runtime_address: 0x1110,
        references: &[immediate(0x0c97, 0x0c9a)],
    },
    RecordSpec {
        id: "help",
        runtime_address: 0x1147,
        references: &[immediate(0x0ccc, 0x0ccf)],
    },
    RecordSpec {
        id: "not-resident",
        runtime_address: 0x136b,
        references: &[immediate(0x0cfc, 0x0cff)],
    },
    RecordSpec {
        id: "both-mouse-drivers-resident",
        runtime_address: 0x1384,
        references: &[immediate(0x0ce8, 0x0cff)],
    },
    RecordSpec {
        id: "resident-status-suffix",
        runtime_address: 0x13a9,
        references: &[immediate(0x0cf7, 0x0cff)],
    },
    RecordSpec {
        id: "invalid-frequency",
        runtime_address: 0x13b6,
        references: &[immediate(0x0d18, 0x0d1b)],
    },
    RecordSpec {
        id: "invalid-command-mode",
        runtime_address: 0x13e5,
        references: &[immediate(0x0d34, 0x0d37)],
    },
];

const PROGRAMS: &[ProgramSpec] = &[
    ProgramSpec {
        filename: "MAD.COM",
        display_sites: MAD_PRINTERS,
        records: MAD_RECORDS,
    },
    ProgramSpec {
        filename: "FPLAY.COM",
        display_sites: FPLAY_PRINTER,
        records: FPLAY_RECORDS,
    },
    ProgramSpec {
        filename: "BPLAY.COM",
        display_sites: BPLAY_PRINTER,
        records: BPLAY_RECORDS,
    },
    ProgramSpec {
        filename: "BSAMP.COM",
        display_sites: BSAMP_PRINTERS,
        records: BSAMP_RECORDS,
    },
    ProgramSpec {
        filename: "NMOUSE.COM",
        display_sites: NMOUSE_PRINTERS,
        records: NMOUSE_RECORDS,
    },
];

pub(crate) fn catalog_program_text<'a, F>(mut load: F) -> Result<ProgramTextCatalog<'a>>
where
    F: FnMut(&str) -> Result<&'a [u8]>,
{
    let programs = PROGRAMS
        .iter()
        .map(|spec| {
            let bytes = load(spec.filename)?;
            catalog_program(bytes, *spec)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ProgramTextCatalog { programs })
}

fn catalog_program<'a>(bytes: &'a [u8], spec: ProgramSpec) -> Result<ProgramCatalog<'a>> {
    let display_sites = spec
        .display_sites
        .iter()
        .map(|site| verify_display_site(bytes, *site, spec.filename))
        .collect::<Result<Vec<_>>>()?;
    let display_offsets = display_sites
        .iter()
        .map(|site| site.file_offset)
        .collect::<BTreeSet<_>>();
    let mut seen_addresses = BTreeSet::new();
    let mut records = Vec::with_capacity(spec.records.len());

    for record in spec.records {
        ensure!(
            seen_addresses.insert(record.runtime_address),
            "{} has duplicate text address {:#x}",
            spec.filename,
            record.runtime_address
        );
        let pointer = u16::try_from(record.runtime_address)
            .context("utility text runtime address does not fit in a COM pointer")?
            .to_le_bytes();
        let references = record
            .references
            .iter()
            .map(|reference| {
                match reference.kind {
                    TextReferenceKind::Immediate => ensure!(
                        bytes.get(reference.file_offset..reference.file_offset + 3)
                            == Some(&[0xba, pointer[0], pointer[1]]),
                        "{} immediate text reference does not match at {:#x}",
                        spec.filename,
                        reference.file_offset
                    ),
                    TextReferenceKind::PointerTable => ensure!(
                        bytes.get(reference.file_offset..reference.file_offset + 2)
                            == Some(&pointer),
                        "{} text pointer table does not match at {:#x}",
                        spec.filename,
                        reference.file_offset
                    ),
                }
                verify_reference_output(bytes, *reference, &display_offsets, spec.filename)?;
                Ok(TextReferenceEvidence {
                    kind: reference.kind,
                    file_offset: reference.file_offset,
                    output_file_offset: reference.output_file_offset,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let file_offset = record
            .runtime_address
            .checked_sub(COM_ORIGIN)
            .context("utility text lies below the COM origin")?;
        let parsed = dos_text::parse_dos_text(bytes.get(file_offset..).with_context(|| {
            format!("{} text {} lies outside the file", spec.filename, record.id)
        })?)
        .with_context(|| format!("parse {} text {}", spec.filename, record.id))?;
        let record_bytes = &bytes[file_offset..file_offset + parsed.byte_len];
        ensure!(
            dos_text::reconstruct_dos_text(&parsed.tokens)? == record_bytes,
            "{} text {} does not reconstruct exactly",
            spec.filename,
            record.id
        );
        records.push(ProgramTextRecord {
            id: record.id,
            runtime_address: record.runtime_address,
            file_offset,
            bytes: record_bytes,
            tokens: parsed.tokens,
            references,
        });
    }

    Ok(ProgramCatalog {
        filename: spec.filename,
        bytes,
        display_sites,
        records,
    })
}

fn verify_display_site(
    bytes: &[u8],
    site: DisplaySiteSpec,
    filename: &str,
) -> Result<DisplaySiteEvidence> {
    let body = bytes
        .get(site.file_offset..site.file_offset + site.bytes.len())
        .with_context(|| {
            format!(
                "{filename} display site is truncated at {:#x}",
                site.file_offset
            )
        })?;
    ensure!(
        body == site.bytes,
        "{filename} display-site signature does not match at {:#x}",
        site.file_offset
    );
    let (decoded_instruction_count, last_instruction) = decode_exact_block(body, filename)?;
    if site.kind == DisplaySiteKind::Ah09Subroutine {
        ensure!(
            matches!(last_instruction, Instruction::Ret { pop: 0 }),
            "{filename} AH=09h display subroutine does not end in a typed V30 RET"
        );
    }
    Ok(DisplaySiteEvidence {
        kind: site.kind,
        file_offset: site.file_offset,
        runtime_address: site.file_offset + COM_ORIGIN,
        byte_len: body.len(),
        decoded_instruction_count,
    })
}

fn verify_reference_output(
    bytes: &[u8],
    reference: ReferenceSpec,
    display_offsets: &BTreeSet<usize>,
    filename: &str,
) -> Result<()> {
    let output = bytes
        .get(reference.output_file_offset..)
        .with_context(|| format!("{filename} text output lies outside the file"))?;
    if output.starts_with(&[0xb4, 0x09, 0xcd, 0x21]) {
        ensure!(
            display_offsets.contains(&reference.output_file_offset),
            "{filename} text output points to unknown direct display site {:#x}",
            reference.output_file_offset
        );
        let _ = decode_exact_block(&output[..4], filename)?;
        return Ok(());
    }

    let call = decode_bytes(
        output
            .get(..3)
            .context("utility text display call is truncated")?,
    )
    .with_context(|| format!("decode {filename} text display call"))?;
    ensure!(
        call.byte_len == 3 && matches!(call.instruction, Instruction::Call { .. }),
        "{filename} text output is not a typed V30 CALL at {:#x}",
        reference.output_file_offset
    );
    let displacement = i16::from_le_bytes([output[1], output[2]]) as isize;
    let target = reference
        .output_file_offset
        .checked_add(3)
        .and_then(|next| next.checked_add_signed(displacement))
        .context("utility text display CALL target overflow")?;
    ensure!(
        display_offsets.contains(&target),
        "{filename} text display CALL at {:#x} targets unknown site {target:#x}",
        reference.output_file_offset
    );
    Ok(())
}

fn decode_exact_block(bytes: &[u8], label: &str) -> Result<(usize, Instruction)> {
    let mut cursor = 0;
    let mut instruction_count = 0;
    let mut last_instruction = None;
    while cursor < bytes.len() {
        let decoded = decode_bytes(&bytes[cursor..])
            .with_context(|| format!("decode V30 instruction at {label} byte {cursor:#x}"))?;
        cursor += decoded.byte_len;
        instruction_count += 1;
        last_instruction = Some(decoded.instruction);
    }
    ensure!(
        cursor == bytes.len(),
        "V30 decoder crossed {label} boundary"
    );
    Ok((
        instruction_count,
        last_instruction.context("V30 evidence block is empty")?,
    ))
}

#[cfg(test)]
#[path = "dos_program_text_tests.rs"]
mod dos_program_text_tests;
