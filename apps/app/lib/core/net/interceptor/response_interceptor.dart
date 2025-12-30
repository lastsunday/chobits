import 'package:dio/dio.dart';

class ResponseInterceptor extends Interceptor {
  @override
  void onResponse(Response response, ResponseInterceptorHandler handler) {
    handler.resolve(response);
  }
}
