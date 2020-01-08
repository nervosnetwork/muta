use syn::{FnArg, Pat, Path, PathArguments, Type};

// expect fn foo() -> ProtocolResult<T>
pub fn get_protocol_result_args(path: &Path) -> Option<&PathArguments> {
    // ::<a>::<b>
    if path.leading_colon.is_some() {
        return None;
    }

    // ProtocolResult<T>
    if path.segments.len() == 1 && path.segments[0].ident == "ProtocolResult" {
        return Some(&path.segments[0].arguments);
    }

    None
}

pub fn get_request_context_pat(bound_name: &str, fn_arg: &FnArg) -> Option<Pat> {
    if let FnArg::Typed(pat_type) = &*fn_arg {
        if let Type::Path(type_path) = &*pat_type.ty {
            if path_is_request_context(&type_path.path, &bound_name) {
                return Some(*pat_type.pat.clone());
            }
        }
    }

    None
}

fn path_is_request_context(path: &Path, bound_name: &str) -> bool {
    // ::<a>::<b>
    if path.leading_colon.is_some() {
        return false;
    }

    // RequestContext
    path.segments.len() == 1 && path.segments[0].ident == bound_name
}

pub fn assert_type_servicecontext(ty: &Type) {
    match ty {
        Type::Path(ty_path) => {
            let path = &ty_path.path;
            assert_eq!(path.leading_colon.is_none(), true);
            assert_eq!(path.segments.len(), 1);
            assert_eq!(path.segments[0].ident, "ServiceContext")
        }
        _ => panic!("The type should be `ServiceContext"),
    }
}
