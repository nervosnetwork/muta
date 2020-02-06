function print(obj, name) {
    var obj_str = JSON.stringify(obj);
    PVM.debug(name || 'obj', ':', obj_str);
}

function _balance_key(asset, account) {
    return 'balance:' + asset + ':' + account;
}
function _balance(asset, account) {
    const key = _balance_key(asset, account);
    const balance = PVM.get_storage(key);
    return balance || '0';
}

function _set_balance(asset, account, amount) {
    const key = _balance_key(asset, account);
    PVM.set_storage(key, amount.toString());
}

function deposit(asset, amount) {
    const caller = PVM.caller();
    const recipient = PVM.address();
    const args = JSON.stringify({method: 'transfer_from', sender: caller, recipient, amount});
    print({args, caller, asset, amount, recipient}, 'deposit');
    const ret = PVM.contract_call(asset, args);
    print({ret}, 'deposit');
    // TODO: safe add
    const amount_before = parseInt(_balance(asset, caller));
    const amount_after = amount_before + amount;
    print({amount_after, amount_before}, 'deposit');
    _set_balance(asset, caller, amount_after);
}

function withdraw(asset, amount) {
    const caller = PVM.caller();
    const amount_before = parseInt(_balance(asset, caller));
    const amount_after = amount_before - amount;
    if (amount_after < 0) {
        throw "balance not enough";
    }
    const args = JSON.stringify({method: 'transfer', recipient: caller, amount});
    PVM.contract_call(asset, args);
    _set_balance(asset, caller, amount_after);
}

function balance_of(args) {
    const account = args.account || PVM.caller();
    print({args, account}, 'balance_of');
    return _balance(args.asset, account);
}

function _main(args) {
    if (args.method == 'deposit') {
        deposit(args.asset, args.amount);
    } else if (args.method == 'withdraw') {
        withdraw(args.asset, args.amount);
    } else if (args.method == 'balance_of') {
        return balance_of(args);
    } else {
        throw 'method not found';
    }
}

function main() {
    const args = JSON.parse(PVM.load_args());
    PVM.debug(JSON.stringify(args));
    return _main(args) || '';
}
