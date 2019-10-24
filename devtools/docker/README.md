# use docker for development

## build `muta_build` and `muta_run` docker

```
docker build -t muta_build -f devtools/docker/dockerfiles/Dockerfile.muta_build .

docker build -t muta_run -f devtools/docker/dockerfiles/Dockerfile.muta_run .
```

Use docker to build muta binary:
```
docker run -it -v `pwd`:/code -v `pwd`/target/docker_target:/code/target -v `pwd`/target/cargo_cache:/usr/local/cargo/registry muta_build bash -c 'cd /code && cargo build'
```

We mount `target` and `cargo/registry` to dirs under `target` to make it faster when recompile the binary.

Use the default config to run a single node muta chain:
```
docker run -it -v `pwd`:/muta muta_run /bin/bash -c 'cd /muta; ./target/docker_target/debug/muta-chain'
```

## use `docker-compose` to run multiple nodes rapidly

### single node

```
# get single node up
$ docker-compose -f devtools/docker/dockercompose/docker-compose-single.yaml up

# make a http jsonrpc to the node
$ curl -X POST -H "Content-Type: application/json" -d ' {"jsonrpc": "2.0", "method": "blockNumber", "params": [], "id": 1}' http://127.0.0.1:3030

# go inside the node
$ docker-compose -f devtools/docker/dockercompose/docker-compose-single.yaml exec node0 bash
```

The chain data is in `target/data/single` according to the config in `config-single.toml` and the mount config in `devtools/docker/dockercompose/docker-compose-single.yaml`.

### multiple nodes

#### master and follower

Start 2 nodes, `node1` is in the validator list, and `node2` is follower, sync blocks from node1.
Check the config if you want.

```
$ docker-compose -f devtools/docker/dockercompose/docker-compose-mul.yaml up

# go inside node1
$ docker-compose -f devtools/docker/dockercompose/docker-compose-mul.yaml exec node1 bash
```

The chain data is in `target/data/mul1` and `target/data/mul2`.


#### four nodes bft

Start 4 nodes bft.

```
$ docker-compose -f devtools/docker/dockercompose/docker-compose-bft.yaml up
```

The nodes names are `bft_node1` ~ `bft_node4` and chain data is in `target/data/bft1` ~ `target/data/bft4`.