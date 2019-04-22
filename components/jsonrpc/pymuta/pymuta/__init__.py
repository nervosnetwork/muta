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


def abi_encode(abi_file, method, params):
    # Examples:
    #   abi_encode('/tmp/SimpleStorage.abi', 'set', [42])
    #
    # Or get the result directly by follow the command below:
    #   python -c "import eth_utils; print(eth_utils.keccak(b'set(uint256)')[0:4].hex())"
    with open(abi_file, 'r', encoding='utf-8') as f:
        abi = json.load(f)
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

    def get_balance(self, user: User, block_number: str = 'latest') -> int:
        b = self.send('getBalance', params=[user.address, block_number])
        return int(b, 16)

    def ping(self):
        return self.send('ping')

    def send_raw_transaction(self, data: str):
        # Send a transaction with hex data
        return self.send('sendRawTransaction', params=[data])

    def sync_raw_transaction(self, data: str):
        # Send a transaction with hex data, and then wait to complete.
        # TODO: needs get receipt apis.
        r = self.send_raw_transaction(data)
        time.sleep(6)
        return r

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


user0 = User(c_private_key_0)
user1 = User(c_private_key_1)
user2 = User(c_private_key_2)
user3 = User(c_private_key_3)
