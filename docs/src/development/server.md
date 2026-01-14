# Server

## File structure

```
.
├── api
│   ├── Cargo.toml
│   ├── locales
│   │   ├── en.yml
│   │   └── zh.yml
│   ├── resources
│   │   └── test
│   ├── src
│   │   ├── asr
│   │   ├── auth_error.rs
│   │   ├── auth.rs
│   │   ├── common
│   │   ├── config
│   │   ├── i18n.rs
│   │   ├── index.rs
│   │   ├── lib.rs
│   │   ├── llm
│   │   ├── mcp
│   │   ├── ota_data.rs
│   │   ├── ota_error.rs
│   │   ├── ota.rs
│   │   ├── tts
│   │   ├── util
│   │   ├── vad
│   │   └── ws
│   └── tests
├── Cargo.toml
├── CHANGELOG.md
├── docker-compose.yml
├── Dockerfile
├── entity
├── framework
├── migration
├── project.json
├── README.md
├── script
│   ├── download_device_assets.sh
│   └── download_model.sh
├── service
├── src
│   └── main.rs
└── web
```

## Data flow

**_TODO_**

## CUDA Toolkit install in fedora 43 & setup env

```shell
sudo sh cuda_12.8.1_570.124.06_linux.run --toolkit --no-drm --silent --override
```

````.zshrc
export PATH="/usr/local/cuda/bin:$PATH"
export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:/usr/local/cuda/lib64
export LIBRARY_PATH=$LIBRARY_PATH:/usr/local/cuda/lib64

``` shell
conda create -n cuda
# for candle library
conda install conda-forge::gcc==14.3.0
conda install conda-forge::gxx==14.3.0
# for openssl library
conda install anaconda::openssl
conda activate cuda
# develop...
````

## Spec

<https://rust-lang.github.io/api-guidelines/>

<https://rust-coding-guidelines.github.io/rust-coding-guidelines-zh/overview.html>
