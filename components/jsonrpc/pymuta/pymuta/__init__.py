import hashlib
import json
import pathlib
import random
import string
import time
import typing

import eth_abi
import eth_keys
import eth_utils
import requests

from . import blockchain_pb2

c_server = 'http://127.0.0.1:3030'
# Four pre-defined accounts' private key.
c_private_key_0 = '0x028590ad352d54387a9c8a0ecf7e180e68c4840c72f958fc5917657f506caa80'
c_private_key_1 = '0x028590ad352d54387a9c8a0ecf7e180e68c4840c72f958fc5917657f506caa81'
c_private_key_2 = '0x028590ad352d54387a9c8a0ecf7e180e68c4840c72f958fc5917657f506caa82'
c_private_key_3 = '0x028590ad352d54387a9c8a0ecf7e180e68c4840c72f958fc5917657f506caa83'
c_root = pathlib.Path(__file__).parent
c_tx_version = 0
c_muta = 1


class User:
    # Create a account with given private_key.
    # Note: The address is depends on global var `c_muta`
    def __init__(self, private_key: str):
        self.private_key = eth_keys.keys.PrivateKey(bytes.fromhex(eth_utils.remove_0x_prefix(private_key)))
        self.public_key = self.private_key.public_key
        if c_muta:
            a = hashlib.sha3_256(self.public_key.to_bytes()[:32]).digest()[-20:]
            c = eth_keys.datatypes.to_checksum_address(a)
            self.address = c
        else:
            self.address = self.public_key.to_checksum_address()

    def __repr__(self):
        return f'{self.address}'


def abi_encode(abi, method, params):
    # Examples:
    #   abi_encode(abijson, 'set', [42])
    #
    # Or get the result directly by follow the command below:
    #   python -c "import eth_utils; print(eth_utils.keccak(b'set(uint256)')[0:4].hex())"
    for member in abi:
        if member['name'] != method:
            continue
        typeis = []
        for i in member['inputs']:
            typeis.append(i['type'])
        break
    r = eth_utils.keccak((method + '(' + ','.join(typeis) + ')').encode())
    part1 = r[:4].hex()
    part2 = eth_abi.encode_abi(typeis, params).hex()
    return eth_utils.add_0x_prefix(part1 + part2)


def abi_decode(abi, method, rets):
    for member in abi:
        if member['name'] != method:
            continue
        typeis = []
        for e in member['outputs']:
            typeis.append(e['type'])
        break
    r = eth_abi.decode_abi(typeis, bytes.fromhex(eth_utils.remove_0x_prefix(rets)))
    if len(r) == 1:
        return list(r)[0]
    return r


class Client:
    def __init__(self, server):
        self.server = server

    def send(self, method: str, params: typing.List = None):
        send_data = {'jsonrpc': '2.0', 'method': method, 'params': params, 'id': 1}
        print("<-", send_data)
        resp = requests.post(self.server, json=send_data)
        recv_data = resp.json()
        print("->", recv_data)
        return recv_data['result']

    def block_number(self) -> int:
        a = self.send('blockNumber')
        return int(a, 16)

    def call(self, to: str, user: User = None, data: str = '0x'):
        send = {'to': to}
        if user:
            send['from'] = user.address
        if data:
            send['data'] = data
        return self.send('call', params=[send])

    def get_balance(self, user: User, block_number: str = 'latest') -> int:
        b = self.send('getBalance', params=[user.address, block_number])
        return int(b, 16)

    def get_code(self, addr: str, block_number: str = 'latest'):
        return self.send('getCode', params=[addr, block_number])

    def get_block_by_hash(self, h: str, include_tx: bool = False):
        return self.send('getBlockByHash', params=[h, include_tx])

    def get_block_header(self, block_number: str = 'latest'):
        return self.send('getBlockHeader', params=[block_number])

    def get_block_by_number(self, block_number: str = 'latest', include_tx: bool = False):
        return self.send('getBlockByNumber', params=[block_number, include_tx])

    def get_logs(self, filter_obj):
        # Params filter_obj Example
        # {"topics":["0x8fb1356be6b2a4e49ee94447eb9dcb8783f51c41dcddfe7919f945017d163bf3"],"fromBlock": "0x0"}
        return self.send('getLogs', params=[filter_obj])

    def get_storage_at(self, addr: str, key: str, block_number: str = 'latest'):
        return self.send('getStorageAt', params=[addr, key, block_number])

    def get_transaction(self, h: str):
        return self.send('getTransaction', params=[h])

    def get_transaction_count(self, addr: str, block_number: str = 'latest') -> int:
        return self.send('getTransactionCount', params=[addr, block_number])

    def get_transaction_receipt(self, h: str):
        return self.send('getTransactionReceipt', params=[h])

    def ping(self):
        return self.send('ping')

    def peer_count(self) -> int:
        return self.send('peerCount')

    def send_raw_transaction(self, data: str):
        # Send a transaction with hex data
        return self.send('sendRawTransaction', params=[data])

    def sync_raw_transaction(self, data: str):
        # Send a transaction with hex data, and then wait to complete.
        r = self.send_raw_transaction(data)
        h = r['hash']
        for _ in range(64):
            r = self.send('getTransactionReceipt', params=[h])
            if r:
                return r
            time.sleep(1)
        return None

    def sign_tx(self, pk: eth_keys.keys.PrivateKey, to: str, data: str, value: int, quota: int):
        # Create transaction
        tx = blockchain_pb2.Transaction()
        tx.valid_until_block = self.block_number() + 16
        tx.nonce = ''.join([random.choice(string.ascii_letters + string.digits) for n in range(32)])
        tx.version = c_tx_version
        if tx.version == 1:
            tx.chain_id_v1 = 0x01.to_bytes(32, byteorder='big')
            if to:
                tx.to_v1 = bytes.fromhex(eth_utils.remove_0x_prefix(to))
        else:
            tx.chain_id = 0x01
            if to:
                tx.to = eth_utils.remove_0x_prefix(to)
        tx.data = bytes.fromhex(eth_utils.remove_0x_prefix(data))
        tx.value = value.to_bytes(32, byteorder='big')
        tx.quota = quota

        message = eth_utils.keccak(tx.SerializeToString())
        signature = pk.sign_msg_hash(message)
        unverify_tx = blockchain_pb2.UnverifiedTransaction()
        unverify_tx.transaction.CopyFrom(tx)
        unverify_tx.signature = signature.to_bytes()
        unverify_tx.crypto = 0
        r = unverify_tx.SerializeToString().hex()

        return eth_utils.add_0x_prefix(r)


class SimpleStorage:

    def __init__(self, client: Client, user: User):
        self.client = client
        self.user = user
        self.address: str
        with open(c_root.joinpath('SimpleStorage.abi'), 'r', encoding='utf-8') as f:
            self.abi = json.load(f)

    def deploy(self):
        with c_root.joinpath('SimpleStorage.bin').open('r') as f:
            code = f.read()
            code = eth_utils.add_0x_prefix(code)
        code = self.client.sign_tx(self.user.private_key, None, code, 0, 1000000)
        r = self.client.sync_raw_transaction(code)
        self.address = r['contractAddress']
        return r

    def set(self, n: int):
        code = abi_encode(self.abi, 'set', [n])
        code = self.client.sign_tx(self.user.private_key, self.address, code, 0, 1000000)
        return self.client.sync_raw_transaction(code)

    def get(self):
        code = abi_encode(self.abi, 'get', [])
        return self.client.call(self.address, None, code)


class Bnb:

    def __init__(self, client: Client, user: User):
        self.client = client
        self.user = user
        self.address: str
        with open(c_root.joinpath('bnb.abi'), 'r', encoding='utf-8') as f:
            self.abi = json.load(f)

    def deploy(self):
        # Params of [400000000, "DOUZ", 6, "DOUZ"]
        data = """0000000000000000000000000000000000000000000000000000000017d7840000000000000000000000000000000000000000
000000000000000000000000800000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000
00000000000000000000000000000000c00000000000000000000000000000000000000000000000000000000000000004444f555a00000000000000
0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004444f555a000000
00000000000000000000000000000000000000000000000000"""
        with c_root.joinpath('bnb.bin').open('r') as f:
            code = f.read()
            code = eth_utils.add_0x_prefix(code)
        code = self.client.sign_tx(self.user.private_key, None, code + data, 0, 5000000)
        r = self.client.sync_raw_transaction(code)
        self.address = r['contractAddress']
        return r

    def owner(self) -> str:
        code = abi_encode(self.abi, 'owner', [])
        return self.client.call(self.address, None, code)

    def total_supply(self) -> int:
        code = abi_encode(self.abi, 'totalSupply', [])
        r = self.client.call(self.address, None, code)
        return int(r, 16)

    def symbol(self) -> str:
        code = abi_encode(self.abi, 'symbol', [])
        r = self.client.call(self.address, None, code)
        return abi_decode(self.abi, 'symbol', r)

    def balance_of(self, addr: str) -> int:
        code = abi_encode(self.abi, 'balanceOf', [addr])
        r = self.client.call(self.address, self.user, code)
        return int(r, 16)

    def freeze_of(self, addr: str) -> int:
        code = abi_encode(self.abi, 'freezeOf', [addr])
        r = self.client.call(self.address, self.user, code)
        return int(r, 16)

    def name(self) -> str:
        code = abi_encode(self.abi, 'name', [])
        r = self.client.call(self.address, None, code)
        return abi_decode(self.abi, 'name', r)

    def decimals(self) -> int:
        code = abi_encode(self.abi, 'decimals', [])
        r = self.client.call(self.address, self.user, code)
        return int(r, 16)

    def transfer(self, to: str, value: int):
        code = abi_encode(self.abi, 'transfer', [to, value])
        code = self.client.sign_tx(self.user.private_key, self.address, code, 0, 1000000)
        return self.client.sync_raw_transaction(code)


user0 = User(c_private_key_0)
user1 = User(c_private_key_1)
user2 = User(c_private_key_2)
user3 = User(c_private_key_3)
