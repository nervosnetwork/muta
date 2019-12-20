use protocol::traits::RequestContext;

use crate::request_context::DefaultRequestContext;
use crate::tests::sdk::mock_address;

#[test]
fn test_request_context() {
    let mut ctx = DefaultRequestContext::new(
        100,
        8,
        10,
        mock_address(),
        1,
        "service_name".to_owned(),
        "service_method".to_owned(),
        "service_payload".to_owned(),
    );

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
