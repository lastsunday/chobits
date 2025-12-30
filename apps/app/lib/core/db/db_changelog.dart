class DbChangelog {
  static final List<Changelog> _changelogs = [];

  static void addChangelog(Changelog changelog) {
    _changelogs.add(changelog);
  }

  static List<Changelog> getChangelogList() {
    return _changelogs;
  }
}

abstract class Changelog {
  List<String> getSqlList();
}
