### Development

#### apps/server

```shell
pnpm i
moon run server-ui:build
chobits-server download
# using cuda: moon run server:run --features cuda
moon run server:run
```

- Access home page <http://127.0.0.1:3000>
- Access admin console <http://127.0.0.1:3000/login>
  - default account: root/Change_Me
- Access api documentation <http://127.0.0.1:3000/docs>
- Access device test page <http://127.0.0.1:3000/test/device/test_page.html>
- Client setting
  - ota url
    <http://127.0.0.1:3000/api/ota/>
  - ws url
    <ws://127.0.0.1:3000/chobits/v1/>

#### apps/server-ui

```shell
pnpm i
moon run server-ui:dev
```

#### apps/app

**_TODO_**

### Building

**_TODO_**

### Using

**_TODO_**
