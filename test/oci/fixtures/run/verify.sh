curl --version
test -s /etc/ssl/certs/ca-certificates.crt
dpkg-query -W ca-certificates curl
/bin/sh /inputs/offline.sh
