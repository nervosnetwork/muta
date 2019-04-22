import shutil
import subprocess
import sys
import time

import pymuta

user0 = pymuta.user0
user1 = pymuta.user1
user2 = pymuta.user2
user3 = pymuta.user3

client = pymuta.Client(pymuta.c_server)
ensure_blank_chain = 0

if shutil.which('cita-cli'):
    c_cita_cli = 'cita-cli'
else:
    c_cita_cli = '/src/cita-cli/target/debug/cita-cli.exe'

if ensure_blank_chain and client.get_balance(user1, '0x00') != 0x400000000000000000:
    raise Exception('Ensure you are on a blank chain')


def call(command):
    print(command)
    r = subprocess.call(command, shell=True)
    if r != 0:
        sys.exit(r)


def test_transfer_balance():
    b_user0_0 = client.get_balance(user0)
    b_user1_0 = client.get_balance(user1)
    b_user2_0 = client.get_balance(user2)

    call(
        f'{c_cita_cli} rpc --debug sendRawTransaction --private-key {user1.private_key} --url {pymuta.c_server} --address {user2.address} --value 10 --code 0x')
    time.sleep(6)

    b_user0_1 = client.get_balance(user0)
    b_user1_1 = client.get_balance(user1)
    b_user2_1 = client.get_balance(user2)

    assert b_user0_1 == b_user0_0 + 21000
    assert b_user1_1 == b_user1_0 - 21000 - 10
    assert b_user2_1 == b_user2_0 + 10


if __name__ == '__main__':
    test_transfer_balance()
