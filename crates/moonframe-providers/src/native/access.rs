//! The host calls a native provider may make, gated by the `capabilities`
//! of its provider.toml as a wasm provider's are: what lets a portable
//! provider behave the same compiled in.

use {
    crate::{Caller, Capabilities, Capability, Context},
    std::sync::{Arc, atomic::AtomicBool},
};

pub(super) struct Access {
    declared: Vec<Capability>,
    host: Capabilities,
    caller: Caller,
    busy: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    #[cfg(feature = "system")]
    system: std::sync::Mutex<Option<crate::system::SystemLoad>>,
}

impl Access {
    pub(super) fn of(context: &Context) -> Self {
        Self {
            declared: context.manifest.capabilities.clone(),
            host: context.capabilities.clone(),
            caller: context.caller.clone(),
            busy: Arc::default(),
            stop: Arc::clone(&context.stop),
            #[cfg(feature = "system")]
            system: std::sync::Mutex::default(),
        }
    }

    pub(super) fn host_call(
        &self,
        capability: &str,
        method: &str,
        args: &str,
    ) -> Result<String, String> {
        if self.stop.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("provider is stopping".into());
        }
        self.host
            .call(&self.caller, &self.busy, capability, method, args)
    }

    pub(super) fn system_load(&self) -> Result<serde_json::Value, String> {
        if !self.declared.contains(&Capability::System) {
            return Err("needs `capabilities = [\"system\"]` in provider.toml".to_owned());
        }
        #[cfg(feature = "system")]
        {
            let mut system = self
                .system
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let system = match &mut *system {
                Some(system) => system,
                empty => empty
                    .insert(crate::system::SystemLoad::new().map_err(|error| error.to_string())?),
            };
            Ok(system.sample())
        }
        #[cfg(not(feature = "system"))]
        {
            Err("this build leaves out the `system` capability; enable its feature".to_owned())
        }
    }
}
