use action::*;
use ofx_sys::*;
use plugin::*;
use result::*;
use std::any::Any;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::ffi::CStr;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::thread::{self, ThreadId};
use types::*;

#[derive(Default)]
pub struct Registry {
    plugins: Vec<Arc<PluginSlot>>,
    plugin_modules: HashMap<String, usize>,
}

pub(crate) struct PluginSlot {
    plugin: UnsafeCell<PluginDescriptor>,
    dispatch_gate: DispatchGate,
}

// `Execute::execute` is a public `&mut self` API, but OFX hosts may synchronously
// reenter `mainEntry` from the same thread. The gate serializes cross-thread
// dispatch while allowing that same-thread recursion.
unsafe impl Sync for PluginSlot {}
unsafe impl Send for PluginSlot {}

impl PluginSlot {
    fn new(plugin: PluginDescriptor) -> Self {
        Self {
            plugin: UnsafeCell::new(plugin),
            dispatch_gate: DispatchGate::new(),
        }
    }

    fn get(&self) -> &PluginDescriptor {
        unsafe { &*self.plugin.get() }
    }

    fn get_mut(&self) -> &mut PluginDescriptor {
        unsafe { &mut *self.plugin.get() }
    }

    fn dispatch(&self, message: RawMessage) -> Result<Int> {
        let _guard = self.dispatch_gate.enter();
        self.get_mut().dispatch(message)
    }
}

struct DispatchGate {
    state: Mutex<DispatchGateState>,
    available: Condvar,
}

struct DispatchGateState {
    owner: Option<ThreadId>,
    depth: usize,
}

struct DispatchGuard<'a> {
    gate: &'a DispatchGate,
}

impl DispatchGate {
    fn new() -> Self {
        Self {
            state: Mutex::new(DispatchGateState {
                owner: None,
                depth: 0,
            }),
            available: Condvar::new(),
        }
    }

    fn enter(&self) -> DispatchGuard<'_> {
        let current_thread = thread::current().id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        while state.owner.map_or(false, |owner| owner != current_thread) {
            state = self
                .available
                .wait(state)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }

        state.owner = Some(current_thread);
        state.depth += 1;

        DispatchGuard { gate: self }
    }
}

impl Drop for DispatchGuard<'_> {
    fn drop(&mut self) {
        let mut state = self
            .gate
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        state.depth -= 1;
        if state.depth == 0 {
            state.owner = None;
            self.gate.available.notify_one();
        }
    }
}

impl Registry {
    pub fn new() -> Registry {
        Self::default()
    }

    pub fn add(
        &mut self,
        module_name: &'static str,
        name: &'static str,
        api_version: ApiVersion,
        plugin_version: PluginVersion,
        instance: Box<dyn Execute>,
        set_host: SetHost,
        main_entry: MainEntry,
    ) -> usize {
        let plugin_index = self.plugins.len();

        self.plugin_modules
            .insert(module_name.to_owned(), plugin_index as usize);

        let plugin = PluginDescriptor::new(
            plugin_index,
            module_name,
            name,
            api_version,
            plugin_version,
            instance,
            set_host,
            main_entry,
        );

        self.plugins.push(Arc::new(PluginSlot::new(plugin)));
        plugin_index
    }

    pub fn count(&self) -> Int {
        self.plugins.len() as Int
    }

    pub fn get_plugin_mut(&mut self, index: usize) -> &mut PluginDescriptor {
        self.plugins[index as usize].get_mut()
    }

    pub fn get_plugin(&self, index: usize) -> &PluginDescriptor {
        self.plugins[index as usize].get()
    }

    pub fn ofx_plugin_ptr(&self, index: Int) -> *const OfxPlugin {
        self.plugins[index as usize].get().ofx_plugin() as *const OfxPlugin
    }

    pub fn dispatch(&mut self, plugin_module: &str, message: RawMessage) -> Result<Int> {
        info!("{}:{:?}", plugin_module, message);
        self.plugin_for_module(plugin_module)
            .ok_or(Error::PluginNotFound)?
            .dispatch(message)
    }

    fn plugin_for_module(&self, plugin_module: &str) -> Option<Arc<PluginSlot>> {
        self.plugin_modules
            .get(plugin_module)
            .and_then(|plugin_index| self.plugins.get(*plugin_index))
            .cloned()
    }
}

struct RegistryState {
    registry: RwLock<Option<Registry>>,
}

impl RegistryState {
    const fn new() -> Self {
        Self {
            registry: RwLock::new(None),
        }
    }

    fn with_registry_mut<R>(&self, f: impl FnOnce(&mut Registry) -> R) -> R {
        let mut registry = self
            .registry
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let registry = registry.as_mut().expect("registry not initialized");
        f(registry)
    }

    fn with_registry<R>(&self, f: impl FnOnce(&Registry) -> R) -> R {
        let registry = self
            .registry
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let registry = registry.as_ref().expect("registry not initialized");
        f(registry)
    }

    fn with_plugin_for_main_entry<R>(
        &self,
        plugin_module: &str,
        action: CharPtr,
        f: impl FnOnce(&PluginSlot) -> R,
    ) -> Result<R> {
        if main_entry_requires_registry_mutation_lock(action) {
            self.with_registry_mut(|registry| {
                registry
                    .plugin_for_module(plugin_module)
                    .ok_or(Error::PluginNotFound)
                    .map(|plugin| f(&plugin))
            })
        } else {
            let plugin = self.with_registry(|registry| {
                registry
                    .plugin_for_module(plugin_module)
                    .ok_or(Error::PluginNotFound)
            })?;
            Ok(f(&plugin))
        }
    }

    fn dispatch_main_entry(
        &self,
        plugin_module: &str,
        action: CharPtr,
        handle: VoidPtr,
        in_args: OfxPropertySetHandle,
        out_args: OfxPropertySetHandle,
    ) -> Result<Int> {
        self.with_plugin_for_main_entry(plugin_module, action, |plugin| {
            plugin.dispatch(RawMessage::MainEntry {
                action,
                handle,
                in_args,
                out_args,
            })
        })?
    }

    fn init<F>(&self, init_function: F)
    where
        F: Fn(&mut Registry),
    {
        let mut slot = self
            .registry
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if slot.is_none() {
            let mut registry = Registry::new();
            init_function(&mut registry);
            for plugin in &registry.plugins {
                info!("Registered plugin {}", plugin.get());
            }
            *slot = Some(registry);
        }
    }
}

static GLOBAL_REGISTRY: RegistryState = RegistryState::new();

fn panic_payload_to_string(payload: &(dyn Any + Send)) -> String {
    if let Some(msg) = payload.downcast_ref::<&'static str>() {
        (*msg).to_owned()
    } else if let Some(msg) = payload.downcast_ref::<String>() {
        msg.clone()
    } else {
        "<non-string panic payload>".to_owned()
    }
}

fn main_entry_requires_registry_mutation_lock(action: CharPtr) -> bool {
    if action.is_null() {
        return false;
    }

    let action = unsafe { CStr::from_ptr(action) }.to_bytes_with_nul();
    action == kOfxActionLoad || action == kOfxActionUnload || action == kOfxActionDescribe
}

pub fn with_registry<R>(f: impl FnOnce(&Registry) -> R) -> R {
    GLOBAL_REGISTRY.with_registry(f)
}

fn with_registry_mut<R>(f: impl FnOnce(&mut Registry) -> R) -> R {
    GLOBAL_REGISTRY.with_registry_mut(f)
}

pub unsafe fn set_host_for_plugin(plugin_module: &str, host: *mut OfxHost) {
    let result = catch_unwind(AssertUnwindSafe(|| {
        with_registry_mut(|registry| {
            registry
                .dispatch(plugin_module, RawMessage::SetHost { host: *host })
                .ok();
        });
    }));

    if let Err(payload) = result {
        error!(
            "panic while setting host for plugin {}: {}",
            plugin_module,
            panic_payload_to_string(payload.as_ref()),
        );
    }
}

pub fn main_entry_for_plugin(
    plugin_module: &str,
    action: CharPtr,
    handle: VoidPtr,
    in_args: OfxPropertySetHandle,
    out_args: OfxPropertySetHandle,
) -> Int {
    match catch_unwind(AssertUnwindSafe(|| {
        GLOBAL_REGISTRY
            .dispatch_main_entry(plugin_module, action, handle, in_args, out_args)
            .ok()
            .unwrap_or(-1)
    })) {
        Ok(status) => status,
        Err(payload) => {
            error!(
                "panic in OFX main entry for plugin {}: {}",
                plugin_module,
                panic_payload_to_string(payload.as_ref()),
            );
            -1
        }
    }
}

pub fn init_registry<F>(init_function: F)
where
    F: Fn(&mut Registry),
{
    GLOBAL_REGISTRY.init(init_function);
}

#[macro_export]
macro_rules! plugin_module {
    ($name:expr, $api_version:expr, $plugin_version:expr, $factory:expr) => {
        pub fn name() -> &'static str {
            $name
        }

        pub fn module_name() -> &'static str {
            module_path!()
        }

        pub fn new_instance() -> Box<dyn Execute> {
            Box::new($factory())
        }

        pub fn api_version() -> ApiVersion {
            $api_version
        }

        pub fn plugin_version() -> PluginVersion {
            $plugin_version
        }

        pub extern "C" fn set_host(host: *mut ofx::OfxHost) {
            unsafe { ofx::set_host_for_plugin(module_name(), host) }
        }

        pub extern "C" fn main_entry(
            action: ofx::CharPtr,
            handle: ofx::VoidPtr,
            in_args: ofx::OfxPropertySetHandle,
            out_args: ofx::OfxPropertySetHandle,
        ) -> super::Int {
            ofx::main_entry_for_plugin(module_name(), action, handle, in_args, out_args)
        }
    };
}

#[macro_export]
macro_rules! register_plugin {
    ($registry:ident, $module:ident) => {
        $registry.add(
            $module::module_name(),
            $module::name(),
            $module::api_version(),
            $module::plugin_version(),
            $module::new_instance(),
            $module::set_host,
            $module::main_entry,
        );
    };
}

#[macro_export]
macro_rules! build_plugin_registry {
    ($init_callback:ident) => {
        fn init() {
            init_registry($init_callback);
        }

        #[no_mangle]
        pub extern "C" fn OfxGetNumberOfPlugins() -> Int {
            init();
            ofx::with_registry(|registry| registry.count())
        }

        #[no_mangle]
        pub extern "C" fn OfxGetPlugin(nth: Int) -> *const OfxPlugin {
            init();
            ofx::with_registry(|registry| registry.ofx_plugin_ptr(nth))
        }

        pub fn show_plugins() -> Vec<String> {
            let n = OfxGetNumberOfPlugins();
            for i in 0..n {
                OfxGetPlugin(i);
            }
            ofx::with_registry(|registry| {
                (0..n)
                    .map(|i| {
                        let plugin = registry.get_plugin(i as usize);
                        format!("{}", plugin)
                    })
                    .collect()
            })
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[derive(Default)]
    struct NoopPlugin;

    impl Execute for NoopPlugin {}

    extern "C" fn noop_set_host(_host: *mut OfxHost) {}

    extern "C" fn noop_main_entry(
        _action: CharPtr,
        _handle: VoidPtr,
        _in_args: OfxPropertySetHandle,
        _out_args: OfxPropertySetHandle,
    ) -> Int {
        eOfxStatus_OK
    }

    fn test_registry_state() -> RegistryState {
        let state = RegistryState::new();
        state.init(|registry| {
            registry.add(
                "test_module",
                "net.ofx-rs.test",
                ApiVersion(1),
                PluginVersion(1, 0),
                Box::new(NoopPlugin),
                noop_set_host,
                noop_main_entry,
            );
        });
        state
    }

    fn action_ptr(action: &'static [u8]) -> CharPtr {
        unsafe { CStr::from_bytes_with_nul_unchecked(action).as_ptr() }
    }

    #[test]
    fn dispatch_gate_allows_same_thread_reentry() {
        let gate = DispatchGate::new();
        let _outer = gate.enter();
        let _inner = gate.enter();
    }

    #[test]
    fn normal_main_entry_lookup_releases_registry_lock_before_dispatch() {
        let state = test_registry_state();
        let action = action_ptr(kOfxActionInstanceChanged);

        let result = state.with_plugin_for_main_entry("test_module", action, |_plugin| {
            assert!(
                state.registry.try_write().is_ok(),
                "normal actions must dispatch outside the global registry lock"
            );
            eOfxStatus_OK
        });

        assert_eq!(result.unwrap(), eOfxStatus_OK);
    }

    #[test]
    fn lifecycle_main_entry_lookup_keeps_registry_mutation_lock() {
        let state = test_registry_state();
        let action = action_ptr(kOfxActionDescribe);

        let result = state.with_plugin_for_main_entry("test_module", action, |_plugin| {
            assert!(
                state.registry.try_write().is_err(),
                "Describe must stay under the registry mutation lock"
            );
            eOfxStatus_OK
        });

        assert_eq!(result.unwrap(), eOfxStatus_OK);
    }
}
