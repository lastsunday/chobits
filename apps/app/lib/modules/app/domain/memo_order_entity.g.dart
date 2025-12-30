// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'memo_order_entity.dart';

// **************************************************************************
// JsonSerializableGenerator
// **************************************************************************

MemoOrderEntity _$MemoOrderEntityFromJson(Map<String, dynamic> json) =>
    MemoOrderEntity(
      displaymode: (json['displaymode'] as num?)?.toInt(),
      id: json['id'] as String,
      seq: (json['seq'] as num).toInt(),
    );

Map<String, dynamic> _$MemoOrderEntityToJson(MemoOrderEntity instance) =>
    <String, dynamic>{
      'displaymode': instance.displaymode,
      'id': instance.id,
      'seq': instance.seq,
    };
