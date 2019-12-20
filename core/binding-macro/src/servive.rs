use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, FnArg, Ident, ImplItem, ImplItemMethod, ItemImpl, Type};

enum ServiceMethod {
    Read(ImplItemMethod),
    Write(ImplItemMethod),
}

struct Hooks {
    before: Option<Ident>,
    after:  Option<Ident>,
}

struct MethodMeta {
    method_ident:  Ident,
    payload_ident: Ident,
    readonly:      bool,
}

pub fn gen_service_code(_: TokenStream, item: TokenStream) -> TokenStream {
    let impl_item = parse_macro_input!(item as ItemImpl);

    let service_ident = get_service_ident(&impl_item);
    let items = &impl_item.items;
    let (impl_generics, ty_generics, where_clause) = impl_item.generics.split_for_impl();

    let mut methods: Vec<ServiceMethod> = vec![];

    for item in items {
        if let ImplItem::Method(method) = item {
            if let Some(service_method) = find_service_method(method) {
                methods.push(service_method)
            }
        }
    }

    let hooks = extract_hooks(items);
    let hook_before = &hooks.before;
    let hook_before_body = match hook_before {
        Some(hook_before) => quote! { self.#hook_before() },
        None => quote! {Ok(())},
    };
    let hook_after = &hooks.after;
    let hook_after_body = match hook_after {
        Some(hook_after) => quote! { self.#hook_after() },
        None => quote! {Ok(())},
    };

    let list_method_meta: Vec<MethodMeta> = methods.into_iter().map(extract_method_meta).collect();

    let (list_read_name, list_read_ident, list_read_payload) =
        split_list_for_metadata(&list_method_meta, true);
    let (list_write_name, list_write_ident, list_write_payload) =
        split_list_for_metadata(&list_method_meta, false);

    TokenStream::from(quote! {
        impl #impl_generics protocol::traits::Service for #service_ident #ty_generics #where_clause {
            fn hook_before_(&mut self) -> protocol::ProtocolResult<()> {
                #hook_before_body
            }

            fn hook_after_(&mut self) -> protocol::ProtocolResult<()> {
                #hook_after_body
            }

            fn read_<Context: protocol::traits::RequestContext>(&self, ctx: Context) -> protocol::ProtocolResult<String> {
                let method = ctx.get_service_method();

                match method {
                    #(#list_read_name => {
                        let payload: #list_read_payload = serde_json::from_str(ctx.get_payload())
                                .map_err(|e| core_binding::ServiceError::JsonParse(e))?;
                        self.#list_read_ident(ctx, payload)
                    },)*
                    _ => Err(core_binding::ServiceError::NotFoundMethod(method.to_owned()).into())
                }
            }

            fn write_<Context: protocol::traits::RequestContext>(&mut self, ctx: Context) -> protocol::ProtocolResult<String> {
                let method = ctx.get_service_method();

                match method {
                    #(#list_write_name => {
                        let payload: #list_write_payload = serde_json::from_str(ctx.get_payload())
                                .map_err(|e| core_binding::ServiceError::JsonParse(e))?;
                        self.#list_write_ident(ctx, payload)
                    },)*
                    _ => Err(core_binding::ServiceError::NotFoundMethod(method.to_owned()).into())
                }
            }
        }

        #impl_item
    })
}

fn split_list_for_metadata(
    list: &[MethodMeta],
    readonly: bool,
) -> (Vec<String>, Vec<Ident>, Vec<Ident>) {
    let mut methods = vec![];
    let mut method_idents = vec![];
    let mut payload_idents = vec![];

    list.iter()
        .filter(|meta| meta.readonly == readonly)
        .for_each(|meta| {
            methods.push(meta.method_ident.to_string());
            method_idents.push(meta.method_ident.clone());
            payload_idents.push(meta.payload_ident.clone());
        });
    (methods, method_idents, payload_idents)
}

fn get_service_ident(impl_item: &ItemImpl) -> Ident {
    match &*impl_item.self_ty {
        Type::Path(type_path) => type_path
            .path
            .get_ident()
            .expect("The identity of the service was not found.")
            .clone(),
        _ => panic!("The identity of the service was not found."),
    }
}

fn find_service_method(method: &ImplItemMethod) -> Option<ServiceMethod> {
    let attrs = &method.attrs;

    for attr in attrs {
        for segment in &attr.path.segments {
            if segment.ident == "read" {
                return Some(ServiceMethod::Read(method.clone()));
            } else if segment.ident == "write" {
                return Some(ServiceMethod::Write(method.clone()));
            }
        }
    }

    None
}

fn extract_hooks(items: &[ImplItem]) -> Hooks {
    let methods: Vec<ImplItemMethod> = items
        .iter()
        .filter(|item| {
            if let ImplItem::Method(_) = item {
                true
            } else {
                false
            }
        })
        .map(|item| {
            if let ImplItem::Method(method) = item {
                method.clone()
            } else {
                unreachable!()
            }
        })
        .collect();

    let mut hooks = Hooks {
        before: None,
        after:  None,
    };

    let mut before_count = 0;
    let mut after_count = 0;

    for method in methods {
        for attr in &method.attrs {
            for segment in &attr.path.segments {
                if segment.ident == "hook_before" {
                    if before_count == 0 {
                        hooks.before = Some(method.sig.ident.clone());
                        before_count = 1;
                    } else {
                        panic!("The before hook can only have one")
                    }
                } else if segment.ident == "hook_after" {
                    if after_count == 0 {
                        hooks.after = Some(method.sig.ident.clone());
                        after_count = 1;
                    } else {
                        panic!("The after hook can only have one")
                    }
                }
            }
        }
    }

    hooks
}

fn extract_method_meta(method: ServiceMethod) -> MethodMeta {
    let (impl_method, readonly) = match method {
        ServiceMethod::Read(impl_method) => (impl_method, true),
        ServiceMethod::Write(impl_method) => (impl_method, false),
    };

    // inputs[0] = &self or &mut self
    // inputs[1] = RequestContext
    // inputs[2] = MethodPayload: FromStr
    if impl_method.sig.inputs.len() != 3 {
        panic!(
            "The correct signature of the service method is: fn
(&self/&mut self, RequestContext, Payload)"
        );
    }

    let payload_arg = &impl_method.sig.inputs[2];
    let pat_type = match payload_arg {
        FnArg::Typed(pat_type) => pat_type,
        _ => unreachable!(),
    };

    let payload_ident = if let Type::Path(path) = &*pat_type.ty {
        path.path.get_ident().expect("")
    } else {
        panic!("")
    };

    MethodMeta {
        method_ident: impl_method.sig.ident,
        payload_ident: payload_ident.clone(),
        readonly,
    }
}
