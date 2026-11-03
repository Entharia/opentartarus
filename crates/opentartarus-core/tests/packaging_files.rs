fn packaging_path(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packaging")
        .join(relative)
}

#[test]
fn udev_rule_allow_list_only() {
    let text =
        std::fs::read_to_string(packaging_path("udev/99-opentartarus.rules")).expect("udev rule");
    assert!(text.contains("idProduct}==\"022b\""));
    assert!(text.contains("idProduct}==\"0244\""));
    assert!(text.contains("idProduct}==\"02eb\""));
    assert!(text.contains("GROUP=\"opentartarus\""));
    assert!(text.contains("KERNEL==\"uinput\""));
    assert!(!text.contains("008f"));
    assert!(!text.contains("0090"));
}

#[test]
fn the_icon_assets_ship_and_are_not_empty() {
    let svg = packaging_path("icons/opentartarus.svg");
    let png = packaging_path("icons/opentartarus-64.png");
    let svg_text = std::fs::read_to_string(&svg).expect("the SVG icon must ship");
    assert!(svg_text.contains("<svg"), "the SVG must be an SVG");
    assert!(
        svg_text.contains("64"),
        "the SVG must declare its 64 unit box"
    );

    let png_bytes = std::fs::read(&png).expect("the PNG icon must ship");
    assert_eq!(
        &png_bytes[..8],
        b"\x89PNG\r\n\x1a\n",
        "the PNG must have a PNG header"
    );
    assert!(png_bytes.len() > 100, "a tiny PNG is a blank PNG");
}

#[test]
fn the_desktop_entry_points_at_the_shipped_icon_name() {
    let text = std::fs::read_to_string(packaging_path("desktop/opentartarus.desktop"))
        .expect("desktop entry");
    assert!(text.contains("Icon=opentartarus"));
    assert!(text.contains("Exec=opentartarus-ui"));
}
