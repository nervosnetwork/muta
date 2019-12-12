use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::{
    parse_macro_input, Block, FnArg, Generics, Ident, ImplItemMethod, ItemFn, LitInt, Pat,
    ReturnType, Token, Visibility,
};

use crate::common::{get_bounds_name_of_request_context, get_request_context_pat};

#[derive(Debug)]
struct Cycles {
    value: u64,
}

impl Parse for Cycles {
    fn parse(input: ParseStream) -> Result<Self> {
        let lit: LitInt = input.parse()?;
        let value = lit.base10_parse::<u64>()?;
        Ok(Self { value })
    }
}

struct CyclesFnItem {
    pub func_name: Ident,
    pub func_vis:  Visibility,
    pub inputs:    Punctuated<FnArg, Token![,]>,
    pub ret:       ReturnType,
    pub body:      Block,
    pub generics:  Generics,
}

impl Parse for CyclesFnItem {
    fn parse(input: ParseStream) -> Result<Self> {
        match input.parse::<ImplItemMethod>() {
            Ok(method_item) => Ok(CyclesFnItem {
                func_name: method_item.sig.ident.clone(),
                func_vis:  method_item.vis.clone(),
                inputs:    method_item.sig.inputs.clone(),
                ret:       method_item.sig.output.clone(),
                body:      method_item.block.clone(),
                generics:  method_item.sig.generics,
            }),
            Err(_) => {
                let item = input.parse::<ItemFn>()?;
                Ok(CyclesFnItem {
                    func_name: item.sig.ident.clone(),
                    func_vis:  item.vis.clone(),
                    inputs:    item.sig.inputs.clone(),
                    ret:       item.sig.output.clone(),
                    body:      *item.block.clone(),
                    generics:  item.sig.generics,
                })
            }
        }
    }
}

pub fn gen_cycles_code(attr: TokenStream, item: TokenStream) -> TokenStream {
    let cycles = parse_macro_input!(attr as Cycles);
    let fn_item = parse_macro_input!(item as CyclesFnItem);

    let func_name = &fn_item.func_name;
    let func_vis = &fn_item.func_vis;
    let inputs = &fn_item.inputs;
    let ret = &fn_item.ret;
    let body = &fn_item.body;
    let generics = &fn_item.generics;

    // Extract the name of the trait bound.
    // eg. fn <Context: RequestContext> The bound name is Context.
    let request_bound_name = get_bounds_name_of_request_context(generics)
        .expect("The bound for RequestContext could not be found");

    let request_pat = find_request_ident(&request_bound_name, inputs)
        .expect("The first parameter to read/write must be RequestContext");

    // Extract the variable name of the RequestContext.
    let request_ident = match request_pat {
        Pat::Ident(pat_ident) => pat_ident.ident,
        _ => panic!("Make sure the RequestContext declaration is ctx: RequestContext."),
    };

    let cycles_value = cycles.value;

    TokenStream::from(quote! {
        #func_vis fn #func_name#generics(#inputs) #ret {
            #request_ident.sub_cycles(#cycles_value)?;
            #body
        }
    })
}

fn find_request_ident(bound_name: &str, inputs: &Punctuated<FnArg, Token![,]>) -> Option<Pat> {
    for fn_arg in inputs {
        let opt_request_pat = get_request_context_pat(bound_name, &fn_arg);
        if opt_request_pat.is_some() {
            return opt_request_pat;
        }
    }

    None
}
