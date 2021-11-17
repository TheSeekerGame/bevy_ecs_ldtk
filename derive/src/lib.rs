extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn;

#[proc_macro_derive(LdtkEntity, attributes(sprite_bundle))]
pub fn ldtk_entity_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();

    expand_ldtk_entity_derive(&ast)
}

static SPRITE_BUNDLE_ATTRIBUTE_NAME: &str = "sprite_bundle";

fn expand_ldtk_entity_derive(ast: &syn::DeriveInput) -> TokenStream {
    let struct_name = &ast.ident;

    let fields = match &ast.data {
        syn::Data::Struct(syn::DataStruct {
            fields: syn::Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => panic!("Expected a struct with named fields."),
    };

    let mut field_constructions = Vec::new();
    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;

        let sprite_bundle = field
            .attrs
            .iter()
            .find(|a| *a.path.get_ident().as_ref().unwrap() == SPRITE_BUNDLE_ATTRIBUTE_NAME);
        if let Some(attribute) = sprite_bundle {
            match attribute.parse_meta().unwrap() {
                syn::Meta::List(syn::MetaList { nested, .. }) => match nested.first().unwrap() {
                    syn::NestedMeta::Lit(syn::Lit::Str(asset)) => {
                        let asset_path = &asset.value();

                        field_constructions.push(quote! {
                            #field_name: SpriteBundle {
                                material: materials.add(asset_server.load(#asset_path).into()),
                                ..Default::default()
                            },
                        })
                    }
                    _ => panic!("Expected asset path as the first argument of sprite_bundle"),
                },
                _ => panic!("Expected arguments with sprite_bundle attribute"),
            }
        } else {
            field_constructions.push(quote! {
                #field_name: #field_type::default(),
            })
        }
    }

    let generics = &ast.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let gen = quote! {
        impl #impl_generics LdtkEntity for #struct_name #ty_generics #where_clause {
            fn from_instance(
                _: &EntityInstance,
                asset_server: &Res<AssetServer>,
                materials: &mut ResMut<Assets<ColorMaterial>>,
                _: &mut ResMut<Assets<TextureAtlas>>,
            ) -> Self {
                Self {
                    #(#field_constructions)*
                }
            }
        }
    };
    gen.into()
}
