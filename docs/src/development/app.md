# App

> [!WARNING]
> App not implement any chobits feature right now,just make a basic framework.

## Framework

- [x] Theme framework
- [x] Adaptive
  - [x] Desktop or not
- [x] Text scale
- [x] I18n
- [x] Network
  - [x] Dio
- [x] Database
  - [x] Sqlite
- [x] Util
  - [x] Unique Id
    - [x] nanoid2
- [x] Event Bus
- [x] Auto upgrade
  - [x] Android
- [ ] Logging
  - [x] Rotating log File(Without Web Env)
  - [ ] Export log file or upload log file
- [ ] Release
  - [x] Android
  - [ ] IOS
  - [ ] Windows
    - [x] exe(unpack)
  - [ ] Linux
  - [ ] MacOS
  - [x] Web
- [x] Env Config
  - [x] Dev
  - [x] Prod

## Advance

- [x] Auth
  - [x] Spring-authorization-server
- [ ] User Profile

## Develop Flow

- [x] Changelog
- [ ] CI
  - [x] Build
  - [ ] Code Quality
  - [ ] Test
- [ ] Testing
  - [x] Unit Test(Example)
  - [x] Widget Test(Example)
  - [ ] Integration Test

## Coding

### Database Versioning

> lib\modules\app\app_store.dart

```dart
//DB init
DbManager.instance().init([ChangeLogV1()]);
Database db = await DbManager.instance().open();
```

### Database Record To View Model

> Example

```dart
class MemoMapper{
    static Future<List<MemoEntity>> selectAll() async {
        List<Map<String, dynamic>> findResult =
            await DbManager.instance().find(MemoEntity.tableName);
        return Future.value(findResult.map((e) => MemoEntity.fromJson(e)).toList());
  }
}

class AppStore{
    Future<List<MemoModel>> getMemoList() async {
        List<MemoEntity> memoEntityList = await MemoMapper.selectAll();
        return Future(() =>
            memoEntityList.map((e) => MemoModel.fromJson(e.toJson())).toList());
      }
}

class _MemoPageState{

    void initState() {
        super.initState();
        Provider.of<AppStore>(context, listen: false).getMemoList().then((value) {
          setState(() {
            memoModelList = value;
          });
        });
      }
}
```

### Pagging(Pull refresh and load more)

> Example

```dart
class _MemoPageState extends State<MemoPage> {
  List<MemoModel> _memoModelList = [];
  late EasyRefreshController _controller;
  late int _pageNum;
  late int _pageSize;
  late int _total;

  @override
  void initState() {
    super.initState();
    resetPageInfo();
    _controller = EasyRefreshController(
      controlFinishRefresh: true,
      controlFinishLoad: true,
    );
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void resetPageInfo() {
    setState(() {
      _pageNum = 1;
      _pageSize = 10;
      _total = 0;
      _memoModelList = [];
    });
  }

  Future<PageResult> _loadData() async {
    var result = await Provider.of<AppStore>(context, listen: false).pageMemo(
        PageParam(
            pageNum: _pageNum, pageSize: _pageSize, orderBy: " datetime desc"));
    if (!mounted) {
      return Future.value(result);
    }
    _memoModelList.addAll(result.rows);
    _total = result.total;
    LogHelper.debug(
        "[Memo] loadData pageNum = $_pageNum,pageSize = $_pageSize,total = $_total");
    return Future.value(result);
  }

  void _refreshList() async {
    resetPageInfo();
    await _loadData();
    setState(() {});
    _controller.finishRefresh();
    _controller.resetFooter();
  }

  void _loadList() async {
    _pageNum++;
    PageResult result = await _loadData();
    setState(() {});
    if (result.hasNext) {
      _controller.finishLoad(IndicatorResult.success);
    } else {
      _controller.finishLoad(IndicatorResult.noMore);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
        body: EasyRefresh(
            refreshOnStart: true,
            controller: _controller,
            header: RefreshHeader(context),
            footer: RefreshFooter(context),
            onRefresh: _refreshList,
            onLoad: _loadList,
            child: CustomScrollView(slivers: [
              SliverGrid(
                delegate: SliverChildBuilderDelegate((context, index) {
                  return _createMemoItem(_memoModelList[index]);
                }, childCount: _memoModelList.length),
                gridDelegate: const SliverGridDelegateWithMaxCrossAxisExtent(
                    maxCrossAxisExtent: 210),
              )
            ])),
        floatingActionButton: _getFloatingActionButton());
  }
}
```

### HttpRequest

```dart
Response result = await HttpClient.instance().get("/");
LogHelper.info(result.body().toString());
```

## JSON Model Gen

```shell
dart run build_runner build
```

## Release

### Android

> [Build and release an Android app | Flutter](https://docs.flutter.dev/deployment/android)

1. Create an keystore

   ```shell
   keytool -genkey -v -keystore .\android-app-keystore.jks -storetype JKS -keyalg RSA -keysize 2048 -validity 10000 -alias android-app
   ```

2. create [project]/android/key.properties and reference the keystore from the app

   ```properties
   storePassword=<password from previous step>
   keyPassword=<password from previous step>
   keyAlias=android-app
   storeFile=<location of the key store file, such as /Users/<user name>/android-app-keystore.jks or C:\\Users\\<user name>\\android-app-keystore.jks>
   ```

3. run release command(eg: prod env)

   ```shell
   flutter build apk --dart-define=DART_DEFINE_APP_ENV=prod
   ```

## Conventional Commits

> <https://github.com/angular/angular/blob/main/CONTRIBUTING.md#-commit-message-format>
>
> <https://www.conventionalcommits.org/en/v1.0.0/>
>
> <https://www.conventionalcommits.org/zh-hans/v1.0.0/>

The Conventional Commits specification is a lightweight convention on top of commit messages. It provides an easy set of rules for creating an explicit commit history; which makes it easier to write automated tools on top of. This convention dovetails with [SemVer](http://semver.org), by describing the features, fixes, and breaking changes made in commit messages.

The commit message should be structured as follows:

---

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

---

The commit contains the following structural elements, to communicate intent to the consumers of your library:

1. **fix:** a commit of the _type_ `fix` patches a bug in your codebase (this correlates with [`PATCH`](http://semver.org/#summary) in Semantic Versioning).
2. **feat:** a commit of the _type_ `feat` introduces a new feature to the codebase (this correlates with [`MINOR`](http://semver.org/#summary) in Semantic Versioning).
3. **BREAKING CHANGE:** a commit that has a footer `BREAKING CHANGE:`, or appends a `!` after the type/scope, introduces a breaking API change (correlating with [`MAJOR`](http://semver.org/#summary) in Semantic Versioning). A BREAKING CHANGE can be part of commits of any _type_.
4. _types_ other than `fix:` and `feat:` are allowed, for example [@commitlint/config-conventional](https://github.com/conventional-changelog/commitlint/tree/master/%40commitlint/config-conventional) (based on the [Angular convention](https://github.com/angular/angular/blob/22b96b9/CONTRIBUTING.md#-commit-message-guidelines)) recommends `build:`, `chore:`, `ci:`, `docs:`, `style:`, `refactor:`, `perf:`, `test:`, and others.
5. _footers_ other than `BREAKING CHANGE: <description>` may be provided and follow a convention similar to [git trailer format](https://git-scm.com/docs/git-interpret-trailers).

Additional types are not mandated by the Conventional Commits specification, and have no implicit effect in Semantic Versioning (unless they include a BREAKING CHANGE). A scope may be provided to a commit’s type, to provide additional contextual information and is contained within parenthesis, e.g., `feat(parser): add ability to parse arrays`.

## Sqlite3 on web

> sqflite_common_ffi_web

### Setup binaries

Implementation requires [sqlite3.wasm binaries](https://github.com/simolus3/sqlite3.dart/releases) into your web folder as well as a sqflite specific shared worker.

You can install binaries using the command:

```bash
dart run sqflite_common_ffi_web:setup
```

It should create the following files in your web folder:

- `sqlite3.wasm`
- `sqflite_sw.js`

that you can put in source control or not (personally I don't)

Note: when sqlite3 and its wasm binary are updated, you may need to run the command again using the force option:

```bash
dart run sqflite_common_ffi_web:setup --force
```

## Offline Map

1. Generate XYZ tiles(Directory)

   QGIS->Toolbox->Generate XYZ tiles(Directory)

2. Gen pubspec.yaml assets path

```bash
ls -R assets | grep ':'
```

## Coordinate

1. Server save the coordinate format is WGS84

2. Client map is gaode map,who's format is GCJ-02;

3. So we must transform the coordinate from WGS84 to GCJ-02 and we use the util [coordtransform_dart](https://pub-web.flutter-io.cn/packages/coordtransform_dart).

## Q&A

Q1. setState() or markNeedsBuild() called during build. This ModelBinding widget cannot be marked as needing to build because the framework is already in the process of building widgets.

> <https://fluttercorner.com/setstate-or-markneedsbuild-called-during-build-a-vertical-renderflex-overflowed/>

Solution 1: use a call back function
You just need to use a call back function. Because Should be setState method call before the build method had completed the process of building the widgets and thats why you are facing this error.

```dart
WidgetsBinding.instance.addPostFrameCallback((_){

// Your Code Here

});
```

## Other

1. pod install slow

```shell
cd ./ios/
#如有clash这类代理软件，则执行下面代理设置命令，使用代理进行依赖库的下载
#export https_proxy=http://127.0.0.1:7890 http_proxy=http://127.0.0.1:7890 all_proxy=socks5://127.0.0.1:7890
pod install --verbose
```

## Relate Project
