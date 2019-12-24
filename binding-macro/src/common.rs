use syn::{FnArg, GenericParam, Generics, Pat, Path, PathArguments, Type, TypeParamBound};

// expect fn foo<Context: RequestContext>
pub fn get_bounds_name_of_request_context(generics: &Generics) -> Option<String> {
    if generics.params.len() != 1 {
        return None;
    }

    let generics_type = &generics.params[0];

    if let GenericParam::Type(t) = generics_type {
        let bound_name = t.ident.to_string();

        if let TypeParamBound::Trait(bound_trait) = &t.bounds[0] {
            let ident = &bound_trait.path.segments[0].ident;
            if ident == "RequestContext" {
                return Some(bound_name);
            }
        }
    }

    None
}

// expect ctx: RequestContext
pub fn arg_is_request_context(fn_arg: &FnArg, bound_name: &str) -> bool {
    let ty = match fn_arg {
        FnArg::Typed(pat_type) => &*pat_type.ty,
        _ => return false,
    };

    match ty {
        Type::Path(type_path) => path_is_request_context(&type_path.path, &bound_name),
        _ => false,
    }
}

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
