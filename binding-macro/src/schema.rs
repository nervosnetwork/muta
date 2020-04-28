use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream, Result};
use syn::{
    Attribute, Data, DeriveInput, Field, Fields, GenericArgument, Ident, LitStr, PathArguments,
    Type,
};

pub fn impl_event(ast: &DeriveInput) -> TokenStream {
    let ident = &ast.ident;

    let mut schema_tokens = quote! {
        let mut r = BTreeMap::<String, String>::new();
    };

    let mut topic_tokens = quote! {};
    let mut event_schema = "union Event = ".to_owned();

    match &ast.data {
        Data::Enum(data) => {
            for vp in data.variants.iter() {
                let ident = vp.ident.clone();
                let ident_str = ident.clone().to_string();
                let topic = ident_str.clone() + " | ";
                event_schema.push_str(topic.as_str());

                let schema_token = quote! {
                    #ident::schema(&mut r);
                };
                schema_tokens = quote! {
                    #schema_tokens
                    #schema_token
                };
                let topic_token = quote! {
                    impl #ident {
                        pub fn topic(&self) -> String {
                            #ident_str.to_owned()
                        }
                    }
                };
                topic_tokens = quote! {
                    #topic_tokens
                    #topic_token
                };
            }
            event_schema = event_schema
                .strip_suffix(" | ")
                .expect("strip suffix should succeed")
                .to_owned();
        }
        _ => panic!("#[derive(Event)] can only mark a Enum"),
    }

    let gen = quote! {
        impl #ident {
            pub fn schema() -> String {
                #schema_tokens
                r.insert("_union_event_".to_owned(), #event_schema.to_owned());
                let mut s = "".to_owned();
                for v in r.values() {
                    s.push_str(v.as_str());
                    s.push_str("\n\n")
                }
                s
            }
        }
        #topic_tokens
    };

    gen.into()
}

pub fn impl_object(ast: &DeriveInput) -> TokenStream {
    let ident = &ast.ident;
    let ident_str = &ident.to_string();

    let comment = extract_comment(&ast.attrs, false).unwrap_or_default();

    let mut tokens = quote! {
        if register.contains_key(#ident_str) {
            return;
        }
        let mut schema = format!("{}type {} {}\n", #comment, #ident_str, "{");
    };

    match &ast.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => {
                for f in fields.named.iter() {
                    let ret = extract_ident_from_ty(&f.ty);
                    let token = generate_field_token(f, &ret.0, ret.1);
                    tokens = quote! {
                        #tokens
                        #token
                    };
                }
            }
            _ => panic!("#[derive(SchemaObject)]: struct fields should be named"),
        },
        _ => panic!("#[derive(SchemaObject)] can only mark a struct"),
    }

    let token = quote! {
        schema.push_str("}");
        register.insert(#ident_str.to_owned(), schema);
    };

    tokens = quote! {
        #tokens
        #token
    };

    let gen = quote! {
        impl SchemaGenerator for #ident {
            fn name() -> String {
                #ident_str.to_owned()
            }

            fn schema(register: &mut BTreeMap<String, String>) {
                #tokens
            }
        }
    };

    gen.into()
}

fn extract_comment(attrs: &[Attribute], is_field: bool) -> Option<String> {
    for attr in attrs.iter() {
        if attr.path.segments.first().is_some()
            && "description" == &attr.path.segments.first().unwrap().ident.to_string()
        {
            let comment: Comment = attr
                .parse_args()
                .expect("#[description]: comments should be string");
            if is_field {
                return Some(format!("  # {}\n", comment.value));
            } else {
                return Some(format!("# {}\n", comment.value));
            }
        }
    }
    None
}

fn generate_field_token(f: &Field, ty: &Ident, is_vec: bool) -> proc_macro2::TokenStream {
    let f_str = f
        .ident
        .as_ref()
        .expect("#[derive(SchemaObject)]: struct fields should be named")
        .to_string();

    let mut field_str = "".to_owned();
    let comment = extract_comment(&f.attrs, true).unwrap_or_default();
    field_str.push_str(comment.as_str());
    let fmt_str = if is_vec {
        "  {}: [{}!]!\n"
    } else {
        "  {}: {}!\n"
    };
    field_str.push_str(fmt_str);

    quote! {
        let ty_name = #ty::name();
        let field_str = format!(#field_str, #f_str, ty_name);
        schema.push_str(field_str.as_str());
        #ty::schema(register);
    }
}

fn extract_ident_from_path(ty: &Type) -> Ident {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .first()
            .expect("#[derive(SchemaObject)]: Vec should contain a type arg")
            .ident
            .clone(),
        _ => panic!("#[derive(SchemaObject)]: Vec arg should be a path type"),
    }
}

fn extract_ident_from_ty(ty: &Type) -> (Ident, bool) {
    match ty {
        Type::Path(ty) => {
            let segs = &ty.path.segments;
            if 1 == segs.len() {
                let concrete_ty = segs.first().unwrap();
                if "Vec" == &concrete_ty.ident.clone().to_string() {
                    if let PathArguments::AngleBracketed(g_ty) = &concrete_ty.arguments {
                        let arg = g_ty
                            .args
                            .first()
                            .expect("#[derive(SchemaObject)]: Vec should contain a type arg");
                        if let GenericArgument::Type(arg_ty) = arg {
                            let ident = extract_ident_from_path(&arg_ty);
                            (ident, true)
                        } else {
                            panic!("#[derive(SchemaObject)]: Vec arg should be a type")
                        }
                    } else {
                        panic!("#[derive(SchemaObject)]: Vec should be AngleBracketed")
                    }
                } else if let PathArguments::None = concrete_ty.arguments {
                    (concrete_ty.ident.clone(), false)
                } else {
                    panic!("#[derive(SchemaObject)]: field type only supports T, Vec<T>, or [T;n]")
                }
            } else {
                panic!("#[derive(SchemaObject)]: length of field type should be 1")
            }
        }
        _ => panic!("#[derive(SchemaObject)]: field type only supports T, Vec<T>, or [T;n]"),
    }
}

struct Comment {
    pub value: String,
}

impl Parse for Comment {
    fn parse(input: ParseStream) -> Result<Self> {
        let lit: LitStr = input.parse()?;
        Ok(Self { value: lit.value() })
    }
}
