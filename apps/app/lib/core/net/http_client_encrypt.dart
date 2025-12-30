// import 'package:encrypt/encrypt.dart';
// import 'dart:math';
// import 'package:pointycastle/asymmetric/api.dart';
//
// class HttpClientEncrypt {
//   static HttpClientEncrypt _instance = HttpClientEncrypt._internal();
//
//   HttpClientEncrypt._internal();
//
//   factory HttpClientEncrypt.instance() => _instance;
//
//   void handleEncrypt(RequestOptions options) {
//     bool isEncrypt = options.headers.containsKey("isEncrypt");
//     // 当开启参数加密
//     if (isEncrypt && (options.method == 'POST' || options.method == 'PUT')) {
//       // 生成一个 AES 密钥
//       String aesKey = generateAesKey();
//       options.headers['encrypt-key'] = rsaPublicKeyEncrypt(
//           Env.config.rsaPublicKey, base64.encode(aesKey.codeUnits));
//       options.data = options.data is Object
//           ? encryptWithAes(jsonEncode(options.data), aesKey)
//           : encryptWithAes(options.data, aesKey);
//     }
//   }
//
//   String encryptWithAes(String content, String key) {
//     final iv = IV.fromLength(16);
//     final encrypter = Encrypter(AES(Key.fromUtf8(key), mode: AESMode.ecb));
//     final encrypted = encrypter.encrypt(content, iv: iv);
//     return encrypted.base64;
//   }
//
//   String rsaPublicKeyEncrypt(String key, String content) {
//     final encrypter =
//         Encrypter(RSA(publicKey: parseKeyFromBase64String<RSAPublicKey>(key)));
//     return encrypter.encrypt(content).base64;
//   }
//
//   T parseKeyFromBase64String<T extends RSAAsymmetricKey>(String key) {
//     String result =
//         "-----BEGIN PUBLIC KEY-----\n$key\n-----END PUBLIC KEY-----";
//     final parser = RSAKeyParser();
//     return parser.parse(result) as T;
//   }
//
//   String generateAesKey() {
//     return generateRandomString(32);
//   }
//
//   String generateRandomString(int len) {
//     var characters =
//         "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
//     var result = '';
//     var charactersLength = characters.length;
//     for (var i = 0; i < len; i++) {
//       result += characters[Random.secure().nextInt(charactersLength)];
//     }
//     return result;
//   }
// }
