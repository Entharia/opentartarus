/// The only privileged commands the README may print. Installing a package
/// needs root, so those lines are allowed: the deb, the rpm, and on Arch the
/// build tools for the PKGBUILD and removing the package again. Anything else means the README
/// is telling a reader to fix permissions by hand, which is the job of the
/// "Fix permissions" helper in the window.
const ALLOWED_ROOT_COMMANDS: [&str; 4] = [
    "sudo apt install ./opentartarus_*_amd64.deb",
    "sudo dnf install ./opentartarus-*.x86_64.rpm",
    "sudo pacman -S --needed base-devel git rust",
    "sudo pacman -R opentartarus",
];

const ROOT_COMMAND_PREFIX: &str = "sudo ";

#[test]
fn readme_uses_official_name_and_github() {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../README.md");
    let t = std::fs::read_to_string(p).unwrap();
    assert!(t.contains("OpenTartarus"));
    assert!(t.contains("https://github.com/PikkonMG/opentartarus"));
    assert!(!t.contains("OpenTaris"));
    assert!(!t.contains("systemctl"));
    assert!(t.contains("Polychromatic"));
    assert!(t.contains("RazerGenie"));
    assert!(t.contains("frontends"));
    assert!(t.contains("org.razer"));
    assert!(t.contains("OpenRGB"));
    assert!(t.contains("1532:022b"));
    assert!(t.contains("~/.local/share/icons/hicolor"));
    assert!(t.contains("opentartarus.svg"));
}

#[test]
fn readme_only_asks_for_root_to_install_a_package() {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../README.md");
    let t = std::fs::read_to_string(p).unwrap();
    for line in t.lines().filter(|line| line.contains(ROOT_COMMAND_PREFIX)) {
        assert!(
            ALLOWED_ROOT_COMMANDS
                .iter()
                .any(|allowed| line.contains(allowed)),
            "README asks for root outside a package install: {line}"
        );
    }
}
