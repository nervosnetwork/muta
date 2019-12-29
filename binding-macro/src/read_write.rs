use proc_macro::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{
    parse_macro_input, FnArg, Generics, ImplItemMethod, ReturnType, Token, Type, Visibility,
};

use crate::common::{
    arg_is_request_context, get_bounds_name_of_request_context, get_protocol_result_args,
};

pub fn verify_read_or_write(item: TokenStream, mutable: bool) -> TokenStream {
    let method_item = parse_macro_input!(item as ImplItemMethod);

    let visibility = &method_item.vis;
    let inputs = &method_item.sig.inputs;
    let generics = &method_item.sig.generics;
    let ret_type = &method_item.sig.output;

    verify_visibiity(visibility);

    verify_inputs(inputs, generics, mutable);

    verify_ret_type(ret_type);

    TokenStream::from(quote! {#method_item})
}

fn verify_visibiity(visibility: &Visibility) {
    match visibility {
        Visibility::Inherited => {}
        _ => panic!("The visibility of read/write method must be private"),
    };
}

fn verify_inputs(inputs: &Punctuated<FnArg, Token![,]>, generics: &Generics, mutable: bool) {
    if inputs.len() < 2 {
        panic!("The two required parameters are missing: `&self/&mut self` and `RequestContext`.")
    }

    if mutable {
        if !arg_is_mutable_receiver(&inputs[0]) {
            panic!("The receiver must be `&mut self`.")
        }
    } else if !arg_is_inmutable_receiver(&inputs[0]) {
        panic!("The receiver must be `&self`.")
    }

    let request_bound_name = get_bounds_name_of_request_context(generics).expect("");
    if !arg_is_request_context(&inputs[1], &request_bound_name) {
        panic!("The first parameter to read/write must be RequestContext")
    }
}

fn verify_ret_type(ret_type: &ReturnType) {
    let real_ret_type = match ret_type {
        ReturnType::Type(_, t) => t.as_ref(),
        _ => panic!("The return type of read/write method must be protocol::ProtocolResult"),
    };

    match real_ret_type {
        Type::Path(type_path) => {
            let path = &type_path.path;
            get_protocol_result_args(&path)
                .expect("The return type of read/write method must be protocol::ProtocolResult");
        }
        _ => panic!("The return type of read/write method must be protocol::ProtocolResult"),
    }
}

// expect &mut self
fn arg_is_mutable_receiver(fn_arg: &FnArg) -> bool {
    match fn_arg {
        FnArg::Receiver(receiver) => receiver.reference.is_some() && receiver.mutability.is_some(),
        _ => false,
    }
}

// expect &self
fn arg_is_inmutable_receiver(fn_arg: &FnArg) -> bool {
    match fn_arg {
        FnArg::Receiver(receiver) => receiver.reference.is_some() && receiver.mutability.is_none(),
        _ => false,
    }
}
