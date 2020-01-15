function main() {
  PVM.debug("hello! it's me, mario");
  PVM.load_json_args();

  var txt_enc = new TextEncoder();
  var txt_dec = new TextDecoder('utf8');

  var test_key = txt_enc.encode('carmen');
  var test_val = txt_enc.encode('red');

  PVM.set_storage(test_key, test_val);

  PVM.debug('carmen ' + txt_dec.decode(PVM.get_storage(test_key)));

  PVM.debug('cycle limit ' + PVM.cycle_limit());

  return JSON.stringify({
    ret: ARGS.x + ARGS.y
  });
}
