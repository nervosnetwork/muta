use proc_macro::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{parse_macro_input, FnArg, ImplItemMethod, ReturnType, Token, Visibility};

use crate::common::{arg_is_immutable_receiver, arg_is_mutable_receiver, assert_type};

pub fn verify_read_or_write(item: TokenStream, mutable: bool) -> TokenStream {
    let method_item = parse_macro_input!(item as ImplItemMethod);

    let visibility = &method_item.vis;
    let inputs = &method_item.sig.inputs;
    let ret_type = &method_item.sig.output;

    verify_visibiity(visibility);

    verify_inputs(inputs, mutable);

    verify_ret_type(ret_type);

    TokenStream::from(quote! {#method_item})
}

fn verify_visibiity(visibility: &Visibility) {
    match visibility {
        Visibility::Inherited => {}
        _ => panic!("The visibility of read/write method must be private"),
    };
}

fn verify_inputs(inputs: &Punctuated<FnArg, Token![,]>, mutable: bool) {
    if inputs.len() < 2 || inputs.len() > 3 {
        panic!("The input parameters should be `(&self/&mut self, ctx: ServiceContext)` or `(&self/&mut self, ctx: ServiceContext, payload: PayloadType)`")
    }

    if mutable {
        if !arg_is_mutable_receiver(&inputs[0]) {
            panic!("The receiver must be `&mut self`.")
        }
    } else if !arg_is_immutable_receiver(&inputs[0]) {
        panic!("The receiver must be `&self`.")
    }

    match &inputs[1] {
        FnArg::Typed(pt) => {
            let ty = pt.ty.as_ref();
            assert_type(ty, "ServiceContext")
        }
        _ => panic!("The second parameter type should be `ServiceContext`."),
    }
}

fn verify_ret_type(ret_type: &ReturnType) {
    let real_ret_type = match ret_type {
        ReturnType::Type(_, t) => t.as_ref(),
        _ => panic!("The return type of read/write method must be protocol::ProtocolResult"),
    };

    assert_type(&real_ret_type, "ServiceResponse");
}
