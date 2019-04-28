Muta's python sdk for human usages.

```s
python3 setup.py develop
```

# Example

```py
import pymuta

client = pymuta.Client(pymuta.c_server)
client.block_number()
```

# How to generate many transactions

```py
import pymuta

client = pymuta.Client(pymuta.c_server)
client.block_number = lambda: 10000
for _ in range(10000):
    tx = client.sign_tx(pymuta.user0.private_key, pymuta.user1.address, "0x", 1, 100000)
    print(tx)
```

and then

```sh
$ python3 main.py > /tmp/txs
```
