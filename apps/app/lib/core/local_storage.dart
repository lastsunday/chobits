import 'package:flutter/cupertino.dart';
import 'package:shared_preferences/shared_preferences.dart';

class LocalStorage {
  static late SharedPreferences _impl;

  static Future<void> init() async {
    _impl = await SharedPreferences.getInstance();
  }

  static Future<void> save(String key, value) async {
    if (value is String) {
      await _impl.setString(key, value);
    } else if (value is int) {
      await _impl.setInt(key, value);
    } else if (value is bool) {
      await _impl.setBool(key, value);
    } else if (value is double) {
      await _impl.setDouble(key, value);
    } else if (value is List<String>) {
      await _impl.setStringList(key, value);
    } else {
      throw UnsupportedError("unsupported-type");
    }
  }

  static dynamic get<T>(String key, T? defaultValue) {
    if (T == String) return (_impl.getString(key) ?? defaultValue);
    if (T == int) return _impl.getInt(key) ?? defaultValue;
    if (T == bool) return _impl.getBool(key) ?? defaultValue;
    if (T == double) return _impl.getDouble(key) ?? defaultValue;
    if (T == List<String>) return _impl.getStringList(key) ?? defaultValue;
    throw UnsupportedError("unsupported-type");
  }

  static Future<void> remove(String key) async {
    await SharedPreferences.getInstance().then((value) => value.remove(key));
  }

  @visibleForTesting
  static Future<void> removeAll() async {
    await SharedPreferences.getInstance().then((value) => value.clear());
  }
}
