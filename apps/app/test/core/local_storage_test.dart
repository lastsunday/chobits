import 'package:flutter_test/flutter_test.dart';
import 'package:app/core/local_storage.dart';

void main() {
  test('Should local storage get string as expect', () {
    LocalStorage.save('test', 'yes');
    expect('yes', LocalStorage.get('test', ''));
  });

  test('Should local storage get string as default', () {
    expect(' ', LocalStorage.get('test', ' '));
  });

  test('Should local storage get int as expect', () {
    LocalStorage.save('test', 1);
    expect(1, LocalStorage.get('test', 0));
  });

  test('Should local storage get int as default', () {
    expect(0, LocalStorage.get('test', 0));
  });

  test('Should local storage get bool as expect', () {
    LocalStorage.save('test', true);
    expect(true, LocalStorage.get('test', false));
  });

  test('Should local storage get bool as default', () {
    expect(false, LocalStorage.get('test', false));
  });

  test('Should local storage get double as expect', () {
    LocalStorage.save('test', 2.0);
    expect(2.0, LocalStorage.get('test', 1.0));
  });

  test('Should local storage get double as default', () {
    expect(1.0, LocalStorage.get('test', 1.0));
  });

  test('Should local storage get list string as expect', () {
    LocalStorage.save('test', ["1", "2"]);
    expect(["1", "2"], LocalStorage.get('test', ["1"]));
  });

  test('Should local storage get list string as default', () {
    expect(["1"], LocalStorage.get('test', ["1"]));
  });

  test('Should local storage get if unsupported type', () {
    expect(() => LocalStorage.get('test', null), throwsUnsupportedError);
  });

  test('Should local storage save if unsupported type', () {
    expect(() => LocalStorage.save('test', null), throwsUnsupportedError);
  });

  test('Should local storage remove one', () async {
    LocalStorage.save('test', ["1", "2"]);
    await LocalStorage.remove('test');
    expect(LocalStorage.get('test', [""]), isNot(["1", "2"]));
  });
}
