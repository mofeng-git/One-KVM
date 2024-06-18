#!/bin/bash
APP_PATH=$(readlink -f $(dirname $0))
echo "-> Apply patches"
cd /usr/lib/python3.10/site-packages/
git apply ${APP_PATH}/*.patch
cd ${APP_PATH}
echo "-> Add otgmsd unlock link"
cp kvmd-helper-otgmsd-unlock /usr/bin/
echo "-> Add sudoer"
echo "kvmd ALL=(ALL) NOPASSWD: /usr/bin/kvmd-helper-otgmsd-unlock" >> /etc/sudoers.d/99_kvmd
echo "-> Apply old kernel msd patch done."