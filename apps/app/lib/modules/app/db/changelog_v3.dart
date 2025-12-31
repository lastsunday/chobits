import 'package:app/core/db/db_changelog.dart';

class ChangeLogV3 implements Changelog {
  @override
  List<String> getSqlList() {
    //displaymode 显示模式
    //id memo的id
    //seq 序号
    String sqlCreateTableMemoOrder = """
      CREATE TABLE memo_order(
        displaymode INTEGER,
        id TEXT,
        seq INTEGER
      );
      """;
    return [sqlCreateTableMemoOrder];
  }
}
