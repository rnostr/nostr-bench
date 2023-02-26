Nostr relay benchmaker (WIP)
======================

```sh

cargo install nostr-bench
nostr-bench --help

```


Get more connections
----------------------

Since the system limits a network interface to connect up to 64k, you can set `--interface` to bind more interface to increase the number of connections

```sh
nostr-bench connect 'ws://127.0.0.1:8080' --interface 192.168.0.2 --interface 192.168.0.3`
```

###  Increase resource usage limits

Linux

```sh
ulimit -n 1000000
sudo sysctl -w net.ipv4.ip_local_port_range="1025 65534"
```

Mac OS

```sh

ulimit -n 1000000
# sysctl net.inet.ip.portrange
sudo sysctl -w net.inet.ip.portrange.first=1025
sudo sysctl -w net.inet.ip.portrange.last=65534

```
