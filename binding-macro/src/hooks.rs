use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, FnArg, ImplItemMethod};

use crate::common::{arg_is_mutable_receiver, assert_reference_type};

pub fn verify_hook(item: TokenStream) -> TokenStream {
    let method_item = parse_macro_input!(item as ImplItemMethod);

    let inputs = &method_item.sig.inputs;
    assert_eq!(inputs.len(), 2);

    assert!(arg_is_mutable_receiver(&inputs[0]));

    match &inputs[1] {
        FnArg::Typed(pt) => {
            let ty = pt.ty.as_ref();
            assert_reference_type(ty, "ExecutorParams")
        }
        _ => panic!("The second parameter type should be `&ExecutorParams`."),
    }

    TokenStream::from(quote! {#method_item})
}
