use std::cell::RefCell;
use std::rc::Rc;

use protocol::traits::RequestContext;
use protocol::types::{Address, Hash};

use crate::{ContextParams, DefaultRequestContext};

#[test]
fn test_request_context() {
    let params = ContextParams {
        cycles_limit:    100,
        cycles_price:    8,
        cycles_used:     Rc::new(RefCell::new(10)),
        caller:          Address::from_hash(Hash::from_empty()).unwrap(),
        epoch_id:        1,
        service_name:    "service_name".to_owned(),
        service_method:  "service_method".to_owned(),
        service_payload: "service_payload".to_owned(),
        events:          Rc::new(RefCell::new(vec![])),
    };
    let mut ctx = DefaultRequestContext::new(params);

    ctx.sub_cycles(8).unwrap();
    assert_eq!(ctx.get_cycles_used(), 18);

    assert_eq!(ctx.get_cycles_limit(), 100);
    assert_eq!(ctx.get_cycles_price(), 8);
    assert_eq!(ctx.get_caller(), mock_address());
    assert_eq!(ctx.get_current_epoch_id(), 1);
    assert_eq!(ctx.get_service_name(), "service_name");
    assert_eq!(ctx.get_service_method(), "service_method");
    assert_eq!(ctx.get_payload(), "service_payload");
}
