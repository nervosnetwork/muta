function _test_init() {
  const args = PVM.load_args();
  return args;
}

function _test_load_args() {
  const raw_args = PVM.load_args();
  return JSON.stringify(JSON.parse(raw_args));
}

function _test_load_json_args() {
  return JSON.stringify(PVM.load_json_args());
}

function _test_caller() {
  return PVM.caller();
}

function _ret_caller_and_origin() {
  return JSON.stringify({
    caller: PVM.caller(),
    origin: PVM.origin()
  });
}

function _test_origin() {
  const args = PVM.load_json_args();
  return PVM.contract_call(args.address, args.call_args);
}

function _test_address() {
  return PVM.address();
}

function _test_cycle_limit() {
  return PVM.cycle_limit().toString();
}

function _test_cycle_used() {
  return PVM.cycle_used().toString();
}

function _test_cycle_price() {
  return PVM.cycle_price().toString();
}

function _test_block_height() {
  return PVM.block_height().toString();
}

function _test_extra() {
  return PVM.extra();
}

function _test_no_extra() {
  if (PVM.extra() == null) {
    return 'no extra';
  }
}

function _test_timestamp() {
  return PVM.timestamp().toString();
}

function _test_emit_event() {
  const args = PVM.load_json_args();
  PVM.emit_event(args.msg);
  return 'emit success';
}

function _test_tx_hash() {
  return PVM.tx_hash();
}

function _test_no_tx_hash() {
  if (PVM.tx_hash() == null) {
    return 'no tx hash';
  }
}

function _test_tx_nonce() {
  return PVM.tx_nonce();
}

function _test_no_tx_nonce() {
  if (PVM.tx_nonce() == null) {
    return 'no tx nonce';
  }
}

function _test_storage() {
  const args = PVM.load_json_args();
  PVM.set_storage(args.key, args.val);
  return JSON.stringify(JSON.parse(PVM.get_storage(args.key)));
}

function _ret_self() {
  return 'self';
}

function _test_contract_call() {
  const args = PVM.load_json_args();
  return PVM.contract_call(args.address, args.call_args);
}

function _test_service_call() {
  const args = PVM.load_json_args();
  return PVM.service_call(
    args.call_service,
    args.call_method,
    args.call_payload
  );
}

function main() {
  'use strict';

  if (PVM.is_init()) {
    return _test_init();
  }

  const args = PVM.load_json_args();

  if (args.method == 'test_load_args') {
    return _test_load_args();
  } else if (args.method == 'test_load_json_args') {
    return _test_load_json_args();
  } else if (args.method == 'test_caller') {
    return _test_caller();
  } else if (args.method == 'test_origin') {
    return _test_origin();
  } else if (args.method == 'test_address') {
    return _test_address();
  } else if (args.method == 'test_cycle_limit') {
    return _test_cycle_limit();
  } else if (args.method == 'test_cycle_used') {
    return _test_cycle_used();
  } else if (args.method == 'test_cycle_price') {
    return _test_cycle_price();
  } else if (args.method == 'test_block_height') {
    return _test_block_height();
  } else if (args.method == 'test_extra') {
    return _test_extra();
  } else if (args.method == 'test_no_extra') {
    return _test_no_extra();
  } else if (args.method == 'test_timestamp') {
    return _test_timestamp();
  } else if (args.method == 'test_emit_event') {
    return _test_emit_event();
  } else if (args.method == 'test_tx_hash') {
    return _test_tx_hash();
  } else if (args.method == 'test_no_tx_hash') {
    return _test_no_tx_hash();
  } else if (args.method == 'test_tx_nonce') {
    return _test_tx_nonce();
  } else if (args.method == 'test_no_tx_nonce') {
    return _test_no_tx_nonce();
  } else if (args.method == 'test_storage') {
    return _test_storage();
  } else if (args.method == 'test_contract_call') {
    return _test_contract_call();
  } else if (args.method == 'test_service_call') {
    return _test_service_call();
  } else if (args.method == '_ret_caller_and_origin') {
    return _ret_caller_and_origin();
  } else if (args.method == '_ret_self') {
    return _ret_self();
  }

  return '';
}
