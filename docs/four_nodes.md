Following steps assume that how to run muta in four nodes.

## Get accounts

First, make sure you have 4 available accounts. We can use the build in accounts, which described at `./devtools/chain/READMD.md`.

| Address                                      | Balance                | PrivKey                                                              |
| -------------------------------------------- | ---------------------- | -------------------------------------------------------------------- |
| `0x7899EE7319601cbC2684709e0eC3A4807bb0Fd74` | `0x400000000000000000` | `0x028590ad352d54387a9c8a0ecf7e180e68c4840c72f958fc5917657f506caa80` |
| `0xC5b874618a1E81C68bEb30A7A219Fee3f9839a01` | `0x400000000000000000` | `0x028590ad352d54387a9c8a0ecf7e180e68c4840c72f958fc5917657f506caa81` |
| `0x9Ad301Eb2A4938070ccda5e4b298C4f867812d6e` | `0x400000000000000000` | `0x028590ad352d54387a9c8a0ecf7e180e68c4840c72f958fc5917657f506caa82` |
| `0x64EC8878182F52af52611dDc575E267ABd012560` | `0x400000000000000000` | `0x028590ad352d54387a9c8a0ecf7e180e68c4840c72f958fc5917657f506caa83` |

## Create 4 workspaces

I will demonstrated in the tmp directory.

```sh
mkdir -p /tmp/muta/node1
mkdir -p /tmp/muta/node2
mkdir -p /tmp/muta/node3
mkdir -p /tmp/muta/node4

cp ./devtools/chain/config.toml /tmp/muta/node1/config.toml
cp ./devtools/chain/config.toml /tmp/muta/node2/config.toml
cp ./devtools/chain/config.toml /tmp/muta/node3/config.toml
cp ./devtools/chain/config.toml /tmp/muta/node4/config.toml
```

## Update the config

Then we should update the config for **EACH** node.

**node1**

```toml
privkey = "028590ad352d54387a9c8a0ecf7e180e68c4840c72f958fc5917657f506caa80"
data_path = "/tmp/muta/node1/data"

[rpc]
address = "127.0.0.1:3031"

[network]
bootstrap_addresses = ["127.0.0.1:1332", "127.0.0.1:1333", "127.0.0.1:1334"]
listening_address = "127.0.0.1:1331"

[consensus]
verifier_list = [ "7899EE7319601cbC2684709e0eC3A4807bb0Fd74", "C5b874618a1E81C68bEb30A7A219Fee3f9839a01", "9Ad301Eb2A4938070ccda5e4b298C4f867812d6e", "64EC8878182F52af52611dDc575E267ABd012560" ]
```

**node2**

```toml
privkey = "028590ad352d54387a9c8a0ecf7e180e68c4840c72f958fc5917657f506caa81"
data_path = "/tmp/muta/node2/data"

[rpc]
address = "127.0.0.1:3032"

[network]
bootstrap_addresses = ["127.0.0.1:1331", "127.0.0.1:1333", "127.0.0.1:1334"]
listening_address = "127.0.0.1:1332"

[consensus]
verifier_list = [ "7899EE7319601cbC2684709e0eC3A4807bb0Fd74", "C5b874618a1E81C68bEb30A7A219Fee3f9839a01", "9Ad301Eb2A4938070ccda5e4b298C4f867812d6e", "64EC8878182F52af52611dDc575E267ABd012560" ]
```

**node3**

```toml
privkey = "028590ad352d54387a9c8a0ecf7e180e68c4840c72f958fc5917657f506caa82"
data_path = "/tmp/muta/node3/data"

[rpc]
address = "127.0.0.1:3033"

[network]
bootstrap_addresses = ["127.0.0.1:1331", "127.0.0.1:1332", "127.0.0.1:1334"]
listening_address = "127.0.0.1:1333"

[consensus]
verifier_list = [ "7899EE7319601cbC2684709e0eC3A4807bb0Fd74", "C5b874618a1E81C68bEb30A7A219Fee3f9839a01", "9Ad301Eb2A4938070ccda5e4b298C4f867812d6e", "64EC8878182F52af52611dDc575E267ABd012560" ]
```

**node4**

```toml
privkey = "028590ad352d54387a9c8a0ecf7e180e68c4840c72f958fc5917657f506caa83"
data_path = "/tmp/muta/node4/data"

[rpc]
address = "127.0.0.1:3034"

[network]
bootstrap_addresses = ["127.0.0.1:1331", "127.0.0.1:1332", "127.0.0.1:1333"]
listening_address = "127.0.0.1:1334"

[consensus]
verifier_list = [ "7899EE7319601cbC2684709e0eC3A4807bb0Fd74", "C5b874618a1E81C68bEb30A7A219Fee3f9839a01", "9Ad301Eb2A4938070ccda5e4b298C4f867812d6e", "64EC8878182F52af52611dDc575E267ABd012560" ]
```

# Start the chain

```
$ cargo build --release

$ ./target/release/muta --config /tmp/muta/node1/config.toml init
$ ./target/release/muta --config /tmp/muta/node2/config.toml init
$ ./target/release/muta --config /tmp/muta/node3/config.toml init
$ ./target/release/muta --config /tmp/muta/node4/config.toml init
```

If everything goes well, you’ll see this appear:

```
Bft starts height 1, round 0
Bft starts height 2, round 0
Bft starts height 3, round 0
...
```
