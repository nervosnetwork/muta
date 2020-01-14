function main() {
  PVM.debug("hello! it's me, mario");
  PVM.load_json_args();

  return JSON.stringify({
    ret: ARGS.x + ARGS.y
  });
}
