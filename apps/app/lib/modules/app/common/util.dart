import 'dart:io';

class Util {
  static int getPlatform() {
    if (Platform.isAndroid) {
      // return context.isTablet ? 8 : 2;
      return 2;
    } else {
      // return context.isTablet ? 9 : 1;
      return 1;
    }
  }
}
