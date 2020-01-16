function _test_init() {
  if (!PVM.is_init()) {
    return 'not init';
  }
  return 'init';
}

function _test_load_args() {
  const raw_args = PVM.load_args();
  return JSON.stringify(JSON.parse(raw_args));
}

function _test_load_json_args() {
  return JSON.stringify(PVM.load_json_args());
}

function _test_cycle_limit() {
  return PVM.cycle_limit().toString();
}

function _test_storage() {
  const args = PVM.load_json_args();
  PVM.set_storage(args.key, args.val);
  return JSON.stringify(JSON.parse(PVM.get_storage(args.key)));
}

function _test_contract_call() {
  const args = PVM.load_json_args();
  const ret = PVM.contract_call(args.address, args.call_args);
  PVM.debug(ret);
  return ret;
}

function main() {
  'use strict';

  if (!PVM.is_init()) {
    return _test_init();
  }

  const args = PVM.load_json_args();

  if (args.method == 'test_init') {
    return _test_init();
  } else if (args.method == 'test_load_args') {
    return _test_load_args();
  } else if (args.method == 'test_load_json_args') {
    return _test_load_json_args();
  } else if (args.method == 'test_cycle_limit') {
    return _test_cycle_limit();
  } else if (args.method == 'test_storage') {
    return _test_storage();
  } else if (args.method == 'test_contract_call') {
    return _test_contract_call();
  }

  return '';
}
