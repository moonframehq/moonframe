//! What [`crate::export!`] expands to on wasm32: the component's exports,
//! turning the WIT world's JSON text into [`Provider`] calls on the one
//! instance this component keeps.

use {crate::Provider, serde_json::Value, std::cell::RefCell};

pub fn start<P: Provider>(
    state: &'static std::thread::LocalKey<RefCell<Option<P>>>,
    settings: &str,
) -> Result<(), String> {
    let settings: Value = serde_json::from_str(settings).map_err(|e| e.to_string())?;
    let provider = P::start(settings)?;
    state.set(Some(provider));
    Ok(())
}

pub fn tick<P: Provider>(
    state: &'static std::thread::LocalKey<RefCell<Option<P>>>,
) -> Option<String> {
    let value = state.with_borrow_mut(|provider| provider.as_mut()?.tick())?;
    Some(value.to_string())
}

pub fn receive<P: Provider>(
    state: &'static std::thread::LocalKey<RefCell<Option<P>>>,
    message: &str,
) -> Option<String> {
    let message: Value = match serde_json::from_str(message) {
        Ok(message) => message,
        Err(problem) => {
            crate::log(&format!("message is not JSON: {problem}"));
            return None;
        },
    };
    let value = state.with_borrow_mut(|provider| provider.as_mut()?.receive(message))?;
    Some(value.to_string())
}

pub fn save<P: Provider>(
    state: &'static std::thread::LocalKey<RefCell<Option<P>>>,
) -> Option<String> {
    state.with_borrow_mut(|provider| provider.as_mut()?.save())
}

pub fn restore<P: Provider>(
    state: &'static std::thread::LocalKey<RefCell<Option<P>>>,
    saved: &str,
) -> Result<(), String> {
    state.with_borrow_mut(|provider| match provider {
        Some(provider) => provider.restore(saved),
        None => Err("restore before start".to_owned()),
    })
}

/// The stateful world's two extra exports, or none.
#[cfg(feature = "stateful")]
#[doc(hidden)]
#[macro_export]
macro_rules! __stateful_exports {
    ($provider:ty) => {
        fn save() -> ::core::option::Option<::std::string::String> {
            $crate::wasm::save::<$provider>(&__MOONFRAME_PROVIDER)
        }

        fn restore(
            state: ::std::string::String,
        ) -> ::core::result::Result<(), ::std::string::String> {
            $crate::wasm::restore::<$provider>(&__MOONFRAME_PROVIDER, &state)
        }
    };
}

#[cfg(not(feature = "stateful"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __stateful_exports {
    ($provider:ty) => {};
}

/// Makes `$provider`, a [`Provider`], what this crate's component exports.
/// Compiled natively it expands to nothing: the app registers the type.
#[macro_export]
macro_rules! export {
    ($provider:ty) => {
        ::std::thread_local! {
            static __MOONFRAME_PROVIDER: ::core::cell::RefCell<::core::option::Option<$provider>> =
                const { ::core::cell::RefCell::new(::core::option::Option::None) };
        }

        struct __MoonframeExports;

        impl $crate::bindings::Guest for __MoonframeExports {
            fn start(settings: ::std::string::String) -> ::core::result::Result<(), ::std::string::String> {
                $crate::wasm::start::<$provider>(&__MOONFRAME_PROVIDER, &settings)
            }

            fn tick() -> ::core::option::Option<::std::string::String> {
                $crate::wasm::tick::<$provider>(&__MOONFRAME_PROVIDER)
            }

            fn receive(message: ::std::string::String) -> ::core::option::Option<::std::string::String> {
                $crate::wasm::receive::<$provider>(&__MOONFRAME_PROVIDER, &message)
            }

            $crate::__stateful_exports!($provider);
        }

        $crate::bindings::export!(__MoonframeExports with_types_in $crate::bindings);
    };
}
