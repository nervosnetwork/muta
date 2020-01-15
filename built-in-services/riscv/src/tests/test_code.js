function main() {
  'use strict';

  PVM.debug("hello! it's me, mario");
  PVM.load_json_args();

  const txt_enc = new TextEncoder();
  const txt_dec = new TextDecoder('utf8');

  const test_key = txt_enc.encode('carmen');
  const test_val = txt_enc.encode('red');

  PVM.set_storage(test_key, test_val);
  PVM.debug('carmen ' + txt_dec.decode(PVM.get_storage(test_key)));

  PVM.debug('cycle limit ' + PVM.cycle_limit());

  const addr = 'cea3d2319b3caa8643942fda60da00f49a693f5e';
  const call_args = '1133';
  PVM.debug(PVM.contract_call(addr, call_args));

  return JSON.stringify({
    ret: ARGS.x + ARGS.y
  });
}
