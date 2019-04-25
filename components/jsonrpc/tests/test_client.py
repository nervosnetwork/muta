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
    assert r == 42


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
    assert b['body']['transactions'][0]['from'] == user1.address
    assert b['body']['transactions'][0]['hash'] == tx_hash


def test_get_block_include_tx_with_data():
    tx = client.sign_tx(user1.private_key, user2.address, "0x1234", 10, 100000)
    r = client.sync_raw_transaction(tx)
    block_number = r['blockNumber']
    block = client.get_block_by_number(block_number, True)

    txs = block['body']['transactions']
    assert len(txs) == 1
    assert txs[0]['content'] == [18, 52]
    assert txs[0]['from'] == '0x2ae83ce578e4bb7968104b5d7c034af36a771a35'
    assert int(block['header']['quotaUsed'], 16) == 21000 + 68 + 68


def test_get_logs():
    bnb = pymuta.Bnb(client, user0)
    # bnb.deploy()
    bnb.address = '0xf7591a8bd1c67b8159ea165ad00d9057d8aff121'
    # assert bnb.owner() == '0x00000000000000000000000019e49d3efd4e81dc82943ad9791c1916e2229138'
    # assert bnb.balance_of(user0.address) == 400000000
    # assert bnb.total_supply() == 400000000
    # assert bnb.symbol() == b'DOUZ'

    # bnb.transfer(user1.address, 10)
    # assert bnb.balance_of(user0.address) == 400000000 - 10
    # assert bnb.balance_of(user1.address) == 10

    # 'logs': [
    #     {'address': '0xf7591a8bd1c67b8159ea165ad00d9057d8aff121',
    #     'blockHash': '0xe7040aa7319abf49cf0d20674b38a0605c5fa30e4e8808d71c06b69c2aee81ae',
    #     'blockNumber': '0x80b',
    #     'data': '0x000000000000000000000000000000000000000000000000000000000000000a',
    #     'logIndex': '0x0',
    #     'topics': [
    #         '0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef',
    #         '0x00000000000000000000000019e49d3efd4e81dc82943ad9791c1916e2229138',
    #         '0x0000000000000000000000002ae83ce578e4bb7968104b5d7c034af36a771a35'],

    #         'transactionHash': '0x4dad5a889eeeedf60bd415b223ee319dd1e1b99f9f601b13f507f4db2ffd0b0e',
    #         'transactionIndex': '0x0', 'transactionLogIndex': '0x0'}],
    #         'logsBloom': '0x0000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000
    #         000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000000
    #         000000000000000000000000000000000000000000000000000000000000000040000000000000000000000800000000000000000000
    #         000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000000000000000
    #         0000000008000000000000000000000000000000000000000000000000000000000000000000000000000000000000001'

    # self.client.get_logs
    # {"topics":["0x8fb1356be6b2a4e49ee94447eb9dcb8783f51c41dcddfe7919f945017d163bf3"],"fromBlock": "0x0"}

    client.get_logs({"topics": [], "fromBlock": "0x0", "toBlock": "latest"})


def test_call():
    ss = pymuta.SimpleStorage(client, user0)
    ss.deploy()
    ss.set(42)
    assert ss.get() == '0x000000000000000000000000000000000000000000000000000000000000002a'
    ss.set(15)
    assert ss.get() == '0x000000000000000000000000000000000000000000000000000000000000000f'


def test_get_transaction():
    pass


def test_get_code():
    ss = pymuta.SimpleStorage(client, user0)
    ss.deploy()
    assert len(client.get_code(ss.address)) == 223


def test_get_abi():
    pass


def test_get_block_header():
    client.get_block_header()


def test_get_storage_at():
    ss = pymuta.SimpleStorage(client, user0)
    ss.deploy()
    ss.set(42)
    v = client.get_storage_at(ss.address, '0x0000000000000000000000000000000000000000000000000000000000000000')
    assert v[-1] == 42


if __name__ == '__main__':
    test_peer_count()
    test_block_number()
    test_transfer_balance()
    test_get_block_by_hash()
    # test_get_block_by_number()
    test_get_block_include_tx_with_data()
    test_get_logs()
    test_call()
    # test_get_transaction()
    test_get_code()
    test_get_abi()
    test_get_block_header()
    test_get_storage_at()
