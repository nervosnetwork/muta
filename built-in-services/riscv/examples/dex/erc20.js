function assert(condition, msg) {
    if (!condition) {
        throw new Error(msg);
    }
}

function print(obj, name) {
    var obj_str = JSON.stringify(obj);
    PVM.debug(name || 'obj', ':', obj_str);
}

function init(name, symbol, supply) {
    if (!PVM.is_init()) {
        throw "init can only be invoked by deploy function";
    }
    PVM.set_storage('name', name);
    PVM.set_storage('symbol', symbol);
    PVM.set_storage('supply', supply.toString());
    // set caller balance to supply
    const caller = PVM.caller();
    _set_balance(caller, supply);
}

function total_supply() {
    return PVM.get_storage('supply');
}

function _balance_key(account) {
    return 'balance:' + account;
}

function _set_balance(account, amount) {
    const key = _balance_key(account);
    PVM.set_storage(key, amount.toString());
}

function _transfer(sender, recipient, amount) {
    if (amount <= 0) {
        throw 'amount must be positive';
    }
    const from_balance = parseInt(balance_of(sender));
    const to_balance = parseInt(balance_of(recipient));
    // TODO: change to some kind of safe math
    from_balance -= amount;
    assert(from_balance > 0, 'balance not enough');
    to_balance += amount;
    _set_balance(sender, from_balance);
    _set_balance(recipient, to_balance);
}

function transfer(recipient, amount) {
    const caller = PVM.caller();
    print({caller, recipient, amount}, 'transfer');
    _transfer(caller, recipient, amount);
}

function balance_of(account) {
    account = account || PVM.caller();
    const key = _balance_key(account);
    const ret = PVM.get_storage(key);
    return ret || '0';
}

function _approve(owner, spender, amount) {
    PVM.set_storage('allowances:' + owner + ':' + spender, amount.toString());
}

function approve(spender, amount) {
    _approve(PVM.caller(), spender, amount);
}

function allowances(owner, spender) {
    return PVM.get_storage('allowances:' + owner + ':' + spender) || '0';
}

function transfer_from(sender, recipient, amount) {
    const caller = PVM.caller();
    const before_allowance = parseInt(allowances(sender, caller));
    const after_allowance = before_allowance - amount;
    if (after_allowance < 0) {
        throw 'allowances not enough';
    }
    _transfer(sender, recipient, amount);
    _approve(sender, caller, after_allowance);
}

function _main(args) {
    if (args.method == 'init') {
        init(args.name, args.symbol, args.supply);
    } else if (args.method == 'total_supply') {
        return total_supply();
    } else if (args.method == 'balance_of') {
        return balance_of(args.account);
    } else if (args.method == 'transfer') {
        return transfer(args.recipient, args.amount);
    } else if (args.method == 'allowances') {
        return allowances(args.owner, args.spender);
    } else if (args.method == 'approve') {
        return approve(args.spender, args.amount);
    } else if (args.method == 'transfer_from') {
        return transfer_from(args.sender, args.recipient, args.amount);
    } else {
        throw 'method not found';
    }
}

function main() {
    const args = JSON.parse(PVM.load_args());
    PVM.debug(JSON.stringify(args));
    return _main(args) || '';
}
