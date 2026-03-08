#!/bin/bash

cd ${0%/*}
gradle wrapper
../wasm/build.py
rsync -r ../wasm/pkg/ app/src/main/assets/www/pkg
./gradlew assembleDebug