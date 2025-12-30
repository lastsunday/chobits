import 'package:json_annotation/json_annotation.dart';

part 'login_user_info.g.dart';

@JsonSerializable()
class LoginUserInfoResult {
  LoginUserInfoResult({required this.user});

  User user;

  factory LoginUserInfoResult.fromJson(Map<String, dynamic> json) =>
      _$LoginUserInfoResultFromJson(json);

  Map<String, dynamic> toJson() => _$LoginUserInfoResultToJson(this);
}

@JsonSerializable()
class User {
  User({required this.userName});

  String userName;
  String? avatar;

  factory User.fromJson(Map<String, dynamic> json) => _$UserFromJson(json);

  Map<String, dynamic> toJson() => _$UserToJson(this);
}
