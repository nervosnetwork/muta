use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, FnArg, Ident, ImplItem, ImplItemMethod, ItemImpl, Type};

const READ_ATTRIBUTE: &str = "read";
const WRITE_ATTRIBUTE: &str = "write";
const GENESIS_ATTRIBUTE: &str = "genesis";
const HOOK_BEFORE_ATTRIBUTE: &str = "hook_before";
const HOOK_AFTER_ATTRIBUTE: &str = "hook_after";

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
    payload_ident: Option<Ident>,
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

    let genesis_method = find_genesis(items);
    let genesis_body = match genesis_method {
        Some(genesis_method) => get_genesis_body(&genesis_method),
        None => quote! {Ok(())},
    };

    let hooks = extract_hooks(items);
    let hook_before = &hooks.before;
    let hook_before_body = match hook_before {
        Some(hook_before) => quote! { self.#hook_before(_params) },
        None => quote! {Ok(())},
    };
    let hook_after = &hooks.after;
    let hook_after_body = match hook_after {
        Some(hook_after) => quote! { self.#hook_after(_params) },
        None => quote! {Ok(())},
    };

    let list_method_meta: Vec<MethodMeta> = methods.into_iter().map(extract_method_meta).collect();

    let (list_read_name, list_read_ident, list_read_payload) =
        split_list_for_metadata(&list_method_meta, true);
    let (list_write_name, list_write_ident, list_write_payload) =
        split_list_for_metadata(&list_method_meta, false);

    let (list_read_name_nonepayload, list_read_ident_nonepayload) =
        split_list_for_metadata_nonepayload(&list_method_meta, true);
    let (list_write_name_nonepayload, list_write_ident_nonepayload) =
        split_list_for_metadata_nonepayload(&list_method_meta, false);

    TokenStream::from(quote! {
        impl #impl_generics protocol::traits::Service for #service_ident #ty_generics #where_clause {
            fn genesis_(&mut self, _payload: String) -> protocol::ProtocolResult<()> {
                #genesis_body
            }

            fn hook_before_(&mut self, _params: &ExecutorParams) -> protocol::ProtocolResult<()> {
                #hook_before_body
            }

            fn hook_after_(&mut self, _params: &ExecutorParams) -> protocol::ProtocolResult<()> {
                #hook_after_body
            }

            fn read_(&self, ctx: protocol::types::ServiceContext) -> protocol::ProtocolResult<String> {
                let service = ctx.get_service_name();
                let method = ctx.get_service_method();

                match method {
                    #(#list_read_name => {
                        let payload: #list_read_payload = serde_json::from_str(ctx.get_payload())
                                .map_err(|e| protocol::traits::BindingMacroError::JsonParse(e))?;
                        let res = self.#list_read_ident(ctx, payload)?;
                        let res_str = serde_json::to_string(&res).map_err(|e| protocol::traits::BindingMacroError::JsonParse(e))?;
                        if res_str.as_str() == "null" {
                            Ok("".to_owned())
                        } else {
                            Ok(res_str)
                        }
                    },)*
                    #(#list_read_name_nonepayload => {
                        let res = self.#list_read_ident_nonepayload(ctx)?;
                        let res_str = serde_json::to_string(&res).map_err(|e| protocol::traits::BindingMacroError::JsonParse(e))?;
                        if res_str.as_str() == "null" {
                            Ok("".to_owned())
                        } else {
                            Ok(res_str)
                        }
                    },)*
                    _ => Err(protocol::traits::BindingMacroError::NotFoundMethod{ service: service.to_owned(), method: method.to_owned() }.into())
                }
            }

            fn write_(&mut self, ctx: protocol::types::ServiceContext) -> protocol::ProtocolResult<String> {
                let service = ctx.get_service_name();
                let method = ctx.get_service_method();

                match method {
                    #(#list_write_name => {
                        let payload: #list_write_payload = serde_json::from_str(ctx.get_payload())
                                .map_err(|e| protocol::traits::BindingMacroError::JsonParse(e))?;
                        let res = self.#list_write_ident(ctx, payload)?;
                        let res_str = serde_json::to_string(&res).map_err(|e| protocol::traits::BindingMacroError::JsonParse(e))?;
                        if res_str.as_str() == "null" {
                            Ok("".to_owned())
                        } else {
                            Ok(res_str)
                        }
                    },)*
                    #(#list_write_name_nonepayload => {
                        let res = self.#list_write_ident_nonepayload(ctx)?;
                        let res_str = serde_json::to_string(&res).map_err(|e| protocol::traits::BindingMacroError::JsonParse(e))?;
                        if res_str.as_str() == "null" {
                            Ok("".to_owned())
                        } else {
                            Ok(res_str)
                        }
                    },)*
                    _ => Err(protocol::traits::BindingMacroError::NotFoundMethod{ service: service.to_owned(), method: method.to_owned() }.into())
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
        .filter(|meta| meta.readonly == readonly && meta.payload_ident.is_some())
        .for_each(|meta| {
            methods.push(meta.method_ident.to_string());
            method_idents.push(meta.method_ident.clone());
            payload_idents.push(
                meta.payload_ident
                    .as_ref()
                    .expect("MethodMeta should have payload ident")
                    .clone(),
            );
        });
    (methods, method_idents, payload_idents)
}

fn split_list_for_metadata_nonepayload(
    list: &[MethodMeta],
    readonly: bool,
) -> (Vec<String>, Vec<Ident>) {
    let mut methods = vec![];
    let mut method_idents = vec![];

    list.iter()
        .filter(|meta| meta.readonly == readonly && meta.payload_ident.is_none())
        .for_each(|meta| {
            methods.push(meta.method_ident.to_string());
            method_idents.push(meta.method_ident.clone());
        });
    (methods, method_idents)
}

fn get_service_ident(impl_item: &ItemImpl) -> Ident {
    match &*impl_item.self_ty {
        Type::Path(type_path) => type_path.path.segments[0].ident.clone(),
        _ => panic!("The identity of the service was not found."),
    }
}

fn find_service_method(method: &ImplItemMethod) -> Option<ServiceMethod> {
    let attrs = &method.attrs;

    for attr in attrs {
        for segment in &attr.path.segments {
            if segment.ident == READ_ATTRIBUTE {
                return Some(ServiceMethod::Read(method.clone()));
            } else if segment.ident == WRITE_ATTRIBUTE {
                return Some(ServiceMethod::Write(method.clone()));
            }
        }
    }

    None
}

fn find_genesis(items: &[ImplItem]) -> Option<ImplItemMethod> {
    let methods: Vec<ImplItemMethod> = find_list_for_item_method(items);

    let mut count = 0;
    let mut genesis: Option<ImplItemMethod> = None;

    for method in methods {
        for attr in &method.attrs {
            for segment in &attr.path.segments {
                if segment.ident == GENESIS_ATTRIBUTE {
                    if count == 0 {
                        genesis = Some(method.clone());
                        count = 1;
                    } else {
                        panic!("The genesis method can only have one")
                    }
                }
            }
        }
    }

    genesis
}

fn get_genesis_body(item: &ImplItemMethod) -> proc_macro2::TokenStream {
    let method_name = item.sig.ident.clone();
    match item.sig.inputs.len() {
        1 => quote!{ self.#method_name()},
        2 => {
                let payload_arg = &item.sig.inputs[1];
                let pat_type = match payload_arg {
                    FnArg::Typed(pat_type) => pat_type,
                    _ => unreachable!(),
                };

                let payload_ident = if let Type::Path(path) = &*pat_type.ty {
                    Some(path.path.get_ident().expect("No payload type found.").clone())
                } else {
                    panic!("No payload type found.")
                };

                quote!{
                    let payload: #payload_ident = serde_json::from_str(&_payload)
                                    .map_err(|e| protocol::traits::BindingMacroError::JsonParse(e))?;
                    self.#method_name(payload)
                }
        },
        _ => panic!("genesis method input params should be `(&mut self)` or `(&mut self, payload: PayloadType)`")
    }
}

fn extract_hooks(items: &[ImplItem]) -> Hooks {
    let methods: Vec<ImplItemMethod> = find_list_for_item_method(items);

    let mut hooks = Hooks {
        before: None,
        after:  None,
    };

    let mut before_count = 0;
    let mut after_count = 0;

    for method in methods {
        for attr in &method.attrs {
            for segment in &attr.path.segments {
                if segment.ident == HOOK_BEFORE_ATTRIBUTE {
                    if before_count == 0 {
                        hooks.before = Some(method.sig.ident.clone());
                        before_count = 1;
                    } else {
                        panic!("The before hook can only have one")
                    }
                } else if segment.ident == HOOK_AFTER_ATTRIBUTE {
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

fn find_list_for_item_method(items: &[ImplItem]) -> Vec<ImplItemMethod> {
    items
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
        .collect()
}

fn extract_method_meta(method: ServiceMethod) -> MethodMeta {
    let (impl_method, readonly) = match method {
        ServiceMethod::Read(impl_method) => (impl_method, true),
        ServiceMethod::Write(impl_method) => (impl_method, false),
    };

    match &impl_method.sig.inputs.len() {
        // Method input params: `(&self/&mut self, ctx: ServiceContext)`
        2 => {
            MethodMeta {
                method_ident: impl_method.sig.ident,
                payload_ident: None,
                readonly,
            }
        },
        // Method input params: `(&self/&mut self, ctx: ServiceContext, payload: PayloadType)`
        3 => {
            let payload_arg = &impl_method.sig.inputs[2];
            let pat_type = match payload_arg {
                FnArg::Typed(pat_type) => pat_type,
                _ => unreachable!(),
            };

            let payload_ident = if let Type::Path(path) = &*pat_type.ty {
                Some(path.path.get_ident().expect("No payload type found.").clone())
            } else {
                panic!("No payload type found.")
            };

            MethodMeta {
                method_ident: impl_method.sig.ident,
                payload_ident,
                readonly,
            }
        },
        _ => panic!("Method input params should be `(&self/&mut self, ctx: ServiceContext)` or `(&self/&mut self, ctx: ServiceContext, payload: PayloadType)`")
    }
}
