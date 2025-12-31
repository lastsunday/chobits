import 'package:flutter_test/flutter_test.dart';
import 'package:app/core/domain/global_time.dart';

void main() {
  test('Should fix time in test', () {
    GlobalTime.reset('2022-01-30T16:55:31.081+08:00');
    expect(GlobalTime.now().millisecondsSinceEpoch, 1643532931081);
  });

  test('Should reset fixed time in test', () {
    GlobalTime.reset('2022-01-30T16:55:31.081+08:00');
    GlobalTime.reset();
    expect(GlobalTime.now().millisecondsSinceEpoch == 1643532931081, false);
  });
}
