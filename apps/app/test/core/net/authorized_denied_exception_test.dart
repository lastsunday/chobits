import 'package:flutter_test/flutter_test.dart';
import 'package:app/core/net/authorized_denied_exception.dart';

void main() {
  test('Should get denied exception as expect', () {
    expect(AuthorizedDeniedException().expMsg(), '403');
  });
}
