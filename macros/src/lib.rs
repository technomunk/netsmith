use proc_macro::TokenStream;

/// Apply all enabled serialization derives to an item.
///
/// Conditionally derives based on crate features:
/// - `bevy` → `bevy_reflect::Reflect`
/// - `bincode` → `bincode::Encode`, `bincode::Decode`
/// - `serde` → `serde::Serialize`, `serde::Deserialize`
/// - `wincode` → `wincode::SchemaWrite`, `wincode::SchemaRead`
///
/// # Example
/// ```rust
/// #[serializable]
/// #[derive(Debug, Clone)]
/// pub struct Packet {
///     pub id: u32,
/// }
/// ```
#[proc_macro_attribute]
pub fn serializable(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut out: TokenStream = concat!(
        r#"#[cfg_attr(feature = "bevy", derive(::bevy_reflect::Reflect))]"#,
        r#"#[cfg_attr(feature = "bincode", derive(::bincode::Encode, ::bincode::Decode))]"#,
        r#"#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]"#,
        r#"#[cfg_attr(feature = "wincode", derive(::wincode::SchemaWrite, ::wincode::SchemaRead))]"#,
    )
    .parse()
    .unwrap();
    out.extend(item);
    out
}
