macro_rules! generate_module_for {
    ([$( $name:ident, )+]) => {
        $( generate_module_for!($name); )+
    };
    ([$( $name:ident ),+]) => {
        $( generate_module_for!($name); )+
    };
    ($name:ident) => {
        pub mod $name {
            use prost_derive::*;
            include!(concat!(env!("OUT_DIR"), "/", stringify!($name), ".rs"));
        }
    };
}

generate_module_for!([blockchain, chain, common, consensus, executor, pool, sync]);
