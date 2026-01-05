# ESP32

## Install

> <https://docs.espressif.com/projects/esp-idf/zh_CN/v5.5.2/esp32/get-started/linux-macos-setup.html>

## run flow

```
. $HOME/esp/esp-idf/export.sh
idf.py set-target esp32-s3
idf.py menuconfig
idf.py build
idf.py -p PORT flash
# macos
idf.py -p /dev/cu.usbserial-14410 flash
# linux
sudo chmod 777 /dev/ttyACM0
idf.py -p /dev/ttyACM0 flash
```

get PORT

```
ls /dev/cu.*
```

debug monitor

```
idf.py monitor
idf.py -p PORT flash monitor

```
