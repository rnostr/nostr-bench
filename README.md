Nostr relay benchmaker
======================

```sh

cargo install nostr-bench
nostr-bench --help

# Usage: nostr-bench <COMMAND>
# 
# Commands:
#   connect  Benchmark create websocket connections
#   echo     Benchmark send websocket message, the server should send back the message
#   event    Benchmark publish nostr event
#   req      Benchmark request nostr event
#   help     Print this message or the help of the given subcommand(s)

```

```sh

nostr-bench connect --help

# Usage: nostr-bench connect [OPTIONS] <URL>
# 
# Arguments:
#   <URL>  Nostr relay host url
# 
# Options:
#   -c, --count <NUM>      Max count of clients [default: 100]
#   -r, --rate <NUM>       Start open connection rate every second [default: 50]
#   -k, --keepalive <NUM>  Close connection after second, ignore when set to 0 [default: 0]
#   -t, --threads <NUM>    Set the amount of threads, default 0 will use all system available cores [default: 0]
#   -i, --interface <IP>   Network interface address list
#       --json             Display stats information as json, time format as milli seconds
#   -h, --help             Print help

```

Get more connections
----------------------

Since the system limits a network interface to connect up to 64k, you can set `--interface` to bind more interface to increase the number of connections

```sh
nostr-bench connect 'ws://127.0.0.1:8080' --interface 192.168.0.2 --interface 192.168.0.3
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
