macro_rules! def_const {
    ($module:ident, $name:ident) => {
        pub mod $module {
            use mashup::*;

            mashup! {
                def["server_port"] = $name _SERVER_PORT;
                def["server_host"] = $name _SERVER_HOST;
                def["client_host"] = $name _CLIENT_HOST;
                def["client_port"] = $name _CLIENT_PORT;
                def["server_threads"] = $name _SERVER_THREADS;
            }

            def! {
                pub const "server_host": &str = stringify!("server_host");
                pub const "server_port": &str = stringify!("server_port");
                pub const "client_host": &str = stringify!("client_host");
                pub const "client_port": &str = stringify!("client_port");
                pub const "server_threads": &str = stringify!("server_threads");
            }

        }

        pub use self::$module::*;
    };
}

def_const!(pool, POOL);
def_const!(chain, CHAIN);
def_const!(consensus, CONSENSUS);
def_const!(network, NETWORK);
def_const!(sync, SYNC);
def_const!(executor, EXECUTOR);
