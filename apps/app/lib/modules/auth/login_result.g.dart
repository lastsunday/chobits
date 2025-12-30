// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'login_result.dart';

// **************************************************************************
// JsonSerializableGenerator
// **************************************************************************

LoginResult _$LoginResultFromJson(Map<String, dynamic> json) => LoginResult(
      access_token: json['access_token'] as String,
      expire_in: (json['expire_in'] as num).toInt(),
      client_id: json['client_id'] as String?,
    )..imToken = json['imToken'] as String?;

Map<String, dynamic> _$LoginResultToJson(LoginResult instance) =>
    <String, dynamic>{
      'access_token': instance.access_token,
      'expire_in': instance.expire_in,
      'client_id': instance.client_id,
      'imToken': instance.imToken,
    };
