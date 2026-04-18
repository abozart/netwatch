//! Global hotkey manager wrapping the `global-hotkey` crate. Owns the OS-level
//! registrations and maps the crate's numeric IDs back to our `FeatureToggle`s.

use anyhow::Result;
use global_hotkey::{hotkey::HotKey, GlobalHotKeyManager};
use std::collections::HashMap;
use std::str::FromStr;

use crate::features::FeatureToggle;

pub struct HotkeyRegistry {
    manager: GlobalHotKeyManager,
    /// OS-assigned hotkey id → which feature it toggles.
    id_to_feature: HashMap<u32, FeatureToggle>,
    /// Feature → currently registered combo string (for round-trip display/save).
    bindings: HashMap<FeatureToggle, String>,
}

impl HotkeyRegistry {
    pub fn new() -> Result<Self> {
        Ok(Self {
            manager: GlobalHotKeyManager::new()?,
            id_to_feature: HashMap::new(),
            bindings: HashMap::new(),
        })
    }

    /// Remove any current registration for this feature. Silently succeeds if
    /// there was none.
    pub fn unbind(&mut self, feat: FeatureToggle) {
        if let Some(existing) = self.bindings.remove(&feat) {
            if let Ok(hk) = HotKey::from_str(&existing) {
                let _ = self.manager.unregister(hk);
                self.id_to_feature.remove(&hk.id());
            }
        }
    }

    /// Register `combo` (e.g. `"Ctrl+Alt+T"`) for `feat`, replacing any prior
    /// binding. Returns Err if the combo can't be parsed or the OS refuses it
    /// (e.g. another app owns it).
    pub fn bind(&mut self, feat: FeatureToggle, combo: &str) -> Result<()> {
        self.unbind(feat);
        let hk = HotKey::from_str(combo)?;
        self.manager.register(hk)?;
        self.id_to_feature.insert(hk.id(), feat);
        self.bindings.insert(feat, combo.to_string());
        Ok(())
    }

    pub fn binding_for(&self, feat: FeatureToggle) -> Option<&str> {
        self.bindings.get(&feat).map(String::as_str)
    }

    /// Which feature (if any) should be toggled in response to the given event id?
    pub fn feature_for_event(&self, id: u32) -> Option<FeatureToggle> {
        self.id_to_feature.get(&id).copied()
    }

    pub fn all_bindings(&self) -> &HashMap<FeatureToggle, String> {
        &self.bindings
    }
}
