import 'package:json_annotation/json_annotation.dart';

part 'user_vo.g.dart';

@JsonSerializable()
class UserVo {
  UserVo({
    required this.userId,
    required this.userName,
    required this.nickName,
    this.email,
    this.phonenumber,
    this.sex,
    this.avatar,
  });

  dynamic userId;
  String userName;
  String nickName;
  String? email;
  String? phonenumber;
  String? sex;
  String? avatar;

  factory UserVo.fromJson(Map<String, dynamic> json) => _$UserVoFromJson(json);

  Map<String, dynamic> toJson() => _$UserVoToJson(this);
}
