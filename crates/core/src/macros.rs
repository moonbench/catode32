//! Crate-wide declarative macros for the kinds of enum→string boilerplate
//! that proliferated across the port. Each macro is narrow and does one
//! thing; see the individual doc comments for which to reach for.

/// Generate `impl $Enum { pub fn $method(self) -> &'static str }` from a
/// `Variant => "string"` table. The body is just one `match self` arm per
/// variant — use this any time the only thing that varies is the returned
/// constant string.
///
/// Works equally for stable English identifiers (save keys, log tags),
/// snake_case ASCII names, and translated `t!(...)` labels — the table
/// values are arbitrary `&'static str` expressions.
///
/// ```ignore
/// enum_str_method! {
///     BehaviorId::name;
///     Idle => "idle",
///     Sleeping => "sleeping",
///     // ...
/// }
/// ```
#[macro_export]
macro_rules! enum_str_method {
    (
        $(#[$meta:meta])*
        $Enum:ident :: $method:ident;
        $($Variant:ident => $value:expr),+ $(,)?
    ) => {
        impl $Enum {
            $(#[$meta])*
            pub fn $method(self) -> &'static str {
                match self {
                    $($Enum::$Variant => $value,)+
                }
            }
        }
    };
}

/// Generate a paired `pub fn $to(_: $Enum) -> &'static str` /
/// `pub fn $from(_: &str) -> Option<$Enum>` from a `Variant => "key"` table.
///
/// Use for save-file fields where an unknown serialized key should bubble
/// up as a load failure rather than silently being replaced with a sentinel.
/// (Matches the Python convention for these enums.)
#[macro_export]
macro_rules! enum_key_pair_option {
    (
        $to:ident, $from:ident, $Enum:ident;
        $($Variant:ident => $key:literal),+ $(,)?
    ) => {
        pub fn $to(e: $Enum) -> &'static str {
            match e {
                $($Enum::$Variant => $key,)+
            }
        }
        pub fn $from(s: &str) -> Option<$Enum> {
            Some(match s {
                $($key => $Enum::$Variant,)+
                _ => return None,
            })
        }
    };
}

/// Declare a dispatch enum (the "no_std box-of-trait substitute") that owns
/// one variant per concrete type implementing `$Trait`. Generates:
///
/// * the enum declaration itself,
/// * `pub fn $from_fn(src: $Source) -> Self` mapping a payload-free source
///   enum to a freshly constructed variant,
/// * `pub fn $as_ref(&self) -> &dyn $Trait` and matching `$as_mut`,
///
/// so adding a new variant means editing one place instead of four. Variant
/// names must match between `$Source` and the dispatch enum. Each variant
/// may optionally bind a payload pattern from its source variant via
/// `Variant(Type)(pat) = ctor`, which expands to
/// `$Source::Variant(pat) => Self::Variant(ctor)`.
///
/// ```ignore
/// dispatch_enum! {
///     pub enum ActiveScene from SceneId via from_id,
///     as dyn Scene via as_scene / as_scene_mut
///     {
///         Inside(InsideScene) = InsideScene::new(),
///         #[cfg(feature = "rgb-led")] // attributes carry through to every arm
///         Flashlight(FlashlightScene) = FlashlightScene::new(),
///         // ...
///     }
/// }
/// ```
#[macro_export]
macro_rules! dispatch_enum {
    (
        $(#[$enum_meta:meta])*
        $vis:vis enum $Enum:ident from $Source:ident via $from_fn:ident,
        as dyn $Trait:ident via $as_ref:ident / $as_mut:ident
        {
            $(
                $(#[$variant_meta:meta])*
                $Variant:ident($Type:ty) $(($($pat:tt)+))? = $ctor:expr
            ),+ $(,)?
        }
    ) => {
        $(#[$enum_meta])*
        $vis enum $Enum {
            $($(#[$variant_meta])* $Variant($Type),)+
        }

        impl $Enum {
            pub fn $from_fn(src: $Source) -> Self {
                match src {
                    $(
                        $(#[$variant_meta])*
                        $Source::$Variant $(($($pat)+))? => Self::$Variant($ctor),
                    )+
                }
            }

            pub fn $as_ref(&self) -> &dyn $Trait {
                match self {
                    $($(#[$variant_meta])* Self::$Variant(b) => b,)+
                }
            }

            pub fn $as_mut(&mut self) -> &mut dyn $Trait {
                match self {
                    $($(#[$variant_meta])* Self::$Variant(b) => b,)+
                }
            }
        }
    };
}

/// Generate a paired `pub fn $to(_: $Enum) -> &'static str` /
/// `pub fn $from(_: &str) -> $Enum` (returns the variant directly, with a
/// designated `default:` for unknown input). Supports per-variant aliases:
/// the first listed key is the canonical one used by `$to`; additional
/// `| "alias"` patterns are accepted by `$from` for legacy-save tolerance.
///
/// ```ignore
/// enum_key_pair_default! {
///     layer_save_key, layer_from_key, PlantLayer;
///     default: PlantLayer::Midground;
///     Background => "background" | "bg",
///     Midground  => "midground",
///     Foreground => "foreground" | "fg",
/// }
/// ```
#[macro_export]
macro_rules! enum_key_pair_default {
    (
        $to:ident, $from:ident, $Enum:ident;
        default: $default:expr;
        $($Variant:ident => $canonical:literal $(| $alias:literal)*),+ $(,)?
    ) => {
        pub fn $to(e: $Enum) -> &'static str {
            match e {
                $($Enum::$Variant => $canonical,)+
            }
        }
        pub fn $from(s: &str) -> $Enum {
            match s {
                $($canonical $(| $alias)* => $Enum::$Variant,)+
                _ => $default,
            }
        }
    };
}
