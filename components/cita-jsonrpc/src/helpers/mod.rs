mod filter_manager;
mod funcs;
#[cfg(test)]
pub mod mock_storage;

pub use self::filter_manager::{FilterManager, FilterType};
pub use self::funcs::{
    get_current_height, get_height_by_block_number, get_logs, transform_data32_to_hash,
};
