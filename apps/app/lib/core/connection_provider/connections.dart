import 'dart:convert';

import 'package:app/core/connection_provider/connection.dart';
import 'package:app/core/domain/token.dart';
import 'package:app/core/local_storage.dart';
import 'package:app/core/log_helper.dart';

class Connections {
  static const String _userKey = 'app-user';
  static bool onAuthTokenRequest = false;

  List<Connection> _connections = [];

  List<Connection> get get => _connections;

  void add(Connection connection) {
    for (var user in _connections) {
      user.active = false;
    }
    _connections.add(connection);
    save();
  }

  void notifyTokenChanged(Token token) async {
    _connections.toList().first.token = token;
  }

  void resetCurrentToken(Token token) {
    if (activeConnection == null) return;
    activeConnection!.token = token;
    save();
  }

  void clearActive() {
    _connections.removeWhere((o) => o.active == true);
    if (_connections.isEmpty) {
      save();
      return;
    }
    _connections.last.active = true;
    save();
  }

  Connection? get activeConnection {
    if (_connections.where((o) => o.active == true).isEmpty) return null;
    return _connections.firstWhere((o) => o.active == true);
  }

  List<String> _toStoredConnections() {
    return _connections.map((e) => jsonEncode(e)).toList();
  }

  void save() {
    LocalStorage.save(_userKey, _toStoredConnections());
  }

  Future<void> restore() async {
    List<String> jsonList = await LocalStorage.get<List<String>>(_userKey, []);
    try {
      _connections = jsonList
          .map((e) => jsonDecode(e))
          .map((e) => Connection.fromJson(e))
          .toList();
    } catch (e) {
      LogHelper.err(e.toString(), e);
      _connections = [];
    }
  }

  void changeAccount(Connection connection) {
    var index = _connections.indexOf(connection);
    for (var c in _connections) {
      c.active = false;
    }
    _connections[index].active = true;
    save();
  }

  void loggedOutSelected(Connection connection) {
    _connections.remove(connection);
    if (_connections.any((o) => o.active == true) || _connections.isEmpty) {
      save();
      return;
    }
    _connections.last.active = true;
    save();
  }
}
