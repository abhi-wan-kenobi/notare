use tauri_plugin_store2::StorePluginExt;

pub trait FlagsPluginExt<R: tauri::Runtime> {
    fn flags_store(&self) -> tauri_plugin_store2::ScopedStore<R, crate::StoreKey>;
    fn is_enabled(&self, flag: crate::StoreKey) -> Result<bool, crate::Error>;
    fn enable(&self, flag: crate::StoreKey) -> Result<(), crate::Error>;
    fn disable(&self, flag: crate::StoreKey) -> Result<(), crate::Error>;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> FlagsPluginExt<R> for T {
    fn flags_store(&self) -> tauri_plugin_store2::ScopedStore<R, crate::StoreKey> {
        self.scoped_store(crate::PLUGIN_NAME).unwrap()
    }

    fn is_enabled(&self, flag: crate::StoreKey) -> Result<bool, crate::Error> {
        let v = self.flags_store().get(flag)?;
        Ok(v.unwrap_or(false))
    }

    fn enable(&self, flag: crate::StoreKey) -> Result<(), crate::Error> {
        self.flags_store().set(flag, true)?;
        Ok(())
    }

    fn disable(&self, flag: crate::StoreKey) -> Result<(), crate::Error> {
        self.flags_store().set(flag, false)?;
        Ok(())
    }
}
