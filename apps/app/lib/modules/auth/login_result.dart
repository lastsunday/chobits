import 'package:json_annotation/json_annotation.dart';

part 'login_result.g.dart';

@JsonSerializable()
class LoginResult {
  LoginResult(
      {required this.access_token, required this.expire_in, this.client_id});

  String access_token;
  int expire_in;
  String? client_id;
  String? imToken;

  factory LoginResult.fromJson(Map<String, dynamic> json) =>
      _$LoginResultFromJson(json);

  Map<String, dynamic> toJson() => _$LoginResultToJson(this);
}
