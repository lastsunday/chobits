import 'package:app/core/db/db_changelog.dart';

class ChangeLogV1 implements Changelog {
  @override
  List<String> getSqlList() {
    String sqlCreateTableMemo = """
    CREATE TABLE memo(
      id TEXT PRIMARY KEY,
      content TEXT,
      datetime TEXT
    )
    """;
    return [sqlCreateTableMemo];
  }
}
