//! Derive macros for [`thoth-plugin-sdk`](https://docs.rs/thoth-plugin-sdk).
//!
//! Currently provides [`PluginMeta`], which generates the `plugin-meta`
//! `Guest::get_info()` implementation from a `#[plugin(...)]` attribute.

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Expr, ExprArray, Ident, parse_macro_input};

/// Derive the `plugin-meta` export from a declarative attribute, replacing the
/// hand-written `impl Guest for … { fn get_info() … }` boilerplate.
///
/// ```ignore
/// use thoth_plugin_sdk::PluginMeta;
///
/// #[derive(PluginMeta)]
/// #[plugin(
///     id          = "com.example.my-plugin",
///     name        = "My Plugin",
///     version     = env!("CARGO_PKG_VERSION"),
///     description = "Does useful things",
///     capabilities = [DataSource, NewUiComponent],
///     author      = "Me",          // optional
///     icon        = "\u{E28C}",    // optional
/// )]
/// struct MyPlugin;
/// ```
///
/// `id`, `name`, `version`, and `description` are required; `author`,
/// `homepage`, and `icon` are optional. Each value is any expression that
/// yields something `ToString` (a string literal or e.g. `env!(...)`).
/// `capabilities` is a bracketed list of the binding's `Capability` variant
/// names (`FileLoader`, `FileViewer`, `DataSource`, `Exporter`,
/// `SearchProvider`, `NewUiComponent`).
///
/// The generated impl references the plugin's `cargo component` bindings at the
/// conventional path (`crate::bindings::…`), so it expects a top-level
/// `mod bindings;` — the standard layout.
#[proc_macro_derive(PluginMeta, attributes(plugin))]
pub fn derive_plugin_meta(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ty = input.ident.clone();

    let Some(attr) = input.attrs.iter().find(|a| a.path().is_ident("plugin")) else {
        return syn::Error::new_spanned(
            &ty,
            "#[derive(PluginMeta)] requires a #[plugin(...)] attribute",
        )
        .to_compile_error()
        .into();
    };

    let mut id: Option<Expr> = None;
    let mut name: Option<Expr> = None;
    let mut version: Option<Expr> = None;
    let mut description: Option<Expr> = None;
    let mut author: Option<Expr> = None;
    let mut homepage: Option<Expr> = None;
    let mut icon: Option<Expr> = None;
    let mut capabilities: Vec<Ident> = Vec::new();

    let parsed = attr.parse_nested_meta(|meta| {
        let key = meta.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
        match key.as_str() {
            "id" => id = Some(meta.value()?.parse()?),
            "name" => name = Some(meta.value()?.parse()?),
            "version" => version = Some(meta.value()?.parse()?),
            "description" => description = Some(meta.value()?.parse()?),
            "author" => author = Some(meta.value()?.parse()?),
            "homepage" => homepage = Some(meta.value()?.parse()?),
            "icon" => icon = Some(meta.value()?.parse()?),
            "capabilities" => {
                let arr: ExprArray = meta.value()?.parse()?;
                for elem in arr.elems {
                    match elem {
                        Expr::Path(p) if p.path.get_ident().is_some() => {
                            capabilities.push(p.path.get_ident().unwrap().clone());
                        }
                        _ => return Err(meta.error("capabilities must be bare identifiers")),
                    }
                }
            }
            other => return Err(meta.error(format!("unknown #[plugin] key `{other}`"))),
        }
        Ok(())
    });
    if let Err(e) = parsed {
        return e.to_compile_error().into();
    }

    macro_rules! require {
        ($field:ident) => {
            match $field {
                Some(v) => v,
                None => {
                    return syn::Error::new_spanned(
                        &ty,
                        concat!("#[plugin] is missing required `", stringify!($field), "`"),
                    )
                    .to_compile_error()
                    .into();
                }
            }
        };
    }
    let id = require!(id);
    let name = require!(name);
    let version = require!(version);
    let description = require!(description);

    let opt = |e: Option<Expr>| match e {
        Some(e) => quote! { Some((#e).to_string()) },
        None => quote! { None },
    };
    let author = opt(author);
    let homepage = opt(homepage);
    let icon = opt(icon);

    let caps = capabilities
        .iter()
        .map(|c| quote! { crate::bindings::thoth::plugin::types::Capability::#c });

    quote! {
        impl crate::bindings::exports::thoth::plugin::plugin_meta::Guest for #ty {
            fn get_info() -> crate::bindings::exports::thoth::plugin::plugin_meta::PluginInfo {
                crate::bindings::exports::thoth::plugin::plugin_meta::PluginInfo {
                    id: (#id).to_string(),
                    name: (#name).to_string(),
                    version: (#version).to_string(),
                    description: (#description).to_string(),
                    capabilities: ::std::vec![#(#caps),*],
                    author: #author,
                    homepage: #homepage,
                    icon: #icon,
                }
            }
        }
    }
    .into()
}
