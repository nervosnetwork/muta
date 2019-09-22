from fabric.api import *
import time

hosts = [
    "192.168.0.35",
    "192.168.0.36",
    "192.168.0.37",
    "192.168.0.38",
]
env.roledefs.update({'hosts': hosts})


def update():
    local('cd ~/muta && git pull && cargo build --release && git log -1 > version')


def deploy_diff():
    for i, h in enumerate(hosts, 1):
        local('scp ./config-bft-{}.toml {}:~/muta_chain/config.toml'.format(
            i, h))


def deploy_common():
    run('mkdir -p ~/muta_chain/devtools/chain/data')
    run('rm -rf ~/muta_chain/muta')
    put('~/muta/target/release/muta', '~/muta_chain')
    put('~/muta/version', '~/muta_chain/version')
    put('genesis.json', '~/muta_chain/devtools/chain')
    run('chmod +x ~/muta_chain/muta')


def deploy():
    execute(deploy_common, hosts=hosts)
    deploy_diff()


@roles('hosts')
def service():
    put('muta.service', '~/muta_chain')
    run('sudo cp ~/muta_chain/muta.service /etc/systemd/system')
    run('sudo systemctl daemon-reload')


@roles('hosts')
def clear():
    run('sudo rm -rf ~/muta_chain/devtools/chain/data')


@roles('hosts')
def dellog():
    run('sudo rm -rf ~/muta_chain/muta.log')
    run('touch ~/muta_chain/muta.log')


@roles('hosts')
def start():
    run('sudo systemctl start muta')


@roles('hosts')
def stop():
    run('sudo systemctl stop muta')


@roles('hosts')
def restart():
    run('sudo systemctl restart muta')


@roles('hosts')
def status():
    run('sudo systemctl status muta')


def all():
    execute(deploy)
    execute(service, hosts=hosts)
    execute(clear, hosts=hosts)
    execute(dellog, hosts=hosts)
    execute(restart, hosts=hosts)


@roles('hosts')
@parallel
def init_server():
    run('sudo apt update && sudo apt install -y policykit-1 tmux')
