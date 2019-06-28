import time

import pymuta

user0 = pymuta.user0
user1 = pymuta.user1
user2 = pymuta.user2
user3 = pymuta.user3

client = pymuta.Client(pymuta.c_server)
ensure_blank_chain = 0

if ensure_blank_chain and client.get_balance(user1, '0x00') != 0x400000000000000000:
    raise Exception('Ensure you are on a blank chain')


def test_peer_count():
    r = client.peer_count()
    assert r != 0


def test_block_number():
    r0 = client.block_number()
    time.sleep(6)
    r1 = client.block_number()
    assert r1 > r0


def test_transfer_balance():
    b_user0_0 = client.get_balance(user0)
    b_user1_0 = client.get_balance(user1)
    b_user2_0 = client.get_balance(user2)

    tx = client.sign_tx(user1.private_key, user2.address, "", 10, 100000)
    client.sync_raw_transaction(tx)

    b_user0_1 = client.get_balance(user0)
    b_user1_1 = client.get_balance(user1)
    b_user2_1 = client.get_balance(user2)

    assert b_user0_1 == b_user0_0 + 21000
    assert b_user1_1 == b_user1_0 - 21000 - 10
    assert b_user2_1 == b_user2_0 + 10


def test_get_block_by_hash():
    b0 = client.get_block_by_number()
    h = b0['hash']
    b1 = client.get_block_by_hash(h)
    assert b0 == b1


def test_get_block_by_number():
    tx = client.sign_tx(user1.private_key, user2.address, "", 10, 100000)
    receipt = client.sync_raw_transaction(tx)
    block_number = receipt['blockNumber']
    tx_hash = receipt['transactionHash']

    b = client.get_block_by_number(block_number, False)
    assert len(b['body']['transactions']) == 1
    assert b['body']['transactions'][0] == tx_hash

    b = client.get_block_by_number(block_number, True)
    assert len(b['body']['transactions']) == 1
    assert b['body']['transactions'][0]['from'] == user1.address.lower()
    assert b['body']['transactions'][0]['hash'] == tx_hash


def test_get_block_include_tx_with_data():
    tx = client.sign_tx(user1.private_key, user2.address, "0x1234", 10, 100000)
    r = client.sync_raw_transaction(tx)
    block_number = r['blockNumber']
    block = client.get_block_by_number(block_number, True)

    txs = block['body']['transactions']
    assert len(txs) == 1
    # assert txs[0]['content'] == [18, 52]
    assert txs[0]['from'] == user1.address.lower()
    assert int(block['header']['quotaUsed'], 16) == 21000 + 68 + 68


def test_get_logs():
    bnb = pymuta.Bnb(client, user0)
    bnb.deploy()
    assert bnb.owner().lower() == '0x000000000000000000000000' + user0.address[2:].lower()
    assert bnb.balance_of(user0.address) == 400000000
    assert bnb.total_supply() == 400000000
    assert bnb.symbol() == b'DOUZ'

    r = bnb.transfer(user1.address, 10)
    assert bnb.balance_of(user0.address) == 400000000 - 10
    assert bnb.balance_of(user1.address) == 10

    block_number = r['blockNumber']
    logs = r['logs']  # address, blockHash, blockNumber, data, logIndex, topics

    r = client.get_logs({"fromBlock": block_number, "toBlock": block_number, "address": bnb.address})
    assert len(r) == 1
    assert r[0]['address'] == bnb.address.lower()
    assert r[0]['topics'][0] == logs[0]['topics'][0]
    assert r[0]['topics'][1] == logs[0]['topics'][1]
    assert r[0]['topics'][2] == logs[0]['topics'][2]

    r = client.get_logs({"fromBlock": block_number, "toBlock": block_number, "topics": [logs[0]['topics'][0]]})
    assert len(r) == 1
    assert r[0]['address'] == bnb.address.lower()
    assert r[0]['topics'][0] == logs[0]['topics'][0]
    assert r[0]['topics'][1] == logs[0]['topics'][1]
    assert r[0]['topics'][2] == logs[0]['topics'][2]

    r = client.get_logs({"fromBlock": block_number, "toBlock": block_number,
                         "topics": [logs[0]['topics'][0], None, logs[0]['topics'][2]]})
    assert len(r) == 1
    assert r[0]['address'] == bnb.address.lower()
    assert r[0]['topics'][0] == logs[0]['topics'][0]
    assert r[0]['topics'][1] == logs[0]['topics'][1]
    assert r[0]['topics'][2] == logs[0]['topics'][2]


def test_call():
    ss = pymuta.SimpleStorage(client, user0)
    ss.deploy()
    ss.set(42)
    assert ss.get() == '0x000000000000000000000000000000000000000000000000000000000000002a'
    ss.set(15)
    assert ss.get() == '0x000000000000000000000000000000000000000000000000000000000000000f'


def test_get_transaction():
    tx = client.sign_tx(user1.private_key, user2.address, "", 10, 100000)
    receipt = client.sync_raw_transaction(tx)
    h = receipt['transactionHash']
    tx = client.get_transaction(h)
    assert tx['hash'] == h
    assert tx['from'] == user1.address.lower()


def test_get_code():
    ss = pymuta.SimpleStorage(client, user0)
    ss.deploy()
    assert len(client.get_code(ss.address)) == 223 * 2 + 2


def test_get_abi():
    pass


def test_get_block_header():
    client.get_block_header()


def test_get_storage_at():
    ss = pymuta.SimpleStorage(client, user0)
    ss.deploy()
    ss.set(42)
    v = client.get_storage_at(ss.address, '0x0000000000000000000000000000000000000000000000000000000000000000')
    assert v == '0x000000000000000000000000000000000000000000000000000000000000002a'


def test_get_transaction_count():
    pre = client.get_transaction_count(user1.address, 'latest')
    tx = client.sign_tx(user1.private_key, user2.address, "", 10, 100000)
    client.sync_raw_transaction(tx)
    new = client.get_transaction_count(user1.address, 'latest')
    assert new == pre + 1


def test_get_state_proof():
    # Not exists
    r = client.get_state_proof(user0.address, '0x0000000000000000000000000000000000000000000000000000000000000000')
    assert r['message'] == 'StateProofNotFoundError'

    # Exists
    ss = pymuta.SimpleStorage(client, user0)
    ss.deploy()
    ss.set(1)
    r = client.get_state_proof(ss.address, '0x0000000000000000000000000000000000000000000000000000000000000001')
    assert r


def test_get_transaction_proof():
    tx = client.sign_tx(user1.private_key, user2.address, "", 10, 100000)
    r = client.sync_raw_transaction(tx)
    h = r['transactionHash']
    time.sleep(6)
    assert client.get_transaction_proof(h)


def test_get_filter_block():
    filter_id = client.new_block_filter()
    time.sleep(6)
    r0 = client.get_filter_changes(filter_id)
    assert r0
    time.sleep(6)
    r1 = client.get_filter_changes(filter_id)
    assert r1
    assert r0 != r1


def test_get_filter():
    bnb = pymuta.Bnb(client, user0)
    bnb.deploy()

    for data in [
        {"fromBlock": "0x00", "toBlock": "latest", "address": bnb.address},
        {"address": bnb.address},
    ]:
        filter_id = client.new_filter(data)

        balance_user0 = bnb.balance_of(user0.address)
        balance_user1 = bnb.balance_of(user1.address)
        r = bnb.transfer(user1.address, 10)
        assert bnb.balance_of(user0.address) == balance_user0 - 10
        assert bnb.balance_of(user1.address) == balance_user1 + 10

        block_number = r['blockNumber']
        logs = r['logs']  # address, blockHash, blockNumber, data, logIndex, topics

        r = client.get_filter_changes(filter_id)
        assert r[0]['address'] == bnb.address.lower()
        assert r[0]['blockHash'] == logs[0]['blockHash']
        assert r[0]['blockNumber'] == logs[0]['blockNumber']


if __name__ == '__main__':
    test_peer_count()
    test_block_number()
    test_transfer_balance()
    test_get_block_by_hash()
    test_get_block_by_number()
    test_get_block_include_tx_with_data()
    test_get_logs()
    test_call()
    test_get_transaction()
    test_get_code()
    test_get_abi()
    test_get_block_header()
    test_get_storage_at()
    test_get_transaction_count()
    test_get_state_proof()
    test_get_transaction_proof()
    test_get_filter_block()
    test_get_filter()
