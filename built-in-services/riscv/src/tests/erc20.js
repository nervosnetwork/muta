function assert(condition, msg) {
    if (!condition) {
        throw msg;
    }
}

function init(name, symbol, supply) {
    if (!PVM.is_init()) {
        throw "init can only be invoked by deploy function";
    }
    PVM.set_storage('name', name);
    PVM.set_storage('symbol', symbol);
    PVM.set_storage('supply', supply.toString());
    // set caller balance to supply
    const caller = PVM.get_caller();
    _set_balance(caller, supply);
}

function total_supply() {
    return PVM.get_storage('supply');
}

function _set_balance(account, amount) {
    PVM.set_storage('balance:' + account, amount.toString());
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
    _transfer(PVM.get_caller(), recipient, amount);
}

function balance_of(account) {
    return PVM.get_storage('balance:' + account) || '0';
}

function _approve(owner, spender, amount) {
    PVM.set_storage('allowances:' + owner + spender, amount.toString());
}

function approve(spender, amount) {
    _approve(PVM.get_caller(), spender, amount);
}

function allowances(owner, spender) {
    return PVM.get_storage('allowances:' + owner + spender) || '0';
}

function transfer_from(sender, recipient, amount) {
    const caller = PVM.get_caller();
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

// function main() {
//     const args = JSON.parse(PVM.load_args());
//     return _main(args);
// }
// PVM.ret(main() || '');

// -------- test ----------------
// run with below code via duktape on local machine

PVM = {
    storage: {},
    ret: "",
    caller: "",
    debug: function(msg) {
        console.log('[ckb-vm]', msg);
    },
    set_storage: function(key, value) {
        this.storage[key] = value;
    },
    get_storage: function(key) {
        return this.storage[key];
    },
    is_init: function() {
        return true;
    },
    ret: function(msg) {
        this.ret = msg;
    },
    _clear_ret: function() {
        this.ret = "";
    },
    get_caller: function() {
        return this.caller;
    },
    _set_caller: function(caller) {
        this.caller = caller;
    }
}

function call_and_print(args, caller) {
    PVM._clear_ret();
    caller = caller || '755cdba6ae4f479f7164792b318b2a06c759833b';
    PVM._set_caller(caller);
    PVM.debug('[' + args.method + ']: ' + _main(args) || '');
}

function test() {
    PVM.debug('-------- start  erc20 test --------');
    // init
    call_and_print({method: 'init', name: 'bitcoin', symbol: 'BTC', supply: 10000000000});
    call_and_print({method: 'total_supply'});
    call_and_print({method: 'balance_of', account: '755cdba6ae4f479f7164792b318b2a06c759833b'});
    call_and_print({method: 'balance_of', account: '0000000000000000000000000000000000000000'});
    call_and_print({method: 'transfer', recipient: '0000000000000000000000000000000000000000', amount: 1000});
    call_and_print({method: 'balance_of', account: '755cdba6ae4f479f7164792b318b2a06c759833b'});
    call_and_print({method: 'balance_of', account: '0000000000000000000000000000000000000000'});

    call_and_print({method: 'allowances', owner: '755cdba6ae4f479f7164792b318b2a06c759833b', spender: '0000000000000000000000000000000000000000'});
    call_and_print({method: 'approve', spender: '0000000000000000000000000000000000000000', amount: 600});
    call_and_print({method: 'allowances', owner: '755cdba6ae4f479f7164792b318b2a06c759833b', spender: '0000000000000000000000000000000000000000'});
    // allowances not enough
    // call_and_print({method: 'transfer_from', sender: '755cdba6ae4f479f7164792b318b2a06c759833b', recipient: '0000000000000000000000000000000000000001', amount: 601}, '0000000000000000000000000000000000000000');
    call_and_print({method: 'transfer_from', sender: '755cdba6ae4f479f7164792b318b2a06c759833b', recipient: '0000000000000000000000000000000000000001', amount: 500}, '0000000000000000000000000000000000000000');
    call_and_print({method: 'allowances', owner: '755cdba6ae4f479f7164792b318b2a06c759833b', spender: '0000000000000000000000000000000000000000'});
    call_and_print({method: 'balance_of', account: '755cdba6ae4f479f7164792b318b2a06c759833b'});
    call_and_print({method: 'balance_of', account: '0000000000000000000000000000000000000000'});
    call_and_print({method: 'balance_of', account: '0000000000000000000000000000000000000001'});

    // balance not enough
    // call_and_print({method: 'transfer', recipient: '0000000000000000000000000000000000000000', amount: 1000}, '0000000000000000000000000000000000000001');
    // console.log(PVM.storage);

    PVM.debug('-------- finish erc20 test --------');
}

test();