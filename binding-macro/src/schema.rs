use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream, Result};
use syn::{
    Attribute, Data, DeriveInput, Field, Fields, GenericArgument, Ident, LitStr, PathArguments,
    Type,
};

pub fn impl_event(ast: &DeriveInput) -> TokenStream {
    let ident = &ast.ident;

    let mut tokens = quote! {
        let mut r = BTreeMap::<String, String>::new();
    };

    match &ast.data {
        Data::Enum(data) => {
            for vp in data.variants.iter() {
                if let Fields::Unnamed(fs) = &vp.fields {
                    let f_ty = &fs
                        .unnamed
                        .first()
                        .expect("#[derive(Event)]: Enum variant should have a field")
                        .ty;
                    let ident_ty = extract_ident_from_path(&f_ty);
                    let token = quote! {
                        #ident_ty::schema(&mut r);
                    };
                    tokens = quote! {
                        #tokens
                        #token
                    };
                } else {
                    panic!("#[derive(Event)]: Variant should be unnamed type")
                }
            }
        }
        _ => panic!("#[derive(Event)] can only mark a Enum"),
    }

    let gen = quote! {
        impl #ident {
            pub fn schema() -> String {
                #tokens
                let mut s = "".to_owned();
                for v in r.values() {
                    s.push_str(v.as_str());
                    s.push_str("\n\n")
                }
                s
            }
        }
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
            _ => panic!("#[derive(ServiceSchema)]: struct fields should be named"),
        },
        _ => panic!("#[derive(ServiceSchema)] can only mark a struct"),
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
        impl ServiceSchema for #ident {
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

fn extract_comment(attrs: &Vec<Attribute>, is_field: bool) -> Option<String> {
    for attr in attrs.iter() {
        if attr.path.segments.first().is_some() {
            if "description".to_owned() == attr.path.segments.first().unwrap().ident.to_string() {
                let comment: Comment = attr
                    .parse_args()
                    .expect("#[description]: comments should be string");
                if is_field {
                    return Some(format!("  // {}\n", comment.value));
                } else {
                    return Some(format!("// {}\n", comment.value));
                }
            }
        }
    }
    None
}

fn generate_field_token(f: &Field, ty: &Ident, is_vec: bool) -> proc_macro2::TokenStream {
    let f_str = f
        .ident
        .as_ref()
        .expect("#[derive(ServiceSchema)]: struct fields should be named")
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
            .expect("#[derive(ServiceSchema)]: Vec should contain a type arg")
            .ident
            .clone(),
        _ => panic!("#[derive(ServiceSchema)]: Vec arg should be a path type"),
    }
}

fn extract_ident_from_ty(ty: &Type) -> (Ident, bool) {
    match ty {
        Type::Path(ty) => {
            let segs = &ty.path.segments;
            if 1 == segs.len() {
                let concrete_ty = segs.first().unwrap();
                if "Vec".to_owned() == concrete_ty.ident.clone().to_string() {
                    if let PathArguments::AngleBracketed(g_ty) = &concrete_ty.arguments {
                        let arg = g_ty
                            .args
                            .first()
                            .expect("#[derive(ServiceSchema)]: Vec should contain a type arg");
                        if let GenericArgument::Type(arg_ty) = arg {
                            let ident = extract_ident_from_path(&arg_ty);
                            (ident, true)
                        } else {
                            panic!("#[derive(ServiceSchema)]: Vec arg should be a type")
                        }
                    } else {
                        panic!("#[derive(ServiceSchema)]: Vec should be AngleBracketed")
                    }
                } else {
                    if let PathArguments::None = concrete_ty.arguments {
                        (concrete_ty.ident.clone(), false)
                    } else {
                        panic!("#[derive(ServiceSchema)]: field type only supports T, Vec<T>, or [T;n]")
                    }
                }
            } else {
                panic!("#[derive(ServiceSchema)]: length of field type should be 1")
            }
        }
        _ => panic!("#[derive(ServiceSchema)]: field type only supports T, Vec<T>, or [T;n]"),
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
