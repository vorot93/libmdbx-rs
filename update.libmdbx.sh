#!/usr/bin/env bash

DIR=$(cd "$(dirname "$0")"; pwd)
set -ex
cd $DIR/..

if [ ! -d "libmdbx" ] ; then
git clone git@github.com:erthink/libmdbx.git
cd libmdbx
else
cd libmdbx
git pull
fi

make dist

libmdbx=$DIR/mdbx-sys/libmdbx
rm -rf $libmdbx
cp -R dist $libmdbx
