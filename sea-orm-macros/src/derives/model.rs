use super::{
    attributes::derive_attr,
    util::{escape_rust_keyword, field_not_ignored, trim_starting_raw_identifier},
};
use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use std::iter::FromIterator;
use syn::{Attribute, Data, Expr, Ident, LitStr};

pub(crate) enum DeriveModelError {
    InputNotStruct,
    Syn(syn::Error),
}

pub(crate) struct DeriveModel {
    column_idents: Vec<Ident>,
    entity_ident: Ident,
    field_idents: Vec<Ident>,
    field_types: Vec<syn::Type>,
    ident: Ident,
    ignore_attrs: Vec<bool>,
}

impl DeriveModel {
    pub fn new(ident: &Ident, data: &Data, attrs: &[Attribute]) -> Result<Self, DeriveModelError> {
        let fields = match data {
            syn::Data::Struct(syn::DataStruct {
                fields: syn::Fields::Named(syn::FieldsNamed { named, .. }),
                ..
            }) => named,
            _ => return Err(DeriveModelError::InputNotStruct),
        };

        let sea_attr = derive_attr::SeaOrm::try_from_attributes(attrs)
            .map_err(DeriveModelError::Syn)?
            .unwrap_or_default();

        let entity_ident = sea_attr.entity.unwrap_or_else(|| format_ident!("Entity"));

        let field_idents = fields
            .iter()
            .map(|field| field.ident.as_ref().unwrap().clone())
            .collect();

        let field_types = fields.iter().map(|field| field.ty.clone()).collect();

        let column_idents = fields
            .iter()
            .map(|field| {
                let ident = field.ident.as_ref().unwrap().to_string();
                let ident = trim_starting_raw_identifier(ident).to_upper_camel_case();
                let ident = escape_rust_keyword(ident);
                let mut ident = format_ident!("{}", &ident);
                field
                    .attrs
                    .iter()
                    .filter(|attr| attr.path().is_ident("sea_orm"))
                    .try_for_each(|attr| {
                        attr.parse_nested_meta(|meta| {
                            if meta.path.is_ident("enum_name") {
                                ident = syn::parse_str(&meta.value()?.parse::<LitStr>()?.value())
                                    .unwrap();
                            } else {
                                // Reads the value expression to advance the parse stream.
                                // Some parameters, such as `primary_key`, do not have any value,
                                // so ignoring an error occurred here.
                                let _: Option<Expr> = meta.value().and_then(|v| v.parse()).ok();
                            }

                            Ok(())
                        })
                        .map_err(DeriveModelError::Syn)
                    })?;
                Ok(ident)
            })
            .collect::<Result<_, _>>()?;

        let ignore_attrs = fields
            .iter()
            .map(|field| !field_not_ignored(field))
            .collect();

        Ok(DeriveModel {
            column_idents,
            entity_ident,
            field_idents,
            field_types,
            ident: ident.clone(),
            ignore_attrs,
        })
    }

    fn expand(&self) -> syn::Result<TokenStream> {
        let expanded_impl_from_query_result = self.impl_from_query_result();
        let expanded_impl_model_trait = self.impl_model_trait();

        Ok(TokenStream::from_iter([
            expanded_impl_from_query_result,
            expanded_impl_model_trait,
        ]))
    }

    fn impl_from_query_result(&self) -> TokenStream {
        let ident = &self.ident;
        let field_idents = &self.field_idents;
        let column_idents = &self.column_idents;
        let field_values: Vec<TokenStream> = column_idents
            .iter()
            .zip(&self.ignore_attrs)
            .map(|(column_ident, ignore)| {
                if *ignore {
                    quote! {
                        Default::default()
                    }
                } else {
                    quote! {
                        row.try_get(pre, sea_orm::IdenStatic::as_str(&<<Self as sea_orm::ModelTrait>::Entity as sea_orm::entity::EntityTrait>::Column::#column_ident).into())?
                    }
                }
            })
            .collect();

        quote!(
            #[automatically_derived]
            impl sea_orm::FromQueryResult for #ident {
                fn from_query_result(row: &sea_orm::QueryResult, pre: &str) -> std::result::Result<Self, sea_orm::DbErr> {
                    Ok(Self {
                        #(#field_idents: #field_values),*
                    })
                }
            }
        )
    }

    pub fn impl_model_trait<'a>(&'a self) -> TokenStream {
        let ident = &self.ident;
        let entity_ident = &self.entity_ident;
        let ignore_attrs = &self.ignore_attrs;
        let ignore = |(ident, ignore): (&'a Ident, &bool)| -> Option<&'a Ident> {
            if *ignore { None } else { Some(ident) }
        };
        let field_idents: Vec<&Ident> = self
            .field_idents
            .iter()
            .zip(ignore_attrs)
            .filter_map(ignore)
            .collect();
        let column_idents: Vec<&Ident> = self
            .column_idents
            .iter()
            .zip(ignore_attrs)
            .filter_map(ignore)
            .collect();
        let get_field_type: Vec<TokenStream> = self
            .field_types
            .iter()
            .zip(ignore_attrs)
            .filter_map(|(ty, ignore)| {
                if *ignore {
                    None
                } else {
                    Some(quote!(<#ty as sea_orm::sea_query::ValueType>::array_type()))
                }
            })
            .collect();

        let missing_field_msg = format!("field does not exist on {ident}");

        quote!(
            #[automatically_derived]
            impl sea_orm::ModelTrait for #ident {
                type Entity = #entity_ident;

                fn get(&self, c: <Self::Entity as sea_orm::entity::EntityTrait>::Column) -> sea_orm::Value {
                    match c {
                        #(<Self::Entity as sea_orm::entity::EntityTrait>::Column::#column_idents => self.#field_idents.clone().into(),)*
                    }
                }

                fn get_value_type(c: <Self::Entity as EntityTrait>::Column) -> sea_orm::sea_query::ArrayType {
                    match c {
                        #(<Self::Entity as sea_orm::entity::EntityTrait>::Column::#column_idents => #get_field_type,)*
                    }
                }

                fn try_set(&mut self, c: <Self::Entity as sea_orm::EntityTrait>::Column, v: sea_orm::Value) -> Result<(), sea_orm::DbErr> {
                    match c {
                        #(<Self::Entity as sea_orm::EntityTrait>::Column::#column_idents => self.#field_idents = sea_orm::sea_query::ValueType::try_from(v).map_err(|e| sea_orm::DbErr::Type(e.to_string()))?,)*
                        _ => return Err(sea_orm::DbErr::Type(#missing_field_msg.to_owned())),
                    }
                    Ok(())
                }
            }
        )
    }
}

impl DeriveModelError {
    pub fn unwrap(self) -> syn::Error {
        match self {
            Self::InputNotStruct => panic!("you can only derive DeriveModel on structs"),
            Self::Syn(err) => err,
        }
    }
}

/// Method to derive an ActiveModel
pub fn expand_derive_model(input: syn::DeriveInput) -> syn::Result<TokenStream> {
    let ident_span = input.ident.span();
    match DeriveModel::new(&input.ident, &input.data, &input.attrs) {
        Ok(model) => model.expand(),
        Err(DeriveModelError::InputNotStruct) => Ok(quote_spanned! {
            ident_span => compile_error!("you can only derive DeriveModel on structs");
        }),
        Err(DeriveModelError::Syn(err)) => Err(err),
    }
}
