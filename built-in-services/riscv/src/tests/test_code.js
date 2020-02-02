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

function _test_block_height() {
  return PVM.block_height().toString();
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
  } else if (args.method == 'test_block_height') {
    return _test_block_height();
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
