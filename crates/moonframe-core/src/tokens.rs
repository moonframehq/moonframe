//! Token enums: the closed sets of names modules write, such as tones and
//! spacing steps. Each one reads itself from a Lua string.

/// Declares a token enum, the names a module writes for it, and its
/// [`Read`](crate::read::Read) impl.
macro_rules! tokens {
    ($(#[$meta:meta])* $name:ident {
        $($(#[$doc:meta])* $variant:ident = $text:literal),+ $(,)?
    }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name {
            $($(#[$doc])* $variant),+
        }

        impl $name {
            pub const NAMES: &[&str] = &[$($text),+];

            /// What a script writes for this token.
            #[allow(dead_code, reason = "not every token is written back out")]
            pub fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $text,)+
                }
            }

            pub(crate) fn parse(text: &str) -> Option<Self> {
                match text {
                    $($text => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }

        impl $crate::read::Read for $name {
            fn read(value: &mlua::Value, cx: &$crate::read::Cx) -> Result<Self, $crate::Error> {
                let names = Self::NAMES.join(", ");
                let mlua::Value::String(name) = value else {
                    return Err(cx.fail(format!("expected one of {names}")));
                };
                let name = name.to_string_lossy();
                Self::parse(&name)
                    .ok_or_else(|| cx.fail(format!("unknown `{name}`; expected one of {names}")))
            }
        }
    };
}
