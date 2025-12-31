import 'package:flutter_test/flutter_test.dart';
import 'package:app/core/id.dart';

void main() {
  test('Should create id object with gid', () {
    Id id = Id.fromGid('gid://gitlab/Group/3');
    expect(id.id, 3);
  });

  test('Should create id object with int value', () {
    Id id = Id.from(5);
    expect(id.id, 5);
  });
}
