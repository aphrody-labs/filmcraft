# GPUI frontend: parity with the egui frontend

> **Status:** in progress. `crates/ui-gpui` is a native GPUI window over the same engine commands;
> `crates/ui-egui` stays the default frontend and is not removed. Start the GPUI one with
> `FILMCRAFT_UI=gpui cargo run -p filmcraft --features gpui` (Linux only).

Every state below comes from what was run: "native, tested" names the test in
`crates/ui-gpui/tests/it/`; "not done" has no GPUI code at all.

| Panel / feature | egui | GPUI state |
|---|---|---|
| Program monitor: frame at the playhead, timecode, step and go-to-start/end | yes | native, tested by `scrubbing_the_ruler_moves_the_playhead_and_the_viewer_frame` (frame follows the playhead); the transport buttons dispatch `playhead.*` and have no test of their own |
| Timeline: tracks, clips, ruler, playhead | yes | native, tested by `opening_a_project_shows_its_sequence_and_clips`, `the_timeline_lists_every_track_of_the_sequence` |
| Timeline: click selects a clip (`timeline.select`) | yes | native, tested by `the_selection_tool_selects_the_clicked_clip` |
| Timeline: Razor cut (`timeline.razor`) | yes | native, tested by `a_razor_click_on_a_clip_dispatches_the_razor_command` |
| Timeline: ruler scrub (`playhead.set`) | yes | native, tested by `scrubbing_the_ruler_moves_the_playhead_and_the_viewer_frame` |
| Timeline: zoom and scroll (buttons, Ctrl+wheel, wheel) | yes | native; the pixel mapping is tested (`geometry::*`), the gestures are not |
| Keyboard: the engine's shortcut map, Timeline bindings first | yes | native, tested by `the_engines_shortcut_map_drives_the_window` (`Right`, and `C` for the frontend-owned razor tool) |
| File ▸ Open (system dialog, `file.open`), Demo, Save, Undo, Redo | yes | native; `file.open` tested by `opening_a_project_shows_its_sequence_and_clips`; the dialog, Demo, Save, Undo and Redo buttons are not tested |
| Accessibility: roles, names, tab stops on buttons, clips, ruler, monitor | partial | native; names and roles are set, no test reads the accessibility tree |
| Theme tokens | yes | shared: `crates/theme` is the one source for both frontends |
| Playback (play/pause, J K L, audio output) | yes | not done |
| Menu bar, command palette, context menus | yes | not done |
| Project panel / media bin, Media Browser, Source monitor | yes | not done |
| Effect Controls, Effects, Lumetri, masks, keyframes | yes | not done |
| Audio meters, Audio Track/Clip Mixer, voice-over | yes | not done |
| Video scopes | yes | not done |
| Export mode, export dialog, queue | yes | not done |
| Text, Graphics, Captions, Transcript, TTS | yes | not done |
| Markers, History, Events, Notes, Metadata, Multicam, Remix | yes | not done |
| Settings, shortcuts editor, About, workspaces, docking | yes | not done |
| Drag and drop, trim modes, snapping, track targeting, track headers (lock, mute, solo) | yes | not done |
| Control channel (`--control`), auto-save, native menus, update check | yes | not done |
| Translations (`tl!`) | yes | not done: GPUI strings are English literals |
| macOS and Windows | yes | not done: no GPUI platform crate is wired (see below) |

## Why egui is not hosted inside GPUI

The brief's first route was `crates/egui_gpui` of aphrody-labs/aphrody. It cannot host this egui:

- it depends on `aphrody_ui`, which is GPL-3.0-or-later (`crates/aphrody_ui/Cargo.toml`); AGENTS.md
  forbids GPL, and the licence list has no entry for it;
- it builds against `aphrody-labs/egui` tag `v0.37.0-aphrody.20261120`, a different crate from the
  pinned `storytold/egui` rev `6d518fc` (the GitHub compare says diverged: 35 ahead, 90 behind, 175
  files), so `egui::Context` and every type of `ui-egui` would be two incompatible types;
- it builds against the `aphrody-labs/wgpu` fork `v30.0.0-aphrody.20261120`, while this workspace's
  `Cargo.lock` has wgpu 30.0.1 from crates.io.

## Why `gpui_platform` is not used

`gpui_platform` pulls `gpui_ios` -> `gpui_apple` -> `cbindgen` -> `toml_datetime ^0.6.5` for every
target, and `muda` -> `gtk` -> `toml_edit 0.19.15` pins `toml_datetime =0.6.3`: Cargo cannot resolve
both. The crate uses `gpui_linux` directly, so there is no macOS or Windows GPUI build yet.
