//! "Press keys…" modal shown while the user is rebinding a feature's
//! global hotkey. Captures the first non-modifier key press with its
//! current modifier set and sends the combo to the `HotkeyRegistry`.

use eframe::egui;

use crate::app::NetWatchApp;
use crate::features::FeatureToggle;

impl NetWatchApp {
    pub(crate) fn draw_hotkey_recording(&mut self, ctx: &egui::Context) {
        let Some(key) = self.state.read().recording_hotkey else { return };
        let Some(feat) = FeatureToggle::ALL.iter().copied().find(|f| f.settings_key() == key)
        else {
            self.state.write().recording_hotkey = None;
            return;
        };

        // ESC cancels.
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.state.write().recording_hotkey = None;
            return;
        }

        // Grab the first non-modifier key pressed this frame, with current modifiers.
        let captured: Option<String> = ctx.input(|i| {
            for ev in &i.events {
                if let egui::Event::Key { key, pressed: true, modifiers, .. } = ev {
                    if let Some(s) = combo_string(*key, *modifiers) {
                        return Some(s);
                    }
                }
            }
            None
        });

        if let Some(combo) = captured {
            if let Some(reg) = self.hotkeys.as_mut() {
                if let Err(e) = reg.bind(feat, &combo) {
                    self.state.write().set_status(
                        false,
                        format!(
                            "{combo} is taken (probably by another app). Try a different combo for {}. ({e})",
                            feat.label()
                        ),
                    );
                } else {
                    self.state
                        .write()
                        .set_status(true, format!("Bound {combo} -> {}", feat.label()));
                }
            }
            self.state.write().recording_hotkey = None;
            return;
        }

        egui::Window::new("Press keys…")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(format!("Recording hotkey for: {}", feat.label()));
                ui.label("Press a combination (with at least one modifier).");
                ui.label("Esc to cancel.");
            });
    }
}

/// Convert an egui key + modifier set into a `global-hotkey`-compatible combo
/// string (e.g. `"Ctrl+Alt+T"`). Returns None for bare-letter presses (we
/// require a modifier so users don't accidentally grab single-letter combos)
/// and for modifier-only events.
fn combo_string(key: egui::Key, mods: egui::Modifiers) -> Option<String> {
    let has_modifier = mods.ctrl || mods.alt || mods.shift || mods.command || mods.mac_cmd;
    if !has_modifier {
        return None;
    }
    let key_name = key_to_code_name(key)?;
    let mut parts: Vec<&str> = Vec::new();
    if mods.ctrl || mods.command {
        parts.push("Ctrl");
    }
    if mods.alt {
        parts.push("Alt");
    }
    if mods.shift {
        parts.push("Shift");
    }
    if mods.mac_cmd {
        parts.push("Meta");
    }
    let mut out = parts.join("+");
    if !out.is_empty() {
        out.push('+');
    }
    out.push_str(&key_name);
    Some(out)
}

fn key_to_code_name(key: egui::Key) -> Option<String> {
    use egui::Key::*;
    let s: &str = match key {
        A => "KeyA", B => "KeyB", C => "KeyC", D => "KeyD", E => "KeyE",
        F => "KeyF", G => "KeyG", H => "KeyH", I => "KeyI", J => "KeyJ",
        K => "KeyK", L => "KeyL", M => "KeyM", N => "KeyN", O => "KeyO",
        P => "KeyP", Q => "KeyQ", R => "KeyR", S => "KeyS", T => "KeyT",
        U => "KeyU", V => "KeyV", W => "KeyW", X => "KeyX", Y => "KeyY",
        Z => "KeyZ",
        Num0 => "Digit0", Num1 => "Digit1", Num2 => "Digit2", Num3 => "Digit3",
        Num4 => "Digit4", Num5 => "Digit5", Num6 => "Digit6", Num7 => "Digit7",
        Num8 => "Digit8", Num9 => "Digit9",
        F1 => "F1", F2 => "F2", F3 => "F3", F4 => "F4", F5 => "F5", F6 => "F6",
        F7 => "F7", F8 => "F8", F9 => "F9", F10 => "F10", F11 => "F11", F12 => "F12",
        Space => "Space",
        ArrowLeft => "ArrowLeft", ArrowRight => "ArrowRight",
        ArrowUp => "ArrowUp", ArrowDown => "ArrowDown",
        _ => return None,
    };
    Some(s.to_string())
}
