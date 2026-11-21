//! Keystrokes to the engine's shortcut map.

use filmcraft_engine::shortcuts::{Chord, Mods, Platform};
use gpui::Keystroke;

/// The engine chord a keystroke stands for, if the engine's key table has its key. `Cmd` is the
/// platform's primary modifier (Control on Linux and Windows), as in the engine's own notation.
pub fn chord_of(k: &Keystroke, platform: Platform) -> Option<Chord> {
    let key = Chord::parse_for(&k.key, platform).ok()?.key;
    let m = &k.modifiers;
    let mods = Mods { cmd: if platform.is_mac() { m.platform } else { m.control }, ctrl: platform.is_mac() && m.control, alt: m.alt, shift: m.shift };
    Some(Chord { mods, key })
}
