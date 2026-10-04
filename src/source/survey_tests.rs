use std::fs;

use super::*;

#[test]
#[ignore = "requires ENDING_CREDITS_PAYLOAD_DIR with user-owned MADDAT and ENDING.COM"]
fn extracted_ending_credits_bind_all_localizable_role_lines() {
    let directory = std::env::var("ENDING_CREDITS_PAYLOAD_DIR").unwrap();
    let maddat = fs::read(std::path::Path::new(&directory).join("MADDAT")).unwrap();
    let ending_com = fs::read(std::path::Path::new(&directory).join("ENDING.COM")).unwrap();

    let catalog = catalog_ending_credits(&maddat, &ending_com).unwrap();
    let roles = catalog
        .text
        .screens
        .iter()
        .flat_map(|screen| &screen.lines)
        .filter(|line| line.requires_localization)
        .collect::<Vec<_>>();

    assert!(!catalog.contains_japanese);
    assert!(catalog.requires_localization);
    assert_eq!(roles.len(), 10);
    assert_eq!(roles[0].reviewed_role_text.as_deref(), Some("STAFF"));
    assert_eq!(roles[9].reviewed_role_text.as_deref(), Some("Presented By"));
    assert!(roles.iter().all(|line| line.byte_size == 20));
}
