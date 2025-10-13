use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, punctuated::Punctuated, token::Comma};

#[derive(Default)]
pub struct EntityLoaderSchema {
    pub fields: Vec<EntityLoaderField>,
}

pub struct EntityLoaderField {
    pub is_one: bool,
    pub field: Ident,
    /// super::bakery::Entity
    pub entity: String,
}

pub fn expand_entity_loader(schema: EntityLoaderSchema) -> TokenStream {
    let mut field_bools: Punctuated<_, Comma> = Punctuated::new();
    let mut field_nests: Punctuated<_, Comma> = Punctuated::new();
    let mut one_fields: Punctuated<_, Comma> = Punctuated::new();
    let mut with_impl = TokenStream::new();
    let mut with_nest_impl = TokenStream::new();
    let mut select_impl = TokenStream::new();
    let mut assemble_one = TokenStream::new();
    let mut load_one = TokenStream::new();
    let mut load_many = TokenStream::new();
    let mut load_one_nest = TokenStream::new();
    let mut load_many_nest = TokenStream::new();
    let mut load_one_nest_nest = TokenStream::new();
    let mut load_many_nest_nest = TokenStream::new();
    let mut arity = 1;

    one_fields.push(quote!(model));

    for entity_field in schema.fields.iter() {
        let field = &entity_field.field;
        let is_one = entity_field.is_one;
        let entity: TokenStream = entity_field.entity.parse().unwrap();
        let entity_module: TokenStream = entity_field
            .entity
            .trim_end_matches("::Entity")
            .parse()
            .unwrap();

        field_bools.push(quote!(pub #field: bool));
        field_nests.push(quote!(pub #field: #entity_module::EntityLoaderWith));

        with_impl.extend(quote! {
            if target == #entity.table_ref() {
                self.#field = true;
            }
        });
        with_nest_impl.extend(quote! {
            if left == #entity.table_ref() {
                self.with.#field = true;
                self.nest.#field.set(right);
                return self;
            }
        });

        if is_one {
            arity += 1;
            if arity <= 3 {
                // do not go beyond SelectThree
                one_fields.push(quote!(#field));

                select_impl.extend(quote! {
                    let select = if self.with.#field && self.nest.#field.is_empty() {
                        self.with.#field = false;
                        select.find_also(Entity, #entity)
                    } else {
                        select.select_also_fake(#entity)
                    };
                });

                assemble_one.extend(quote! {
                    model.#field = #field.map(Into::into).map(Box::new);
                });
            }

            load_one.extend(quote! {
                if with.#field {
                    let #field = models.as_slice().load_one_ex(#entity, db).await?;
                    let #field = #entity_module::EntityLoader::load_nest(#field, &nest.#field, db).await?;

                    for (model, #field) in models.iter_mut().zip(#field) {
                        model.#field = #field.map(Into::into).map(Box::new);
                    }
                }
            });
            load_one_nest.extend(quote! {
                if with.#field {
                    let #field = models.as_slice().load_one_ex(#entity, db).await?;

                    for (model, #field) in models.iter_mut().zip(#field) {
                        if let Some(model) = model.as_mut() {
                            model.#field = #field.map(Into::into).map(Box::new);
                        }
                    }
                }
            });
            load_one_nest_nest.extend(quote! {
                if with.#field {
                    let #field = models.as_slice().load_one_ex(#entity, db).await?;

                    for (models, #field) in models.iter_mut().zip(#field) {
                        for (model, #field) in models.iter_mut().zip(#field) {
                            model.#field = #field.map(Into::into).map(Box::new);
                        }
                    }
                }
            });
        } else {
            load_many.extend(quote! {
                if with.#field {
                    let #field = models.as_slice().load_many_ex(#entity, db).await?;
                    let #field = #entity_module::EntityLoader::load_nest_nest(#field, &nest.#field, db).await?;

                    for (model, #field) in models.iter_mut().zip(#field) {
                        model.#field = #field;
                    }
                }
            });
            load_many_nest.extend(quote! {
                if with.#field {
                    let #field = models.as_slice().load_many_ex(#entity, db).await?;

                    for (model, #field) in models.iter_mut().zip(#field) {
                        if let Some(model) = model.as_mut() {
                            model.#field = #field;
                        }
                    }
                }
            });
            load_many_nest_nest.extend(quote! {
                if with.#field {
                    let #field = models.as_slice().load_many_ex(#entity, db).await?;

                    for (models, #field) in models.iter_mut().zip(#field) {
                        for (model, #field) in models.iter_mut().zip(#field) {
                            model.#field = #field;
                        }
                    }
                }
            });
        }
    }

    quote! {

    #[doc = " Generated by sea-orm-macros"]
    pub struct EntityLoader {
        select: sea_orm::Select<Entity>,
        with: EntityLoaderWith,
        nest: EntityLoaderNest,
    }

    #[doc = " Generated by sea-orm-macros"]
    #[derive(Debug, Default, PartialEq, Eq)]
    pub struct EntityLoaderWith {
        #field_bools
    }

    #[doc = " Generated by sea-orm-macros"]
    #[derive(Debug, Default, PartialEq, Eq)]
    pub struct EntityLoaderNest {
        #field_nests
    }

    impl EntityLoaderWith {
        #[doc = " Generated by sea-orm-macros"]
        pub fn is_empty(&self) -> bool {
            self == &Self::default()
        }
        #[doc = " Generated by sea-orm-macros"]
        pub fn set(&mut self, target: sea_orm::sea_query::TableRef) {
            #with_impl
        }
    }

    #[automatically_derived]
    impl std::fmt::Debug for EntityLoader {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("EntityLoader")
            .field("select", &match (Entity::default().schema_name(), Entity::default().table_name()) {
                (Some(s), t) => format!("{s}.{t}"),
                (None, t) => t.to_owned(),
            })
            .field("with", &self.with)
            .field("nest", &self.nest)
            .finish()
        }
    }

    #[automatically_derived]
    impl sea_orm::QueryFilter for EntityLoader {
        type QueryStatement = <sea_orm::Select<Entity> as sea_orm::QueryFilter>::QueryStatement;

        fn query(&mut self) -> &mut Self::QueryStatement {
            sea_orm::QueryFilter::query(&mut self.select)
        }
    }

    #[automatically_derived]
    impl sea_orm::QueryOrder for EntityLoader {
        type QueryStatement = <sea_orm::Select<Entity> as sea_orm::QueryOrder>::QueryStatement;

        fn query(&mut self) -> &mut Self::QueryStatement {
            sea_orm::QueryOrder::query(&mut self.select)
        }
    }

    #[automatically_derived]
    impl sea_orm::compound::EntityLoaderTrait<Entity> for EntityLoader {}

    impl Entity {
        #[doc = " Generated by sea-orm-macros"]
        pub fn load() -> EntityLoader {
            EntityLoader {
                select: Entity::find(),
                with: Default::default(),
                nest: Default::default(),
            }
        }
    }

    impl EntityLoader {
        #[doc = " Generated by sea-orm-macros"]
        pub async fn one<C: sea_orm::ConnectionTrait>(
            mut self,
            db: &C,
        ) -> Result<Option<ModelEx>, sea_orm::DbErr> {
            use sea_orm::QuerySelect;

            self.select = self.select.limit(1);
            Ok(self.all(db).await?.into_iter().next())
        }

        #[doc = " Generated by sea-orm-macros"]
        pub fn with<T: sea_orm::compound::EntityLoaderWithParam<Entity>>(mut self, param: T) -> Self {
            match param.into_with_param() {
                (left, None) => self.with_1(left),
                (left, Some(right)) => self.with_2(left, right),
            }
        }

        fn with_1(mut self, table_ref: sea_orm::sea_query::TableRef) -> Self {
            self.with.set(table_ref);
            self
        }

        fn with_2(mut self, left: sea_orm::sea_query::TableRef, right: sea_orm::sea_query::TableRef) -> Self {
            #with_nest_impl
            self
        }

        #[doc = " Generated by sea-orm-macros"]
        pub async fn all<C: sea_orm::ConnectionTrait>(mut self, db: &C) -> Result<Vec<ModelEx>, sea_orm::DbErr> {
            let select = self.select;

            #select_impl

            let models = select.all(db).await?;

            let models = models.into_iter().map(|(#one_fields)| {
                let mut model = model.into_ex();
                #assemble_one
                model
            }).collect::<Vec<_>>();

            let models = Self::load(models, &self.with, &self.nest, db).await?;

            Ok(models)
        }

        #[doc = " Generated by sea-orm-macros"]
        pub async fn load<C: sea_orm::ConnectionTrait>(mut models: Vec<ModelEx>, with: &EntityLoaderWith, nest: &EntityLoaderNest, db: &C) -> Result<Vec<ModelEx>, DbErr> {
            use sea_orm::LoaderTraitEx;
            #load_one
            #load_many
            Ok(models)
        }

        #[doc = " Generated by sea-orm-macros"]
        pub async fn load_nest<C: sea_orm::ConnectionTrait>(mut models: Vec<Option<ModelEx>>, with: &EntityLoaderWith, db: &C) -> Result<Vec<Option<ModelEx>>, DbErr> {
            use sea_orm::LoaderTraitEx;
            #load_one_nest
            #load_many_nest
            Ok(models)
        }

        #[doc = " Generated by sea-orm-macros"]
        pub async fn load_nest_nest<C: sea_orm::ConnectionTrait>(mut models: Vec<Vec<ModelEx>>, with: &EntityLoaderWith, db: &C) -> Result<Vec<Vec<ModelEx>>, DbErr> {
            use sea_orm::NestedLoaderTrait;
            #load_one_nest_nest
            #load_many_nest_nest
            Ok(models)
        }
    }

    }
}
