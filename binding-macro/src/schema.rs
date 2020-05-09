use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream, Result};
use syn::{
    Attribute, Data, DeriveInput, Field, Fields, GenericArgument, Ident, LitStr, PathArguments,
    Type,
};

pub fn impl_event(ast: &DeriveInput) -> TokenStream {
    let ident = &ast.ident;

    let mut event_tokens = quote! {
        let mut r = BTreeMap::<String, DataMeta>::new();
        let mut events = vec![];
    };
    let mut topic_tokens = quote! {};

    match &ast.data {
        Data::Enum(data) => {
            for vp in data.variants.iter() {
                let ident = vp.ident.clone();
                let ident_str = ident.clone().to_string();
                event_tokens = quote! {
                    #event_tokens
                    events.push(#ident_str.to_owned());
                    #ident::meta(&mut r);
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
        }
        _ => panic!("#[derive(SchemaEvent)] can only mark a Enum"),
    }

    let gen = quote! {
        impl #ident {
            pub fn meta() -> (Vec<String>, BTreeMap<String, DataMeta>) {
                #event_tokens
                (events, r)
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
        let mut fields_meta = vec![];
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

    tokens = quote! {
        #tokens
        let struct_meta = StructMeta {
            name: #ident_str.to_owned(),
            fields: fields_meta,
            comment: #comment.to_owned()
        };
        register.insert(#ident_str.to_owned(), DataMeta::Struct(struct_meta));
    };

    let gen = quote! {
        impl MetaGenerator for #ident {
            fn name() -> String {
                #ident_str.to_owned()
            }

            fn meta(register: &mut BTreeMap<String, DataMeta>) {
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

    let comment = extract_comment(&f.attrs, true).unwrap_or_default();

    quote! {
        {
            let field_meta = FieldMeta {
                name: #f_str.to_owned(),
                ty: #ty::name(),
                is_vec: #is_vec,
                comment: #comment.to_owned(),
            };
            fields_meta.push(field_meta);
            #ty::meta(register);
        }
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
    let pty = if let Type::Path(ty) = ty {
        ty
    } else {
        panic!("#[derive(SchemaObject)]: field type only supports T, Vec<T>");
    };

    let segs = &pty.path.segments;
    if 1 != segs.len() {
        panic!("#[derive(SchemaObject)]: length of field type should be 1");
    }
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
        panic!("#[derive(SchemaObject)]: field type only supports T, Vec<T>")
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
