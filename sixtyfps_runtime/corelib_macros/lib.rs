/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!
    This crate contains the internal procedural macros
    used by the sixtyfps corelib crate
*/

extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;

/// This derive macro is used with structures in the run-time library that are meant
/// to be exposed to the language. The structure is introspected for properties and fields
/// marked with the `rtti_field` attribute and generates run-time type information for use
/// with the interpeter.
/// In addition all `Property<T> foo` fields get a convenient getter function generated
/// that works on a `Pin<&Self>` receiver.
#[proc_macro_derive(SixtyFPSElement, attributes(rtti_field))]
pub fn sixtyfps_element(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    let fields = match &input.data {
        syn::Data::Struct(syn::DataStruct { fields: f @ syn::Fields::Named(..), .. }) => f,
        _ => {
            return syn::Error::new(
                input.ident.span(),
                "Only `struct` with named field are supported",
            )
            .to_compile_error()
            .into()
        }
    };

    let mut pub_prop_field_names = Vec::new();
    let mut pub_prop_field_types = Vec::new();
    let mut property_names = Vec::new();
    let mut property_visibility = Vec::new();
    let mut property_types = Vec::new();
    for field in fields {
        if let Some(property_type) = property_type(&field.ty) {
            let name = field.ident.as_ref().unwrap();
            if matches!(field.vis, syn::Visibility::Public(_)) {
                pub_prop_field_names.push(name);
                pub_prop_field_types.push(&field.ty);
            }
            property_names.push(name);
            property_visibility.push(field.vis.clone());
            property_types.push(property_type);
        }
    }

    let (plain_field_names, plain_field_types): (Vec<_>, Vec<_>) = fields
        .iter()
        .filter(|f| {
            f.attrs
                .iter()
                .find(|attr| {
                    attr.parse_meta()
                        .ok()
                        .map(|meta| match meta {
                            syn::Meta::Path(path) => path
                                .get_ident()
                                .map(|ident| ident.to_string() == "rtti_field")
                                .unwrap_or(false),
                            _ => false,
                        })
                        .unwrap_or(false)
                })
                .is_some()
        })
        .map(|f| (f.ident.as_ref().unwrap(), &f.ty))
        .unzip();

    let callback_field_names =
        fields.iter().filter(|f| is_callback(&f.ty)).map(|f| f.ident.as_ref().unwrap());

    let item_name = &input.ident;

    quote!(
        #[cfg(feature = "rtti")]
        impl BuiltinItem for #item_name {
            fn name() -> &'static str {
                stringify!(#item_name)
            }
            fn properties<Value: ValueType>() -> Vec<(&'static str, &'static dyn PropertyInfo<Self, Value>)> {
                vec![#( {
                    const O : MaybeAnimatedPropertyInfoWrapper<#item_name, #pub_prop_field_types> =
                        MaybeAnimatedPropertyInfoWrapper(#item_name::FIELD_OFFSETS.#pub_prop_field_names);
                    (stringify!(#pub_prop_field_names), (&O).as_property_info())
                } ),*]
            }
            fn fields<Value: ValueType>() -> Vec<(&'static str, &'static dyn FieldInfo<Self, Value>)> {
                vec![#( {
                    const O : const_field_offset::FieldOffset<#item_name, #plain_field_types, const_field_offset::AllowPin> =
                        #item_name::FIELD_OFFSETS.#plain_field_names;
                    (stringify!(#plain_field_names), &O as &'static dyn FieldInfo<Self, Value>)
                } ),*]
            }
            fn callbacks<Value: ValueType>() -> Vec<(&'static str, &'static dyn CallbackInfo<Self, Value>)> {
                vec![#( {
                    const O : const_field_offset::FieldOffset<#item_name, Callback<()>, const_field_offset::AllowPin> =
                         #item_name::FIELD_OFFSETS.#callback_field_names;
                    (stringify!(#callback_field_names), &O as  &'static dyn CallbackInfo<Self, Value>)
                } ),*]
            }
        }

        impl #item_name {
            #(
                #property_visibility fn #property_names(self: core::pin::Pin<&Self>) -> #property_types {
                    Self::FIELD_OFFSETS.#property_names.apply_pin(self).get()
                }
            )*
        }
    )
    .into()
}

fn type_name(ty: &syn::Type) -> String {
    quote!(#ty).to_string()
}

// Try to match `Property<Foo>` on the syn tree and return Foo if found
fn property_type(ty: &syn::Type) -> Option<&syn::Type> {
    if let syn::Type::Path(syn::TypePath { path: syn::Path { segments, .. }, .. }) = ty {
        if let Some(syn::PathSegment {
            ident,
            arguments:
                syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments { args, .. }),
        }) = segments.first()
        {
            match args.first() {
                Some(syn::GenericArgument::Type(property_type))
                    if ident.to_string() == "Property" =>
                {
                    return Some(property_type)
                }
                _ => {}
            }
        }
    }
    None
}

fn is_callback(ty: &syn::Type) -> bool {
    type_name(ty).to_string().starts_with("Callback <")
}

#[proc_macro_derive(MappedKeyCode)]
pub fn keycode_mapping(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    let variants = match &input.data {
        syn::Data::Enum(syn::DataEnum { variants, .. }) => variants,
        _ => {
            return syn::Error::new(input.ident.span(), "Only `enum` types are supported")
                .to_compile_error()
                .into()
        }
    }
    .iter()
    .collect::<Vec<_>>();

    quote!(
        impl From<winit::event::VirtualKeyCode> for KeyCode {
            fn from(code: winit::event::VirtualKeyCode) -> Self {
                match code {
                    #(winit::event::VirtualKeyCode::#variants => Self::#variants),*
                }
            }
        }
    )
    .into()
}
