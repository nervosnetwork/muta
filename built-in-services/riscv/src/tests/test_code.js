function main() {
  'use strict';

  PVM.debug(PVM.is_init());

  PVM.debug("hello! it's me, mario");
  const args = PVM.load_json_args();

  PVM.set_storage('carmen', 'red');
  PVM.debug('carmen ' + PVM.get_storage('carmen'));
  PVM.debug('cycle limit ' + PVM.cycle_limit());

  const addr = 'cea3d2319b3caa8643942fda60da00f49a693f5e';
  const call_args = '1133';
  PVM.debug(PVM.contract_call(addr, call_args));

  return JSON.stringify({
    ret: args.x + args.y
  });
}
