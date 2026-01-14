# ESP32

## Checkout

```shell
git clone git@github.com:78/xiaozhi-esp32.git
```

## Install ESP IDF

> <https://docs.espressif.com/projects/esp-idf/zh_CN/v5.5.2/esp32/get-started/linux-macos-setup.html>

## Development

### Setup enviroment and flash device

- esp32-s3

```shell
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

### Other useful command

- Get PORT

```shell
ls /dev/cu.*
```

- Debug monitor

```shell
idf.py monitor
idf.py -p PORT flash monitor

```
