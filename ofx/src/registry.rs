use action::*;
use ofx_sys::*;
use plugin::*;
use result::*;
use std::any::Any;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;
use types::*;

#[derive(Default)]
pub struct Registry {
    plugins: Vec<PluginDescriptor>,
    plugin_modules: HashMap<String, usize>,
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

        self.plugins.push(plugin);
        plugin_index
    }

    pub fn count(&self) -> Int {
        self.plugins.len() as Int
    }

    pub fn get_plugin_mut(&mut self, index: usize) -> &mut PluginDescriptor {
        &mut self.plugins[index as usize]
    }

    pub fn get_plugin(&self, index: usize) -> &PluginDescriptor {
        &self.plugins[index as usize]
    }

    pub fn ofx_plugin_ptr(&self, index: Int) -> *const OfxPlugin {
        self.plugins[index as usize].ofx_plugin() as *const OfxPlugin
    }

    pub fn dispatch(&mut self, plugin_module: &str, message: RawMessage) -> Result<Int> {
        info!("{}:{:?}", plugin_module, message);
        let found_plugin = self.plugin_modules.get(plugin_module).cloned();
        if let Some(plugin_index) = found_plugin {
            let plugin = self.get_plugin_mut(plugin_index);
            plugin.dispatch(message)
        } else {
            Err(Error::PluginNotFound)
        }
    }
}

struct RegistryState {
    lock: Mutex<()>,
    registry: UnsafeCell<Option<Registry>>,
}

unsafe impl Sync for RegistryState {}

impl RegistryState {
    const fn new() -> Self {
        Self {
            lock: Mutex::new(()),
            registry: UnsafeCell::new(None),
        }
    }

    fn with_registry_mut<R>(&self, f: impl FnOnce(&mut Registry) -> R) -> R {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let registry = unsafe { &mut *self.registry.get() };
        let registry = registry.as_mut().expect("registry not initialized");
        f(registry)
    }

    fn with_registry<R>(&self, f: impl FnOnce(&Registry) -> R) -> R {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let registry = unsafe { &*self.registry.get() };
        let registry = registry.as_ref().expect("registry not initialized");
        f(registry)
    }

    fn init<F>(&self, init_function: F)
    where
        F: Fn(&mut Registry),
    {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let slot = unsafe { &mut *self.registry.get() };
        if slot.is_none() {
            let mut registry = Registry::new();
            init_function(&mut registry);
            for plugin in &registry.plugins {
                info!("Registered plugin {}", plugin);
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
        with_registry_mut(|registry| {
            registry
                .dispatch(
                    plugin_module,
                    RawMessage::MainEntry {
                        action,
                        handle,
                        in_args,
                        out_args,
                    },
                )
                .ok()
                .unwrap_or(-1)
        })
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
