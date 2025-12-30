// import 'dart:io';
//
// import 'package:app/env.dart';
//
// class MyHttpOverrides extends HttpOverrides {
//   @override
//   HttpClient createHttpClient(SecurityContext? context) {
//     return super.createHttpClient(context)
//       ..badCertificateCallback = (X509Certificate cert, String host, int port) {
//         return cert.pem.trim() ==
//             Env.config.serverPem.trim(); // Verify the certificate.
//       };
//   }
// }
