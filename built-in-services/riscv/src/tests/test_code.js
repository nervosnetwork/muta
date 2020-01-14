function (args) {
  PVM.debug("hello! it's me, mario");

  return JSON.stringify({
    ret: args.x + args.y
  });
};
