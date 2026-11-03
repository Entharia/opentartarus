fn packaging_path(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packaging")
        .join(relative)
}

#[test]
fn udev_rule_allow_list_only() {
    let text =
        std::fs::read_to_string(packaging_path("udev/70-opentartarus.rules")).expect("udev rule");
    assert!(text.contains("idProduct}==\"022b\""));
    assert!(text.contains("idProduct}==\"0244\""));
    assert!(text.contains("idProduct}==\"02eb\""));
    assert!(text.contains("GROUP=\"opentartarus\""));
    assert!(text.contains("KERNEL==\"uinput\""));
    assert!(!text.contains("008f"));
    assert!(!text.contains("0090"));
}

#[test]
fn the_udev_rule_runs_before_systemd_applies_uaccess() {
    // systemd's 73-seat-late.rules turns the `uaccess` tag into an ACL for
    // the logged-in user. A rule that sorts after it adds the tag too late.
    let dir = packaging_path("udev");
    let names: Vec<String> = std::fs::read_dir(&dir)
        .expect("udev dir")
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, vec!["70-opentartarus.rules".to_string()]);
    assert!(names[0].as_str() < "73-seat-late.rules");
    let text = std::fs::read_to_string(dir.join(&names[0])).unwrap();
    assert!(text.contains("TAG+=\"uaccess\""));
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

#[test]
fn the_arch_package_installs_what_the_deb_does_at_the_crate_version() {
    let pkgbuild = std::fs::read_to_string(packaging_path("arch/PKGBUILD")).expect("PKGBUILD");
    let daemon_toml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../opentartarus-daemon/Cargo.toml"),
    )
    .expect("daemon Cargo.toml");
    // Every file the deb ships, by its installed path.
    let deb = daemon_toml
        .split("[package.metadata.deb]")
        .nth(1)
        .and_then(|rest| rest.split("[package.metadata.generate-rpm]").next())
        .expect("deb metadata");
    let mut checked = 0;
    for line in deb.lines().filter(|l| l.trim_start().starts_with("[\"")) {
        let fields: Vec<&str> = line.split('"').collect();
        let source = fields[1];
        let dest_dir = fields[3];
        let name = source.rsplit('/').next().unwrap();
        let installed = format!("$pkgdir/{dest_dir}{name}");
        assert!(
            pkgbuild.contains(&installed),
            "the PKGBUILD must install {installed}"
        );
        checked += 1;
    }
    assert!(checked >= 10, "found only {checked} deb assets");

    let version = env!("CARGO_PKG_VERSION");
    assert!(
        pkgbuild.contains(&format!("pkgver={version}\n")),
        "pkgver must be {version}"
    );
    let install =
        std::fs::read_to_string(packaging_path("arch/opentartarus.install")).expect("install");
    assert!(install.contains("groupadd --system opentartarus"));
}
