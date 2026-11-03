# OpenTartarus

**A Razer Synapse replacement for Linux. Key remapping, per-game profiles, macros and RGB lighting for the Razer Tartarus V2, Tartarus Pro and Tartarus V2 Pro keypad.**

![The OpenTartarus window, showing the profile list on the left and the Tartarus V2 key pad in the middle](docs/screenshots/window-keys.png)

A Linux app for the Razer Tartarus V2 (`1532:022b`), Tartarus Pro (`1532:0244`) and Tartarus V2 Pro (`1532:02eb`). It remaps the keypad with per-game profiles, the way Razer Synapse does on Windows, and it drives the lighting through OpenRazer. If OpenRazer is not running it tries OpenRGB. With neither, remaps still work and the lighting controls stay off.

Two processes make up the app. A tray daemon grabs the keypad, plays the remaps, and talks to lighting. A window, built with iced, edits profiles and shows the pad. Close the window and the daemon keeps remapping. Quit from the tray menu to stop it.

Source: https://github.com/PikkonMG/opentartarus

**Status: testing.** Version 0.1.0 is an early build, marked as a pre-release on GitHub. It works on the developer's machine with a Tartarus V2. The Tartarus Pro path is built from Razer's specification and has not been tried on a real Pro. The Tartarus V2 Pro key map was recorded from a real V2 Pro. Bug reports are welcome; see CONTRIBUTING.md for what to include.

## What you need

- Linux, on X11 or Wayland
- A Tartarus V2, Tartarus Pro or Tartarus V2 Pro
- Rust, to build from this tree
- OpenRazer, if you want lighting. Keys work without it.

Polychromatic and RazerGenie are frontends for `openrazer-daemon`, not a second driver. You can keep them open. OpenTartarus uses the same daemon on the session bus name `org.razer`, and the same `razerkbd` sysfs nodes. Remaps stay in OpenTartarus.

## What it does

The pad is drawn in the window. Click a key to change what it sends. A key can send:

- a key, with or without Ctrl, Shift, Alt or Super
- a bare Shift, Ctrl or Alt, for sprint, crouch and walk
- a mouse button or wheel step
- a macro, a list of key steps with delays
- the same key over and over while held
- a jump to one profile, or a step to the next one, so the pad can switch its own profile

To set a key, press "Record a key" and then press the key you want on your keyboard or on the pad. You can also type a combo into the box, or click one of the chips.

Thirteen profiles ship with the app. Default is Razer's own layout: the left of a keyboard, with 1 to 5 on the top row, Tab Q W E R under that, Caps A S D F on the rest row, Shift Z X C at the bottom, Space on the thumb key and the arrow keys on the thumb pad. Any game's own key settings work with it as is.

The other twelve are built from each game's default PC bindings: Counter-Strike 2, Deadlock, Diablo 4, Dota 2, Elden Ring, Elder Scrolls Online, Final Fantasy XIV, Guild Wars 2, Minecraft, Overwatch 2, Path of Exile and World of Warcraft. All of them run on Linux, natively or through Proton. Games that move with WASD put movement on the thumb pad and the most used actions on the rest row. Two profiles need a step inside the game first. Diablo 4 needs its Keyboard Movement preset switched on. Elden Ring uses the layout the game shipped with, and a save made after patch 1.12 has Jump and Dodge on different keys. The window shows that note when the profile is active and no key is selected.

The Tartarus V2 Pro's face is shaped differently, so every profile has a second layout made for it, and the window shows that one when a V2 Pro is plugged in. The busiest actions sit on the rest row and the rows just above and below it, so a small hand reaches them without moving. The top row and the far right column hold only menus and maps. Games that move with WASD put movement on the stick, and pressing the stick does something you want while moving, such as crouch, sprint or dodge. Editing a key on a V2 Pro changes only the V2 Pro layout.

You can make your own profiles. "+ New profile" at the bottom of the list copies whatever is selected under a new name. Custom profiles get a delete button. Shipped profiles cannot be deleted, but any edit to one can be undone with "Revert to shipped".

The first time you apply a shipped profile the app copies it into `~/.config/opentartarus/profiles/`. Edits go to that copy. A copy you have never edited follows the app: when a new version ships a better layout for that game, your copy is replaced with it. The moment you change one key, the copy becomes yours and the app stops touching it. "Revert to shipped" throws your changes away and puts it back under the app's care.

## Tartarus V2 Pro

The V2 Pro has 27 keys in rows of 6, 6, 6, 5 and 4, a thumbstick that clicks, and no scroll wheel. The window draws that layout and numbers the keys 01 to 27 along the rows.

On its own the pad sends the left of a keyboard: `` ` `` 1 2 3 4 5, Tab Q W E R T, Caps A S D F G, Shift Z X C V, and Ctrl B I Space. Each of those keys uses the same key slot as the V2 key that sends the same thing, so every shipped profile works on it unchanged. The seven keys the V2 does not have (`` ` ``, T, G, V, Ctrl, B, I) start unbound and keep sending what the pad sends.

The thumbstick shows up in Linux as a game controller. While the active profile binds no stick direction and no stick click, OpenTartarus leaves the controller alone and games see the stick as usual. Bind any of them and OpenTartarus takes the stick over: a direction presses its key once the stick is pushed 40% of the way, and lets go below 30%.

The three thumb buttons and the onboard profile and lighting keys sent nothing to Linux when they were tested, so they are not drawn.

## Game rules

OpenTartarus is an input remapper, the same kind of tool as keyd or input-remapper. It grabs the keypad and re-emits presses through a virtual keyboard and mouse named "OpenTartarus Keyboard" and "OpenTartarus Mouse". It never reads or touches a game process, and it never pretends to be a different device. Every shipped profile sends exactly one action for one press, and a test enforces that.

That matters because most publishers allow rebinding but ban "one press, many actions". Blizzard applies a one action per keypress rule to World of Warcraft, Diablo IV and Overwatch 2, and counts software key repeat as a violation. Grinding Gear Games allows one server action per input in Path of Exile. ArenaNet's Guild Wars 2 policy is one key for one function. ZeniMax bans all macros in The Elder Scrolls Online. Valve bans hardware and software input automation on its Counter-Strike 2 servers, and FACEIT and ESEA have their own rules. Minecraft servers set their own rules; Hypixel bans auto-clickers at any speed.

So the plain profiles are fine to use. Macros and hold to repeat are not, in most games, and the window says so in red the moment you turn one on. Read your game's rules before you use either in play. Turbo through this app is no different from turbo through Razer Synapse in the eyes of those rules.

There are no profiles for games that do not run on Linux. Riot's Vanguard blocks League of Legends and Valorant, EA stopped Apex Legends on Linux in October 2024, and Epic has never enabled Fortnite's anti-cheat for Proton, so none of those ship.

Nobody has documented a ban for using a uinput remapper on Linux. Nobody can promise one will never happen either. The app keeps its behaviour simple and visible so that, if a publisher ever looks, there is nothing hidden to find.

## Install

Packages are on the Releases page: https://github.com/PikkonMG/opentartarus/releases

- Debian, Ubuntu, Mint, Pop!_OS: `sudo apt install ./opentartarus_*_amd64.deb`
- Fedora, openSUSE, RHEL: `sudo dnf install ./opentartarus-*.x86_64.rpm`
- Other distros: build from source, as described below.

Both packages install the udev rule, the polkit policy and the permissions helper. After installing one, open the app and press "Fix permissions" once, then unplug and replug the keypad. There is no AppImage: it could not install those system files, so the keypad would stay locked until the user did it by hand.

A release is built by the GitHub workflow in `.github/workflows/release.yml`. It is started by hand from the Actions tab with the tag to publish, and it refuses a tag that does not match the version in `Cargo.toml`. It builds the deb with `cargo-deb` and the rpm with `cargo-generate-rpm`, then creates the release with both attached.

## How to build and run

Start the daemon. It owns the tray icon.

```
cargo run -p opentartarus-daemon
```

Then open the window:

```
cargo run -p opentartarus-ui
```

The window connects to `$XDG_RUNTIME_DIR/opentartarus/daemon.sock`. If nothing is listening there it starts the daemon itself. It looks next to its own binary, then in Cargo's `CARGO_BIN_EXE_opentartarus_daemon`, `CARGO_TARGET_DIR`, `target/debug` and `target/release`. Build the daemon crate at least once so that binary exists. Neither name is on PATH until you install them.

If the window cannot start the daemon, the red banner names the missing program, the permission error, or the socket path. That message is not about OpenRazer. Lighting messages only appear once the window is talking to the daemon.

The window has no system title bar. Drag it by its own header. The header holds the minimize, maximize and close buttons. On Wayland the corners are rounded and closing the window exits the window process. On X11 the corners are square and closing hides the window to the tray. The daemon runs on in both cases.

The daemon writes its log to `~/.local/state/opentartarus/opentartarus.log`, or under `$XDG_STATE_HOME` if that is set.

## How to test

```
cargo test --workspace
```

The check that talks to a real Tartarus V2 through OpenRazer sysfs is skipped unless you set:

```
OPENTARTARUS_HW_TEST=1
```

## First-run permissions

The keypad and `/dev/uinput` need the group `opentartarus`. The udev rule in `packaging/udev/99-opentartarus.rules` assigns that group to Tartarus V2, Pro and V2 Pro nodes and to uinput.

`opentartarus-fix-permissions` is a polkit helper. Its policy is in `packaging/polkit/com.opentartarus.fix-permissions.policy`. It creates the group, adds your user, and writes the udev rule if it is missing. The "Fix permissions" entry in the window's menu runs that helper through `pkexec` at `/usr/libexec/opentartarus-fix-permissions`.

After a successful fix, unplug the Tartarus, wait a second, and plug it back in. If the app still cannot open the device, sign out and back in so the new group applies.

Login start files are in `packaging/`: a systemd user unit at `packaging/systemd/opentartarus-daemon.service`, and `packaging/autostart/opentartarus-daemon.desktop`. The window launcher is `packaging/desktop/opentartarus.desktop`.

### Icon

The tray icon and the window icon are built into the binaries. No install step is needed for them.

To give the desktop menu the same icon, copy the SVG into your own icon directory and refresh the cache:

    mkdir -p ~/.local/share/icons/hicolor/scalable/apps
    cp packaging/icons/opentartarus.svg ~/.local/share/icons/hicolor/scalable/apps/
    gtk-update-icon-cache -f -t ~/.local/share/icons/hicolor

The last line is optional. Most desktops pick the file up on the next login.

## Layout

- `crates/`
  - `opentartarus-core`: profile format, remap engine, IPC types, and the shipped layouts in `profiles/`
  - `opentartarus-daemon`: tray process, device grab, remap playback, lighting
  - `opentartarus-ui`: the iced window
  - `opentartarus-fix-permissions`: polkit helper that installs the udev rule
- `packaging/`: udev rule, systemd user unit, polkit policy, desktop entry, login autostart, icons, the deb postinst and the release notes

## If you searched for this

People look for this tool under many names. Every one of these describes OpenTartarus:

- Razer Synapse for Linux, Razer Synapse alternative, Synapse replacement
- Razer Tartarus V2 Linux, Tartarus Pro Linux, Tartarus V2 Pro Linux, Razer keypad Linux
- Razer keypad remapper, keypad rebind, key remapping, macro editor
- OpenRazer GUI, OpenRazer keypad support, OpenRazer profile manager
- Razer RGB lighting on Linux, Chroma lighting without Windows
- Per-game key profiles for Counter-Strike 2, Deadlock, Diablo 4, Elden Ring, Final Fantasy XIV,
  Guild Wars 2, Minecraft, Overwatch 2, Path of Exile and World of Warcraft

Not supported: the Razer Orbweaver, the Razer Nostromo and the first Tartarus. Only the
Tartarus V2 (`1532:022b`), the Tartarus Pro (`1532:0244`) and the Tartarus V2 Pro (`1532:02eb`) are handled.

## License

MIT.
