#!/bin/sh
# What firewall rulesets, IDS/IPS, transparent proxies and network tests ask
# of the kernel: nftables expressions and families, the iptables matches and
# targets iptables-nft hands to xtables, ipset types, tc, socket diagnostics
# and the usual interfaces. Runs inside a privileged Alpine container and
# prints "<name> ok" or "<name> FAIL: <why>" for each; tests/e2e.sh checks
# every line. Started from issue #14's probe. Any engine can run it:
#
#   docker run --rm --privileged -v "$PWD/tests/kernel-features.sh:/k.sh" alpine:3.22 sh /k.sh
timeout 120 apk add -q nftables iptables ip6tables ebtables iproute2 iproute2-ss ipset ipvsadm >/dev/null 2>&1 || { echo "setup FAIL: apk add"; exit 1; }

say() { if [ "$2" = 0 ]; then echo "$1 ok"; else echo "$1 FAIL: $(head -c 200 /tmp/e | tr '\n' ' ')"; fi; }
# run NAME COMMAND...: one check, ok when the command succeeds.
run() { n=$1; shift; "$@" >/tmp/e 2>&1; say "$n" $?; }
# nft NAME FAMILY HOOK RULE: a chain with RULE in it loads.
nftr() {
  printf 'table %s probe {\n chain c {\n  type filter hook %s priority 0;\n  %s\n }\n}\n' "$2" "$3" "$4" > /tmp/r
  nft -f /tmp/r >/tmp/e 2>&1; say "nft-$1" $?
  nft delete table "$2" probe 2>/dev/null
}

# nftables expressions (issue #14's list first)
nftr reject-tcp inet prerouting "meta l4proto tcp reject with tcp reset"
nftr reject-icmpx inet input "reject with icmpx admin-prohibited"
nftr queue inet forward "queue num 0 bypass"
nftr ct-label inet prerouting "ct label set 3"
nftr socket inet prerouting "socket transparent 1"
nftr tproxy inet prerouting "meta l4proto tcp tproxy ip to 127.0.0.1:8443"
nftr log-group inet input "log group 1"
nftr limit inet prerouting "limit rate 10/second"
nftr quota inet prerouting "quota 10 mbytes"
nftr connlimit inet input "ct count over 10 drop"
nftr numgen inet prerouting "meta mark set numgen inc mod 2"
nftr hash inet prerouting "meta mark set jhash ip saddr mod 2"
nftr synproxy inet input "tcp dport 80 ct state invalid,untracked synproxy mss 1460 wscale 7"
nftr osf inet input "osf name \"Linux\" accept"
nftr dup ip prerouting "dup to 127.0.0.1"
nftr xfrm inet input "meta secpath exists accept"
# families and flowtables
printf 'table netdev probe {\n chain c {\n  type filter hook ingress device lo priority 0;\n  accept\n }\n}\n' > /tmp/r
run nft-netdev nft -f /tmp/r; nft delete table netdev probe 2>/dev/null
printf 'table arp probe {\n chain c {\n  type filter hook input priority 0;\n  accept\n }\n}\n' > /tmp/r
run nft-arp nft -f /tmp/r; nft delete table arp probe 2>/dev/null
printf 'table bridge probe {\n chain c {\n  type filter hook input priority 0;\n  meta ibrname "x" reject\n }\n}\n' > /tmp/r
run nft-bridge nft -f /tmp/r; nft delete table bridge probe 2>/dev/null
printf 'table inet probe {\n flowtable f {\n  hook ingress priority 0; devices = { lo };\n }\n chain c {\n  type filter hook forward priority 0;\n  meta l4proto tcp flow add @f\n }\n}\n' > /tmp/r
run nft-flowtable nft -f /tmp/r; nft delete table inet probe 2>/dev/null

# iptables (iptables-nft, so through xtables)
ipt() { n=$1; shift; iptables -A INPUT "$@" >/tmp/e 2>&1; r=$?; [ $r = 0 ] && iptables -D INPUT "$@" 2>/dev/null; say "ipt-$n" $r; }
ipt nfqueue -p tcp --dport 9 -j NFQUEUE --queue-num 0 --queue-bypass
ipt nflog -j NFLOG --nflog-group 1
ipt recent -m recent --name probe --rcheck --seconds 60 -j DROP
ipt hashlimit -m hashlimit --hashlimit-above 10/sec --hashlimit-name probe -j DROP
ipt connlimit -p tcp -m connlimit --connlimit-above 10 -j DROP
ipt string -m string --algo bm --string probe -j DROP
ipt u32 -m u32 --u32 "0>>22&0x3C@0>>16=0x50" -j DROP
ipt state -m state --state NEW -j ACCEPT
ipt time -m time --timestart 09:00 --timestop 17:00 -j ACCEPT
ipt length -m length --length 1000:1500 -j ACCEPT
ipt mac -m mac --mac-source 02:00:00:00:00:01 -j DROP
run ipt-tproxy iptables -t mangle -A PREROUTING -p tcp --dport 9 -j TPROXY --on-port 9 --tproxy-mark 1
run ipt-tcpmss iptables -t mangle -A FORWARD -p tcp --tcp-flags SYN,RST SYN -j TCPMSS --clamp-mss-to-pmtu
run ipt-notrack iptables -t raw -A PREROUTING -p udp --dport 9 -j CT --notrack
run ip6t-raw ip6tables -t raw -A PREROUTING -p udp --dport 9 -j CT --notrack
run ebtables ebtables -t filter -L

# ipset, IPVS, tc
run ipset-netnet ipset create probe1 hash:net,net
run ipset-list ipset create probe2 list:set
ipset destroy probe2 2>/dev/null; ipset destroy probe1 2>/dev/null
run ipvs ipvsadm -A -t 10.255.0.1:80 -s rr
ipvsadm -C 2>/dev/null
run tc-netem tc qdisc add dev lo root netem delay 1ms
tc qdisc del dev lo root 2>/dev/null
run tc-ingress tc qdisc add dev lo clsact
run tc-flower tc filter add dev lo ingress protocol ip flower ip_proto tcp action drop
tc qdisc del dev lo clsact 2>/dev/null

# socket diagnostics
run ss ss -tan
ss -K dst 192.0.2.1 >/tmp/e 2>&1; if grep -qi "not supported" /tmp/e; then say ss-kill 1; else say ss-kill 0; fi

# interfaces and tunnels
# link NAME ARGS...: `ip link add ARGS` makes probe0, which goes again after.
link() { n=$1; shift; ip link add "$@" >/tmp/e 2>&1; say "link-$n" $?; ip link del probe0 2>/dev/null; }
link macvlan link eth0 probe0 type macvlan mode bridge
link ipvlan link eth0 probe0 type ipvlan mode l2
link vlan link eth0 probe0 type vlan id 10
link wireguard probe0 type wireguard
link vrf probe0 type vrf table 10
link bond probe0 type bond
link geneve probe0 type geneve id 10 remote 192.0.2.1
link gre probe0 type gre remote 192.0.2.1
link ipip probe0 type ipip remote 192.0.2.1
link ip6tnl probe0 type ip6tnl remote 2001:db8::1
link ifb probe0 type ifb
run xfrm ip xfrm state add src 192.0.2.1 dst 192.0.2.2 proto esp spi 0x1 mode tunnel enc 'cbc(aes)' 0x00112233445566778899aabbccddeeff auth 'hmac(sha256)' 0x00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff
ip xfrm state flush 2>/dev/null
