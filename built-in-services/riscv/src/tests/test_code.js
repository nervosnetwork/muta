function main() {
  PVM.debug("hello! it's me, mario");
  PVM.load_json_args();

  PVM.debug('cycle limit ' + PVM.cycle_limit());

  return JSON.stringify({
    ret: ARGS.x + ARGS.y
  });
}
