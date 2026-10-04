use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read};

use anyhow::{Context, Result, bail, ensure};
use delharc::LhaDecodeReader;

use crate::source::disk::sha256_hex;

const EXPECTED_PAYLOAD: [ExpectedPayloadFile; 11] = [
    ExpectedPayloadFile::new(
        "CONFIG.SYS",
        37,
        "6f27b06a8164802bda6c4b5dc4c04012aba01ad38570f05c8d03d628edb6c283",
    ),
    ExpectedPayloadFile::new(
        "DATAFILE",
        3_072,
        "d0bf93be8d10d5f0a22d9817a48474b830a086575641bb9df69e5c8c4ea6f85f",
    ),
    ExpectedPayloadFile::new(
        "MADDAT",
        919_585,
        "e69cbb6342226799abea8f320cad3c8114d0ce15a131698821ce822aaa18c99a",
    ),
    ExpectedPayloadFile::new(
        "MADO95.BAT",
        9,
        "556a9d1ab9a82e5b617a3a71f6969b932bec68ab1854951625c280c19b34fb0d",
    ),
    ExpectedPayloadFile::new(
        "ENDING.COM",
        18_973,
        "363a3cd17d20577ddbb9d306d1baa5f34e607d8c3849818e4e92b6dcc42011b5",
    ),
    ExpectedPayloadFile::new(
        "GAIJI.COM",
        6_419,
        "f451467620f3368ccf627f754bcaf0277baea0ee9f20a937a37e8cefe7a9335e",
    ),
    ExpectedPayloadFile::new(
        "MAD.COM",
        63_906,
        "7cf0f789f4cc9e8c98192036f314ab7f318ec438092a88ba622278941bda946f",
    ),
    ExpectedPayloadFile::new(
        "MST.COM",
        2_153,
        "dac059ccaab73b27564210aeadc274f802715ec40662b4f420c86a3d528cb3ba",
    ),
    ExpectedPayloadFile::new(
        "OPENING.COM",
        14_710,
        "b07b21a874ae2259c029e74408c54696f096af4d7059d2abf4ac6a6c205f69e3",
    ),
    ExpectedPayloadFile::new(
        "SELECT.COM",
        20_204,
        "281da5bf2579fb871f4768e824ddeefede8c927d79c12f38c61eb41a897216f5",
    ),
    ExpectedPayloadFile::new(
        "SONG.DAT",
        18_947,
        "af557d0e4212833135b69f97807a54d9dbc74248a93b068f00fe73b6797bce88",
    ),
];

const PAYLOAD_WRITE_ORDER: [&str; 11] = [
    "CONFIG.SYS",
    "DATAFILE",
    "MADDAT",
    "MADO95.BAT",
    "ENDING.COM",
    "GAIJI.COM",
    "MAD.COM",
    "MST.COM",
    "OPENING.COM",
    "SELECT.COM",
    "SONG.DAT",
];

#[derive(Clone, Copy)]
struct ExpectedPayloadFile {
    name: &'static str,
    size: usize,
    sha256: &'static str,
}

impl ExpectedPayloadFile {
    const fn new(name: &'static str, size: usize, sha256: &'static str) -> Self {
        Self { name, size, sha256 }
    }
}

pub(crate) fn payload_write_order() -> &'static [&'static str] {
    &PAYLOAD_WRITE_ORDER
}

pub(crate) fn extract_installer_payload(installer: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    let archive_offset = mz_executable_length(installer)?;
    let archive = installer
        .get(archive_offset..)
        .context("installer executable does not contain an LHa archive body")?;
    ensure!(
        archive
            .get(2..7)
            .is_some_and(|method| method.starts_with(b"-lh")),
        "installer archive does not start at the MZ-declared executable boundary"
    );

    let cursor = Cursor::new(archive);
    let mut reader = LhaDecodeReader::new(cursor)
        .map_err(|error| anyhow::anyhow!("failed to parse installer LHa header: {error}"))?;
    let mut files = BTreeMap::new();

    loop {
        let name = reader.header().parse_pathname_to_str().to_ascii_uppercase();
        ensure!(!name.is_empty(), "installer contains an empty file name");
        ensure!(
            !name.contains('/') && !name.contains('\\'),
            "installer contains an unsupported nested path: {name}"
        );
        ensure!(
            reader.is_decoder_supported(),
            "installer uses an unsupported LHa method for {name}: {:?}",
            reader.header().compression_method()
        );

        let mut bytes = Vec::with_capacity(reader.header().original_size as usize);
        reader
            .read_to_end(&mut bytes)
            .with_context(|| format!("failed to decode installer entry: {name}"))?;
        reader
            .crc_check()
            .map_err(|error| anyhow::anyhow!("LHa CRC check failed for {name}: {error}"))?;
        if files.insert(name.clone(), bytes).is_some() {
            bail!("installer contains a duplicate file name: {name}");
        }

        if !reader
            .seek_next_file()
            .map_err(|error| anyhow::anyhow!("failed to parse the next installer entry: {error}"))?
        {
            break;
        }
    }

    validate_payload(&files)?;
    Ok(files)
}

fn validate_payload(files: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    let actual_names: BTreeSet<_> = files.keys().map(String::as_str).collect();
    let expected_names: BTreeSet<_> = EXPECTED_PAYLOAD.iter().map(|file| file.name).collect();
    ensure!(
        actual_names == expected_names,
        "installer payload file set differs from the supported profile: expected {expected_names:?}, got {actual_names:?}"
    );

    for expected in EXPECTED_PAYLOAD {
        let bytes = files
            .get(expected.name)
            .with_context(|| format!("installer payload is missing {}", expected.name))?;
        ensure!(
            bytes.len() == expected.size,
            "installer payload {} has size {}, expected {}",
            expected.name,
            bytes.len(),
            expected.size
        );
        let actual_sha256 = sha256_hex(bytes);
        ensure!(
            actual_sha256 == expected.sha256,
            "installer payload {} failed SHA-256 verification: expected {}, got {}",
            expected.name,
            expected.sha256,
            actual_sha256
        );
    }
    Ok(())
}

fn mz_executable_length(executable: &[u8]) -> Result<usize> {
    ensure!(executable.len() >= 6, "installer has a truncated MZ header");
    ensure!(
        &executable[..2] == b"MZ",
        "installer is not an MZ executable"
    );
    let last_page_bytes = u16::from_le_bytes([executable[2], executable[3]]) as usize;
    let page_count = u16::from_le_bytes([executable[4], executable[5]]) as usize;
    ensure!(page_count > 0, "installer MZ header declares zero pages");
    ensure!(
        last_page_bytes <= 512,
        "installer MZ header has an invalid last-page size"
    );
    let length = if last_page_bytes == 0 {
        page_count
            .checked_mul(512)
            .context("installer MZ length overflow")?
    } else {
        page_count
            .checked_sub(1)
            .and_then(|pages| pages.checked_mul(512))
            .and_then(|bytes| bytes.checked_add(last_page_bytes))
            .context("installer MZ length overflow")?
    };
    ensure!(
        length < executable.len(),
        "installer MZ header does not leave an appended archive"
    );
    Ok(length)
}

#[cfg(test)]
#[path = "installer_tests.rs"]
mod installer_tests;
