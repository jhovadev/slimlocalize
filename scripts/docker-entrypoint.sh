#!/bin/sh
set -e
mkdir -p /data
chown -R 10001:10001 /data
exec setpriv --reuid 10001 --regid 10001 --clear-groups /usr/local/bin/slimlocalize-server
